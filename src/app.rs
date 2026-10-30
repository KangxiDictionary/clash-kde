// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

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
use crate::error::{ServiceError, codes};
use crate::paths::Paths;
use crate::profile::{self, Profile};
use crate::proxy_settings;
use crate::rules::{self, OwnRule};
use crate::state::{self, Settings};
use crate::tun::{self, RouteHealth, TunOptions};

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

/// Stable phase names for the UI. These must not be translated: the KCM uses
/// them to decide whether a start is still in progress or has failed.
const PHASE_STOPPED: &str = "stopped";
const PHASE_STARTING: &str = "starting";
const PHASE_DOWNLOADING: &str = "downloading";
const PHASE_RUNNING: &str = "running";
const PHASE_FAILED: &str = "failed";

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
    message_code: String,
    message_detail: String,
    message_params: BTreeMap<String, String>,
    phase: String,
    tun_route_ready: bool,
    tun_route_known: bool,
    tun_route_detail: String,
    tun_route_checked_at: i64,
    running: Option<Running>,
    restart_attempts: u32,
}

fn message_parts(message: &str) -> (String, String, BTreeMap<String, String>) {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(message)
        && let Some(code) = value.get("code").and_then(serde_json::Value::as_str)
        && !code.is_empty()
    {
        let detail = value
            .get("detail")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let params = value
            .get("params")
            .and_then(serde_json::Value::as_object)
            .map(|object| {
                object
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        return (code.to_string(), detail.to_string(), params);
    }
    (String::new(), message.to_string(), BTreeMap::new())
}

impl Inner {
    fn set_message(&mut self, message: String) {
        let (code, detail, params) = message_parts(&message);
        self.message = message;
        self.message_code = code;
        self.message_detail = detail;
        self.message_params = params;
    }

    fn clear_message(&mut self) {
        self.message.clear();
        self.message_code.clear();
        self.message_detail.clear();
        self.message_params.clear();
    }
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

async fn probe_health(options: TunOptions) -> RouteHealth {
    let probe = tokio::task::spawn_blocking(move || tun::probe(&options));
    match tokio::time::timeout(Duration::from_secs(5), probe).await {
        Ok(Ok(health)) => health,
        Ok(Err(error)) => RouteHealth {
            ready: true,
            known: false,
            detail: format!("route probe failed: {error}"),
        },
        Err(_) => RouteHealth {
            ready: true,
            known: false,
            detail: "route probe timed out".to_string(),
        },
    }
}

#[derive(Serialize)]
struct Status {
    core_desired: bool,
    core_running: bool,
    system_proxy_enabled: bool,
    system_proxy_applied: bool,
    tun_enabled: bool,
    tun_machine: bool,
    tun_route_table: u32,
    tun_rule_index: u32,
    tun_exclude_private: bool,
    tun_route_ready: bool,
    tun_route_known: bool,
    tun_route_detail: String,
    tun_route_checked_at: i64,
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
    message_code: String,
    message_detail: String,
    message_params: BTreeMap<String, String>,
    phase: String,
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
    speed_test_mib: u32,
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
        for profile in profile::load_profiles(&paths) {
            if profile.url.is_empty() {
                continue;
            }
            let _ = profile::read_subscription_body(&paths, &profile.id);
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
                message_code: String::new(),
                message_detail: String::new(),
                message_params: BTreeMap::new(),
                phase: PHASE_STOPPED.to_string(),
                tun_route_ready: true,
                tun_route_known: true,
                tun_route_detail: String::new(),
                tun_route_checked_at: 0,
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
        // A proxy left over from a previous run points at a port that is not
        // listening yet. Clear it before starting; sync_system_proxy applies it
        // again once the new core is ready.
        self.withdraw_proxy().await;
        let desired = self.inner.lock().await.settings.core_desired;
        if desired {
            tracing::info!("restoring the previous core state");
            if let Err(error) = self.spawn_core().await {
                tracing::error!("could not restore the core: {error}");
            }
        } else {
            self.set_phase(PHASE_STOPPED).await;
            self.emit(Event::Status);
        }
    }

    pub async fn shutdown(&self) {
        let _ = self.stop_process().await;
        self.withdraw_proxy().await;
        self.set_phase(PHASE_STOPPED).await;
    }

    pub async fn start(self: &Arc<Self>) -> Result<(), String> {
        tracing::info!("starting the core");
        {
            let mut inner = self.inner.lock().await;
            inner.settings.core_desired = true;
            inner.restart_attempts = 0;
            inner.clear_message();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            if inner.core_running {
                inner.phase = PHASE_RUNNING.to_string();
                drop(inner);
                self.emit(Event::Status);
                return Ok(());
            }
            inner.phase = PHASE_STARTING.to_string();
        }
        self.spawn_core().await
    }

    pub async fn stop(self: &Arc<Self>) -> Result<(), String> {
        tracing::info!("stopping the core");
        {
            let mut inner = self.inner.lock().await;
            inner.settings.core_desired = false;
            inner.restart_attempts = 0;
            inner.clear_message();
            inner.phase = PHASE_STOPPED.to_string();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
        }
        let stopped = self.stop_process().await;
        self.withdraw_proxy().await;
        self.schedule_region_lookup();
        if let Err(error) = &stopped {
            self.set_message(error.clone()).await;
            self.set_phase(PHASE_FAILED).await;
        }
        self.emit(Event::Status);
        stopped
    }

    pub async fn restart(self: &Arc<Self>) -> Result<(), String> {
        tracing::info!("restarting the core");
        {
            let mut inner = self.inner.lock().await;
            inner.settings.core_desired = true;
            inner.restart_attempts = 0;
            inner.clear_message();
            inner.phase = PHASE_STARTING.to_string();
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
            inner.clear_message();
        }
        self.sync_system_proxy().await?;
        self.schedule_region_lookup();
        self.emit(Event::Status);
        Ok(())
    }

    async fn ensure_capture_grant(scope: &str) -> Result<(), String> {
        let grant = crate::admin::own_grant().await.unwrap_or_default();
        if grant == scope {
            return Ok(());
        }
        match scope {
            "user" => crate::admin::authorize_tun().await,
            "machine" => crate::admin::authorize_machine_tun().await,
            _ => Err(format!("unknown capture scope {scope}")),
        }
    }

    pub async fn set_tun(self: &Arc<Self>, enabled: bool) -> Result<(), String> {
        if enabled {
            let path = self.inner.lock().await.settings.mihomo_path.clone();
            let binary = core::resolve_binary(&path)?;
            crate::privilege::ensure_system_binary(&binary)?;
            Self::ensure_capture_grant("user").await?;
        }
        self.finish_capture(enabled, false).await
    }

    pub async fn set_machine_tun(self: &Arc<Self>, enabled: bool) -> Result<(), String> {
        if enabled {
            let path = self.inner.lock().await.settings.mihomo_path.clone();
            let binary = core::resolve_binary(&path)?;
            crate::privilege::ensure_system_binary(&binary)?;
            Self::ensure_capture_grant("machine").await?;
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
            inner.clear_message();
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
                Err(_) => Some(ServiceError::new(codes::TUN_REVOKE_TIMEOUT).into()),
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
            inner.clear_message();
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
            return Err(ServiceError::new(codes::SETTINGS_MIHOMO_EMPTY).into());
        }
        if mihomo_path.chars().count() > 1024 {
            return Err(ServiceError::new(codes::SETTINGS_MIHOMO_TOO_LONG)
                .param("limit", "1024")
                .into());
        }
        let tun_device = state::normalize_tun_device(&tun_device)?;
        if override_path.chars().count() > 4096 {
            return Err(ServiceError::new(codes::SETTINGS_OVERRIDE_TOO_LONG)
                .param("limit", "4096")
                .into());
        }
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
            inner.clear_message();
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
        if url.chars().count() > 2048 {
            return Err(ServiceError::new(codes::PROFILE_URL_TOO_LONG)
                .param("limit", "2048")
                .into());
        }
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(ServiceError::new(codes::PROFILE_URL_SCHEME).into());
        }
        let name = {
            let trimmed = name.trim();
            if trimmed.chars().count() > 128 {
                return Err(ServiceError::new(codes::PROFILE_NAME_TOO_LONG)
                    .param("limit", "128")
                    .into());
            }
            if trimmed.is_empty() {
                url.clone()
            } else {
                trimmed.to_string()
            }
        };
        let fetched = controller::fetch_subscription(&url).await?;
        let text = profile::subscription_yaml(&fetched.body)?;
        let id = uuid::Uuid::new_v4().to_string();
        profile::write_profile_body(&self.paths, &id, &text)
            .map_err(|error| profile::explain_io("write profile", &error))?;
        profile::write_subscription_body(&self.paths, &id, &text)
            .map_err(|error| error.to_string())?;
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
        inner.clear_message();
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
            return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
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
            inner.clear_message();
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
                self.set_message(ServiceError::new(codes::PROFILE_NOT_FOUND).into())
                    .await;
                self.emit(Event::Status);
            }
            None => self.emit(Event::Status),
        }
        Ok(())
    }

    pub async fn update_profile(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let profiles = profile::load_profiles(&self.paths);
        let Some(profile) = profiles.iter().find(|profile| profile.id == id) else {
            return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
        };
        if profile.url.is_empty() {
            return Err(ServiceError::new(codes::PROFILE_NO_URL).into());
        }
        let fetched = controller::fetch_subscription(&profile.url).await?;
        let text = profile::subscription_yaml(&fetched.body)?;
        profile::write_subscription_body(&self.paths, id, &text)
            .map_err(|error| error.to_string())?;
        profile::write_profile_body(&self.paths, id, &text)
            .map_err(|error| profile::explain_io("write profile", &error))?;
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
            inner.clear_message();
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
                return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
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
            inner.clear_message();
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
        speed_test_mib: i32,
    ) -> Result<(), String> {
        if !(1..=100).contains(&speed_test_mib) {
            return Err(ServiceError::new(codes::SETTINGS_SPEED_RANGE).into());
        }
        {
            let mut inner = self.inner.lock().await;
            inner.settings.sort_global = sort_global;
            inner.settings.auto_test_delay = auto_test_delay;
            inner.settings.blur_ip = blur_ip;
            inner.settings.show_widget_hint = show_widget_hint;
            inner.settings.speed_test_mib = speed_test_mib as u32;
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

    pub async fn connections_json(&self) -> Result<String, String> {
        let rows = self.running_controller().await?.connections().await?;
        Ok(serde_json::to_string(&rows).expect("connections json"))
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), String> {
        self.running_controller()
            .await?
            .close_connection(id)
            .await
            .map_err(String::from)
    }

    pub async fn close_all_connections(&self) -> Result<(), String> {
        self.running_controller()
            .await?
            .close_all_connections()
            .await
            .map_err(String::from)
    }

    pub async fn proxy_providers_json(&self) -> String {
        let profile = self.active_profile_value().await;
        let names = rules::provider_names(&profile, "proxy-providers");
        let live = self.live_proxy_providers().await;
        rules::provider_json(&rules::merge_provider_lists(&names, &live))
    }

    pub async fn update_proxy_provider(&self, name: &str) -> Result<(), String> {
        self.running_controller()
            .await?
            .update_proxy_provider(name)
            .await
            .map_err(String::from)
    }

    pub async fn update_rule_provider(&self, name: &str) -> Result<(), String> {
        self.running_controller()
            .await?
            .update_rule_provider(name)
            .await
            .map_err(String::from)
    }

    pub async fn rule_view_json(&self) -> Result<String, String> {
        let own = rules::load_rules(&rules::rules_file(&self.paths))?;
        let profile = self.active_profile_value().await;
        let names = rules::provider_names(&profile, "rule-providers");
        let live = self.live_rule_providers().await;
        let providers = rules::merge_provider_lists(&names, &live);
        let targets = rules::policy_names(&profile);
        let profile_rules = rules::profile_rules(&profile);
        let id = self.inner.lock().await.settings.active_profile.clone();
        let changed: Vec<bool> = self
            .subscription_text(&id)
            .map_or_else(Vec::new, |snapshot| {
                let Ok(working) = profile::read_profile_body(&self.paths, &id) else {
                    return Vec::new();
                };
                (0..profile_rules.len())
                    .map(|index| profile::rule_differs(&working, &snapshot, index))
                    .collect()
            });
        let mut view: serde_json::Value = serde_json::from_str(&rules::rule_view_json(
            &own,
            &profile_rules,
            &providers,
            &targets,
        ))
        .expect("rule view");
        view["changed"] = serde_json::json!(changed);
        Ok(view.to_string())
    }

    fn subscription_text(&self, id: &str) -> Option<String> {
        let profiles = profile::load_profiles(&self.paths);
        let profile = profiles.iter().find(|profile| profile.id == id)?;
        if profile.url.is_empty() {
            return None;
        }
        profile::read_subscription_body(&self.paths, id).ok()
    }

    pub async fn add_own_rule(
        self: &Arc<Self>,
        kind: String,
        payload: String,
        target: String,
    ) -> Result<(), String> {
        let mut rules = rules::load_rules(&rules::rules_file(&self.paths))?;
        rules.push(OwnRule {
            kind,
            payload,
            target,
            enabled: true,
        });
        self.store_own_rules(rules).await
    }

    pub async fn remove_own_rule(self: &Arc<Self>, index: u32) -> Result<(), String> {
        let mut rules = rules::load_rules(&rules::rules_file(&self.paths))?;
        if index as usize >= rules.len() {
            return Err(ServiceError::new(codes::RULE_NOT_FOUND).into());
        }
        rules.remove(index as usize);
        self.store_own_rules(rules).await
    }

    pub async fn set_own_rules(self: &Arc<Self>, body: &str) -> Result<(), String> {
        let rules = rules::rules_from_json(body)?;
        self.store_own_rules(rules).await
    }

    async fn store_own_rules(self: &Arc<Self>, rules: Vec<OwnRule>) -> Result<(), String> {
        let profile = self.active_profile_value().await;
        rules::validate_rules(&rules, &rules::policy_names(&profile))?;
        rules::save_rules(&rules::rules_file(&self.paths), &rules)?;
        self.reload_runtime().await
    }

    async fn running_controller(&self) -> Result<Controller, String> {
        let (running, port, secret) = self.controller_endpoint().await;
        if !running {
            return Err(ServiceError::new(codes::CORE_NOT_RUNNING).into());
        }
        Ok(Controller::new(port, &secret))
    }

    async fn active_profile_value(&self) -> serde_yaml::Value {
        let id = self.inner.lock().await.settings.active_profile.clone();
        if id.is_empty() {
            return serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
        }
        let Ok(body) = profile::read_profile_body(&self.paths, &id) else {
            return serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
        };
        profile::decode_subscription(&body)
            .unwrap_or(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
    }

    async fn live_proxy_providers(&self) -> Vec<(String, String, i64)> {
        let Ok(controller) = self.running_controller().await else {
            return Vec::new();
        };
        controller
            .proxy_providers()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|row| (row.name, row.updated_at, row.count))
            .collect()
    }

    async fn live_rule_providers(&self) -> Vec<(String, String, i64)> {
        let Ok(controller) = self.running_controller().await else {
            return Vec::new();
        };
        controller
            .rule_providers()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|row| (row.name, row.updated_at, row.count))
            .collect()
    }

    pub async fn route_detail(&self, id: &str, name: &str) -> Result<String, String> {
        let body = self.profile_body(id).await?;
        let found = profile::route_fields(&body, name)?;
        let in_file = found.is_some();
        let changed = in_file
            && self
                .subscription_text(id)
                .is_some_and(|snapshot| profile::route_differs(&body, &snapshot, name));
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
            changed: bool,
        }
        Ok(serde_json::to_string(&Detail {
            name: &fields.name,
            kind: &fields.kind,
            server: &fields.server,
            port: fields.port,
            secret: &fields.secret,
            in_file,
            changed,
        })
        .expect("route detail"))
    }

    #[allow(clippy::too_many_arguments)]
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

    #[allow(clippy::too_many_arguments)]
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

    pub async fn restore_route(self: &Arc<Self>, id: &str, name: &str) -> Result<(), String> {
        let profiles = profile::load_profiles(&self.paths);
        let Some(profile) = profiles.iter().find(|profile| profile.id == id) else {
            return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
        };
        if profile.url.is_empty() {
            return Err(ServiceError::new(codes::PROFILE_NO_URL).into());
        }
        let snapshot = profile::read_subscription_body(&self.paths, id)?;
        let working = profile::read_profile_body(&self.paths, id)
            .map_err(|error| profile::explain_io("read profile", &error))?;
        let next = profile::restore_route(&working, &snapshot, name)?;
        self.set_profile_body(id, next).await
    }

    pub async fn restore_profile_rule(
        self: &Arc<Self>,
        id: &str,
        index: u32,
    ) -> Result<(), String> {
        let profiles = profile::load_profiles(&self.paths);
        let Some(profile) = profiles.iter().find(|profile| profile.id == id) else {
            return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
        };
        if profile.url.is_empty() {
            return Err(ServiceError::new(codes::PROFILE_NO_URL).into());
        }
        let snapshot = profile::read_subscription_body(&self.paths, id)?;
        let working = profile::read_profile_body(&self.paths, id)
            .map_err(|error| profile::explain_io("read profile", &error))?;
        let next = profile::restore_profile_rule(&working, &snapshot, index as usize)?;
        self.set_profile_body(id, next).await
    }

    pub async fn notes_json(&self) -> String {
        serde_json::to_string(&crate::state::load_notes(&self.paths))
            .unwrap_or_else(|_| "{}".to_string())
    }

    pub async fn set_notes(&self, body: &str) -> Result<(), String> {
        if body.len() > 1_000_000 {
            return Err(ServiceError::new(codes::SETTINGS_NOTES_TOO_LARGE).into());
        }
        let notes = serde_json::from_str(body)
            .map_err(|_| String::from(ServiceError::new(codes::SETTINGS_NOTES_PARSE)))?;
        crate::state::save_notes(&self.paths, &notes).map_err(String::from)
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
        let text = profile::read_profile_body(&self.paths, id)
            .map_err(|error| profile::explain_io("read profile", &error))?;
        let groups = profile::selector_groups(&text);
        Ok(serde_json::to_string(&groups).unwrap_or_else(|_| "[]".to_string()))
    }

    pub async fn set_active_profile(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let profiles = profile::load_profiles(&self.paths);
        if !profiles.iter().any(|profile| profile.id == id) {
            return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
        }
        let restart = {
            let mut inner = self.inner.lock().await;
            inner.settings.active_profile = id.to_string();
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            inner.clear_message();
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
            return Err(ServiceError::new(codes::CORE_NOT_RUNNING).into());
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
        inner.clear_message();
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
            return Err(ServiceError::new(codes::CORE_NOT_RUNNING).into());
        }
        let controller = Controller::new(port, &secret);
        let groups = controller.proxies().await?;
        let Some(found) = groups.into_iter().find(|item| item.name == group) else {
            return Err(ServiceError::new(codes::GROUP_NOT_FOUND).into());
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
            return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
        }
        profile::read_profile_body(&self.paths, id)
            .map_err(|error| profile::explain_io("read profile", &error))
            .map_err(String::from)
    }

    pub async fn set_profile_body(self: &Arc<Self>, id: &str, body: String) -> Result<(), String> {
        if body.len() > 8_000_000 {
            return Err(ServiceError::new(codes::PROFILE_TOO_LARGE).into());
        }
        if !profile::load_profiles(&self.paths)
            .iter()
            .any(|profile| profile.id == id)
        {
            return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
        }
        let text = profile::subscription_yaml(&body)?;
        profile::write_profile_body(&self.paths, id, &text)
            .map_err(|error| profile::explain_io("write profile", &error))?;
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
            return Err(ServiceError::new(codes::PROFILE_NAME_EMPTY).into());
        }
        if name.chars().count() > 128 {
            return Err(ServiceError::new(codes::PROFILE_NAME_TOO_LONG)
                .param("limit", "128")
                .into());
        }
        if url.chars().count() > 2048 {
            return Err(ServiceError::new(codes::PROFILE_URL_TOO_LONG)
                .param("limit", "2048")
                .into());
        }
        if !url.is_empty() && !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(ServiceError::new(codes::PROFILE_URL_SCHEME).into());
        }
        let mut profiles = profile::load_profiles(&self.paths);
        let Some(profile) = profiles.iter_mut().find(|profile| profile.id == id) else {
            return Err(ServiceError::new(codes::PROFILE_NOT_FOUND).into());
        };
        profile.name = name;
        profile.url = url;
        profile::save_profiles(&self.paths, &profiles).map_err(|error| error.to_string())?;
        self.inner.lock().await.clear_message();
        self.emit(Event::Profiles);
        self.emit(Event::Status);
        Ok(())
    }

    pub async fn import_profile(&self, name: String, path: String) -> Result<String, String> {
        let raw = std::fs::read_to_string(&path).map_err(|error| {
            ServiceError::new(codes::PROFILE_IMPORT_READ).detail(error.to_string())
        })?;
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
                    .unwrap_or("Imported profile")
                    .to_string()
            }
        };
        if name.chars().count() > 128 {
            return Err(ServiceError::new(codes::PROFILE_NAME_TOO_LONG)
                .param("limit", "128")
                .into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        profile::write_profile_body(&self.paths, &id, &text)
            .map_err(|error| profile::explain_io("write profile", &error))?;
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
        inner.clear_message();
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
                    inner.clear_message();
                    drop(inner);
                    self.emit(Event::Status);
                }
                Ok(region)
            }
            Err(error) => {
                if self.region_epoch.load(Ordering::Relaxed) == epoch {
                    self.set_message(String::from(error.clone())).await;
                    self.emit(Event::Status);
                }
                Err(String::from(error))
            }
        }
    }

    fn schedule_region_lookup(self: &Arc<Self>) {
        let epoch = self.region_epoch.fetch_add(1, Ordering::Relaxed) + 1;
        let app = Arc::clone(self);
        tokio::spawn(async move {
            let probe = {
                let mut inner = app.inner.lock().await;
                inner.exit_region = "detecting".to_string();
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
                    inner.clear_message();
                }
                Err(error) => {
                    inner.exit_region.clear();
                    inner.set_message(String::from(error));
                }
            }
            drop(inner);
            app.emit(Event::Status);
        });
    }

    async fn reload_active_if_running(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let active = {
            let mut inner = self.inner.lock().await;
            inner.clear_message();
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
            inner.clear_message();
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
                Err(_) => return Err(ServiceError::new(codes::TUN_REVOKE_TIMEOUT).into()),
            }
        }
        Ok(())
    }

    pub async fn tun_health_interval_secs(&self) -> u64 {
        self.inner
            .lock()
            .await
            .settings
            .tun_health_secs
            .clamp(1, 60) as u64
    }

    /// Re-read the TUN policy rule and table while the core is running.
    ///
    /// Tailscale and firewalls can rewrite policy routing after startup. This
    /// turns that into an observable status instead of silent traffic bypass.
    pub async fn refresh_tun_health(&self) {
        let (enabled, running, options) = {
            let inner = self.inner.lock().await;
            (
                inner.settings.tun_enabled,
                inner.core_running,
                TunOptions::from(&inner.settings),
            )
        };
        if !enabled || !running {
            let changed = {
                let mut inner = self.inner.lock().await;
                let changed = inner.tun_route_ready
                    || !inner.tun_route_known
                    || !inner.tun_route_detail.is_empty()
                    || inner.tun_route_checked_at != 0;
                inner.tun_route_ready = true;
                inner.tun_route_known = true;
                inner.tun_route_detail.clear();
                inner.tun_route_checked_at = 0;
                changed
            };
            if changed {
                self.emit(Event::Status);
            }
            return;
        }

        let health = probe_health(options).await;
        let detail = if health.known && !health.ready {
            format!("{}; check UFW/firewalld or Tailscale", health.detail)
        } else {
            health.detail.clone()
        };
        let changed = {
            let mut inner = self.inner.lock().await;
            let previous_ready = inner.tun_route_ready;
            let changed = inner.tun_route_ready != health.ready
                || inner.tun_route_known != health.known
                || inner.tun_route_detail != detail;
            inner.tun_route_ready = health.ready;
            inner.tun_route_known = health.known;
            inner.tun_route_detail = detail.clone();
            inner.tun_route_checked_at = profile::now_unix();
            if changed {
                if health.known && !health.ready {
                    tracing::warn!("TUN policy route is missing: {detail}");
                } else if health.known && !previous_ready {
                    tracing::info!("TUN policy route recovered: {detail}");
                }
            }
            changed
        };
        if changed {
            self.emit(Event::Status);
        }
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
        let tun_active = inner.settings.tun_enabled && inner.core_running;
        let (tun_route_ready, tun_route_known, tun_route_detail, tun_route_checked_at) =
            if tun_active {
                (
                    inner.tun_route_ready,
                    inner.tun_route_known,
                    inner.tun_route_detail.clone(),
                    inner.tun_route_checked_at,
                )
            } else {
                (true, true, String::new(), 0)
            };
        let status = Status {
            core_desired: inner.settings.core_desired,
            core_running: inner.core_running,
            system_proxy_enabled: inner.settings.system_proxy_enabled,
            system_proxy_applied: inner.system_proxy_applied,
            tun_enabled: inner.settings.tun_enabled,
            tun_machine: inner.settings.tun_machine,
            tun_route_table: inner.settings.tun_route_table,
            tun_rule_index: inner.settings.tun_rule_index,
            tun_exclude_private: inner.settings.tun_exclude_private,
            tun_route_ready,
            tun_route_known,
            tun_route_detail,
            tun_route_checked_at,
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
            message_code: inner.message_code.clone(),
            message_detail: inner.message_detail.clone(),
            message_params: inner.message_params.clone(),
            phase: inner.phase.clone(),
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
            speed_test_mib: inner.settings.speed_test_mib,
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
        self.set_phase(PHASE_STARTING).await;
        tracing::info!(
            "preparing core: binary={}, mixed_port={}, controller_port={}, profile={}",
            settings.mihomo_path,
            settings.mixed_port,
            settings.controller_port,
            settings.active_profile
        );
        let binary = match core::resolve_binary(&settings.mihomo_path) {
            Ok(binary) => binary,
            Err(error) => return self.fail_before_start(String::from(error)).await,
        };
        if let Err(error) = profile::write_runtime_config(&self.paths, &settings) {
            return self.fail_before_start(String::from(error)).await;
        }
        // A restart closes the old listener. Stop advertising it before the
        // stop/launch window; sync_system_proxy puts it back only after the new
        // controller answers.
        self.withdraw_proxy().await;
        if let Err(error) = self.stop_process().await {
            self.mark_stopped(error.clone()).await;
            self.emit(Event::Status);
            return Err(ServiceError::new(codes::CORE_CHECK).detail(error).into());
        }

        // mihomo exits immediately when another program (often mihomo-party)
        // still owns one of its ports. Check before launching so the UI gets a
        // useful message instead of a timeout.
        let lan_open = crate::lan::permitted(&crate::lan::policy_dir(), crate::lan::current_uid())
            && settings.allow_lan;
        let mixed_host = if lan_open { "0.0.0.0" } else { "127.0.0.1" };
        let mut busy = Vec::new();
        for attempt in 0..10 {
            busy.clear();
            if !core::port_available(mixed_host, settings.mixed_port) {
                busy.push(format!("mixed port {}", settings.mixed_port));
            }
            if !core::port_available("127.0.0.1", settings.controller_port) {
                busy.push(format!("controller port {}", settings.controller_port));
            }
            if busy.is_empty() {
                break;
            }
            if attempt < 9 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        if !busy.is_empty() {
            return self
                .fail_before_start(String::from(
                    ServiceError::new(codes::CORE_PORT_BUSY).param("ports", busy.join(", ")),
                ))
                .await;
        }

        let geodata = core::geodata_sources();
        match core::geodata_mode(&geodata) {
            "dat" => tracing::info!("no local MMDB; using geoip.dat/geosite.dat"),
            "download" => {
                tracing::warn!(
                    "no usable local geodata; mihomo may download MMDB/GeoSite on first start"
                )
            }
            _ => {}
        }

        let launch = self.launch_process(settings, binary).await;
        if let Err(error) = launch {
            self.withdraw_proxy().await;
            self.mark_stopped(error.clone()).await;
            self.emit(Event::Status);
            return Err(ServiceError::new(codes::CORE_CHECK).detail(error).into());
        }
        self.sync_system_proxy().await?;
        self.schedule_region_lookup();
        self.emit(Event::Status);
        self.emit(Event::Proxies);
        Ok(())
    }

    async fn fail_before_start(&self, error: String) -> Result<(), String> {
        let error = if error.trim().is_empty() {
            "The core failed to start (no error message was returned).".to_string()
        } else {
            error
        };
        tracing::error!("core failed to start: {error}");
        // resolve_binary/write_runtime_config can fail before the old process
        // is stopped. Tear it down first so a failed restart cannot leave a
        // running core behind a "failed" status.
        self.withdraw_proxy().await;
        if let Err(stop_error) = self.stop_process().await {
            tracing::warn!("could not stop the old core: {stop_error}");
        }
        self.set_message(error.clone()).await;
        self.set_phase(PHASE_FAILED).await;
        self.emit(Event::Status);
        Err(error)
    }

    async fn wait_until_ready(self: &Arc<Self>, process: &mut CoreProcess) -> Result<(), String> {
        let started = Instant::now();
        let limit = Duration::from_secs(180);
        let mut announced = String::from(ServiceError::new(codes::CORE_STARTING));
        self.set_message(announced.clone()).await;
        self.set_phase(PHASE_STARTING).await;
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
                        tracing::error!("TUN failed to start: {error}");
                        return Err(String::from(
                            ServiceError::new(codes::CORE_TUN_LISTEN).detail(error),
                        ));
                    }
                }
                let (tun, options) = {
                    let inner = self.inner.lock().await;
                    (
                        inner.settings.tun_enabled,
                        TunOptions::from(&inner.settings),
                    )
                };
                if tun {
                    let expected = options.expected_device().to_string();
                    let route_started = Instant::now();
                    loop {
                        let health = probe_health(options.clone()).await;
                        if health.ready {
                            if !health.known {
                                tracing::warn!("TUN route was not verified: {}", health.detail);
                            }
                            let mut inner = self.inner.lock().await;
                            inner.tun_route_ready = true;
                            inner.tun_route_known = health.known;
                            inner.tun_route_detail = health.detail.clone();
                            inner.tun_route_checked_at = profile::now_unix();
                            break;
                        }
                        if let Ok(Some(status)) = process.try_wait() {
                            return Err(String::from(
                                ServiceError::new(codes::CORE_EXITED).detail(format!(
                                    "mihomo exited before TUN was ready: {status}"
                                )),
                            ));
                        }
                        if route_started.elapsed() >= Duration::from_secs(tun::ROUTE_GRACE_SECS) {
                            // TUN traffic can still work through auto-redirect even when the
                            // dedicated table has no default route. Keep the core running and
                            // let the periodic health check clear or report the degraded state.
                            tracing::warn!(
                                "TUN route is not ready (degraded): {}; if the mihomo device is not {expected}, set tun_device",
                                health.detail
                            );
                            let mut inner = self.inner.lock().await;
                            inner.tun_route_ready = false;
                            inner.tun_route_known = health.known;
                            inner.tun_route_detail = health.detail.clone();
                            inner.tun_route_checked_at = profile::now_unix();
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                }
                return Ok(());
            }
            match process.try_wait() {
                Ok(Some(status)) => {
                    tracing::error!("mihomo exited before the controller became ready: {status}");
                    return Err(String::from(
                        ServiceError::new(codes::CORE_EXITED).detail(status.to_string()),
                    ));
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(ServiceError::new(codes::CORE_CHECK)
                        .detail(error.to_string())
                        .into());
                }
            }
            if started.elapsed() >= limit {
                tracing::error!(
                    "waiting for the mihomo controller timed out ({} seconds)",
                    limit.as_secs()
                );
                return Err(String::from(ServiceError::new(
                    codes::CORE_CONTROLLER_TIMEOUT,
                )));
            }
            let hint = core::startup_hint(&self.paths.core_log());
            let hint_json = String::from(hint.clone());
            if hint_json != announced {
                announced = hint_json;
                let phase = if core::startup_downloading(&self.paths.core_log()) {
                    PHASE_DOWNLOADING
                } else {
                    PHASE_STARTING
                };
                tracing::info!("core startup progress: {hint}");
                self.set_message(String::from(hint)).await;
                self.set_phase(phase).await;
                self.emit(Event::Status);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
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
            tracing::info!("started the TUN core through the system service: pid={pid}");
            core::CoreProcess::adopt(pid)
        } else {
            let process = core::CoreProcess::spawn(&self.paths, &binary)?;
            tracing::info!("started the core: pid={}", process.pid());
            process
        };
        let controller = Controller::new(settings.controller_port, &settings.controller_secret);
        if let Err(error) = self.wait_until_ready(&mut process).await {
            if matches!(process.try_wait(), Ok(None)) {
                process.terminate().await;
            }
            return Err(ServiceError::new(codes::CORE_CHECK).detail(error).into());
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
            inner.clear_message();
            inner.phase = PHASE_RUNNING.to_string();
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
                        tracing::info!("mihomo exited: {status}");
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
                return Err(ServiceError::new(codes::CORE_STOP_TIMEOUT).into());
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
                ExitKind::Requested => {
                    inner.phase = PHASE_STOPPED.to_string();
                    false
                }
                ExitKind::Unexpected
                    if inner.settings.core_desired && inner.restart_attempts < 3 =>
                {
                    inner.restart_attempts += 1;
                    inner.phase = PHASE_STARTING.to_string();
                    tracing::warn!(
                        "core exited unexpectedly; retrying (attempt {})",
                        inner.restart_attempts
                    );
                    true
                }
                ExitKind::Unexpected => {
                    inner.exit_region.clear();
                    inner.set_message(String::from(
                        ServiceError::new(codes::CORE_EXITED)
                            .detail(core::log_tail(&self.paths.core_log())),
                    ));
                    inner.phase = PHASE_FAILED.to_string();
                    tracing::error!(
                        "core exited unexpectedly; stopped retrying: {}",
                        inner.message
                    );
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
                        tracing::error!("could not release the edited system proxy: {error}");
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
            tracing::error!("could not save the system-proxy preference: {error}");
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
            proxy_settings::apply_if_needed(&self.paths, port).map_err(ServiceError::from_io)?;
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
            tracing::error!("could not restore the KDE proxy: {error}");
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
        inner.set_message(message);
        inner.phase = PHASE_FAILED.to_string();
    }

    async fn set_message(&self, message: String) {
        self.inner.lock().await.set_message(message);
    }

    async fn set_phase(&self, phase: &str) {
        self.inner.lock().await.phase = phase.to_string();
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
