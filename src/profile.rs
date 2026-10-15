use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

use crate::paths::Paths;
use crate::state::{Settings, write_private};

const OVERRIDE_TEMPLATE: &str = "\
# 这里的内容会合并到订阅之上。
# 监听地址、控制器和 TUN 由 clash-kde 在合并之后强制覆盖。
";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub updated_unix: i64,
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub expire_unix: i64,
}

pub fn load_profiles(paths: &Paths) -> Vec<Profile> {
    let index = paths.profiles_dir().join("index.json");
    let Ok(text) = fs::read_to_string(index) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_profiles(paths: &Paths, profiles: &[Profile]) -> std::io::Result<()> {
    fs::create_dir_all(paths.profiles_dir())?;
    let text = serde_json::to_string_pretty(profiles).expect("profiles serialize");
    write_private(&paths.profiles_dir().join("index.json"), text.as_bytes())
}

pub fn profile_body_path(paths: &Paths, id: &str) -> PathBuf {
    paths.profiles_dir().join(format!("{id}.yaml"))
}

pub fn read_profile_body(paths: &Paths, id: &str) -> std::io::Result<String> {
    fs::read_to_string(profile_body_path(paths, id))
}

pub fn write_profile_body(paths: &Paths, id: &str, body: &str) -> std::io::Result<()> {
    fs::create_dir_all(paths.profiles_dir())?;
    write_private(&profile_body_path(paths, id), body.as_bytes())
}

pub fn ensure_override_file(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_private(path, OVERRIDE_TEMPLATE.as_bytes())
}

pub fn subscription_yaml(body: &str) -> Result<String, String> {
    let trimmed = body.trim().trim_start_matches('\u{feff}');
    if parse_mapping(trimmed).is_some() {
        return Ok(trimmed.to_string());
    }
    let compact: String = trimmed.chars().filter(|ch| !ch.is_whitespace()).collect();
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &compact)
        .or_else(|_| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD_NO_PAD, &compact)
        })
        .or_else(|_| {
            base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &compact)
        })
        .map_err(|_| "订阅不是有效的 YAML".to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| "订阅不是有效的 YAML".to_string())?;
    let text = text.trim().trim_start_matches('\u{feff}');
    if parse_mapping(text).is_none() {
        return Err("订阅不是有效的 YAML".to_string());
    }
    Ok(text.to_string())
}

pub fn decode_subscription(body: &str) -> Result<Value, String> {
    let text = subscription_yaml(body)?;
    parse_mapping(&text).ok_or_else(|| "订阅不是有效的 YAML".to_string())
}

pub fn check_profile_text(body: &str) -> Result<(), String> {
    let text = body.trim().trim_start_matches('\u{feff}');
    if text.is_empty() {
        return Err("配置是空的".to_string());
    }
    match serde_yaml::from_str::<Value>(text) {
        Ok(value) if value.as_mapping().is_some() => Ok(()),
        Ok(_) => Err("配置需要是一组键值".to_string()),
        Err(error) => {
            if subscription_yaml(text).is_ok() {
                Ok(())
            } else {
                Err(format!("YAML 读不出来: {error}"))
            }
        }
    }
}

fn parse_mapping(text: &str) -> Option<Value> {
    let value: Value = serde_yaml::from_str(text).ok()?;
    value.as_mapping()?;
    Some(value)
}

pub fn load_override(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Mapping(Mapping::new()));
    }
    let text = fs::read_to_string(path).map_err(|error| format!("无法读取本地覆盖: {error}"))?;
    if text.trim().is_empty() {
        return Ok(Value::Mapping(Mapping::new()));
    }
    match serde_yaml::from_str::<Value>(&text) {
        Ok(Value::Null) => Ok(Value::Mapping(Mapping::new())),
        Ok(value) if value.as_mapping().is_some() => Ok(value),
        Ok(_) => Err("本地覆盖必须是 YAML 映射".to_string()),
        Err(error) => Err(format!("本地覆盖无法解析: {error}")),
    }
}

pub fn prepare_runtime_config(profile: Value, overlay: Value, settings: &Settings) -> Value {
    let mut config = profile;
    deep_merge(&mut config, &overlay);
    deep_merge(&mut config, &managed_keys(settings));
    if settings.tun_enabled {
        if let Value::Mapping(map) = &mut config {
            let uid = unsafe { libc::getuid() };
            map.insert(Value::String("tun".into()), enabled_tun(uid));
            // 系统代理把域名交给内核。TUN 截到的是 IP，域名规则对不上，
            // 握手会打到错误的地址。嗅探 TLS 里的域名后再解析，才和系统代理同一条路。
            map.insert(Value::String("sniffer".into()), tun_sniffer());
        }
    }
    config
}

