use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{Mutex, oneshot};
use zbus::fdo;
use zbus::fdo::DBusProxy;
use zbus::message::Header;

use crate::privilege;

const POLKIT_TUN: &str = "org.kde.clashkde.tun";
const POLKIT_USERS: &str = "org.kde.clashkde.manage-users";

struct Slot {
    stop: oneshot::Sender<()>,
    done: oneshot::Receiver<()>,
}

#[derive(Default)]
struct TunTable {
    slots: Mutex<HashMap<u32, Slot>>,
}

impl TunTable {
    async fn replace(&self, uid: u32, stop: oneshot::Sender<()>, done: oneshot::Receiver<()>) {
        if let Some(previous) = self.slots.lock().await.insert(uid, Slot { stop, done }) {
            let _ = previous.stop.send(());
            let _ = tokio::time::timeout(Duration::from_secs(3), previous.done).await;
        }
    }

    async fn stop_all(&self) {
        let slots: Vec<Slot> = self
            .slots
            .lock()
            .await
            .drain()
            .map(|(_, slot)| slot)
            .collect();
        for slot in slots {
            let _ = slot.stop.send(());
            let _ = tokio::time::timeout(Duration::from_secs(3), slot.done).await;
        }
    }
}

pub struct Admin {
    tun: Arc<TunTable>,
}

#[zbus::interface(name = "org.kde.clashkde.Admin")]
impl Admin {
    #[zbus(name = "AuthorizeTun")]
    async fn authorize_tun(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        let (uid, sender) = caller(&header, connection).await?;
        allow(&sender, POLKIT_TUN).await?;
        privilege::write_stamp(Path::new(privilege::STAMP_DIR), uid)
            .map_err(|error| fdo::Error::Failed(error.to_string()))
    }

    #[zbus(name = "RevokeTun")]
    async fn revoke_tun(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        let (uid, _) = caller(&header, connection).await?;
        privilege::remove_stamp(Path::new(privilege::STAMP_DIR), uid)
            .map_err(|error| fdo::Error::Failed(error.to_string()))
    }

    #[zbus(name = "SpawnTun")]
    async fn spawn_tun(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        binary: String,
        runtime: String,
        config: String,
        log: String,
    ) -> fdo::Result<u32> {
        let (uid, _) = caller(&header, connection).await?;
        if !privilege::stamp_exists(Path::new(privilege::STAMP_DIR), uid) {
            return Err(fdo::Error::Failed("TUN 还没有得到授权".to_string()));
        }
        privilege::ensure_tun_launch(
            Path::new(&binary),
            Path::new(&runtime),
            Path::new(&config),
            uid,
        )
        .map_err(fdo::Error::Failed)?;
        privilege::log_path_is_runtime(Path::new(&log), Path::new(&runtime))
            .map_err(fdo::Error::Failed)?;
        let (pid, child) = launch(uid, &binary, &runtime, &config, &log)
            .await
            .map_err(fdo::Error::Failed)?;
        let (stop_tx, stop_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        self.tun.replace(uid, stop_tx, done_rx).await;
        tokio::spawn(async move {
            supervise(child, stop_rx).await;
            let _ = done_tx.send(());
        });
        Ok(pid)
    }

    #[zbus(name = "List")]
    async fn list(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<String> {
        let (_, sender) = caller(&header, connection).await?;
        allow(&sender, POLKIT_USERS).await?;
        let mut users = Vec::new();
        for uid in privilege::signed_in_uids() {
            users.push(describe(uid).await);
        }
        Ok(serde_json::to_string(&users).expect("user list"))
    }

    #[zbus(name = "Start")]
    async fn start(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        uid: u32,
    ) -> fdo::Result<()> {
        let (_, sender) = caller(&header, connection).await?;
        allow(&sender, POLKIT_USERS).await?;
        bridge(uid, "Start").await.map_err(fdo::Error::Failed)?;
        Ok(())
    }

    #[zbus(name = "Stop")]
    async fn stop(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        uid: u32,
    ) -> fdo::Result<()> {
        let (_, sender) = caller(&header, connection).await?;
        allow(&sender, POLKIT_USERS).await?;
        bridge(uid, "Stop").await.map_err(fdo::Error::Failed)?;
        Ok(())
    }
}

pub async fn run() -> Result<(), String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("系统服务需要 root".to_string());
    }
    let tun = Arc::new(TunTable::default());
    let connection = zbus::connection::Builder::system()
        .map_err(|error| error.to_string())?
        .name("org.kde.clashkde.Admin")
        .map_err(|error| error.to_string())?
        .serve_at(
            "/org/kde/clashkde",
            Admin {
                tun: Arc::clone(&tun),
            },
        )
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;
    let mut terminate = signal(SignalKind::terminate()).map_err(|error| error.to_string())?;
    let mut interrupt = signal(SignalKind::interrupt()).map_err(|error| error.to_string())?;
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
    }
    tun.stop_all().await;
    drop(connection);
    Ok(())
}

pub async fn authorize_tun() -> Result<(), String> {
    call("AuthorizeTun", &()).await
}

pub async fn revoke_tun() -> Result<(), String> {
    call("RevokeTun", &()).await
}

