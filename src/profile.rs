// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

use crate::controller::{ProxyGroup, ProxyNode};
use crate::error::{ServiceError, codes};
use crate::paths::Paths;
use crate::state::{Settings, write_private};
use crate::tun::{self, TunOptions};

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

pub fn subscription_body_path(paths: &Paths, id: &str) -> PathBuf {
    paths.profiles_dir().join(format!("{id}.subscription.yaml"))
}

pub fn write_subscription_body(paths: &Paths, id: &str, body: &str) -> std::io::Result<()> {
    fs::create_dir_all(paths.profiles_dir())?;
    write_private(&subscription_body_path(paths, id), body.as_bytes())
}

pub fn explain_io(action: &str, error: &std::io::Error) -> ServiceError {
    let code = match error.kind() {
        std::io::ErrorKind::NotFound => codes::IO_NOT_FOUND,
        std::io::ErrorKind::PermissionDenied => codes::IO_DENIED,
        _ => codes::IO_FAILED,
    };
    ServiceError::new(code).detail(format!("{action}: {error}"))
}

/// The last downloaded subscription, kept apart from the working profile.
/// An older profile gets a snapshot of its current file the first time this is read.
pub fn read_subscription_body(paths: &Paths, id: &str) -> Result<String, ServiceError> {
    let path = subscription_body_path(paths, id);
    if path.exists() {
        return fs::read_to_string(&path).map_err(|error| explain_io("read subscription", &error));
    }
    let profiles = load_profiles(paths);
    let Some(profile) = profiles.iter().find(|profile| profile.id == id) else {
        return Err(ServiceError::new(codes::PROFILE_NOT_FOUND));
    };
    if profile.url.is_empty() {
        return Err(ServiceError::new(codes::PROFILE_NO_URL));
    }
    let body = read_profile_body(paths, id).map_err(|error| explain_io("read profile", &error))?;
    write_subscription_body(paths, id, &body)
        .map_err(|error| explain_io("write subscription", &error))?;
    Ok(body)
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

pub fn subscription_yaml(body: &str) -> Result<String, ServiceError> {
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
        .map_err(|_| ServiceError::new(codes::PROFILE_YAML_INVALID))?;
    let text =
        String::from_utf8(bytes).map_err(|_| ServiceError::new(codes::PROFILE_YAML_INVALID))?;
    let text = text.trim().trim_start_matches('\u{feff}');
    if parse_mapping(text).is_none() {
        return Err(ServiceError::new(codes::PROFILE_YAML_INVALID));
    }
    Ok(text.to_string())
}

pub fn decode_subscription(body: &str) -> Result<Value, ServiceError> {
    let text = subscription_yaml(body)?;
    parse_mapping(&text).ok_or_else(|| ServiceError::new(codes::PROFILE_YAML_INVALID))
}

pub fn check_profile_text(body: &str) -> Result<(), ServiceError> {
    let text = body.trim().trim_start_matches('\u{feff}');
    if text.is_empty() {
        return Err(ServiceError::new(codes::PROFILE_EMPTY));
    }
    match serde_yaml::from_str::<Value>(text) {
        Ok(value) if value.as_mapping().is_some() => Ok(()),
        Ok(_) => Err(ServiceError::new(codes::PROFILE_NOT_MAPPING)),
        Err(error) => {
            if subscription_yaml(text).is_ok() {
                Ok(())
            } else {
                Err(ServiceError::new(codes::PROFILE_CONFIG_PARSE).detail(error.to_string()))
            }
        }
    }
}

pub fn selector_groups(body: &str) -> Vec<ProxyGroup> {
    let Ok(value) = serde_yaml::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(groups) = value.get("proxy-groups").and_then(Value::as_sequence) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for group in groups {
        let Some(name) = group.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(nodes_value) = group.get("proxies").and_then(Value::as_sequence) else {
            continue;
        };
        let nodes: Vec<ProxyNode> = nodes_value
            .iter()
            .filter_map(Value::as_str)
            .map(|node_name| ProxyNode {
                name: node_name.to_string(),
                delay_ms: -1,
            })
            .collect();
        if nodes.is_empty() {
            continue;
        }
        let raw_kind = group
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("select");
        let kind = if raw_kind.eq_ignore_ascii_case("select")
            || raw_kind.eq_ignore_ascii_case("selector")
        {
            "Selector".to_string()
        } else {
            raw_kind.to_string()
        };
        out.push(ProxyGroup {
            name: name.to_string(),
            kind,
            now: String::new(),
            nodes,
        });
    }
    out.sort_by(|left, right| left.name.cmp(&right.name));
    out
}

fn parse_mapping(text: &str) -> Option<Value> {
    let value: Value = serde_yaml::from_str(text).ok()?;
    value.as_mapping()?;
    Some(value)
}

pub fn load_override(path: &Path) -> Result<Value, ServiceError> {
    if !path.exists() {
        return Ok(Value::Mapping(Mapping::new()));
    }
    let text = fs::read_to_string(path).map_err(|error| explain_io("read override", &error))?;
    if text.trim().is_empty() {
        return Ok(Value::Mapping(Mapping::new()));
    }
    match serde_yaml::from_str::<Value>(&text) {
        Ok(Value::Null) => Ok(Value::Mapping(Mapping::new())),
        Ok(value) if value.as_mapping().is_some() => Ok(value),
        Ok(_) => Err(ServiceError::new(codes::PROFILE_OVERRIDE_MAPPING)),
        Err(error) => {
            Err(ServiceError::new(codes::PROFILE_OVERRIDE_PARSE).detail(error.to_string()))
        }
    }
}

pub fn prepare_runtime_config(
    profile: Value,
    overlay: Value,
    settings: &Settings,
    lan_open: bool,
) -> Value {
    let mut config = profile;
    deep_merge(&mut config, &overlay);
    deep_merge(&mut config, &managed_keys(settings, lan_open));
    if settings.tun_enabled
        && let Value::Mapping(map) = &mut config
    {
        let uid = unsafe { libc::getuid() };
        map.insert(
            Value::String("tun".into()),
            tun::config(
                uid,
                &settings.tun_device,
                settings.tun_machine,
                &TunOptions::from(settings),
            ),
        );
        // 系统代理把域名交给内核。TUN 截到的是 IP，域名规则对不上，
        // 握手会打到错误的地址。嗅探 TLS 里的域名后再解析，才和系统代理同一条路。
        map.insert(Value::String("sniffer".into()), tun_sniffer());
    }
    config
}

fn tun_sniffer() -> Value {
    let http_ports = vec![Value::Number(80.into()), Value::String("8080-8880".into())];
    let tls_ports = vec![Value::Number(443.into()), Value::Number(8443.into())];

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

    let skip = vec![
        Value::String("Mijia Cloud".into()),
        Value::String("+.push.apple.com".into()),
    ];

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

fn managed_keys(settings: &Settings, lan_open: bool) -> Value {
    let expose = lan_open && settings.allow_lan;
    let mut managed = Mapping::new();
    managed.insert(
        Value::String("mixed-port".into()),
        Value::Number(settings.mixed_port.into()),
    );
    managed.insert(Value::String("allow-lan".into()), Value::Bool(expose));
    managed.insert(
        Value::String("bind-address".into()),
        Value::String(if expose { "*" } else { "127.0.0.1" }.into()),
    );
    let mut authentication = Vec::new();
    if expose && !settings.lan_username.is_empty() && !settings.lan_password.is_empty() {
        authentication.push(Value::String(format!(
            "{}:{}",
            settings.lan_username, settings.lan_password
        )));
    }
    managed.insert(
        Value::String("authentication".into()),
        Value::Sequence(authentication),
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
    // 不打开这项时，连接列表里的进程名一直是空的。
    managed.insert(
        Value::String("find-process-mode".into()),
        Value::String("always".into()),
    );
    if !settings.tun_enabled {
        let mut tun = Mapping::new();
        tun.insert(Value::String("enable".into()), Value::Bool(false));
        managed.insert(Value::String("tun".into()), Value::Mapping(tun));
        // A subscription may ship sniffer.enable: true. Disable it after the
        // merge as well, so turning TUN off cannot leave active sniffing.
        let mut sniffer = Mapping::new();
        sniffer.insert(Value::String("enable".into()), Value::Bool(false));
        managed.insert(Value::String("sniffer".into()), Value::Mapping(sniffer));
    }
    Value::Mapping(managed)
}

pub fn write_runtime_config(paths: &Paths, settings: &Settings) -> Result<(), ServiceError> {
    if settings.active_profile.is_empty() {
        return Err(ServiceError::new(codes::PROFILE_NOT_FOUND));
    }
    let body = read_profile_body(paths, &settings.active_profile)
        .map_err(|error| ServiceError::new(codes::PROFILE_NOT_FOUND).detail(error.to_string()))?;
    let profile = decode_subscription(&body)?;
    let overlay = load_override(&crate::state::effective_override(
        paths,
        &settings.override_path,
    ))?;
    let mut config = prepare_runtime_config(
        profile,
        overlay,
        settings,
        crate::lan::permitted(&crate::lan::policy_dir(), crate::lan::current_uid()),
    );
    let own = crate::rules::load_rules(&crate::rules::rules_file(paths))?;
    let names = crate::rules::policy_names(&config);
    crate::rules::validate_rules(&own, &names)?;
    crate::rules::insert_own_rules(&mut config, &own);
    // openSUSE's mihomo package does not ship an MMDB. If v2ray-core supplied
    // geoip.dat/geosite.dat, use those instead of waiting for a download.
    let geodata = crate::core::geodata_sources();
    if crate::core::geodata_mode(&geodata) == "dat" {
        if let Value::Mapping(map) = &mut config {
            map.insert(Value::String("geodata-mode".into()), Value::Bool(true));
        }
        tracing::info!("no MMDB; using geoip.dat/geosite.dat as geodata");
    }
    let text = serde_yaml::to_string(&config).map_err(|error| {
        ServiceError::new(codes::PROFILE_RUNTIME_WRITE).detail(error.to_string())
    })?;
    fs::create_dir_all(paths.runtime_dir()).map_err(ServiceError::from_io)?;
    write_private(&paths.runtime_config(), text.as_bytes()).map_err(ServiceError::from_io)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteFields {
    pub name: String,
    pub kind: String,
    pub server: String,
    pub port: u16,
    pub secret: String,
}

pub fn route_fields(body: &str, name: &str) -> Result<Option<RouteFields>, ServiceError> {
    let value = decode_subscription(body)?;
    let Some(list) = value.get("proxies").and_then(Value::as_sequence) else {
        return Ok(None);
    };
    Ok(list.iter().find_map(|item| {
        (item.get("name").and_then(Value::as_str) == Some(name)).then(|| read_route(item))
    }))
}

pub fn add_route(body: &str, group: &str, fields: &RouteFields) -> Result<String, ServiceError> {
    let fields = checked_route(fields)?;
    edit_document(body, |map| {
        if route_exists(map, &fields.name) {
            return Err(ServiceError::new(codes::PROFILE_ROUTE_DUPLICATE));
        }
        if group_named(map, &fields.name) {
            return Err(ServiceError::new(codes::PROFILE_GROUP_DUPLICATE));
        }
        proxy_list(map).push(route_mapping(&fields));
        append_group_member(map, group, &fields.name);
        Ok(())
    })
}

pub fn remove_route(body: &str, name: &str) -> Result<String, ServiceError> {
    let name = name.trim();
    if name.is_empty() || reserved_route(name) || group_name_in(body, name) {
        return Err(ServiceError::new(codes::PROFILE_ROUTE_NOT_REMOVABLE));
    }
    edit_document(body, |map| {
        let removed = {
            let list = proxy_list(map);
            let before = list.len();
            list.retain(|item| route_name(item) != name);
            list.len() != before
        };
        if !removed {
            return Err(ServiceError::new(codes::PROFILE_ROUTE_NOT_FOUND));
        }
        drop_member(map, name);
        Ok(())
    })
}

pub fn update_route(
    body: &str,
    previous: &str,
    fields: &RouteFields,
) -> Result<String, ServiceError> {
    let previous = previous.trim().to_string();
    if previous.is_empty() || reserved_route(&previous) {
        return Err(ServiceError::new(codes::PROFILE_ROUTE_NOT_EDITABLE));
    }
    let fields = checked_route(fields)?;
    edit_document(body, |map| {
        if fields.name != previous && route_exists(map, &fields.name) {
            return Err(ServiceError::new(codes::PROFILE_ROUTE_DUPLICATE));
        }
        if fields.name != previous && group_named(map, &fields.name) {
            return Err(ServiceError::new(codes::PROFILE_ROUTE_DUPLICATE));
        }
        let found = {
            let list = proxy_list(map);
            let Some(item) = list
                .iter_mut()
                .find(|item| route_name(item) == previous)
                .and_then(Value::as_mapping_mut)
            else {
                return Err(ServiceError::new(codes::PROFILE_ROUTE_NOT_FOUND));
            };
            write_route(item, &fields);
            true
        };
        if found && fields.name != previous {
            rename_member(map, &previous, &fields.name);
        }
        Ok(())
    })
}

pub fn restore_route(working: &str, snapshot: &str, name: &str) -> Result<String, ServiceError> {
    let name = name.trim();
    if name.is_empty() || reserved_route(name) {
        return Err(ServiceError::new(codes::PROFILE_ROUTE_NOT_RESTORABLE));
    }
    let snapshot_value = decode_subscription(snapshot)?;
    let Some(snapshot_map) = snapshot_value.as_mapping() else {
        return Err(ServiceError::new(codes::PROFILE_SUBSCRIPTION_UNPARSEABLE));
    };
    let original = snapshot_map
        .get(Value::String("proxies".into()))
        .and_then(Value::as_sequence)
        .and_then(|list| list.iter().find(|item| route_name(item) == name))
        .cloned()
        .ok_or_else(|| ServiceError::new(codes::PROFILE_SUBSCRIPTION_MISSING))?;
    edit_document(working, |map| {
        let list = proxy_list(map);
        let Some(item) = list.iter_mut().find(|item| route_name(item) == name) else {
            return Err(ServiceError::new(codes::PROFILE_ROUTE_NOT_FOUND));
        };
        *item = original;
        Ok(())
    })
}

pub fn route_differs(working: &str, snapshot: &str, name: &str) -> bool {
    let Ok(working) = decode_subscription(working) else {
        return false;
    };
    let Ok(snapshot) = decode_subscription(snapshot) else {
        return false;
    };
    let Some(current) = proxy_named(&working, name) else {
        return false;
    };
    let Some(original) = proxy_named(&snapshot, name) else {
        return false;
    };
    current != original
}

pub fn rule_differs(working: &str, snapshot: &str, index: usize) -> bool {
    let Ok(working) = decode_subscription(working) else {
        return false;
    };
    let Ok(snapshot) = decode_subscription(snapshot) else {
        return false;
    };
    let Some(current) = sequence_item(&working, "rules", index) else {
        return false;
    };
    let Some(original) = sequence_item(&snapshot, "rules", index) else {
        return false;
    };
    current != original
}

fn proxy_named<'a>(document: &'a Value, name: &str) -> Option<&'a Value> {
    document
        .as_mapping()?
        .get(Value::String("proxies".into()))?
        .as_sequence()?
        .iter()
        .find(|item| route_name(item) == name)
}

fn sequence_item<'a>(document: &'a Value, key: &str, index: usize) -> Option<&'a Value> {
    document
        .as_mapping()?
        .get(Value::String(key.into()))?
        .as_sequence()?
        .get(index)
}

