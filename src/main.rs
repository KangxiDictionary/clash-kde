//! Session service that supervises a per-user mihomo process.
//!
//! The KDE settings module talks to `org.kde.clashkde` on the session bus.
//! Start, stop, restart, system-proxy and mode methods stay stable so a later
//! system service can call the same interface for each user.

mod app;
mod controller;
mod core;
mod dbus_api;
mod paths;
mod profile;
mod proxy_settings;
mod state;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if std::env::args().skip(1).any(|arg| arg == "--restore-proxy") {
        return match proxy_settings::restore_if_applied(&paths::Paths::from_env()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                tracing::error!("{error}");
                ExitCode::from(1)
            }
        };
    }

    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
    {
        eprintln!("Usage: clash-kde [--restore-proxy]");
        return ExitCode::SUCCESS;
    }

    match dbus_api::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::from(1)
        }
    }
}
