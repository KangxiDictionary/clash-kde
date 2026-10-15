use std::sync::Arc;

use tokio::signal::unix::{SignalKind, signal};
use zbus::fdo;
use zbus::object_server::SignalEmitter;

use crate::app::{App, Event};
use crate::paths::Paths;

pub struct Manager {
    app: Arc<App>,
}

#[zbus::interface(name = "org.kde.clashkde.Manager")]
impl Manager {
    /// Start the core without changing the KDE system proxy preference.
    #[zbus(name = "Start")]
    async fn start(&self) -> fdo::Result<()> {
        self.app.start().await.map_err(failed)
    }

    /// Stop the core and withdraw a system proxy this service applied.
    #[zbus(name = "Stop")]
    async fn stop(&self) -> fdo::Result<()> {
        self.app.stop().await.map_err(failed)
    }

    #[zbus(name = "Restart")]
    async fn restart(&self) -> fdo::Result<()> {
        self.app.restart().await.map_err(failed)
    }

    #[zbus(name = "SetSystemProxy")]
    async fn set_system_proxy(&self, enabled: bool) -> fdo::Result<()> {
        self.app.set_system_proxy(enabled).await.map_err(failed)
    }

    #[zbus(name = "SetTun")]
    async fn set_tun(&self, enabled: bool) -> fdo::Result<()> {
        self.app.set_tun(enabled).await.map_err(failed)
    }

    #[zbus(name = "SetMode")]
    async fn set_mode(&self, mode: String) -> fdo::Result<()> {
        self.app.set_mode(&mode).await.map_err(failed)
    }

    #[zbus(name = "SetSettings")]
    async fn set_settings(
        &self,
        mixed_port: u32,
        controller_port: u32,
        mihomo_path: String,
    ) -> fdo::Result<()> {
        self.app
            .set_settings(mixed_port, controller_port, mihomo_path)
            .await
            .map_err(failed)
    }

    #[zbus(name = "AddSubscription")]
    async fn add_subscription(&self, name: String, url: String) -> fdo::Result<String> {
        self.app.add_subscription(name, url).await.map_err(failed)
    }

    #[zbus(name = "RemoveProfile")]
    async fn remove_profile(&self, id: String) -> fdo::Result<()> {
        self.app.remove_profile(&id).await.map_err(failed)
    }

    #[zbus(name = "UpdateProfile")]
    async fn update_profile(&self, id: String) -> fdo::Result<()> {
        self.app.update_profile(&id).await.map_err(failed)
    }

    #[zbus(name = "SetActiveProfile")]
    async fn set_active_profile(&self, id: String) -> fdo::Result<()> {
        self.app.set_active_profile(&id).await.map_err(failed)
    }

    #[zbus(name = "GetProfileBody")]
    async fn get_profile_body(&self, id: String) -> fdo::Result<String> {
        self.app.profile_body(&id).await.map_err(failed)
    }

    #[zbus(name = "SetProfileBody")]
    async fn set_profile_body(&self, id: String, body: String) -> fdo::Result<()> {
        self.app.set_profile_body(&id, body).await.map_err(failed)
    }

    #[zbus(name = "CheckProfileText")]
    async fn check_profile_text(&self, body: String) -> fdo::Result<String> {
        Ok(match crate::profile::check_profile_text(&body) {
            Ok(()) => String::new(),
            Err(error) => error,
        })
    }

    #[zbus(name = "SetProfileMeta")]
    async fn set_profile_meta(&self, id: String, name: String, url: String) -> fdo::Result<()> {
        self.app
            .set_profile_meta(&id, name, url)
            .await
            .map_err(failed)
    }

    #[zbus(name = "ImportProfile")]
    async fn import_profile(&self, name: String, path: String) -> fdo::Result<String> {
        self.app.import_profile(name, path).await.map_err(failed)
    }

    #[zbus(name = "SelectProxy")]
    async fn select_proxy(&self, group: String, name: String) -> fdo::Result<()> {
        self.app.select_proxy(&group, &name).await.map_err(failed)
    }

    #[zbus(name = "TestGroupDelay")]
    async fn test_group_delay(&self, group: String) -> fdo::Result<()> {
        self.app.test_group_delay(&group).await.map_err(failed)
    }

    #[zbus(name = "RefreshExitRegion")]
    async fn refresh_exit_region(&self) -> fdo::Result<String> {
        self.app.refresh_exit_region().await.map_err(failed)
    }

    #[zbus(name = "GetStatus")]
    async fn get_status(&self) -> fdo::Result<String> {
        Ok(self.app.status_json().await)
    }

    #[zbus(name = "GetProfiles")]
    async fn get_profiles(&self) -> fdo::Result<String> {
        Ok(self.app.profiles_json().await)
    }

    #[zbus(name = "GetProxies")]
    async fn get_proxies(&self) -> fdo::Result<String> {
        self.app.proxies_json().await.map_err(failed)
    }

    #[zbus(signal)]
    async fn status_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn profiles_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn proxies_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn delay_updated(
        emitter: &SignalEmitter<'_>,
        group: &str,
        name: &str,
        delay_ms: i64,
    ) -> zbus::Result<()>;
}

fn failed(error: String) -> fdo::Error {
    fdo::Error::Failed(error)
}

pub async fn run() -> Result<(), String> {
    let app = App::new(Paths::from_env())?;
    let mut events = app.subscribe();
    let connection = zbus::connection::Builder::session()
        .map_err(|error| error.to_string())?
        .name("org.kde.clashkde")
        .map_err(|error| error.to_string())?
        .serve_at(
            "/org/kde/clashkde",
            Manager {
                app: Arc::clone(&app),
            },
        )
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;

    let interface = connection
        .object_server()
        .interface::<_, Manager>("/org/kde/clashkde")
        .await
        .map_err(|error| error.to_string())?;
    let emitter = interface.signal_emitter().clone();
    drop(interface);

    app.restore_desired().await;

    let mut exits = app.take_exit_notices().await;
    let mut terminate = signal(SignalKind::terminate()).map_err(|error| error.to_string())?;
    let mut interrupt = signal(SignalKind::interrupt()).map_err(|error| error.to_string())?;
    loop {
        tokio::select! {
            _ = terminate.recv() => break,
            _ = interrupt.recv() => break,
            notice = exits.recv() => {
                if let Some(notice) = notice {
                    app.on_core_exit(notice).await;
                }
            }
            event = events.recv() => {
                let Ok(event) = event else {
                    continue;
                };
                let result = match event {
                    Event::Status => Manager::status_changed(&emitter).await,
                    Event::Profiles => Manager::profiles_changed(&emitter).await,
                    Event::Proxies => Manager::proxies_changed(&emitter).await,
                    Event::Delay {
                        group,
                        name,
                        delay_ms,
                    } => Manager::delay_updated(&emitter, &group, &name, delay_ms).await,
                };
                if let Err(error) = result {
                    tracing::debug!("无法发出状态信号: {error}");
                }
            }
        }
    }

    app.shutdown().await;
    drop(connection);
    Ok(())
}