fn enabled_tun(uid: u32) -> Value {
    let mut hijack = serde_yaml::Sequence::new();
    hijack.push(Value::String("any:53".into()));
    hijack.push(Value::String("tcp://any:53".into()));
    let mut include = serde_yaml::Sequence::new();
    include.push(Value::Number(uid.into()));

    let mut tun = Mapping::new();
    tun.insert(Value::String("enable".into()), Value::Bool(true));
    tun.insert(Value::String("stack".into()), Value::String("mixed".into()));
    // 默认 9000 会让客户端一次发出超过路径能承载的握手包，Google 的 TLS 会停在这里。
    tun.insert(Value::String("mtu".into()), Value::Number(1500.into()));
    tun.insert(Value::String("auto-route".into()), Value::Bool(true));
    tun.insert(Value::String("auto-redirect".into()), Value::Bool(true));
    tun.insert(
        Value::String("auto-detect-interface".into()),
        Value::Bool(true),
    );
    tun.insert(Value::String("strict-route".into()), Value::Bool(true));
    tun.insert(Value::String("dns-hijack".into()), Value::Sequence(hijack));
    tun.insert(
        Value::String("include-uid".into()),
        Value::Sequence(include),
    );
    Value::Mapping(tun)
}

fn tun_sniffer() -> Value {
    let mut http_ports = serde_yaml::Sequence::new();
    http_ports.push(Value::Number(80.into()));
    http_ports.push(Value::String("8080-8880".into()));
    let mut tls_ports = serde_yaml::Sequence::new();
    tls_ports.push(Value::Number(443.into()));
    tls_ports.push(Value::Number(8443.into()));

    let mut http = Mapping::new();
    http.insert(Value::String("ports".into()), Value::Sequence(http_ports));
    http.insert(
        Value::String("override-destination".into()),
        Value::Bool(true),
    );
    let mut tls = Mapping::new();
    tls.insert(
        Value::String("ports".into()),
        Value::Sequence(tls_ports.clone()),
    );
    tls.insert(
        Value::String("override-destination".into()),
        Value::Bool(true),
    );
    let mut quic = Mapping::new();
    quic.insert(Value::String("ports".into()), Value::Sequence(tls_ports));
    quic.insert(
        Value::String("override-destination".into()),
        Value::Bool(true),
    );

    let mut sniff = Mapping::new();
    sniff.insert(Value::String("HTTP".into()), Value::Mapping(http));
    sniff.insert(Value::String("TLS".into()), Value::Mapping(tls));
    sniff.insert(Value::String("QUIC".into()), Value::Mapping(quic));

    let mut skip = serde_yaml::Sequence::new();
    skip.push(Value::String("Mijia Cloud".into()));
    skip.push(Value::String("+.push.apple.com".into()));

    let mut sniffer = Mapping::new();
    sniffer.insert(Value::String("enable".into()), Value::Bool(true));
    sniffer.insert(Value::String("force-dns-mapping".into()), Value::Bool(true));
    sniffer.insert(Value::String("parse-pure-ip".into()), Value::Bool(true));
    sniffer.insert(
        Value::String("override-destination".into()),
        Value::Bool(true),
    );
    sniffer.insert(Value::String("sniff".into()), Value::Mapping(sniff));
    sniffer.insert(Value::String("skip-domain".into()), Value::Sequence(skip));
    Value::Mapping(sniffer)
}

