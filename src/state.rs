// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::Paths;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub core_desired: bool,
    #[serde(default)]
    pub system_proxy_enabled: bool,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_mixed_port")]
    pub mixed_port: u16,
    #[serde(default = "default_controller_port")]
    pub controller_port: u16,
    #[serde(default)]
    pub controller_secret: String,
    #[serde(default = "default_mihomo_path")]
    pub mihomo_path: String,
    #[serde(default)]
    pub active_profile: String,
    #[serde(default)]
    pub selections: BTreeMap<String, String>,
    #[serde(default)]
    pub tun_enabled: bool,
    #[serde(default)]
    pub tun_machine: bool,
    #[serde(default = "default_tun_route_table")]
    pub tun_route_table: u32,
    #[serde(default = "default_tun_rule_index")]
    pub tun_rule_index: u32,
    #[serde(default = "default_tun_exclude_private")]
    pub tun_exclude_private: bool,
    #[serde(default = "default_tun_health_secs")]
    pub tun_health_secs: u32,
    #[serde(default)]
    pub sort_global: bool,
    #[serde(default = "default_sort_mode")]
    pub sort_mode: String,
    #[serde(default = "default_show_unreachable")]
    pub show_unreachable: bool,
    #[serde(default)]
    pub sort_prefs: BTreeMap<String, SortPref>,
    #[serde(default)]
    pub auto_test_delay: bool,
    #[serde(default)]
    pub blur_ip: bool,
    #[serde(default)]
    pub tun_device: String,
    #[serde(default)]
    pub override_path: String,
    #[serde(default)]
    pub compact: bool,
    #[serde(default = "default_show_widget_hint")]
    pub show_widget_hint: bool,
    #[serde(default = "default_speed_test_mib")]
    pub speed_test_mib: u32,
    #[serde(default)]
    pub allow_lan: bool,
    #[serde(default)]
    pub lan_username: String,
    #[serde(default)]
    pub lan_password: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortPref {
    #[serde(default = "default_sort_mode")]
    pub mode: String,
    #[serde(default = "default_show_unreachable")]
    pub show_unreachable: bool,
}

fn default_sort_mode() -> String {
    "profile".to_string()
}

fn default_show_unreachable() -> bool {
    true
}

fn default_show_widget_hint() -> bool {
    true
}

fn default_speed_test_mib() -> u32 {
    20
}

fn default_tun_route_table() -> u32 {
    crate::tun::DEFAULT_ROUTE_TABLE
}

fn default_tun_rule_index() -> u32 {
    crate::tun::DEFAULT_RULE_INDEX
}

fn default_tun_exclude_private() -> bool {
    true
}

fn default_tun_health_secs() -> u32 {
    crate::tun::DEFAULT_HEALTH_SECS
}

fn default_mode() -> String {
    "rule".to_string()
}

fn default_mixed_port() -> u16 {
    7890
}

fn default_controller_port() -> u16 {
    9090
}

fn default_mihomo_path() -> String {
    "mihomo".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            core_desired: false,
            system_proxy_enabled: false,
            mode: default_mode(),
            mixed_port: default_mixed_port(),
            controller_port: default_controller_port(),
            controller_secret: String::new(),
            mihomo_path: default_mihomo_path(),
            active_profile: String::new(),
            selections: BTreeMap::new(),
            tun_enabled: false,
            tun_machine: false,
            tun_route_table: default_tun_route_table(),
            tun_rule_index: default_tun_rule_index(),
            tun_exclude_private: default_tun_exclude_private(),
            tun_health_secs: default_tun_health_secs(),
            sort_global: false,
            sort_mode: default_sort_mode(),
            show_unreachable: default_show_unreachable(),
            sort_prefs: BTreeMap::new(),
            auto_test_delay: false,
            blur_ip: false,
            tun_device: String::new(),
            override_path: String::new(),
            compact: false,
            show_widget_hint: true,
            speed_test_mib: default_speed_test_mib(),
            allow_lan: false,
            lan_username: String::new(),
            lan_password: String::new(),
        }
    }
}

