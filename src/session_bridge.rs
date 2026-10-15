use std::process::ExitCode;

use serde::Serialize;
use zbus::fdo::DBusProxy;

use crate::paths::Paths;
use crate::privilege::{self, Account};
use crate::state::Settings;

pub fn run(args: &[String]) -> ExitCode {
    let Some(uid) = args.get(1).and_then(|text| text.parse::<u32>().ok()) else {
        eprintln!("缺少账户");
        return ExitCode::from(1);
    };
    let Some(action) = args.get(2).map(String::as_str) else {
        eprintln!("缺少动作");
        return ExitCode::from(1);
    };
    if let Err(error) = switch_account(uid) {
        eprintln!("{error}");
        return ExitCode::from(1);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    match runtime.block_on(call(uid, action)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn switch_account(uid: u32) -> Result<(), String> {
    let current = unsafe { libc::geteuid() };
    if current != 0 && current != uid {
        return Err("无法切换到这个账户".to_string());
    }
    let account = privilege::lookup_account(uid)?;
    if current == 0 {
        let c_name =
            std::ffi::CString::new(account.name.clone()).map_err(|_| "账户名无效".to_string())?;
        if unsafe { libc::initgroups(c_name.as_ptr(), account.gid) } != 0 {
            return Err("无法切换到这个账户".to_string());
        }
        if unsafe { libc::setgid(account.gid) } != 0 || unsafe { libc::setuid(uid) } != 0 {
            return Err("无法切换到这个账户".to_string());
        }
    }
    unsafe {
        std::env::set_var("HOME", &account.home);
        std::env::set_var("USER", &account.name);
        std::env::set_var("LOGNAME", &account.name);
        std::env::set_var("XDG_RUNTIME_DIR", format!("/run/user/{uid}"));
        std::env::set_var(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path=/run/user/{uid}/bus"),
        );
    }
    Ok(())
}

async fn call(uid: u32, action: &str) -> Result<(), String> {
    let account = privilege::lookup_account(uid)?;
    match action {
        "Status" => print_status(&account).await,
        "Start" => session_method(true).await,
        "Stop" => session_method(false).await,
        _ => Err("缺少动作".to_string()),
    }
}

async fn print_status(account: &Account) -> Result<(), String> {
    let slim = match live_status().await {
        Some(slim) => slim,
        None => saved_status(account),
    };
    println!("{}", serde_json::to_string(&slim).expect("status json"));
    Ok(())
}

async fn live_status() -> Option<SlimStatus> {
    let connection = zbus::connection::Builder::session()
        .ok()?
        .build()
        .await
        .ok()?;
    let dbus = DBusProxy::new(&connection).await.ok()?;
    let owned = dbus
        .name_has_owner("org.kde.clashkde".try_into().ok()?)
        .await
        .ok()?;
    if !owned {
        return None;
    }
    let proxy = SessionManagerProxy::new(&connection).await.ok()?;
    let text = proxy.get_status().await.ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(SlimStatus {
        core_running: value
            .get("core_running")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        tun_enabled: value
            .get("tun_enabled")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        system_proxy_enabled: value
            .get("system_proxy_enabled")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        mode: value
            .get("mode")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

fn saved_status(account: &Account) -> SlimStatus {
    let paths = Paths {
        config_dir: account.home.join(".config/clash-kde"),
        data_dir: account.home.join(".local/share/clash-kde"),
        kioslaverc: account.home.join(".config/kioslaverc"),
    };
    let settings = Settings::load(&paths).unwrap_or_default();
    SlimStatus {
        core_running: false,
        tun_enabled: settings.tun_enabled,
        system_proxy_enabled: settings.system_proxy_enabled,
        mode: settings.mode,
    }
}

async fn session_method(start: bool) -> Result<(), String> {
    let connection = zbus::connection::Builder::session()
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;
    let proxy = SessionManagerProxy::new(&connection)
        .await
        .map_err(|error| error.to_string())?;
    let result = if start {
        proxy.start().await
    } else {
        proxy.stop().await
    };
    result.map_err(|error| error.to_string())
}

#[derive(Serialize)]
struct SlimStatus {
    core_running: bool,
    tun_enabled: bool,
    system_proxy_enabled: bool,
    mode: String,
}

#[zbus::proxy(
    interface = "org.kde.clashkde.Manager",
    default_service = "org.kde.clashkde",
    default_path = "/org/kde/clashkde"
)]
trait SessionManager {
    fn start(&self) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    fn get_status(&self) -> zbus::Result<String>;
}