pub fn restore_profile_rule(
    working: &str,
    snapshot: &str,
    index: usize,
) -> Result<String, ServiceError> {
    let snapshot_value = decode_subscription(snapshot)?;
    let original = snapshot_value
        .as_mapping()
        .and_then(|map| map.get(Value::String("rules".into())))
        .and_then(Value::as_sequence)
        .and_then(|list| list.get(index).cloned())
        .ok_or_else(|| ServiceError::new(codes::PROFILE_RULE_SUBSCRIPTION_MISSING))?;
    edit_document(working, |map| {
        let key = Value::String("rules".into());
        let Some(list) = map.get_mut(&key).and_then(Value::as_sequence_mut) else {
            return Err(ServiceError::new(codes::PROFILE_NO_RULES));
        };
        let Some(item) = list.get_mut(index) else {
            return Err(ServiceError::new(codes::PROFILE_RULE_MISSING));
        };
        *item = original;
        Ok(())
    })
}

fn group_name_in(body: &str, name: &str) -> bool {
    decode_subscription(body)
        .ok()
        .and_then(|value| value.as_mapping().cloned())
        .is_some_and(|map| group_named(&map, name))
}

fn checked_route(fields: &RouteFields) -> Result<RouteFields, ServiceError> {
    let name = fields.name.trim();
    if name.is_empty() || reserved_route(name) {
        return Err(ServiceError::new(codes::PROFILE_ROUTE_NAME_EMPTY));
    }
    if name.chars().count() > 128 {
        return Err(ServiceError::new(codes::PROFILE_ROUTE_NAME_TOO_LONG).param("limit", "128"));
    }
    let kind = fields.kind.trim().to_ascii_lowercase();
    if !matches!(kind.as_str(), "http" | "socks5" | "ss" | "vmess" | "trojan") {
        return Err(ServiceError::new(codes::PROFILE_ROUTE_KIND));
    }
    let server = fields.server.trim();
    if server.is_empty() {
        return Err(ServiceError::new(codes::PROFILE_SERVER_EMPTY));
    }
    if server.chars().count() > 255 {
        return Err(ServiceError::new(codes::PROFILE_SERVER_TOO_LONG).param("limit", "255"));
    }
    if fields.secret.chars().count() > 512 {
        return Err(ServiceError::new(codes::PROFILE_SECRET_TOO_LONG).param("limit", "512"));
    }
    if fields.port == 0 {
        return Err(ServiceError::new(codes::PROFILE_PORT_INVALID));
    }
    Ok(RouteFields {
        name: name.to_string(),
        kind,
        server: server.to_string(),
        port: fields.port,
        secret: fields.secret.clone(),
    })
}