impl Settings {
    pub fn load(paths: &Paths) -> std::io::Result<Self> {
        let path = paths.settings_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(&path)?;
        let mut settings: Self = match toml::from_str(&text) {
            Ok(settings) => settings,
            Err(error) => {
                // A corrupt config must not be silently replaced by defaults:
                // keep a copy next to it so mihomo_path, active_profile, and
                // the TUN keys can be recovered by hand.
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_millis());
                let backup = path.with_file_name(format!("config.toml.broken-{stamp}"));
                match fs::copy(path, &backup) {
                    Ok(_) => {
                        tracing::error!(
                            "could not parse the config; backed it up to {}: {error}",
                            backup.display()
                        )
                    }
                    Err(copy_error) => tracing::error!(
                        "could not back up the unreadable config to {} ({copy_error}): {error}",
                        backup.display()
                    ),
                }
                Self::default()
            }
        };
        if !matches!(settings.mode.as_str(), "rule" | "global" | "direct") {
            settings.mode = default_mode();
        }
        if !matches!(settings.sort_mode.as_str(), "profile" | "name" | "delay") {
            settings.sort_mode = default_sort_mode();
        }
        if settings.mixed_port == 0 {
            settings.mixed_port = default_mixed_port();
        }
        if settings.controller_port == 0 {
            settings.controller_port = default_controller_port();
        }
        if !(1..=100).contains(&settings.speed_test_mib) {
            settings.speed_test_mib = default_speed_test_mib();
        }
        if !(1..=2_147_483_647).contains(&settings.tun_route_table) {
            settings.tun_route_table = default_tun_route_table();
        }
        if !(1..=32765).contains(&settings.tun_rule_index) {
            settings.tun_rule_index = default_tun_rule_index();
        }
        if !(1..=60).contains(&settings.tun_health_secs) {
            settings.tun_health_secs = default_tun_health_secs();
        }
        settings.tun_device = normalize_tun_device(&settings.tun_device).unwrap_or_default();
        settings.override_path =
            normalize_override_path(paths, &settings.override_path).unwrap_or_default();
        Ok(settings)
    }

    pub fn save(&self, paths: &Paths) -> std::io::Result<()> {
        fs::create_dir_all(&paths.config_dir)?;
        let text = toml::to_string_pretty(self).expect("settings serialize");
        write_private(&paths.settings_file(), text.as_bytes())
    }
}

pub fn notes_file(paths: &Paths) -> PathBuf {
    paths.config_dir.join("notes.json")
}

pub fn load_notes(paths: &Paths) -> BTreeMap<String, String> {
    let Ok(text) = fs::read_to_string(notes_file(paths)) else {
        return BTreeMap::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_notes(
    paths: &Paths,
    notes: &BTreeMap<String, String>,
) -> Result<(), crate::error::ServiceError> {
    let clean: BTreeMap<String, String> = notes
        .iter()
        .filter_map(|(key, value)| {
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key.to_string(), value.to_string()))
            }
        })
        .collect();
    fs::create_dir_all(&paths.config_dir).map_err(crate::error::ServiceError::from_io)?;
    let text = serde_json::to_string_pretty(&clean).expect("notes serialize");
    write_private(&notes_file(paths), text.as_bytes()).map_err(crate::error::ServiceError::from_io)
}

pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
    }
    fs::rename(tmp, path)
}

pub fn ensure_secret(settings: &mut Settings) -> bool {
    if !settings.controller_secret.is_empty() {
        return false;
    }
    let mut bytes = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut bytes);
    settings.controller_secret = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    true
}

