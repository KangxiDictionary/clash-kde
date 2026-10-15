use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

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
        }
    }
}

impl Settings {
    pub fn load(paths: &Paths) -> std::io::Result<Self> {
        let path = paths.settings_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)?;
        let mut settings: Self = toml::from_str(&text).unwrap_or_else(|error| {
            tracing::warn!("忽略无法解析的配置，改用默认值: {error}");
            Self::default()
        });
        if !matches!(settings.mode.as_str(), "rule" | "global" | "direct") {
            settings.mode = default_mode();
        }
        if settings.mixed_port == 0 {
            settings.mixed_port = default_mixed_port();
        }
        if settings.controller_port == 0 {
            settings.controller_port = default_controller_port();
        }
        Ok(settings)
    }

    pub fn save(&self, paths: &Paths) -> std::io::Result<()> {
        fs::create_dir_all(&paths.config_dir)?;
        let text = toml::to_string_pretty(self).expect("settings serialize");
        write_private(&paths.settings_file(), text.as_bytes())
    }
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

pub fn validate_ports(mixed_port: u32, controller_port: u32) -> Result<(u16, u16), String> {
    let mixed = u16::try_from(mixed_port).map_err(|_| "端口无效".to_string())?;
    let controller = u16::try_from(controller_port).map_err(|_| "端口无效".to_string())?;
    if mixed == 0 || controller == 0 {
        return Err("端口无效".to_string());
    }
    if mixed == controller {
        return Err("混合端口和控制器端口不能相同".to_string());
    }
    Ok((mixed, controller))
}

pub fn normalize_mode(mode: &str) -> Result<String, String> {
    match mode {
        "rule" | "global" | "direct" => Ok(mode.to_string()),
        _ => Err("模式只能是 rule、global 或 direct".to_string()),
    }
}
