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

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Status,
    Profiles,
    Proxies,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExitKind {
    Requested,
    Unexpected,
}

struct Running {
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

pub struct App {
    paths: Paths,
    inner: Mutex<Inner>,
    generation: AtomicU64,
    events: broadcast::Sender<Event>,
    exit_tx: mpsc::UnboundedSender<ExitNotice>,
    exit_rx: Mutex<Option<mpsc::UnboundedReceiver<ExitNotice>>>,
}

#[derive(Serialize)]
struct Status {
    core_desired: bool,
    core_running: bool,
    system_proxy_enabled: bool,
    system_proxy_applied: bool,
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
}

impl App {
    pub fn new(paths: Paths) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(&paths.config_dir).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(&paths.data_dir).map_err(|error| error.to_string())?;
        profile::ensure_override_file(&paths.override_file()).map_err(|error| error.to_string())?;
        let mut settings = Settings::load(&paths).map_err(|error| error.to_string())?;
        if state::ensure_secret(&mut settings) {
            settings.save(&paths).map_err(|error| error.to_string())?;
        }
        let (events, _) = broadcast::channel(32);
        let (exit_tx, exit_rx) = mpsc::unbounded_channel();
        Ok(Arc::new(Self {
            paths: paths.clone(),
            generation: AtomicU64::new(1),
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
        } else if self.inner.lock().await.system_proxy_applied {
            self.withdraw_proxy().await;
            self.emit(Event::Status);
        }
    }

    pub async fn shutdown(&self) {
        self.stop_process().await;
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

    pub async fn stop(&self) -> Result<(), String> {
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
        self.stop_process().await;
        self.withdraw_proxy().await;
        self.emit(Event::Status);
        Ok(())
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

    pub async fn set_system_proxy(&self, enabled: bool) -> Result<(), String> {
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
        self.emit(Event::Status);
        Ok(())
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
    ) -> Result<(), String> {
        let (mixed_port, controller_port) = state::validate_ports(mixed_port, controller_port)?;
        let mihomo_path = mihomo_path.trim().to_string();
        if mihomo_path.is_empty() {
            return Err("mihomo 路径不能为空".to_string());
        }
        let should_restart = {
            let mut inner = self.inner.lock().await;
            inner.settings.mixed_port = mixed_port;
            inner.settings.controller_port = controller_port;
            inner.settings.mihomo_path = mihomo_path;
            inner
                .settings
                .save(&self.paths)
                .map_err(|error| error.to_string())?;
            inner.message.clear();
            inner.core_running
        };
        if should_restart {
            self.spawn_core().await?;
        } else {
            self.sync_system_proxy().await?;
            self.emit(Event::Status);
        }
        Ok(())
    }

    pub async fn profiles_json(&self) -> String {
        serde_json::to_string(&profile::load_profiles(&self.paths)).expect("profiles json")
    }

    pub async fn add_subscription(&self, name: String, url: String) -> Result<String, String> {
        let url = url.trim().to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("订阅地址需要以 http:// 或 https:// 开头".to_string());
        }
        let body = controller::fetch_subscription(&url).await?;
        let text = profile::subscription_yaml(&body)?;
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
                self.stop_process().await;
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
        let body = controller::fetch_subscription(&profile.url).await?;
        let text = profile::subscription_yaml(&body)?;
        profile::write_profile_body(&self.paths, id, &text).map_err(|error| error.to_string())?;
        let mut profiles = profile::load_profiles(&self.paths);
        if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == id) {
            profile.updated_unix = profile::now_unix();
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

    pub async fn select_proxy(&self, group: &str, name: &str) -> Result<(), String> {
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
        Ok(())
    }

    pub async fn test_group_delay(&self, group: &str) -> Result<(), String> {
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
        let mut tasks = Vec::new();
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
            tasks.push(tokio::spawn(async move {
                let _permit = permit.acquire_owned().await;
                controller.test_delay(&name).await
            }));
        }
        for task in tasks {
            let _ = task.await;
        }
        self.emit(Event::Proxies);
        Ok(())
    }

    pub async fn refresh_exit_region(&self) -> Result<String, String> {
        let (running, port) = {
            let inner = self.inner.lock().await;
            (inner.core_running, inner.settings.mixed_port)
        };
        if !running {
            return Err("内核没有在运行".to_string());
        }
        match controller::lookup_exit_region(port).await {
            Ok(region) => {
                self.inner.lock().await.exit_region = region.clone();
                self.inner.lock().await.message.clear();
                self.emit(Event::Status);
                Ok(region)
            }
            Err(error) => {
                self.set_message(error.clone()).await;
                self.emit(Event::Status);
                Err(error)
            }
        }
    }

    pub async fn status_json(&self) -> String {
        let inner = self.inner.lock().await;
        let status = Status {
            core_desired: inner.settings.core_desired,
            core_running: inner.core_running,
            system_proxy_enabled: inner.settings.system_proxy_enabled,
            system_proxy_applied: inner.system_proxy_applied,
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
            override_path: self.paths.override_file().display().to_string(),
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
        self.stop_process().await;

        let launch = self.launch_process(settings, binary).await;
        if let Err(error) = launch {
            self.withdraw_proxy().await;
            self.mark_stopped(error.clone()).await;
            self.emit(Event::Status);
            return Err(error);
        }
        self.sync_system_proxy().await?;
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
            tokio::time::sleep(Duration::from_millis(400)).await;
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
        let mut process = CoreProcess::spawn(&self.paths, &binary)?;
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
                let (running, port) = {
                    let inner = app.inner.lock().await;
                    (inner.core_running, inner.settings.mixed_port)
                };
                if !running {
                    break;
                }
                if let Ok(region) = controller::lookup_exit_region(port).await {
                    app.inner.lock().await.exit_region = region;
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
            if group.nodes.iter().any(|node| &node.name == selected) {
                if let Err(error) = controller.select_proxy(&group.name, selected).await {
                    tracing::warn!("{error}");
                }
            }
        }
    }

    async fn stop_process(&self) {
        let running = {
            let mut inner = self.inner.lock().await;
            inner.core_running = false;
            inner.upload_bps = 0;
            inner.download_bps = 0;
            inner.running.take()
        };
        if let Some(running) = running {
            if let Some(task) = running.traffic {
                task.abort();
            }
            if let Some(task) = running.region {
                task.abort();
            }
            let _ = running.kill.send(());
            let _ = tokio::time::timeout(Duration::from_secs(3), running.done).await;
        }
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

    async fn sync_system_proxy(&self) -> Result<(), String> {
        let (enabled, running, port) = {
            let inner = self.inner.lock().await;
            (
                inner.settings.system_proxy_enabled,
                inner.core_running,
                inner.settings.mixed_port,
            )
        };
        if enabled && running {
            proxy_settings::apply_if_needed(&self.paths, port)
                .map_err(|error| error.to_string())?;
            self.inner.lock().await.system_proxy_applied = true;
        } else {
            self.withdraw_proxy().await;
        }
        Ok(())
    }

    async fn withdraw_proxy(&self) {
        if let Err(error) = proxy_settings::restore_if_applied(&self.paths) {
            tracing::error!("恢复 KDE 代理失败: {error}");
            return;
        }
        self.inner.lock().await.system_proxy_applied = false;
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
