use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio::task::AbortHandle;

use crate::controller::{self, Controller};
use crate::core::{self, CoreProcess};
use crate::paths::Paths;
use crate::profile::{self, Profile};
use crate::proxy_settings;
use crate::state::{self, Settings};

#[derive(Clone, Debug)]
pub enum Event {
    Status,
    Profiles,
    Proxies,
    Delay {
        group: String,
        name: String,
        delay_ms: i64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExitKind {
    Requested,
    Unexpected,
}

struct Running {
    pid: u32,
    kill: oneshot::Sender<()>,
    done: oneshot::Receiver<()>,
    generation: u64,
    traffic: Option<AbortHandle>,
    region: Option<AbortHandle>,
}

pub struct ExitNotice {
    generation: u64,
    unexpected: bool,
}

struct Inner {
    settings: Settings,
    core_running: bool,
    system_proxy_applied: bool,
    upload_bps: u64,
    download_bps: u64,
    exit_region: String,
    message: String,
    running: Option<Running>,
    restart_attempts: u32,
}

fn region_probe_of(inner: &Inner) -> controller::RegionProbe {
    controller::region_probe(
        inner.system_proxy_applied,
        inner.core_running && inner.settings.tun_enabled,
        inner.settings.mixed_port,
    )
}

struct LabelCache {
    at: Instant,
    port: u16,
    kde: String,
    gnome: String,
}

pub struct App {
    paths: Paths,
    inner: Mutex<Inner>,
    delay_tests: Mutex<HashSet<String>>,
    proxy_labels: Mutex<LabelCache>,
    reconciled_at: Mutex<Instant>,
    generation: AtomicU64,
    region_epoch: AtomicU64,
    events: broadcast::Sender<Event>,
    exit_tx: mpsc::UnboundedSender<ExitNotice>,
    exit_rx: Mutex<Option<mpsc::UnboundedReceiver<ExitNotice>>>,
}

fn claim_delay_group(testing: &mut HashSet<String>, group: &str) -> bool {
    testing.insert(group.to_string())
}

#[derive(Serialize)]
struct Status {
    core_desired: bool,
    core_running: bool,
    system_proxy_enabled: bool,
    system_proxy_applied: bool,
    tun_enabled: bool,
    tun_machine: bool,
    kde_proxy: String,
    gnome_proxy: String,
    mode: String,
    listen_host: String,
    mixed_port: u16,
    controller_port: u16,
    mihomo_path: String,
    upload_bps: u64,
    download_bps: u64,
    exit_region: String,
    active_profile: String,
    message: String,
    override_path: String,
    selections: BTreeMap<String, String>,
    sort_global: bool,
    sort_mode: String,
    show_unreachable: bool,
    sort_prefs: BTreeMap<String, crate::state::SortPref>,
    auto_test_delay: bool,
    blur_ip: bool,
    compact: bool,
    show_widget_hint: bool,
    tun_device: String,
    override_default: String,
    allow_lan: bool,
    lan_username: String,
    lan_password: String,
    lan_allowed: bool,
}

impl App {
    pub fn new(paths: Paths) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(&paths.config_dir).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(&paths.data_dir).map_err(|error| error.to_string())?;
        let mut settings = Settings::load(&paths).map_err(|error| error.to_string())?;
        profile::ensure_override_file(&paths.override_file()).map_err(|error| error.to_string())?;
        let override_path = state::effective_override(&paths, &settings.override_path);
        if override_path != paths.override_file() {
            profile::ensure_override_file(&override_path).map_err(|error| error.to_string())?;
        }
        if state::ensure_secret(&mut settings) {
            settings.save(&paths).map_err(|error| error.to_string())?;
        }
        let (events, _) = broadcast::channel(1024);
        let (exit_tx, exit_rx) = mpsc::unbounded_channel();
        Ok(Arc::new(Self {
            paths: paths.clone(),
            delay_tests: Mutex::new(HashSet::new()),
            proxy_labels: Mutex::new(LabelCache {
                at: Instant::now() - Duration::from_secs(60),
                port: 0,
                kde: "off".to_string(),
                gnome: "off".to_string(),
            }),
            reconciled_at: Mutex::new(Instant::now() - Duration::from_secs(60)),
            generation: AtomicU64::new(1),
            region_epoch: AtomicU64::new(0),
            events,
            exit_tx,
            exit_rx: Mutex::new(Some(exit_rx)),
            inner: Mutex::new(Inner {
                settings,
                core_running: false,
                system_proxy_applied: paths.proxy_flag().exists(),
                upload_bps: 0,
                download_bps: 0,
                exit_region: String::new(),
                message: String::new(),
                running: None,
                restart_attempts: 0,
            }),
        }))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub async fn take_exit_notices(&self) -> mpsc::UnboundedReceiver<ExitNotice> {
        self.exit_rx
            .lock()
            .await
            .take()
            .expect("exit notices are taken once")
    }

    pub async fn restore_desired(self: &Arc<Self>) {
        let desired = self.inner.lock().await.settings.core_desired;
        if desired {
            if let Err(error) = self.spawn_core().await {
                tracing::error!("{error}");
            }
        } else {
            // 内核不该运行时，现场的系统代理要撤掉。标记文件丢失时也要关掉
            // 仍指向本程序的 GNOME 代理。
            self.withdraw_proxy().await;
            self.emit(Event::Status);
        }
    }

    pub async fn shutdown(&self) {
        let _ = self.stop_process().await;
        self.withdraw_proxy().await;
    }

    pub async fn start(self: &Arc<Self>) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            inner.settings.core_desired = true;
            inner.restart_attempts = 0;
            inner.message.clear();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            if inner.core_running {
                self.emit(Event::Status);
                return Ok(());
            }
        }
        self.spawn_core().await
    }

