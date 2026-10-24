// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

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

    #[zbus(name = "SetMachineTun")]
    async fn set_machine_tun(&self, enabled: bool) -> fdo::Result<()> {
        self.app.set_machine_tun(enabled).await.map_err(failed)
    }

    #[zbus(name = "SetMode")]
    async fn set_mode(&self, mode: String) -> fdo::Result<()> {
        self.app.set_mode(&mode).await.map_err(failed)
    }

    #[zbus(name = "ResetPreferences")]
    async fn reset_preferences(&self) -> fdo::Result<()> {
        self.app.reset_preferences().await.map_err(failed)
    }

    #[zbus(name = "SetSettings")]
    async fn set_settings(
        &self,
        mixed_port: u32,
        controller_port: u32,
        mihomo_path: String,
        tun_device: String,
        override_path: String,
    ) -> fdo::Result<()> {
        self.app
            .set_settings(
                mixed_port,
                controller_port,
                mihomo_path,
                tun_device,
                override_path,
            )
            .await
            .map_err(failed)
    }

    #[zbus(name = "EnsureOverride")]
    async fn ensure_override(&self, path: String) -> fdo::Result<String> {
        self.app.ensure_override(&path).await.map_err(failed)
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

    #[zbus(name = "SetChoices")]
    async fn set_choices(&self, profile: String, selections: String) -> fdo::Result<()> {
        let parsed = serde_json::from_str(&selections).unwrap_or_default();
        self.app.set_choices(&profile, parsed).await.map_err(failed)
    }

    #[zbus(name = "SetBehavior")]
    async fn set_behavior(
        &self,
        sort_global: bool,
        auto_test_delay: bool,
        blur_ip: bool,
        show_widget_hint: bool,
        speed_test_mib: i32,
    ) -> fdo::Result<()> {
        self.app
            .set_behavior(
                sort_global,
                auto_test_delay,
                blur_ip,
                show_widget_hint,
                speed_test_mib,
            )
            .await
            .map_err(failed)
    }

    #[zbus(name = "SetCompact")]
    async fn set_compact(&self, compact: bool) -> fdo::Result<()> {
        self.app.set_compact(compact).await.map_err(failed)
    }

    #[zbus(name = "SetLan")]
    async fn set_lan(&self, allow: bool, username: String, password: String) -> fdo::Result<()> {
        self.app
            .set_lan(allow, username, password)
            .await
            .map_err(failed)
    }

    #[zbus(name = "ReloadRuntime")]
    async fn reload_runtime(&self) -> fdo::Result<()> {
        self.app.reload_runtime().await.map_err(failed)
    }

    #[zbus(name = "RouteDetail")]
    async fn route_detail(&self, id: String, name: String) -> fdo::Result<String> {
        self.app.route_detail(&id, &name).await.map_err(failed)
    }

    #[zbus(name = "AddRoute")]
    async fn add_route(
        &self,
        id: String,
        group: String,
        name: String,
        kind: String,
        server: String,
        port: u16,
        secret: String,
    ) -> fdo::Result<()> {
        self.app
            .add_route(&id, &group, name, kind, server, port, secret)
            .await
            .map_err(failed)
    }

    #[zbus(name = "RemoveRoute")]
    async fn remove_route(&self, id: String, name: String) -> fdo::Result<()> {
        self.app.remove_route(&id, &name).await.map_err(failed)
    }

    #[zbus(name = "RestoreRoute")]
    async fn restore_route(&self, id: String, name: String) -> fdo::Result<()> {
        self.app.restore_route(&id, &name).await.map_err(failed)
    }

    #[zbus(name = "RestoreProfileRule")]
    async fn restore_profile_rule(&self, id: String, index: u32) -> fdo::Result<()> {
        self.app
            .restore_profile_rule(&id, index)
            .await
            .map_err(failed)
    }

    #[zbus(name = "GetNotes")]
    async fn get_notes(&self) -> String {
        self.app.notes_json().await
    }

    #[zbus(name = "SetNotes")]
    async fn set_notes(&self, body: String) -> fdo::Result<()> {
        self.app.set_notes(&body).await.map_err(failed)
    }

    #[zbus(name = "UpdateRoute")]
    async fn update_route(
        &self,
        id: String,
        previous: String,
        name: String,
        kind: String,
        server: String,
        port: u16,
        secret: String,
    ) -> fdo::Result<()> {
        self.app
            .update_route(&id, &previous, name, kind, server, port, secret)
            .await
            .map_err(failed)
    }

    #[zbus(name = "SetViewPref")]
    async fn set_view_pref(
        &self,
        profile: String,
        group: String,
        mode: String,
        show_unreachable: bool,
        global: bool,
    ) -> fdo::Result<()> {
        self.app
            .set_view_pref(&profile, &group, &mode, show_unreachable, global)
            .await
            .map_err(failed)
    }

    #[zbus(name = "GetProfileGroups")]
    async fn get_profile_groups(&self, id: String) -> fdo::Result<String> {
        self.app.profile_groups_json(&id).await.map_err(failed)
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

    #[zbus(name = "GetConnections")]
    async fn get_connections(&self) -> fdo::Result<String> {
        self.app.connections_json().await.map_err(failed)
    }

    #[zbus(name = "CloseConnection")]
    async fn close_connection(&self, id: String) -> fdo::Result<()> {
        self.app.close_connection(&id).await.map_err(failed)
    }

    #[zbus(name = "CloseAllConnections")]
    async fn close_all_connections(&self) -> fdo::Result<()> {
        self.app.close_all_connections().await.map_err(failed)
    }

    #[zbus(name = "GetProxyProviders")]
    async fn get_proxy_providers(&self) -> fdo::Result<String> {
        Ok(self.app.proxy_providers_json().await)
    }

    #[zbus(name = "UpdateProxyProvider")]
    async fn update_proxy_provider(&self, name: String) -> fdo::Result<()> {
        self.app.update_proxy_provider(&name).await.map_err(failed)
    }

    #[zbus(name = "UpdateRuleProvider")]
    async fn update_rule_provider(&self, name: String) -> fdo::Result<()> {
        self.app.update_rule_provider(&name).await.map_err(failed)
    }

    #[zbus(name = "GetRuleView")]
    async fn get_rule_view(&self) -> fdo::Result<String> {
        self.app.rule_view_json().await.map_err(failed)
    }

    #[zbus(name = "AddOwnRule")]
    async fn add_own_rule(&self, kind: String, payload: String, target: String) -> fdo::Result<()> {
        self.app
            .add_own_rule(kind, payload, target)
            .await
            .map_err(failed)
    }

    #[zbus(name = "RemoveOwnRule")]
    async fn remove_own_rule(&self, index: u32) -> fdo::Result<()> {
        self.app.remove_own_rule(index).await.map_err(failed)
    }

    #[zbus(name = "SetOwnRules")]
    async fn set_own_rules(&self, body: String) -> fdo::Result<()> {
        self.app.set_own_rules(&body).await.map_err(failed)
    }

    #[zbus(name = "RulesCsv")]
    async fn rules_csv(&self, body: String) -> fdo::Result<String> {
        crate::rules::csv_from_json(&body).map_err(failed)
    }

    #[zbus(name = "ParseRulesCsv")]
    async fn parse_rules_csv(&self, body: String) -> fdo::Result<String> {
        crate::rules::json_from_csv(&body).map_err(failed)
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
