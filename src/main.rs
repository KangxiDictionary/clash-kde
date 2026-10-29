// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

//! Session service that supervises a per-user mihomo process.
//!
//! The KDE settings module talks to `org.kde.clashkde` on the session bus.
//! Start, stop, restart, system-proxy and mode methods stay stable so the
//! system service can call the same interface for each signed-in user.

mod admin;
mod app;
mod controller;
mod core;
mod dbus_api;
mod diagnostics;
mod error;
mod lan;
mod paths;
mod privilege;
mod profile;
mod proxy_settings;
mod rules;
mod session_bridge;
mod state;
mod tun;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--as-user") {
        return session_bridge::run(&args);
    }
    if args.first().is_some_and(|arg| arg == "--spawn-tun") {
        let uid = args.get(1).and_then(|text| text.parse().ok()).unwrap_or(0);
        let code = privilege::exec_tun(
            uid,
            std::path::Path::new(args.get(2).map(String::as_str).unwrap_or("")),
            std::path::Path::new(args.get(3).map(String::as_str).unwrap_or("")),
            std::path::Path::new(args.get(4).map(String::as_str).unwrap_or("")),
            std::path::Path::new(args.get(5).map(String::as_str).unwrap_or("")),
        );
        return ExitCode::from(code as u8);
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        eprintln!("Usage: clash-kde [--diagnose | --restore-proxy]");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|arg| arg == "--diagnose") {
        return match diagnostics::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        };
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build();
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    runtime.block_on(async_main(args))
}

async fn async_main(args: Vec<String>) -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if args.iter().any(|arg| arg == "--restore-proxy") {
        let paths = paths::Paths::from_env();
        let port = state::Settings::load(&paths)
            .map(|settings| settings.mixed_port)
            .unwrap_or(7890);
        return match proxy_settings::restore_if_applied(&paths, port) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                tracing::error!("{error}");
                ExitCode::from(1)
            }
        };
    }

    if args.iter().any(|arg| arg == "--system") {
        return match admin::run().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                tracing::error!("{error}");
                ExitCode::from(1)
            }
        };
    }

    match dbus_api::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::from(1)
        }
    }
}