fn reserved_route(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "DIRECT" | "REJECT" | "GLOBAL" | "PASS" | "COMPATIBLE"
    )
}

fn read_route(item: &Value) -> RouteFields {
    let kind = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let secret = if kind.eq_ignore_ascii_case("vmess") {
        item.get("uuid")
    } else {
        item.get("password")
    }
    .and_then(Value::as_str)
    .unwrap_or_default()
    .to_string();
    let port = item
        .get("port")
        .and_then(Value::as_u64)
        .or_else(|| {
            item.get("port")
                .and_then(Value::as_i64)
                .map(|value| value as u64)
        })
        .unwrap_or(0);
    RouteFields {
        name: route_name(item),
        kind,
        server: item
            .get("server")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        port: port.min(u64::from(u16::MAX)) as u16,
        secret,
    }
}

fn route_mapping(fields: &RouteFields) -> Value {
    let mut item = Mapping::new();
    write_route(&mut item, fields);
    Value::Mapping(item)
}

fn write_route(item: &mut Mapping, fields: &RouteFields) {
    let secret_key = if fields.kind == "vmess" {
        "uuid"
    } else {
        "password"
    };
    let other = if secret_key == "uuid" {
        "password"
    } else {
        "uuid"
    };
    item.insert(
        Value::String("name".into()),
        Value::String(fields.name.clone()),
    );
    item.insert(
        Value::String("type".into()),
        Value::String(fields.kind.clone()),
    );
    item.insert(
        Value::String("server".into()),
        Value::String(fields.server.clone()),
    );
    item.insert(
        Value::String("port".into()),
        Value::Number(fields.port.into()),
    );
    item.remove(Value::String(other.into()));
    if fields.secret.is_empty() {
        item.remove(Value::String(secret_key.into()));
    } else {
        item.insert(
            Value::String(secret_key.into()),
            Value::String(fields.secret.clone()),
        );
    }
}

