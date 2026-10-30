// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

use std::process::ExitCode;

use serde::Serialize;
use zbus::fdo::DBusProxy;

use crate::error::{ServiceError, codes};
use crate::paths::Paths;
use crate::privilege::{self, Account};
use crate::state::Settings;

pub fn run(args: &[String]) -> ExitCode {
    let Some(uid) = args.get(1).and_then(|text| text.parse::<u32>().ok()) else {
        print_error(&ServiceError::new(codes::BRIDGE_ACCOUNT));
        return ExitCode::from(1);
    };
    let Some(action) = args.get(2).map(String::as_str) else {
        print_error(&ServiceError::new(codes::BRIDGE_ACTION));
        return ExitCode::from(1);
    };
    if let Err(error) = switch_account(uid) {
        print_error(&error);
        return ExitCode::from(1);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            print_error(&ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()));
            return ExitCode::from(1);
        }
    };
    match runtime.block_on(call(uid, action)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            print_error(&error);
            ExitCode::from(1)
        }
    }
}

fn print_error(error: &ServiceError) {
    eprintln!(
        "{}",
        serde_json::to_string(error).unwrap_or_else(|_| error.code.clone())
    );
}

fn switch_account(uid: u32) -> Result<(), ServiceError> {
    let current = unsafe { libc::geteuid() };
    if current != 0 && current != uid {
        return Err(ServiceError::new(codes::BRIDGE_SWITCH));
    }
    let account = privilege::lookup_account(uid)?;
    if current == 0 {
        let c_name = std::ffi::CString::new(account.name.clone())
            .map_err(|_| ServiceError::new(codes::TUN_ACCOUNT_NAME))?;
        if unsafe { libc::initgroups(c_name.as_ptr(), account.gid) } != 0 {
            return Err(ServiceError::new(codes::BRIDGE_SWITCH));
        }
        if unsafe { libc::setgid(account.gid) } != 0 || unsafe { libc::setuid(uid) } != 0 {
            return Err(ServiceError::new(codes::BRIDGE_SWITCH));
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

async fn call(uid: u32, action: &str) -> Result<(), ServiceError> {
    let account = privilege::lookup_account(uid)?;
    match action {
        "Status" => print_status(&account).await,
        "Start" => session_method(true).await,
        "Stop" => session_method(false).await,
        "Reload" => reload_runtime().await,
        _ => Err(ServiceError::new(codes::BRIDGE_ACTION)),
    }
}

async fn print_status(account: &Account) -> Result<(), ServiceError> {
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
        tun_machine: value
            .get("tun_machine")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        exit_region: value
            .get("exit_region")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
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
        tun_machine: settings.tun_machine,
        exit_region: String::new(),
        system_proxy_enabled: settings.system_proxy_enabled,
        mode: settings.mode,
    }
}

async fn session_method(start: bool) -> Result<(), ServiceError> {
    let connection = zbus::connection::Builder::session()
        .map_err(|error| ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()))?
        .build()
        .await
        .map_err(|error| ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()))?;
    let proxy = SessionManagerProxy::new(&connection)
        .await
        .map_err(|error| ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()))?;
    let result = if start {
        proxy.start().await
    } else {
        proxy.stop().await
    };
    result.map_err(|error| ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()))
}

async fn reload_runtime() -> Result<(), ServiceError> {
    let connection = zbus::connection::Builder::session()
        .map_err(|error| ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()))?
        .build()
        .await
        .map_err(|error| ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()))?;
    let proxy = SessionManagerProxy::new(&connection)
        .await
        .map_err(|error| ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()))?;
    proxy
        .reload_runtime()
        .await
        .map_err(|error| ServiceError::new(codes::BRIDGE_SESSION).detail(error.to_string()))
}

#[derive(Serialize)]
struct SlimStatus {
    core_running: bool,
    tun_enabled: bool,
    tun_machine: bool,
    exit_region: String,
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
    fn reload_runtime(&self) -> zbus::Result<()>;
}
