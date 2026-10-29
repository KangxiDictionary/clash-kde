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

impl From<ServiceError> for String {
    fn from(error: ServiceError) -> Self {
        if error.detail.is_empty() {
            error.code.to_string()
        } else {
            error.detail
        }
    }
}