fn edit_document(
    body: &str,
    edit: impl FnOnce(&mut Mapping) -> Result<(), ServiceError>,
) -> Result<String, ServiceError> {
    let mut value = decode_subscription(body)?;
    {
        let Some(map) = value.as_mapping_mut() else {
            return Err(ServiceError::new(codes::PROFILE_NOT_MAPPING));
        };
        edit(map)?;
    }
    serde_yaml::to_string(&value)
        .map_err(|error| ServiceError::new(codes::PROFILE_RUNTIME_WRITE).detail(error.to_string()))
}

fn proxy_list(map: &mut Mapping) -> &mut Vec<Value> {
    let key = Value::String("proxies".into());
    if !matches!(map.get(&key), Some(Value::Sequence(_))) {
        map.insert(key.clone(), Value::Sequence(Vec::new()));
    }
    map.get_mut(&key)
        .and_then(Value::as_sequence_mut)
        .expect("proxies")
}

fn group_list(map: &mut Mapping) -> &mut Vec<Value> {
    let key = Value::String("proxy-groups".into());
    if !matches!(map.get(&key), Some(Value::Sequence(_))) {
        map.insert(key.clone(), Value::Sequence(Vec::new()));
    }
    map.get_mut(&key)
        .and_then(Value::as_sequence_mut)
        .expect("groups")
}