pub fn validate_ports(
    mixed_port: u32,
    controller_port: u32,
) -> Result<(u16, u16), crate::error::ServiceError> {
    let mixed = u16::try_from(mixed_port)
        .map_err(|_| crate::error::ServiceError::new(crate::error::codes::SETTINGS_PORT_INVALID))?;
    let controller = u16::try_from(controller_port)
        .map_err(|_| crate::error::ServiceError::new(crate::error::codes::SETTINGS_PORT_INVALID))?;
    if mixed == 0 || controller == 0 {
        return Err(crate::error::ServiceError::new(
            crate::error::codes::SETTINGS_PORT_INVALID,
        ));
    }
    if mixed == controller {
        return Err(crate::error::ServiceError::new(
            crate::error::codes::SETTINGS_PORT_SAME,
        ));
    }
    Ok((mixed, controller))
}

pub fn normalize_mode(mode: &str) -> Result<String, crate::error::ServiceError> {
    match mode {
        "rule" | "global" | "direct" => Ok(mode.to_string()),
        _ => Err(crate::error::ServiceError::new(
            crate::error::codes::SETTINGS_MODE_INVALID,
        )),
    }
}

pub fn normalize_tun_device(name: &str) -> Result<String, crate::error::ServiceError> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(String::new());
    }
    let ok = (1..=15).contains(&name.len())
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if !ok {
        return Err(crate::error::ServiceError::new(
            crate::error::codes::SETTINGS_TUN_DEVICE_INVALID,
        ));
    }
    Ok(name.to_string())
}

pub fn effective_override(paths: &Paths, stored: &str) -> PathBuf {
    let stored = stored.trim();
    if stored.is_empty() {
        paths.override_file()
    } else {
        PathBuf::from(stored)
    }
}

