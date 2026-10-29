// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

//! Stable, language-independent service errors.
//!
//! The Rust service must not decide which language the caller sees. It returns
//! a code plus parameters; the KCM/applet translate that code with the user's
//! locale. `detail` is diagnostic text only and is never used as a format
//! string.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

pub mod codes {
    pub const IO_NOT_FOUND: &str = "io.not_found";
    pub const IO_DENIED: &str = "io.denied";
    pub const IO_FAILED: &str = "io.failed";
    pub const ERROR_LEGACY: &str = "error.legacy";

    pub const SETTINGS_PORT_INVALID: &str = "settings.port_invalid";
    pub const SETTINGS_PORT_SAME: &str = "settings.port_same";
    pub const SETTINGS_MODE_INVALID: &str = "settings.mode_invalid";
    pub const SETTINGS_TUN_DEVICE_INVALID: &str = "settings.tun_device_invalid";
    pub const SETTINGS_OVERRIDE_ABSOLUTE: &str = "settings.override_absolute";
    pub const SETTINGS_OVERRIDE_FILENAME: &str = "settings.override_filename";
    pub const SETTINGS_OVERRIDE_IS_DIR: &str = "settings.override_is_dir";

    pub const LAN_AUTH_BOTH: &str = "lan.auth_both";
    pub const LAN_AUTH_COLON: &str = "lan.auth_colon";
    pub const LAN_USERNAME_TOO_LONG: &str = "lan.username_too_long";
    pub const LAN_PASSWORD_TOO_LONG: &str = "lan.password_too_long";

    pub const ACCOUNT_NOT_FOUND: &str = "account.not_found";
    pub const TUN_BINARY_NOT_SYSTEM: &str = "tun.binary_not_system";
    pub const TUN_SYSTEM_BINARY_MISSING: &str = "tun.system_binary_missing";
    pub const TUN_BINARY_MISSING: &str = "tun.binary_missing";
    pub const TUN_CONFIG_NOT_OWNED: &str = "tun.config_not_owned";
    pub const TUN_CONFIG_PARSE: &str = "tun.config_parse";
    pub const TUN_CONFIG_MISSING: &str = "tun.config_missing";
    pub const TUN_CONFIG_DISABLED: &str = "tun.config_disabled";
    pub const TUN_MACHINE_INCLUDE_UID: &str = "tun.machine_include_uid";
    pub const TUN_MACHINE_EXCLUDE_UID: &str = "tun.machine_exclude_uid";
    pub const TUN_INCLUDE_UID_INVALID: &str = "tun.include_uid_invalid";
    pub const TUN_ALREADY_OTHER_USER: &str = "tun.already_other_user";
    pub const TUN_LOG_SYMLINK: &str = "tun.log_symlink";
    pub const TUN_LOG_CHECK: &str = "tun.log_check";
    pub const TUN_NOT_AUTHORIZED: &str = "tun.not_authorized";

    pub const PROFILE_NOT_FOUND: &str = "profile.not_found";
    pub const PROFILE_NO_URL: &str = "profile.no_url";
    pub const PROFILE_YAML_INVALID: &str = "profile.yaml_error";
    pub const PROFILE_EMPTY: &str = "profile.empty";
    pub const PROFILE_NOT_MAPPING: &str = "profile.not_mapping";
    pub const PROFILE_OVERRIDE_MAPPING: &str = "profile.override_not_mapping";
    pub const PROFILE_OVERRIDE_PARSE: &str = "profile.override_parse";
    pub const PROFILE_CONFIG_PARSE: &str = "profile.config_parse";
    pub const PROFILE_RUNTIME_WRITE: &str = "profile.runtime_write";
    pub const PROFILE_ROUTE_NAME_EMPTY: &str = "route.name_empty";
    pub const PROFILE_ROUTE_NAME_TOO_LONG: &str = "route.name_too_long";
    pub const PROFILE_ROUTE_KIND: &str = "route.kind_unsupported";
    pub const PROFILE_SERVER_EMPTY: &str = "route.server_empty";
    pub const PROFILE_SERVER_TOO_LONG: &str = "route.server_too_long";
    pub const PROFILE_SECRET_TOO_LONG: &str = "route.secret_too_long";
    pub const PROFILE_PORT_INVALID: &str = "route.port_invalid";
    pub const PROFILE_ROUTE_DUPLICATE: &str = "route.duplicate";
    pub const PROFILE_GROUP_DUPLICATE: &str = "route.group_duplicate";
    pub const PROFILE_ROUTE_NOT_REMOVABLE: &str = "route.cannot_remove";
    pub const PROFILE_ROUTE_NOT_FOUND: &str = "route.not_found";
    pub const PROFILE_ROUTE_NOT_EDITABLE: &str = "route.cannot_edit";
    pub const PROFILE_ROUTE_NOT_RESTORABLE: &str = "route.cannot_restore";
    pub const PROFILE_SUBSCRIPTION_MISSING: &str = "route.subscription_missing";
    pub const PROFILE_SUBSCRIPTION_UNPARSEABLE: &str = "profile.subscription_unparseable";
    pub const PROFILE_RULE_SUBSCRIPTION_MISSING: &str = "rule.subscription_missing";
    pub const PROFILE_RULE_MISSING: &str = "rule.not_found";
    pub const PROFILE_NO_RULES: &str = "rule.no_rules";