fn route_name(item: &Value) -> String {
    item.get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn named_sequence<'a>(map: &'a Mapping, key: &str) -> &'a [Value] {
    map.get(Value::String(key.into()))
        .and_then(Value::as_sequence)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn route_exists(map: &Mapping, name: &str) -> bool {
    named_sequence(map, "proxies")
        .iter()
        .any(|item| route_name(item) == name)
}

fn group_named(map: &Mapping, name: &str) -> bool {
    named_sequence(map, "proxy-groups")
        .iter()
        .any(|item| route_name(item) == name)
}

fn append_group_member(map: &mut Mapping, group: &str, name: &str) {
    let Some(item) = group_list(map)
        .iter_mut()
        .find(|item| route_name(item) == group)
        .and_then(Value::as_mapping_mut)
    else {
        return;
    };
    let key = Value::String("proxies".into());
    if !matches!(item.get(&key), Some(Value::Sequence(_))) {
        item.insert(key.clone(), Value::Sequence(Vec::new()));
    }
    let Some(members) = item.get_mut(&key).and_then(Value::as_sequence_mut) else {
        return;
    };
    if !members.iter().any(|item| item.as_str() == Some(name)) {
        members.push(Value::String(name.to_string()));
    }
}

fn drop_member(map: &mut Mapping, name: &str) {
    for group in group_list(map) {
        let Some(members) = group
            .as_mapping_mut()
            .and_then(|item| item.get_mut("proxies"))
            .and_then(Value::as_sequence_mut)
        else {
            continue;
        };
        members.retain(|item| item.as_str() != Some(name));
    }
}