pub async fn spawn_tun(
    binary: &Path,
    runtime: &Path,
    config: &Path,
    log: &Path,
) -> Result<u32, String> {
    let reply = call_body(
        "SpawnTun",
        &(
            binary.display().to_string(),
            runtime.display().to_string(),
            config.display().to_string(),
            log.display().to_string(),
        ),
    )
    .await?;
    reply
        .body()
        .deserialize()
        .map_err(|error| error.to_string())
}

async fn call(method: &str, body: &()) -> Result<(), String> {
    call_body(method, body).await.map(|_| ())
}

async fn call_body<B>(method: &str, body: &B) -> Result<zbus::message::Message, String>
where
    B: serde::ser::Serialize + zbus::zvariant::DynamicType + Send + Sync,
{
    let connection = zbus::connection::Builder::system()
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;
    connection
        .call_method(
            Some("org.kde.clashkde.Admin"),
            "/org/kde/clashkde",
            Some("org.kde.clashkde.Admin"),
            method,
            body,
        )
        .await
        .map_err(|error| error.to_string())
}

async fn caller(header: &Header<'_>, connection: &zbus::Connection) -> fdo::Result<(u32, String)> {
    let sender = header
        .sender()
        .ok_or_else(|| fdo::Error::Failed("缺少调用方".to_string()))?
        .to_string();
    let dbus = DBusProxy::new(connection)
        .await
        .map_err(|error| fdo::Error::Failed(error.to_string()))?;
    let name = zbus::names::BusName::try_from(sender.as_str())
        .map_err(|error| fdo::Error::Failed(error.to_string()))?;
    let creds = dbus
        .get_connection_credentials(name)
        .await
        .map_err(|error| fdo::Error::Failed(error.to_string()))?;
    let uid = creds
        .unix_user_id()
        .ok_or_else(|| fdo::Error::Failed("缺少调用方".to_string()))?;
    Ok((uid, sender))
}

async fn allow(sender: &str, action: &str) -> fdo::Result<()> {
    let status = Command::new("/usr/bin/pkcheck")
        .args([
            "--action-id",
            action,
            "--system-bus-name",
            sender,
            "--allow-user-interaction",
        ])
        .status()
        .await
        .map_err(|error| fdo::Error::Failed(format!("无法询问授权: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(fdo::Error::Failed("没有得到授权".to_string()))
    }
}

async fn launch(
    uid: u32,
    binary: &str,
    runtime: &str,
    config: &str,
    log: &str,
) -> Result<(u32, Child), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut child = Command::new(exe)
        .arg("--spawn-tun")
        .arg(uid.to_string())
        .arg(binary)
        .arg(runtime)
        .arg(config)
        .arg(log)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("无法启动 mihomo: {error}"))?;
    for _ in 0..10 {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut text = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut text).await;
                }
                let text = text.trim();
                if text.is_empty() {
                    return Err(format!("mihomo 已退出: {status}"));
                }
                return Err(text.to_string());
            }
            Ok(None) => tokio::time::sleep(Duration::from_millis(50)).await,
            Err(error) => return Err(error.to_string()),
        }
    }
    drop(child.stderr.take());
    let pid = child.id().ok_or_else(|| "mihomo 没有进程号".to_string())?;
    Ok((pid, child))
}

async fn supervise(mut child: Child, mut stop: oneshot::Receiver<()>) {
    tokio::select! {
        _ = &mut stop => {
            if let Some(pid) = child.id() {
                unsafe { libc::kill(pid as i32, libc::SIGTERM); }
            }
            tokio::select! {
                _ = child.wait() => {}
                _ = tokio::time::sleep(Duration::from_secs(2)) => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                }
            }
        }
        status = child.wait() => {
            if let Ok(status) = status {
                tracing::info!("mihomo 退出: {status}");
            }
        }
    }
}

async fn bridge(uid: u32, action: &str) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let output = Command::new(exe)
        .arg("--as-user")
        .arg(uid.to_string())
        .arg(action)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .await
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let error = error.trim();
        if error.is_empty() {
            return Err("无法联系这个账户的 Mihomo".to_string());
        }
        return Err(error.to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn describe(uid: u32) -> ListedUser {
    let name = privilege::lookup_account(uid)
        .map(|account| account.name)
        .unwrap_or_else(|_| uid.to_string());
    let parsed = match bridge(uid, "Status").await {
        Ok(text) => serde_json::from_str::<SlimStatus>(&text).ok(),
        Err(error) => {
            tracing::debug!("无法读取账户 {name} 的状态: {error}");
            None
        }
    };
    let status = parsed.unwrap_or(SlimStatus {
        core_running: false,
        tun_enabled: false,
        system_proxy_enabled: false,
        mode: String::new(),
    });
    ListedUser {
        uid,
        name,
        core_running: status.core_running,
        tun_enabled: status.tun_enabled,
        system_proxy_enabled: status.system_proxy_enabled,
        mode: status.mode,
    }
}

#[derive(Serialize)]
struct ListedUser {
    uid: u32,
    name: String,
    core_running: bool,
    tun_enabled: bool,
    system_proxy_enabled: bool,
    mode: String,
}

#[derive(serde::Deserialize)]
struct SlimStatus {
    core_running: bool,
    tun_enabled: bool,
    system_proxy_enabled: bool,
    mode: String,
}