    pub async fn stop(self: &Arc<Self>) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            inner.settings.core_desired = false;
            inner.restart_attempts = 0;
            inner.message.clear();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
        }
        let stopped = self.stop_process().await;
        self.withdraw_proxy().await;
        self.schedule_region_lookup();
        self.emit(Event::Status);
        stopped
    }

    pub async fn restart(self: &Arc<Self>) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            inner.settings.core_desired = true;
            inner.restart_attempts = 0;
            inner.message.clear();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
        }
        self.spawn_core().await
    }

    pub async fn set_system_proxy(self: &Arc<Self>, enabled: bool) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            inner.settings.system_proxy_enabled = enabled;
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            inner.message.clear();
        }
        self.sync_system_proxy().await?;
        self.schedule_region_lookup();
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn set_tun(self: &Arc<Self>, enabled: bool) -> Result<(), String> {
        if enabled {
            let path = self.inner.lock().await.settings.mihomo_path.clone();
            let binary = core::resolve_binary(&path)?;
            crate::privilege::ensure_system_binary(&binary)?;
            let grant = crate::admin::own_grant().await.unwrap_or_default();
            if grant != "user" {
                crate::admin::authorize_tun().await?;
            }
        }
        self.finish_capture(enabled, false).await
    }

    pub async fn set_machine_tun(self: &Arc<Self>, enabled: bool) -> Result<(), String> {
        if enabled {
            let path = self.inner.lock().await.settings.mihomo_path.clone();
            let binary = core::resolve_binary(&path)?;
            crate::privilege::ensure_system_binary(&binary)?;
            crate::admin::authorize_machine_tun().await?;
        }
        self.finish_capture(enabled, enabled).await
    }

    async fn finish_capture(self: &Arc<Self>, enabled: bool, machine: bool) -> Result<(), String> {
        let running = {
            let mut inner = self.inner.lock().await;
            inner.settings.tun_enabled = enabled;
            inner.settings.tun_machine = machine;
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            inner.message.clear();
            inner.core_running
        };
        let revoke = if enabled {
            None
        } else {
            Some(tokio::spawn(crate::admin::revoke_quickly()))
        };
        let started = if running {
            self.spawn_core().await
        } else {
            self.sync_system_proxy().await?;
            self.emit(Event::Status);
            Ok(())
        };
        self.schedule_region_lookup();
        let revoke_error = match revoke {
            Some(task) => match task.await {
                Ok(Err(error)) => Some(error),
                Ok(Ok(())) => None,
                Err(_) => Some("没能撤销 TUN 授权".to_string()),
            },
            None => None,
        };
        match (started, revoke_error) {
            (Err(start), Some(revoke)) => Err(format!("{start}\n{revoke}")),
            (Err(start), None) => Err(start),
            (Ok(()), Some(revoke)) => Err(revoke),
            (Ok(()), None) => Ok(()),
        }
    }

    pub async fn set_mode(&self, mode: &str) -> Result<(), String> {
        let mode = state::normalize_mode(mode)?;
        let (running, port, secret) = {
            let mut inner = self.inner.lock().await;
            inner.settings.mode = mode.clone();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            inner.message.clear();
            (
                inner.core_running,
                inner.settings.controller_port,
                inner.settings.controller_secret.clone(),
            )
        };
        if running {
            Controller::new(port, &secret).set_mode(&mode).await?;
        }
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn set_settings(
        self: &Arc<Self>,
        mixed_port: u32,
        controller_port: u32,
        mihomo_path: String,
        tun_device: String,
        override_path: String,
    ) -> Result<(), String> {
        let (mixed_port, controller_port) = state::validate_ports(mixed_port, controller_port)?;
        let mihomo_path = mihomo_path.trim().to_string();
        if mihomo_path.is_empty() {
            return Err("mihomo 路径不能为空".to_string());
        }
        let tun_device = state::normalize_tun_device(&tun_device)?;
        let override_path = state::normalize_override_path(&self.paths, &override_path)?;
        let effective = state::effective_override(&self.paths, &override_path);
        profile::ensure_override_file(&effective).map_err(|error| error.to_string())?;
        let should_restart = {
            let mut inner = self.inner.lock().await;
            let ports_changed = inner.settings.mixed_port != mixed_port
                || inner.settings.controller_port != controller_port
                || inner.settings.mihomo_path != mihomo_path;
            let device_changed = inner.settings.tun_device != tun_device;
            let override_changed = inner.settings.override_path != override_path;
            let running = inner.core_running;
            let tun_on = inner.settings.tun_enabled;
            inner.settings.mixed_port = mixed_port;
            inner.settings.controller_port = controller_port;
            inner.settings.mihomo_path = mihomo_path;
            inner.settings.tun_device = tun_device;
            inner.settings.override_path = override_path;
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            inner.message.clear();
            running && (ports_changed || override_changed || (device_changed && tun_on))
        };
        if should_restart {
            self.spawn_core().await?;
        } else {
            self.sync_system_proxy().await?;
            self.emit(Event::Status);
        }
        Ok(())
    }

    pub async fn ensure_override(&self, requested: &str) -> Result<String, String> {
        let stored = state::normalize_override_path(&self.paths, requested)?;
        let path = state::effective_override(&self.paths, &stored);
        profile::ensure_override_file(&path).map_err(|error| error.to_string())?;
        Ok(path.display().to_string())
    }

    pub async fn profiles_json(&self) -> String {
        serde_json::to_string(&profile::load_profiles(&self.paths)).expect("profiles json")
    }

    pub async fn add_subscription(&self, name: String, url: String) -> Result<String, String> {
        let url = url.trim().to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("订阅地址需要以 http:// 或 https:// 开头".to_string());
        }
        let fetched = controller::fetch_subscription(&url).await?;
        let text = profile::subscription_yaml(&fetched.body)?;
        let name = {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                url.clone()
            } else {
                trimmed.to_string()
            }
        };
        let id = uuid::Uuid::new_v4().to_string();
        profile::write_profile_body(&self.paths, &id, &text).map_err(|error| error.to_string())?;
        let mut profiles = profile::load_profiles(&self.paths);
        profiles.push(Profile {
            id: id.clone(),
            name,
            url,
            updated_unix: profile::now_unix(),
            upload: fetched.usage.upload,
            download: fetched.usage.download,
            total: fetched.usage.total,
            expire_unix: fetched.usage.expire_unix,
        });
        profile::save_profiles(&self.paths, &profiles).map_err(|error| error.to_string())?;
        let mut inner = self.inner.lock().await;
        if inner.settings.active_profile.is_empty() {
            inner.settings.active_profile = id.clone();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
        }
        inner.message.clear();
        drop(inner);
        self.emit(Event::Profiles);
        self.emit(Event::Status);
        Ok(id)
    }

    pub async fn remove_profile(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let mut profiles = profile::load_profiles(&self.paths);
        let before = profiles.len();
        profiles.retain(|profile| profile.id != id);
        if profiles.len() == before {
            return Err("找不到这个配置".to_string());
        }
        profile::save_profiles(&self.paths, &profiles).map_err(|error| error.to_string())?;
        let _ = std::fs::remove_file(profile::profile_body_path(&self.paths, id));
        let follow_up = {
            let mut inner = self.inner.lock().await;
            let was_active = inner.settings.active_profile == id;
            if was_active {
                inner.settings.active_profile = profiles
                    .first()
                    .map(|profile| profile.id.clone())
                    .unwrap_or_default();
                inner
                    .settings
                    .save(&self.paths)
                    .map_err(|error| error.to_string())?;
            }
            inner.message.clear();
            if was_active && inner.core_running && inner.settings.active_profile.is_empty() {
                Some(false)
            } else if was_active && inner.core_running {
                Some(true)
            } else {
                None
            }
        };
        self.emit(Event::Profiles);
        match follow_up {
            Some(true) => self.spawn_core().await?,
            Some(false) => {
                if let Err(error) = self.stop_process().await {
                    self.set_message(error).await;
                }
                self.withdraw_proxy().await;
                self.set_message("没有选择配置".to_string()).await;
                self.emit(Event::Status);
            }
            None => self.emit(Event::Status),
        }
        Ok(())
    }

    pub async fn update_profile(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let profiles = profile::load_profiles(&self.paths);
        let Some(profile) = profiles.iter().find(|profile| profile.id == id) else {
            return Err("找不到这个配置".to_string());
        };
        if profile.url.is_empty() {
            return Err("这个配置没有订阅地址".to_string());
        }
        let fetched = controller::fetch_subscription(&profile.url).await?;
        let text = profile::subscription_yaml(&fetched.body)?;
        profile::write_profile_body(&self.paths, id, &text).map_err(|error| error.to_string())?;
        let mut profiles = profile::load_profiles(&self.paths);
        if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == id) {
            profile.updated_unix = profile::now_unix();
            profile.upload = fetched.usage.upload;
            profile.download = fetched.usage.download;
            profile.total = fetched.usage.total;
            profile.expire_unix = fetched.usage.expire_unix;
        }
        profile::save_profiles(&self.paths, &profiles).map_err(|error| error.to_string())?;
        let active = {
            let mut inner = self.inner.lock().await;
            inner.message.clear();
            inner.settings.active_profile == id && inner.core_running
        };
        self.emit(Event::Profiles);
        if active {
            self.spawn_core().await?;
        } else {
            self.emit(Event::Status);
        }
        Ok(())
    }

    pub async fn set_choices(
        self: &Arc<Self>,
        profile: &str,
        selections: BTreeMap<String, String>,
    ) -> Result<(), String> {
        if !profile.is_empty() {
            let profiles = profile::load_profiles(&self.paths);
            if !profiles.iter().any(|item| item.id == profile) {
                return Err("找不到这个配置".to_string());
            }
        }
        let (profile_changed, running, port, secret) = {
            let mut inner = self.inner.lock().await;
            let profile_changed = inner.settings.active_profile != profile;
            inner.settings.selections = selections.clone();
            if profile_changed {
                inner.settings.active_profile = profile.to_string();
            }
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            inner.message.clear();
            (
                profile_changed,
                inner.core_running,
                inner.settings.controller_port,
                inner.settings.controller_secret.clone(),
            )
        };
        if profile_changed {
            self.emit(Event::Profiles);
        }
        if !running {
            self.emit(Event::Status);
            return Ok(());
        }
        if profile_changed {
            return self.spawn_core().await;
        }
        let controller = Controller::new(port, &secret);
        for (group, name) in &selections {
            if let Err(error) = controller.select_proxy(group, name).await {
                tracing::warn!("{error}");
            }
        }
        self.emit(Event::Proxies);
        self.schedule_region_lookup();
        Ok(())
    }

    pub async fn set_behavior(
        &self,
        sort_global: bool,
        auto_test_delay: bool,
        blur_ip: bool,
        show_widget_hint: bool,
    ) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            inner.settings.sort_global = sort_global;
            inner.settings.auto_test_delay = auto_test_delay;
            inner.settings.blur_ip = blur_ip;
            inner.settings.show_widget_hint = show_widget_hint;
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
        }
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn set_compact(&self, compact: bool) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            inner.settings.compact = compact;
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
        }
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn set_lan(
        self: &Arc<Self>,
        allow: bool,
        username: String,
        password: String,
    ) -> Result<(), String> {
        let (username, password) = crate::lan::normalize_auth(&username, &password)?;
        let restart = {
            let mut inner = self.inner.lock().await;
            let changed = inner.settings.allow_lan != allow
                || inner.settings.lan_username != username
                || inner.settings.lan_password != password;
            inner.settings.allow_lan = allow;
            inner.settings.lan_username = username;
            inner.settings.lan_password = password;
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            changed && inner.core_running
        };
        if restart {
            self.spawn_core().await?;
        }
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn reload_runtime(self: &Arc<Self>) -> Result<(), String> {
        let running = self.inner.lock().await.core_running;
        if running {
            self.spawn_core().await?;
        }
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn route_detail(&self, id: &str, name: &str) -> Result<String, String> {
        let body = self.profile_body(id).await?;
        let found = profile::route_fields(&body, name)?;
        let in_file = found.is_some();
        let fields = found.unwrap_or(profile::RouteFields {
            name: name.to_string(),
            kind: String::new(),
            server: String::new(),
            port: 0,
            secret: String::new(),
        });
        #[derive(Serialize)]
        struct Detail<'a> {
            name: &'a str,
            kind: &'a str,
            server: &'a str,
            port: u16,
            secret: &'a str,
            in_file: bool,
        }
        Ok(serde_json::to_string(&Detail {
            name: &fields.name,
            kind: &fields.kind,
            server: &fields.server,
            port: fields.port,
            secret: &fields.secret,
            in_file,
        })
        .expect("route detail"))
    }

    pub async fn add_route(
        self: &Arc<Self>,
        id: &str,
        group: &str,
        name: String,
        kind: String,
        server: String,
        port: u16,
        secret: String,
    ) -> Result<(), String> {
        let body = self.profile_body(id).await?;
        let next = profile::add_route(
            &body,
            group,
            &profile::RouteFields {
                name,
                kind,
                server,
                port,
                secret,
            },
        )?;
        self.set_profile_body(id, next).await
    }

    pub async fn remove_route(self: &Arc<Self>, id: &str, name: &str) -> Result<(), String> {
        let body = self.profile_body(id).await?;
        let next = profile::remove_route(&body, name)?;
        self.set_profile_body(id, next).await
    }

    pub async fn update_route(
        self: &Arc<Self>,
        id: &str,
        previous: &str,
        name: String,
        kind: String,
        server: String,
        port: u16,
        secret: String,
    ) -> Result<(), String> {
        let body = self.profile_body(id).await?;
        let next = profile::update_route(
            &body,
            previous,
            &profile::RouteFields {
                name,
                kind,
                server,
                port,
                secret,
            },
        )?;
        self.set_profile_body(id, next).await
    }

    pub async fn set_view_pref(
        &self,
        profile: &str,
        group: &str,
        mode: &str,
        show_unreachable: bool,
        global: bool,
    ) -> Result<(), String> {
        let mode = match mode {
            "name" | "delay" | "profile" => mode.to_string(),
            _ => "profile".to_string(),
        };
        {
            let mut inner = self.inner.lock().await;
            if global {
                inner.settings.sort_mode = mode;
                inner.settings.show_unreachable = show_unreachable;
            } else {
                let key = format!("{profile}\n{group}");
                inner.settings.sort_prefs.insert(
                    key,
                    state::SortPref {
                        mode,
                        show_unreachable,
                    },
                );
            }
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
        }
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn profile_groups_json(&self, id: &str) -> Result<String, String> {
        if id.is_empty() {
            return Ok("[]".to_string());
        }
        let text =
            profile::read_profile_body(&self.paths, id).map_err(|error| error.to_string())?;
        let groups = profile::selector_groups(&text);
        Ok(serde_json::to_string(&groups).unwrap_or_else(|_| "[]".to_string()))
    }

    pub async fn set_active_profile(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let profiles = profile::load_profiles(&self.paths);
        if !profiles.iter().any(|profile| profile.id == id) {
            return Err("找不到这个配置".to_string());
        }
        let restart = {
            let mut inner = self.inner.lock().await;
            inner.settings.active_profile = id.to_string();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            inner.message.clear();
            inner.core_running
        };
        self.emit(Event::Profiles);
        if restart {
            self.spawn_core().await?;
        } else {
            self.emit(Event::Status);
        }
        Ok(())
    }

    pub async fn proxies_json(&self) -> Result<String, String> {
        let (running, port, secret) = self.controller_endpoint().await;
        if !running {
            return Ok("[]".to_string());
        }
        let groups = Controller::new(port, &secret).proxies().await?;
        Ok(serde_json::to_string(&groups).expect("proxies json"))
    }

    pub async fn select_proxy(self: &Arc<Self>, group: &str, name: &str) -> Result<(), String> {
        let (running, port, secret) = self.controller_endpoint().await;
        if !running {
            return Err("内核没有在运行".to_string());
        }
        Controller::new(port, &secret)
            .select_proxy(group, name)
            .await?;
        let mut inner = self.inner.lock().await;
        inner
            .settings
            .selections
            .insert(group.to_string(), name.to_string());
        inner
            .settings
            .save(&self.paths)
            .map_err(|error| error.to_string())?;
        inner.message.clear();
        drop(inner);
        self.emit(Event::Proxies);
        self.emit(Event::Status);
        self.schedule_region_lookup();
        Ok(())
    }

    pub async fn test_group_delay(&self, group: &str) -> Result<(), String> {
        {
            let mut testing = self.delay_tests.lock().await;
            if !claim_delay_group(&mut testing, group) {
                return Ok(());
            }
        }
        let result = self.probe_group_delay(group).await;
        self.delay_tests.lock().await.remove(group);
        result
    }

    async fn probe_group_delay(&self, group: &str) -> Result<(), String> {
        let (running, port, secret) = self.controller_endpoint().await;
        if !running {
            return Err("内核没有在运行".to_string());
        }
        let controller = Controller::new(port, &secret);
        let groups = controller.proxies().await?;
        let Some(found) = groups.into_iter().find(|item| item.name == group) else {
            return Err("找不到这个策略组".to_string());
        };
        let semaphore = Arc::new(tokio::sync::Semaphore::new(8));
        let mut tasks = tokio::task::JoinSet::new();
        for node in found.nodes {
            if matches!(
                node.name.as_str(),
                "DIRECT" | "REJECT" | "PASS" | "COMPATIBLE"
            ) {
                continue;
            }
            let controller = controller.clone();
            let permit = Arc::clone(&semaphore);
            let name = node.name;
            tasks.spawn(async move {
                let _permit = permit.acquire_owned().await;
                let delay_ms = controller.test_delay(&name).await;
                (name, delay_ms)
            });
        }
        while let Some(joined) = tasks.join_next().await {
            if let Ok((name, delay_ms)) = joined {
                self.emit(Event::Delay {
                    group: group.to_string(),
                    name,
                    delay_ms,
                });
            }
        }
        Ok(())
    }

    pub async fn profile_body(&self, id: &str) -> Result<String, String> {
        if !profile::load_profiles(&self.paths)
            .iter()
            .any(|profile| profile.id == id)
        {
            return Err("找不到这个配置".to_string());
        }
        profile::read_profile_body(&self.paths, id)
            .map_err(|error| format!("无法读取配置: {error}"))
    }

    pub async fn set_profile_body(self: &Arc<Self>, id: &str, body: String) -> Result<(), String> {
        if !profile::load_profiles(&self.paths)
            .iter()
            .any(|profile| profile.id == id)
        {
            return Err("找不到这个配置".to_string());
        }
        let text = profile::subscription_yaml(&body)?;
        profile::write_profile_body(&self.paths, id, &text).map_err(|error| error.to_string())?;
        self.reload_active_if_running(id).await
    }

    pub async fn set_profile_meta(
        &self,
        id: &str,
        name: String,
        url: String,
    ) -> Result<(), String> {
        let name = name.trim().to_string();
        let url = url.trim().to_string();
        if name.is_empty() {
            return Err("名称不能为空".to_string());
        }
        if !url.is_empty() && !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("订阅地址需要以 http:// 或 https:// 开头".to_string());
        }
        let mut profiles = profile::load_profiles(&self.paths);
        let Some(profile) = profiles.iter_mut().find(|profile| profile.id == id) else {
            return Err("找不到这个配置".to_string());
        };
        profile.name = name;
        profile.url = url;
        profile::save_profiles(&self.paths, &profiles).map_err(|error| error.to_string())?;
        self.inner.lock().await.message.clear();
        self.emit(Event::Profiles);
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn import_profile(&self, name: String, path: String) -> Result<String, String> {
        let raw =
            std::fs::read_to_string(&path).map_err(|error| format!("无法读取配置: {error}"))?;
        let text = profile::subscription_yaml(&raw)?;
        let name = {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                trimmed.to_string()
            } else {
                std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .filter(|stem| !stem.is_empty())
                    .unwrap_or("导入的配置")
                    .to_string()
            }
        };
        let id = uuid::Uuid::new_v4().to_string();
        profile::write_profile_body(&self.paths, &id, &text).map_err(|error| error.to_string())?;
        let mut profiles = profile::load_profiles(&self.paths);
        profiles.push(Profile {
            id: id.clone(),
            name,
            url: String::new(),
            updated_unix: profile::now_unix(),
            upload: 0,
            download: 0,
            total: 0,
            expire_unix: 0,
        });
        profile::save_profiles(&self.paths, &profiles).map_err(|error| error.to_string())?;
        let mut inner = self.inner.lock().await;
        if inner.settings.active_profile.is_empty() {
            inner.settings.active_profile = id.clone();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
        }
        inner.message.clear();
        drop(inner);
        self.emit(Event::Profiles);
        self.emit(Event::Status);
        Ok(id)
    }

    pub async fn refresh_exit_region(self: &Arc<Self>) -> Result<String, String> {
        let probe = {
            let inner = self.inner.lock().await;
            region_probe_of(&inner)
        };
        let epoch = self.region_epoch.fetch_add(1, Ordering::Relaxed) + 1;
        match controller::lookup_exit_region(probe).await {
            Ok(region) => {
                if self.region_epoch.load(Ordering::Relaxed) == epoch {
                    let mut inner = self.inner.lock().await;
                    inner.exit_region = region.clone();
                    inner.message.clear();
                    drop(inner);
                    self.emit(Event::Status);
                }
                Ok(region)
            }
            Err(error) => {
                if self.region_epoch.load(Ordering::Relaxed) == epoch {
                    self.set_message(error.clone()).await;
                    self.emit(Event::Status);
                }
                Err(error)
            }
        }
    }

    fn schedule_region_lookup(self: &Arc<Self>) {
        let epoch = self.region_epoch.fetch_add(1, Ordering::Relaxed) + 1;
        let app = Arc::clone(self);
        tokio::spawn(async move {
            let probe = {
                let mut inner = app.inner.lock().await;
                inner.exit_region = "正在检测".to_string();
                let probe = region_probe_of(&inner);
                drop(inner);
                app.emit(Event::Status);
                probe
            };
            let result = controller::lookup_exit_region(probe).await;
            if app.region_epoch.load(Ordering::Relaxed) != epoch {
                return;
            }
            let mut inner = app.inner.lock().await;
            match result {
                Ok(region) => {
                    inner.exit_region = region;
                    inner.message.clear();
                }
                Err(error) => {
                    inner.exit_region.clear();
                    inner.message = error;
                }
            }
            drop(inner);
            app.emit(Event::Status);
        });
    }

    async fn reload_active_if_running(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let active = {
            let mut inner = self.inner.lock().await;
            inner.message.clear();
            inner.settings.active_profile == id && inner.core_running
        };
        self.emit(Event::Profiles);
        if active {
            self.spawn_core().await?;
        } else {
            self.emit(Event::Status);
        }
        Ok(())
    }

    pub async fn reset_preferences(self: &Arc<Self>) -> Result<(), String> {
        let was_tun = {
            let mut inner = self.inner.lock().await;
            let was_tun = inner.settings.tun_enabled;
            let active = inner.settings.active_profile.clone();
            let secret = inner.settings.controller_secret.clone();
            inner.settings = crate::state::Settings {
                active_profile: active,
                controller_secret: secret,
                ..crate::state::Settings::default()
            };
            inner.message.clear();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            was_tun
        };
        let revoke = if was_tun {
            Some(tokio::spawn(crate::admin::revoke_quickly()))
        } else {
            None
        };
        self.stop().await?;
        if let Some(task) = revoke {
            match task.await {
                Ok(result) => result?,
                Err(_) => return Err("没能撤销 TUN 授权".to_string()),
            }
        }
        Ok(())
    }

    async fn reconcile_if_due(&self) {
        {
            let mut at = self.reconciled_at.lock().await;
            if at.elapsed() < Duration::from_secs(3) {
                return;
            }
            *at = Instant::now();
        }
        self.reconcile_external_proxy().await;
    }

    async fn proxy_labels(&self, port: u16) -> (String, String) {
        {
            let cache = self.proxy_labels.lock().await;
            if cache.port == port && cache.at.elapsed() < Duration::from_secs(3) {
                return (cache.kde.clone(), cache.gnome.clone());
            }
        }
        let paths = self.paths.clone();
        let (kde, gnome) = tokio::task::spawn_blocking(move || {
            (
                proxy_settings::kde_proxy_label(&paths, port).to_string(),
                proxy_settings::gnome_proxy_label(port).to_string(),
            )
        })
        .await
        .unwrap_or_else(|_| ("off".to_string(), "off".to_string()));
        let mut cache = self.proxy_labels.lock().await;
        *cache = LabelCache {
            at: Instant::now(),
            port,
            kde: kde.clone(),
            gnome: gnome.clone(),
        };
        (kde, gnome)
    }

    pub async fn status_json(&self) -> String {
        self.reconcile_if_due().await;
        let port = self.inner.lock().await.settings.mixed_port;
        let (kde_proxy, gnome_proxy) = self.proxy_labels(port).await;
        let inner = self.inner.lock().await;
        let status = Status {
            core_desired: inner.settings.core_desired,
            core_running: inner.core_running,
            system_proxy_enabled: inner.settings.system_proxy_enabled,
            system_proxy_applied: inner.system_proxy_applied,
            tun_enabled: inner.settings.tun_enabled,
            tun_machine: inner.settings.tun_machine,
            kde_proxy,
            gnome_proxy,
            mode: inner.settings.mode.clone(),
            listen_host: "127.0.0.1".to_string(),
            mixed_port: inner.settings.mixed_port,
            controller_port: inner.settings.controller_port,
            mihomo_path: inner.settings.mihomo_path.clone(),
            upload_bps: inner.upload_bps,
            download_bps: inner.download_bps,
            exit_region: inner.exit_region.clone(),
            active_profile: inner.settings.active_profile.clone(),
            message: inner.message.clone(),
            override_path: state::effective_override(&self.paths, &inner.settings.override_path)
                .display()
                .to_string(),
            selections: inner.settings.selections.clone(),
            sort_global: inner.settings.sort_global,
            sort_mode: inner.settings.sort_mode.clone(),
            show_unreachable: inner.settings.show_unreachable,
            sort_prefs: inner.settings.sort_prefs.clone(),
            auto_test_delay: inner.settings.auto_test_delay,
            blur_ip: inner.settings.blur_ip,
            compact: inner.settings.compact,
            show_widget_hint: inner.settings.show_widget_hint,
            tun_device: inner.settings.tun_device.clone(),
            override_default: self.paths.override_file().display().to_string(),
            allow_lan: inner.settings.allow_lan,
            lan_username: inner.settings.lan_username.clone(),
            lan_password: inner.settings.lan_password.clone(),
            lan_allowed: crate::lan::permitted(
                &crate::lan::policy_dir(),
                crate::lan::current_uid(),
            ),
        };
        let expose = status.lan_allowed && status.allow_lan;
        let status = Status {
            listen_host: if expose {
                "0.0.0.0".to_string()
            } else {
                "127.0.0.1".to_string()
            },
            ..status
        };
        serde_json::to_string(&status).expect("status json")
    }

    async fn spawn_core(self: &Arc<Self>) -> Result<(), String> {
        let settings = self.inner.lock().await.settings.clone();
        let binary = match core::resolve_binary(&settings.mihomo_path) {
            Ok(binary) => binary,
            Err(error) => return self.fail_before_start(error).await,
        };
        if let Err(error) = profile::write_runtime_config(&self.paths, &settings) {
            return self.fail_before_start(error).await;
        }
        if let Err(error) = self.stop_process().await {
            self.mark_stopped(error.clone()).await;
            self.emit(Event::Status);
            return Err(error);
        }

        let launch = self.launch_process(settings, binary).await;
        if let Err(error) = launch {
            self.withdraw_proxy().await;
            self.mark_stopped(error.clone()).await;
            self.emit(Event::Status);
            return Err(error);
        }
        self.sync_system_proxy().await?;
        self.schedule_region_lookup();
        self.emit(Event::Status);
        self.emit(Event::Proxies);
        Ok(())
    }

    async fn fail_before_start(&self, error: String) -> Result<(), String> {
        self.set_message(error.clone()).await;
        self.emit(Event::Status);
        Err(error)
    }

    async fn wait_until_ready(self: &Arc<Self>, process: &mut CoreProcess) -> Result<(), String> {
        let started = Instant::now();
        let limit = Duration::from_secs(180);
        let mut announced = "正在启动内核".to_string();
        self.set_message(announced.clone()).await;
        self.emit(Event::Status);
        loop {
            let controller = {
                let inner = self.inner.lock().await;
                Controller::new(
                    inner.settings.controller_port,
                    &inner.settings.controller_secret,
                )
            };
            if controller.probe().await {
                if let Some(error) = core::tun_listen_error(&self.paths.core_log()) {
                    let tun = self.inner.lock().await.settings.tun_enabled;
                    if tun {
                        return Err(error);
                    }
                }
                return Ok(());
            }
            match process.try_wait() {
                Ok(Some(status)) => {
                    return Err(self.startup_failure(&format!("mihomo 已退出: {status}")));
                }
                Ok(None) => {}
                Err(error) => return Err(format!("无法检查 mihomo: {error}")),
            }
            if started.elapsed() >= limit {
                return Err(self.startup_failure("mihomo 控制器没有在限定时间内响应"));
            }
            let hint = core::startup_hint(&self.paths.core_log());
            if hint != announced {
                announced = hint.clone();
                self.set_message(hint).await;
                self.emit(Event::Status);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn startup_failure(&self, error: &str) -> String {
        let tail = core::log_tail(&self.paths.core_log());
        if tail.is_empty() {
            error.to_string()
        } else {
            format!("{error}\n{tail}")
        }
    }

    async fn launch_process(
        self: &Arc<Self>,
        settings: Settings,
        binary: std::path::PathBuf,
    ) -> Result<(), String> {
        core::seed_geodata(&self.paths.runtime_dir());
        let mut process = if settings.tun_enabled {
            crate::privilege::ensure_system_binary(&binary)?;
            let pid = crate::admin::spawn_tun(
                &binary,
                &self.paths.runtime_dir(),
                &self.paths.runtime_config(),
                &self.paths.core_log(),
            )
            .await?;
            core::CoreProcess::adopt(pid)
        } else {
            core::CoreProcess::spawn(&self.paths, &binary)?
        };
        let controller = Controller::new(settings.controller_port, &settings.controller_secret);
        if let Err(error) = self.wait_until_ready(&mut process).await {
            if matches!(process.try_wait(), Ok(None)) {
                process.terminate().await;
            }
            return Err(error);
        }
        if let Err(error) = controller.set_mode(&settings.mode).await {
            tracing::warn!("{error}");
        }
        self.apply_saved_selections(&controller).await;

        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let (kill_tx, kill_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        {
            let mut inner = self.inner.lock().await;
            inner.core_running = true;
            inner.restart_attempts = 0;
            inner.upload_bps = 0;
            inner.download_bps = 0;
            inner.message.clear();
            inner.running = Some(Running {
                pid: process.pid(),
                kill: kill_tx,
                done: done_rx,
                generation,
                traffic: None,
                region: None,
            });
        }

        let app = Arc::clone(self);
        tokio::spawn(async move {
            let unexpected = tokio::select! {
                _ = kill_rx => false,
                status = process.wait() => {
                    if let Ok(status) = status {
                        tracing::info!("mihomo 退出: {status}");
                    }
                    true
                }
            };
            if !unexpected {
                process.terminate().await;
            }
            let _ = app.exit_tx.send(ExitNotice {
                generation,
                unexpected,
            });
            let _ = done_tx.send(());
        });

        let traffic = self.spawn_traffic_task(controller);
        let region = self.spawn_region_task();
        let mut inner = self.inner.lock().await;
        if inner
            .running
            .as_ref()
            .is_some_and(|running| running.generation == generation)
        {
            if let Some(running) = inner.running.as_mut() {
                running.traffic = Some(traffic);
                running.region = Some(region);
            }
        } else {
            traffic.abort();
            region.abort();
        }
        Ok(())
    }

    fn spawn_traffic_task(self: &Arc<Self>, controller: Controller) -> AbortHandle {
        let app = Arc::clone(self);
        tokio::spawn(async move {
            let mut buffer = String::new();
            loop {
                let running = app.inner.lock().await.core_running;
                if !running {
                    break;
                }
                let Ok(mut stream) = controller.traffic_stream().await else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                };
                while let Some(chunk) = stream.next().await {
                    let Ok(bytes) = chunk else {
                        break;
                    };
                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(index) = buffer.find('\n') {
                        let line = buffer[..index].trim().to_string();
                        buffer.drain(..=index);
                        if line.is_empty() {
                            continue;
                        }
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                            let up = value.get("up").and_then(|item| item.as_u64()).unwrap_or(0);
                            let down = value
                                .get("down")
                                .and_then(|item| item.as_u64())
                                .unwrap_or(0);
                            let mut inner = app.inner.lock().await;
                            if inner.core_running {
                                inner.upload_bps = up;
                                inner.download_bps = down;
                            }
                        }
                    }
                }
                buffer.clear();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .abort_handle()
    }

    fn spawn_region_task(self: &Arc<Self>) -> AbortHandle {
        let app = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let (running, probe) = {
                    let inner = app.inner.lock().await;
                    (inner.core_running, region_probe_of(&inner))
                };
                if !running {
                    break;
                }
                let epoch = app.region_epoch.load(Ordering::Relaxed);
                if let Ok(region) = controller::lookup_exit_region(probe).await
                    && app.region_epoch.load(Ordering::Relaxed) == epoch
                {
                    app.inner.lock().await.exit_region = region;
                    app.emit(Event::Status);
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        })
        .abort_handle()
    }

    async fn apply_saved_selections(&self, controller: &Controller) {
        let selections = self.inner.lock().await.settings.selections.clone();
        let Ok(groups) = controller.proxies().await else {
            return;
        };
        for group in groups {
            let Some(selected) = selections.get(&group.name) else {
                continue;
            };
            if group.nodes.iter().any(|node| &node.name == selected)
                && let Err(error) = controller.select_proxy(&group.name, selected).await
            {
                tracing::warn!("{error}");
            }
        }
    }

    async fn stop_process(&self) -> Result<(), String> {
        let running = {
            let mut inner = self.inner.lock().await;
            inner.core_running = false;
            inner.upload_bps = 0;
            inner.download_bps = 0;
            inner.running.take()
        };
        let Some(running) = running else {
            return Ok(());
        };
        if let Some(task) = running.traffic {
            task.abort();
        }
        if let Some(task) = running.region {
            task.abort();
        }
        let pid = running.pid;
        let _ = running.kill.send(());
        if tokio::time::timeout(Duration::from_millis(800), running.done)
            .await
            .is_err()
        {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
            for _ in 0..5 {
                if !core::process_alive(pid) {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(40)).await;
            }
            if core::process_alive(pid) {
                return Err("内核没有在限定时间内退出".to_string());
            }
        }
        Ok(())
    }

    pub async fn on_core_exit(self: &Arc<Self>, notice: ExitNotice) {
        let kind = if notice.unexpected {
            ExitKind::Unexpected
        } else {
            ExitKind::Requested
        };
        let generation = notice.generation;
        let retry = {
            let mut inner = self.inner.lock().await;
            let still_current = inner
                .running
                .as_ref()
                .is_some_and(|running| running.generation == generation);
            if !still_current {
                return;
            }
            if let Some(running) = inner.running.take() {
                if let Some(task) = running.traffic {
                    task.abort();
                }
                if let Some(task) = running.region {
                    task.abort();
                }
            }
            inner.core_running = false;
            inner.upload_bps = 0;
            inner.download_bps = 0;
            match kind {
                ExitKind::Requested => false,
                ExitKind::Unexpected
                    if inner.settings.core_desired && inner.restart_attempts < 3 =>
                {
                    inner.restart_attempts += 1;
                    true
                }
                ExitKind::Unexpected => {
                    inner.exit_region.clear();
                    inner.message =
                        format!("mihomo 已退出\n{}", core::log_tail(&self.paths.core_log()));
                    false
                }
            }
        };
        self.withdraw_proxy().await;
        self.emit(Event::Status);
        if retry {
            let app = Arc::clone(self);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                if let Err(error) = app.spawn_core().await {
                    app.set_message(error).await;
                    app.emit(Event::Status);
                }
            });
        }
    }

    async fn reconcile_external_proxy(&self) {
        let (port, enabled) = {
            let inner = self.inner.lock().await;
            (
                inner.settings.mixed_port,
                inner.settings.system_proxy_enabled,
            )
        };
        let released = if self.paths.proxy_flag().exists() {
            if proxy_settings::still_points_at_us(&self.paths, port) {
                false
            } else {
                match proxy_settings::release_ownership(&self.paths, port) {
                    Ok(()) => true,
                    Err(error) => {
                        tracing::error!("放开被改过的系统代理失败: {error}");
                        return;
                    }
                }
            }
        } else if enabled && proxy_settings::left_proxy_was_edited(&self.paths) {
            let _ = std::fs::remove_file(self.paths.proxy_left());
            true
        } else {
            false
        };
        if !released {
            return;
        }
        let mut inner = self.inner.lock().await;
        inner.settings.system_proxy_enabled = false;
        inner.system_proxy_applied = false;
        if let Err(error) = inner.settings.save(&self.paths) {
            tracing::error!("保存系统代理开关失败: {error}");
            return;
        }
        drop(inner);
        self.emit(Event::Status);
    }

    async fn sync_system_proxy(&self) -> Result<(), String> {
        self.reconcile_external_proxy().await;
        let (enabled, running, port, tun) = {
            let inner = self.inner.lock().await;
            (
                inner.settings.system_proxy_enabled,
                inner.core_running,
                inner.settings.mixed_port,
                inner.settings.tun_enabled,
            )
        };
        if proxy_settings::system_proxy_should_apply(enabled, running, tun) {
            proxy_settings::apply_if_needed(&self.paths, port)
                .map_err(|error| error.to_string())?;
            self.inner.lock().await.system_proxy_applied = true;
        } else {
            self.withdraw_proxy().await;
        }
        self.proxy_labels.lock().await.at = Instant::now() - Duration::from_secs(60);
        Ok(())
    }

    async fn withdraw_proxy(&self) {
        let port = self.inner.lock().await.settings.mixed_port;
        if let Err(error) = proxy_settings::restore_if_applied(&self.paths, port) {
            tracing::error!("恢复 KDE 代理失败: {error}");
            return;
        }
        self.inner.lock().await.system_proxy_applied = false;
        self.proxy_labels.lock().await.at = Instant::now() - Duration::from_secs(60);
    }

    async fn mark_stopped(&self, message: String) {
        let mut inner = self.inner.lock().await;
        inner.core_running = false;
        inner.upload_bps = 0;
        inner.download_bps = 0;
        inner.exit_region.clear();
        inner.message = message;
    }

    async fn set_message(&self, message: String) {
        self.inner.lock().await.message = message;
    }

    async fn controller_endpoint(&self) -> (bool, u16, String) {
        let inner = self.inner.lock().await;
        (
            inner.core_running,
            inner.settings.controller_port,
            inner.settings.controller_secret.clone(),
        )
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_already_testing_is_not_claimed_again() {
        let mut testing = HashSet::new();
        assert!(claim_delay_group(&mut testing, "happy"));
        assert!(!claim_delay_group(&mut testing, "happy"));
        assert!(claim_delay_group(&mut testing, "other"));
        testing.remove("happy");
        assert!(claim_delay_group(&mut testing, "happy"));
    }
}