fn rename_member(map: &mut Mapping, previous: &str, name: &str) {
    for group in group_list(map) {
        let Some(members) = group
            .as_mapping_mut()
            .and_then(|item| item.get_mut("proxies"))
            .and_then(Value::as_sequence_mut)
        else {
            continue;
        };
        for member in members {
            if member.as_str() == Some(previous) {
                *member = Value::String(name.to_string());
            }
        }
    }
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
        assert_eq!(error.code, codes::PROFILE_CONFIG_PARSE);
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
            "allow-lan: true\nbind-address: '*'\nfind-process-mode: off\ntun:\n  enable: true\n  stack: system\nsniffer:\n  enable: true\nproxies: []\n",
        )
        .unwrap();
        let overlay =
            serde_yaml::from_str("mode: direct\nexternal-controller: 0.0.0.0:1\n").unwrap();
        let settings = Settings {
            mixed_port: 7897,
            controller_port: 19090,
            controller_secret: "secret".into(),
            mode: "rule".into(),
            ..Settings::default()
        };
        let config = prepare_runtime_config(profile, overlay, &settings, false);
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
            config.get("find-process-mode").and_then(Value::as_str),
            Some("always")
        );
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
        assert_eq!(
            config
                .get("sniffer")
                .and_then(|sniffer| sniffer.get("enable"))
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn enabled_tun_replaces_the_subscription_block() {
        let profile = serde_yaml::from_str(
            "tun:\n  enable: true\n  device: evil\n  include-uid: [0]\nsniffer:\n  enable: false\nproxies: []\n",
        )
        .unwrap();
        let mut settings = Settings {
            tun_enabled: true,
            mode: "rule".into(),
            ..Settings::default()
        };
        let config =
            prepare_runtime_config(profile, Value::Mapping(Mapping::new()), &settings, false);
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
        let excluded = tun
            .get("route-exclude-address")
            .and_then(Value::as_sequence)
            .expect("route exclusions");
        assert!(
            excluded
                .iter()
                .any(|item| item.as_str() == Some("100.64.0.0/10"))
        );
        assert!(
            excluded
                .iter()
                .any(|item| item.as_str() == Some("192.168.0.0/16"))
        );
        assert!(tun.get("device").is_none());
        settings.tun_device = "clash0".into();
        let again = serde_yaml::from_str(
            "tun:\n  enable: true\n  device: evil\n  include-uid: [0]\nsniffer:\n  enable: false\nproxies: []\n",
        )
        .unwrap();
        let named = prepare_runtime_config(again, Value::Mapping(Mapping::new()), &settings, false);
        assert_eq!(
            named
                .get("tun")
                .and_then(|tun| tun.get("device"))
                .and_then(Value::as_str),
            Some("clash0")
        );
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
        crate::privilege::assess_tun_document(&text, uid, "user").unwrap();
        settings.tun_machine = true;
        let machine = prepare_runtime_config(
            serde_yaml::from_str(
                "tun:\n  enable: true\n  device: evil\n  include-uid: [0]\nsniffer:\n  enable: false\nproxies: []\n",
            )
            .unwrap(),
            Value::Mapping(Mapping::new()),
            &settings,
            false,
        );
        assert!(machine.get("tun").unwrap().get("include-uid").is_none());
        assert!(machine.get("tun").unwrap().get("exclude-uid").is_none());
        let machine_text = serde_yaml::to_string(&machine).unwrap();
        crate::privilege::assess_tun_document(&machine_text, uid, "machine").unwrap();
        assert!(crate::privilege::assess_tun_document(&machine_text, uid, "user").is_err());
        assert_eq!(
            config.get("allow-lan").and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn selector_groups_come_from_the_profile_file() {
        let body = "\
proxy-groups:
  - name: 备用
    type: url-test
    proxies:
      - B
  - name: 节点选择
    type: select
    proxies:
      - A
      - DIRECT
";
        let groups = selector_groups(body);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "备用");
        assert_eq!(groups[0].kind, "url-test");
        assert_eq!(groups[1].kind, "Selector");
        assert_eq!(groups[1].nodes.len(), 2);
    }

    #[test]
    fn routes_are_written_back_into_the_profile_and_its_groups() {
        let body = "\
proxies:
  - name: A
    type: ss
    server: one.example
    port: 1
    password: keep
proxy-groups:
  - name: 选择
    type: select
    proxies:
      - A
      - DIRECT
";
        let added = add_route(
            body,
            "选择",
            &RouteFields {
                name: "B".into(),
                kind: "HTTP".into(),
                server: " two.example ".into(),
                port: 80,
                secret: "pw".into(),
            },
        )
        .unwrap();
        let detail = route_fields(&added, "B").unwrap().unwrap();
        assert_eq!(detail.kind, "http");
        assert_eq!(detail.server, "two.example");
        assert_eq!(detail.secret, "pw");
        assert!(
            selector_groups(&added)[0]
                .nodes
                .iter()
                .any(|node| node.name == "B")
        );
        let renamed = update_route(
            &added,
            "B",
            &RouteFields {
                name: "C".into(),
                kind: "vmess".into(),
                server: "three.example".into(),
                port: 443,
                secret: "uuid".into(),
            },
        )
        .unwrap();
        assert!(route_fields(&renamed, "B").unwrap().is_none());
        assert_eq!(route_fields(&renamed, "C").unwrap().unwrap().secret, "uuid");
        assert!(
            selector_groups(&renamed)[0]
                .nodes
                .iter()
                .any(|node| node.name == "C")
        );
        let removed = remove_route(&renamed, "C").unwrap();
        assert!(route_fields(&removed, "C").unwrap().is_none());
        assert!(
            selector_groups(&removed)[0]
                .nodes
                .iter()
                .all(|node| node.name != "C")
        );
        assert!(remove_route(body, "DIRECT").is_err());
        assert!(remove_route(body, "选择").is_err());
    }

    #[test]
    fn lan_opens_the_mixed_port_only_when_allowed() {
        let settings = Settings {
            mixed_port: 7890,
            controller_port: 9090,
            allow_lan: true,
            lan_username: "ada".into(),
            lan_password: "secret".into(),
            ..Settings::default()
        };
        let profile = decode_subscription("proxies: []\n").unwrap();
        let open = prepare_runtime_config(
            profile.clone(),
            Value::Mapping(Mapping::new()),
            &settings,
            true,
        );
        assert_eq!(open.get("allow-lan").and_then(Value::as_bool), Some(true));
        assert_eq!(open.get("bind-address").and_then(Value::as_str), Some("*"));
        assert_eq!(
            open.get("external-controller").and_then(Value::as_str),
            Some("127.0.0.1:9090")
        );
        let auth = open
            .get("authentication")
            .and_then(Value::as_sequence)
            .unwrap();
        assert_eq!(auth[0].as_str(), Some("ada:secret"));
        let closed =
            prepare_runtime_config(profile, Value::Mapping(Mapping::new()), &settings, false);
        assert_eq!(
            closed.get("allow-lan").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            closed.get("bind-address").and_then(Value::as_str),
            Some("127.0.0.1")
        );
        assert!(
            closed
                .get("authentication")
                .and_then(Value::as_sequence)
                .unwrap()
                .is_empty()
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

    #[test]
    fn own_rules_are_prepended_without_changing_the_override_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            data_dir: dir.path().join("data"),
            kioslaverc: dir.path().join("kioslaverc"),
        };
        let body = "rules:\n- DOMAIN,a.com,PROXY\n- MATCH,DIRECT\nproxy-groups:\n- {name: PROXY, type: select, proxies: [DIRECT]}\n";
        write_profile_body(&paths, "profile", body).unwrap();
        let override_path = paths.override_file();
        fs::create_dir_all(override_path.parent().unwrap()).unwrap();
        fs::write(&override_path, "mode: rule\n").unwrap();
        let before = fs::read(&override_path).unwrap();
        crate::rules::save_rules(
            &crate::rules::rules_file(&paths),
            &[crate::rules::OwnRule {
                kind: "DOMAIN-SUFFIX".into(),
                payload: "b.com".into(),
                target: "REJECT".into(),
                enabled: true,
            }],
        )
        .unwrap();
        let settings = Settings {
            active_profile: "profile".into(),
            ..Settings::default()
        };
        write_runtime_config(&paths, &settings).unwrap();
        assert_eq!(fs::read(&override_path).unwrap(), before);
        let runtime = fs::read_to_string(paths.runtime_config()).unwrap();
        let config: Value = serde_yaml::from_str(&runtime).unwrap();
        let lines: Vec<&str> = config
            .get("rules")
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(
            lines,
            vec![
                "DOMAIN-SUFFIX,b.com,REJECT",
                "DOMAIN,a.com,PROXY",
                "MATCH,DIRECT"
            ]
        );
    }

    #[test]
    fn restore_puts_the_subscription_proxy_back() {
        let snapshot = "proxies:\n- {name: A, type: ss, server: one.example, port: 1, password: old, cipher: aes-128-gcm}\n";
        let working = "proxies:\n- {name: A, type: ss, server: two.example, port: 2, password: new, cipher: aes-128-gcm}\n";
        let restored = restore_route(working, snapshot, "A").unwrap();
        let fields = route_fields(&restored, "A").unwrap().unwrap();
        assert_eq!(fields.server, "one.example");
        assert_eq!(fields.port, 1);
        assert_eq!(fields.secret, "old");
        assert!(restored.contains("aes-128-gcm"));
        assert!(restore_route(working, snapshot, "B").is_err());
    }

    #[test]
    fn an_unchanged_item_does_not_differ_from_the_subscription() {
        let snapshot = "proxies:\n- {name: A, type: ss, server: one.example, port: 1, password: old, cipher: aes-128-gcm}\nrules:\n- DOMAIN,a.com,DIRECT\n";
        let edited = "proxies:\n- {name: A, type: ss, server: two.example, port: 1, password: old, cipher: aes-128-gcm}\nrules:\n- DOMAIN,b.com,DIRECT\n";
        assert!(!route_differs(snapshot, snapshot, "A"));
        assert!(route_differs(edited, snapshot, "A"));
        assert!(!route_differs(edited, snapshot, "B"));
        assert!(!rule_differs(snapshot, snapshot, 0));
        assert!(rule_differs(edited, snapshot, 0));
        assert!(!rule_differs(edited, snapshot, 3));
    }

    #[test]
    fn a_missing_file_is_explained_without_english_errno_text() {
        let missing =
            std::io::Error::new(std::io::ErrorKind::NotFound, "No such file or directory");
        let error = explain_io("read profile", &missing);
        assert_eq!(error.code, codes::IO_NOT_FOUND);
        assert!(error.detail.contains("read profile"));
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied");
        let error = explain_io("write profile", &denied);
        assert_eq!(error.code, codes::IO_DENIED);
        assert!(error.detail.contains("write profile"));
    }

    #[test]
    fn restore_puts_the_subscription_rule_back() {
        let snapshot = "rules:\n- DOMAIN,a.com,DIRECT\n- MATCH,REJECT\n";
        let working = "rules:\n- DOMAIN,b.com,PROXY\n- MATCH,DIRECT\n";
        let restored = restore_profile_rule(working, snapshot, 0).unwrap();
        let value: Value = serde_yaml::from_str(&restored).unwrap();
        let rules = value.get("rules").and_then(Value::as_sequence).unwrap();
        assert_eq!(rules[0].as_str(), Some("DOMAIN,a.com,DIRECT"));
        assert_eq!(rules[1].as_str(), Some("MATCH,DIRECT"));
        assert_eq!(
            restore_profile_rule(working, snapshot, 4).unwrap_err().code,
            codes::PROFILE_RULE_SUBSCRIPTION_MISSING
        );
        let short = "rules:\n- DOMAIN,b.com,PROXY\n";
        assert_eq!(
            restore_profile_rule(short, snapshot, 1).unwrap_err().code,
            codes::PROFILE_RULE_MISSING
        );
    }
}