    pub const RULES_READ: &str = "rules.read_failed";
    pub const RULES_WRITE: &str = "rules.write_failed";
    pub const RULES_PARSE: &str = "rules.parse_failed";
    pub const RULES_JSON_PARSE: &str = "rules.json_parse";
    pub const RULES_ROW_PREFIX: &str = "rules.row_prefix";
    pub const RULES_CSV_HEADER: &str = "rules.csv_header";
    pub const RULES_CSV_ROW: &str = "rules.csv_row";
    pub const RULES_TYPE_UNKNOWN: &str = "rules.type_unknown";
    pub const RULES_COMMA: &str = "rules.comma";
    pub const RULES_PAYLOAD_TOO_LONG: &str = "rules.payload_too_long";
    pub const RULES_TARGET_TOO_LONG: &str = "rules.target_too_long";
    pub const RULES_TARGET_EMPTY: &str = "rules.target_empty";
    pub const RULES_TARGET_UNKNOWN: &str = "rules.target_unknown";
    pub const RULES_PAYLOAD_EMPTY: &str = "rules.payload_empty";
    pub const RULES_CIDR: &str = "rules.cidr";
    pub const RULES_PORT: &str = "rules.port";
    pub const RULES_GEOIP: &str = "rules.geoip";
    pub const RULES_ENABLED: &str = "rules.csv_enabled";
    pub const RULES_KIND: &str = "rules.csv_kind";

    pub const CONTROLLER_MODE: &str = "controller.mode_switch";
    pub const CONTROLLER_SELECT: &str = "controller.select_proxy";
    pub const CONTROLLER_READ_GROUPS: &str = "controller.read_groups";
    pub const CONTROLLER_PARSE_GROUPS: &str = "controller.parse_groups";
    pub const CONTROLLER_READ_TRAFFIC: &str = "controller.read_traffic";
    pub const CONTROLLER_READ_CONNECTIONS: &str = "controller.read_connections";
    pub const CONTROLLER_PARSE_CONNECTIONS: &str = "controller.parse_connections";
    pub const CONTROLLER_CLOSE_CONNECTION: &str = "controller.close_connection";
    pub const CONTROLLER_NO_CONNECTION: &str = "controller.no_connection";
    pub const CONTROLLER_READ_PROVIDERS: &str = "controller.read_providers";
    pub const CONTROLLER_PARSE_PROVIDERS: &str = "controller.parse_providers";
    pub const CONTROLLER_UPDATE_PROVIDER: &str = "controller.update_provider";
    pub const CONTROLLER_SUBSCRIPTION_DOWNLOAD: &str = "controller.subscription_download";
    pub const CONTROLLER_SUBSCRIPTION_READ: &str = "controller.subscription_read";
    pub const CONTROLLER_REGION_FAILED: &str = "controller.region_failed";
    pub const CONTROLLER_REGION_EMPTY: &str = "controller.region_empty";
    pub const CONTROLLER_REGION_HTTP: &str = "controller.region_http";
    pub const CONTROLLER_REGION_PARSE: &str = "controller.region_parse";
    pub const CONTROLLER_REGION_PROXY: &str = "controller.region_proxy";

    pub const CORE_LOG_OPEN: &str = "core.log_open";
    pub const CORE_SPAWN_FAILED: &str = "core.spawn_failed";
    pub const CORE_NO_PID: &str = "core.no_pid";
    pub const CORE_BINARY_MISSING: &str = "core.binary_missing";

}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ServiceError {
    /// Stable dot-separated identifier. Never shown directly to users.
    pub code: &'static str,
    /// Values substituted into the translated template.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
    /// English technical detail, for logs and diagnostics only.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

impl ServiceError {
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            params: BTreeMap::new(),
            detail: String::new(),
        }
    }

    pub fn param(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }

    pub fn detail(mut self, value: impl Into<String>) -> Self {
        self.detail = value.into();
        self
    }

    pub fn from_io(error: std::io::Error) -> Self {
        let code = match error.kind() {
            std::io::ErrorKind::NotFound => codes::IO_NOT_FOUND,
            std::io::ErrorKind::PermissionDenied => codes::IO_DENIED,
            _ => codes::IO_FAILED,
        };
        Self::new(code).detail(error.to_string())
    }

    pub fn into_fdo(self) -> zbus::fdo::Error {
        zbus::fdo::Error::Failed(
            serde_json::to_string(&self).expect("ServiceError serializes"),
        )
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for zbus::fdo::Error {
    fn from(error: ServiceError) -> Self {
        error.into_fdo()
    }
}

impl From<String> for ServiceError {
    fn from(error: String) -> Self {
        ServiceError::new(codes::ERROR_LEGACY).detail(error)
    }
}

impl From<ServiceError> for String {
    fn from(error: ServiceError) -> Self {
        serde_json::to_string(&error).unwrap_or_else(|_| error.code.to_string())
    }
}