pub fn normalize_override_path(
    paths: &Paths,
    text: &str,
) -> Result<String, crate::error::ServiceError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(String::new());
    }
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(crate::error::ServiceError::new(
            crate::error::codes::SETTINGS_OVERRIDE_ABSOLUTE,
        ));
    }
    if path.file_name().is_none_or(|name| name.is_empty()) {
        return Err(crate::error::ServiceError::new(
            crate::error::codes::SETTINGS_OVERRIDE_FILENAME,
        ));
    }
    if path.is_dir() {
        return Err(crate::error::ServiceError::new(
            crate::error::codes::SETTINGS_OVERRIDE_IS_DIR,
        ));
    }
    if path == paths.override_file() {
        return Ok(String::new());
    }
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_paths() -> Paths {
        Paths {
            config_dir: PathBuf::from("/tmp/clash-kde-settings"),
            data_dir: PathBuf::from("/tmp/clash-kde-data"),
            kioslaverc: PathBuf::from("/tmp/kioslaverc"),
        }
    }

    #[test]
    fn tun_device_name_is_empty_or_a_short_interface_name() {
        assert_eq!(normalize_tun_device("  ").unwrap(), "");
        assert_eq!(normalize_tun_device(" clash0 ").unwrap(), "clash0");
        assert_eq!(normalize_tun_device("a").unwrap(), "a");
        assert_eq!(normalize_tun_device("ok-name_1").unwrap(), "ok-name_1");
        assert_eq!(
            normalize_tun_device("a".repeat(15).as_str()).unwrap().len(),
            15
        );
        assert!(normalize_tun_device(&"a".repeat(16)).is_err());
        assert!(normalize_tun_device("bad name").is_err());
        assert!(normalize_tun_device("bad.name").is_err());
        assert!(normalize_tun_device("网卡").is_err());
    }

    #[test]
    fn override_path_empty_or_default_is_stored_as_empty() {
        let paths = sample_paths();
        let default = paths.override_file().display().to_string();
        assert_eq!(normalize_override_path(&paths, "  ").unwrap(), "");
        assert_eq!(normalize_override_path(&paths, &default).unwrap(), "");
        assert_eq!(
            normalize_override_path(&paths, "/tmp/custom-override.yaml").unwrap(),
            "/tmp/custom-override.yaml"
        );
        assert!(normalize_override_path(&paths, "override.yaml").is_err());
        assert!(normalize_override_path(&paths, "/").is_err());
    }

    #[test]
    fn override_path_rejects_an_existing_directory() {
        let paths = sample_paths();
        let dir = std::env::temp_dir().join("clash-kde-override-dir-test");
        fs::create_dir_all(&dir).unwrap();
        let error = normalize_override_path(&paths, dir.to_str().unwrap()).unwrap_err();
        assert_eq!(error.code, crate::error::codes::SETTINGS_OVERRIDE_IS_DIR);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn corrupt_settings_are_backed_up_before_defaults_are_used() {
        let dir =
            std::env::temp_dir().join(format!("clash-kde-settings-{}-broken", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let paths = Paths {
            config_dir: dir.clone(),
            data_dir: dir.join("data"),
            kioslaverc: dir.join("kioslaverc"),
        };
        fs::write(paths.settings_file(), "mihomo_path = [unterminated\n").unwrap();
        let settings = Settings::load(&paths).unwrap();
        assert_eq!(settings.mihomo_path, "mihomo");
        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.toml.broken-")
            })
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            fs::read_to_string(backups[0].path()).unwrap(),
            "mihomo_path = [unterminated\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tun_route_settings_have_defaults_and_are_validated() {
        let missing: Settings = toml::from_str("core_desired = false\n").unwrap();
        assert_eq!(missing.tun_route_table, 2022);
        assert_eq!(missing.tun_rule_index, 9000);
        assert!(missing.tun_exclude_private);
        assert_eq!(missing.tun_health_secs, 5);

        let dir = std::env::temp_dir().join(format!(
            "clash-kde-settings-{}-tun-route",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let paths = Paths {
            config_dir: dir.clone(),
            data_dir: dir.join("data"),
            kioslaverc: dir.join("kioslaverc"),
        };
        fs::write(
            paths.settings_file(),
            "tun_route_table = 0\ntun_rule_index = 32766\ntun_health_secs = 0\n",
        )
        .unwrap();
        let settings = Settings::load(&paths).unwrap();
        assert_eq!(settings.tun_route_table, 2022);
        assert_eq!(settings.tun_rule_index, 9000);
        assert_eq!(settings.tun_health_secs, 5);

        fs::write(
            paths.settings_file(),
            "tun_route_table = 4040\ntun_rule_index = 9100\ntun_exclude_private = false\ntun_health_secs = 10\n",
        )
        .unwrap();
        let settings = Settings::load(&paths).unwrap();
        assert_eq!(settings.tun_route_table, 4040);
        assert_eq!(settings.tun_rule_index, 9100);
        assert!(!settings.tun_exclude_private);
        assert_eq!(settings.tun_health_secs, 10);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn speed_test_size_defaults_to_20_and_out_of_range_values_are_clamped() {
        let missing: Settings = toml::from_str("core_desired = false\n").unwrap();
        assert_eq!(missing.speed_test_mib, 20);
        let dir = std::env::temp_dir().join(format!(
            "clash-kde-settings-{}-{}",
            std::process::id(),
            "size"
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let paths = Paths {
            config_dir: dir.clone(),
            data_dir: dir.join("data"),
            kioslaverc: dir.join("kioslaverc"),
        };
        fs::write(paths.settings_file(), "speed_test_mib = 0\n").unwrap();
        assert_eq!(Settings::load(&paths).unwrap().speed_test_mib, 20);
        fs::write(paths.settings_file(), "speed_test_mib = 250\n").unwrap();
        assert_eq!(Settings::load(&paths).unwrap().speed_test_mib, 20);
        fs::write(paths.settings_file(), "speed_test_mib = 40\n").unwrap();
        assert_eq!(Settings::load(&paths).unwrap().speed_test_mib, 40);
        let _ = fs::remove_dir_all(&dir);
    }
}