pub fn deep_merge(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Mapping(base_map), Value::Mapping(overlay_map)) => {
            for (key, value) in overlay_map {
                match base_map.get_mut(key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        base_map.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

fn managed_keys(settings: &Settings) -> Value {
    let mut managed = Mapping::new();
    managed.insert(
        Value::String("mixed-port".into()),
        Value::Number(settings.mixed_port.into()),
    );
    managed.insert(Value::String("allow-lan".into()), Value::Bool(false));
    managed.insert(
        Value::String("bind-address".into()),
        Value::String("127.0.0.1".into()),
    );
    managed.insert(
        Value::String("external-controller".into()),
        Value::String(format!("127.0.0.1:{}", settings.controller_port)),
    );
    managed.insert(
        Value::String("secret".into()),
        Value::String(settings.controller_secret.clone()),
    );
    managed.insert(
        Value::String("mode".into()),
        Value::String(settings.mode.clone()),
    );
    if !settings.tun_enabled {
        let mut tun = Mapping::new();
        tun.insert(Value::String("enable".into()), Value::Bool(false));
        managed.insert(Value::String("tun".into()), Value::Mapping(tun));
    }
    Value::Mapping(managed)
}

pub fn write_runtime_config(paths: &Paths, settings: &Settings) -> Result<(), String> {
    if settings.active_profile.is_empty() {
        return Err("没有选择配置".to_string());
    }
    let body = read_profile_body(paths, &settings.active_profile)
        .map_err(|_| "找不到所选配置的原文".to_string())?;
    let profile = decode_subscription(&body)?;
    let overlay = load_override(&paths.override_file())?;
    let config = prepare_runtime_config(profile, overlay, settings);
    let text =
        serde_yaml::to_string(&config).map_err(|error| format!("无法生成运行配置: {error}"))?;
    fs::create_dir_all(paths.runtime_dir()).map_err(|error| error.to_string())?;
    write_private(&paths.runtime_config(), text.as_bytes()).map_err(|error| error.to_string())
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_profile_text_reports_the_yaml_error() {
        assert!(check_profile_text("mode: rule\n").is_ok());
        let error = check_profile_text("mode: [\n").expect_err("broken");
        assert!(error.contains("YAML"));
        assert!(check_profile_text("").is_err());
    }

    #[test]
    fn decodes_plain_yaml_and_base64() {
        let yaml = "mixed-port: 1\nproxies: []\n";
        let parsed = decode_subscription(yaml).expect("yaml");
        assert!(parsed.as_mapping().is_some());

        let encoded =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, yaml.as_bytes());
        let parsed = decode_subscription(&encoded).expect("base64");
        assert_eq!(parsed.get("mixed-port").and_then(Value::as_i64), Some(1));
    }

    #[test]
    fn managed_keys_win_and_tun_stays_off() {
        let profile = serde_yaml::from_str(
            "allow-lan: true\nbind-address: '*'\ntun:\n  enable: true\n  stack: system\nproxies: []\n",
        )
        .unwrap();
        let overlay =
            serde_yaml::from_str("mode: direct\nexternal-controller: 0.0.0.0:1\n").unwrap();
        let mut settings = Settings::default();
        settings.mixed_port = 7897;
        settings.controller_port = 19090;
        settings.controller_secret = "secret".into();
        settings.mode = "rule".into();
        let config = prepare_runtime_config(profile, overlay, &settings);
        assert_eq!(
            config.get("allow-lan").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            config.get("bind-address").and_then(Value::as_str),
            Some("127.0.0.1")
        );
        assert_eq!(
            config.get("external-controller").and_then(Value::as_str),
            Some("127.0.0.1:19090")
        );
        assert_eq!(config.get("mode").and_then(Value::as_str), Some("rule"));
        assert_eq!(
            config
                .get("tun")
                .and_then(|tun| tun.get("enable"))
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            config
                .get("tun")
                .and_then(|tun| tun.get("stack"))
                .and_then(Value::as_str),
            Some("system")
        );
        assert!(config.get("sniffer").is_none());
    }

    #[test]
    fn enabled_tun_replaces_the_subscription_block() {
        let profile = serde_yaml::from_str(
            "tun:\n  enable: true\n  device: evil\n  include-uid: [0]\nsniffer:\n  enable: false\nproxies: []\n",
        )
        .unwrap();
        let mut settings = Settings::default();
        settings.tun_enabled = true;
        settings.mode = "rule".into();
        let config = prepare_runtime_config(profile, Value::Mapping(Mapping::new()), &settings);
        let tun = config.get("tun").expect("tun");
        assert_eq!(tun.get("enable").and_then(Value::as_bool), Some(true));
        assert_eq!(tun.get("stack").and_then(Value::as_str), Some("mixed"));
        assert_eq!(tun.get("mtu").and_then(Value::as_u64), Some(1500));
        assert_eq!(tun.get("auto-route").and_then(Value::as_bool), Some(true));
        assert_eq!(
            tun.get("auto-redirect").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            tun.get("auto-detect-interface").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(tun.get("strict-route").and_then(Value::as_bool), Some(true));
        assert!(tun.get("device").is_none());
        let hijack = tun.get("dns-hijack").and_then(Value::as_sequence).unwrap();
        assert_eq!(hijack.len(), 2);
        assert_eq!(hijack[0].as_str(), Some("any:53"));
        assert_eq!(hijack[1].as_str(), Some("tcp://any:53"));
        let include = tun.get("include-uid").and_then(Value::as_sequence).unwrap();
        assert_eq!(include.len(), 1);
        let uid = unsafe { libc::getuid() };
        assert_eq!(include[0].as_u64(), Some(u64::from(uid)));
        let sniffer = config.get("sniffer").expect("sniffer");
        assert_eq!(sniffer.get("enable").and_then(Value::as_bool), Some(true));
        assert_eq!(
            sniffer.get("parse-pure-ip").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            sniffer.get("override-destination").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            sniffer
                .get("sniff")
                .and_then(|sniff| sniff.get("TLS"))
                .and_then(|tls| tls.get("override-destination"))
                .and_then(Value::as_bool),
            Some(true)
        );
        let text = serde_yaml::to_string(&config).unwrap();
        crate::privilege::assess_tun_document(&text, uid).unwrap();
        assert_eq!(
            config.get("allow-lan").and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn old_profile_index_still_loads() {
        let text = r#"[{"id":"a","name":"n","url":"https://example","updated_unix":1}]"#;
        let profiles: Vec<Profile> = serde_json::from_str(text).unwrap();
        assert_eq!(profiles[0].total, 0);
        assert_eq!(profiles[0].expire_unix, 0);
        assert_eq!(profiles[0].upload, 0);
    }
}
