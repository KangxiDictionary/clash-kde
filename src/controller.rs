// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

use std::time::Duration;

use bytes::Bytes;
use futures_util::Stream;
use reqwest::Client;
use serde_json::{Value, json};

use crate::error::{ServiceError, codes};

const DELAY_URL: &str = "http://www.gstatic.com/generate_204";

#[derive(Clone)]
pub struct Controller {
    client: Client,
    traffic: Client,
    base: String,
}

impl Controller {
    pub fn new(port: u16, secret: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        if !secret.is_empty() {
            let value = reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}"))
                .expect("secret is header-safe");
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let client = Client::builder()
            .default_headers(headers.clone())
            .timeout(Duration::from_secs(15))
            .build()
            .expect("controller client");
        let traffic = Client::builder()
            .default_headers(headers)
            .build()
            .expect("traffic client");
        Self {
            client,
            traffic,
            base: format!("http://127.0.0.1:{port}"),
        }
    }

    pub async fn probe(&self) -> bool {
        self.client
            .get(format!("{}/version", self.base))
            .timeout(Duration::from_millis(500))
            .send()
            .await
            .is_ok()
    }

    pub async fn set_mode(&self, mode: &str) -> Result<(), ServiceError> {
        let response = self
            .client
            .patch(format!("{}/configs", self.base))
            .json(&json!({ "mode": mode }))
            .send()
            .await
            .map_err(|error| ServiceError::new(codes::CONTROLLER_MODE).detail(error.to_string()))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ServiceError::new(codes::CONTROLLER_MODE)
                .detail(format!("HTTP {}", response.status())))
        }
    }

    pub async fn select_proxy(&self, group: &str, name: &str) -> Result<(), ServiceError> {
        let response = self
            .client
            .put(format!("{}/proxies/{}", self.base, path_segment(group)))
            .json(&json!({ "name": name }))
            .send()
            .await
            .map_err(|error| {
                ServiceError::new(codes::CONTROLLER_SELECT).detail(error.to_string())
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ServiceError::new(codes::CONTROLLER_SELECT)
                .detail(format!("HTTP {}", response.status())))
        }
    }

    pub async fn proxies(&self) -> Result<Vec<ProxyGroup>, ServiceError> {
        let response = self
            .client
            .get(format!("{}/proxies", self.base))
            .send()
            .await
            .map_err(|error| {
                ServiceError::new(codes::CONTROLLER_READ_GROUPS).detail(error.to_string())
            })?;
        if !response.status().is_success() {
            return Err(ServiceError::new(codes::CONTROLLER_READ_GROUPS)
                .detail(format!("HTTP {}", response.status())));
        }
        let body: Value = response.json().await.map_err(|error| {
            ServiceError::new(codes::CONTROLLER_PARSE_GROUPS).detail(error.to_string())
        })?;
        Ok(parse_groups(&body))
    }

    pub async fn test_delay(&self, name: &str) -> i64 {
        let url = format!(
            "{}/proxies/{}/delay?timeout=5000&url={}",
            self.base,
            path_segment(name),
            urlencoding::encode(DELAY_URL)
        );
        let Ok(response) = self.client.get(url).send().await else {
            return 0;
        };
        if !response.status().is_success() {
            return 0;
        }
        response
            .json::<Value>()
            .await
            .ok()
            .and_then(|value| value.get("delay").and_then(Value::as_i64))
            .filter(|delay| *delay > 0)
            // 0 is the QML "Timeout" sentinel; -1 is "Not tested".
            .unwrap_or(0)
    }

    pub async fn traffic_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<Bytes, reqwest::Error>>, ServiceError> {
        let response = self
            .traffic
            .get(format!("{}/traffic", self.base))
            .send()
            .await
            .map_err(|error| {
                ServiceError::new(codes::CONTROLLER_READ_TRAFFIC).detail(error.to_string())
            })?;
        if !response.status().is_success() {
            return Err(ServiceError::new(codes::CONTROLLER_READ_TRAFFIC)
                .detail(format!("HTTP {}", response.status())));
        }
        Ok(response.bytes_stream())
    }

    pub async fn connections(&self) -> Result<Vec<ConnectionRow>, ServiceError> {
        let response = self
            .client
            .get(format!("{}/connections", self.base))
            .send()
            .await
            .map_err(|error| {
                ServiceError::new(codes::CONTROLLER_READ_CONNECTIONS).detail(error.to_string())
            })?;
        if !response.status().is_success() {
            return Err(ServiceError::new(codes::CONTROLLER_READ_CONNECTIONS)
                .detail(format!("HTTP {}", response.status())));
        }
        let body: Value = response.json().await.map_err(|error| {
            ServiceError::new(codes::CONTROLLER_PARSE_CONNECTIONS).detail(error.to_string())
        })?;
        Ok(parse_connections(&body))
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), ServiceError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(ServiceError::new(codes::CONTROLLER_NO_CONNECTION));
        }
        let response = self
            .client
            .delete(format!("{}/connections/{}", self.base, path_segment(id)))
            .send()
            .await
            .map_err(|error| {
                ServiceError::new(codes::CONTROLLER_CLOSE_CONNECTION).detail(error.to_string())
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ServiceError::new(codes::CONTROLLER_CLOSE_CONNECTION)
                .detail(format!("HTTP {}", response.status())))
        }
    }

    pub async fn close_all_connections(&self) -> Result<(), ServiceError> {
        let response = self
            .client
            .delete(format!("{}/connections", self.base))
            .send()
            .await
            .map_err(|error| {
                ServiceError::new(codes::CONTROLLER_CLOSE_CONNECTION).detail(error.to_string())
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ServiceError::new(codes::CONTROLLER_CLOSE_CONNECTION)
                .detail(format!("HTTP {}", response.status())))
        }
    }

    pub async fn proxy_providers(&self) -> Result<Vec<LiveProvider>, ServiceError> {
        self.provider_list("proxies", ProviderKind::Proxy).await
    }

    pub async fn rule_providers(&self) -> Result<Vec<LiveProvider>, ServiceError> {
        self.provider_list("rules", ProviderKind::Rule).await
    }

    async fn provider_list(
        &self,
        kind: &str,
        count: ProviderKind,
    ) -> Result<Vec<LiveProvider>, ServiceError> {
        let response = self
            .client
            .get(format!("{}/providers/{kind}", self.base))
            .send()
            .await
            .map_err(|error| {
                ServiceError::new(codes::CONTROLLER_READ_PROVIDERS).detail(error.to_string())
            })?;
        if !response.status().is_success() {
            return Err(ServiceError::new(codes::CONTROLLER_READ_PROVIDERS)
                .detail(format!("HTTP {}", response.status())));
        }
        let body: Value = response.json().await.map_err(|error| {
            ServiceError::new(codes::CONTROLLER_PARSE_PROVIDERS).detail(error.to_string())
        })?;
        Ok(parse_providers(&body, count))
    }

    pub async fn update_proxy_provider(&self, name: &str) -> Result<(), ServiceError> {
        self.update_provider("proxies", name).await
    }

    pub async fn update_rule_provider(&self, name: &str) -> Result<(), ServiceError> {
        self.update_provider("rules", name).await
    }

    async fn update_provider(&self, kind: &str, name: &str) -> Result<(), ServiceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ServiceError::new(codes::CONTROLLER_UPDATE_PROVIDER));
        }
        let response = self
            .client
            .put(format!(
                "{}/providers/{kind}/{}",
                self.base,
                path_segment(name)
            ))
            .send()
            .await
            .map_err(|error| {
                ServiceError::new(codes::CONTROLLER_UPDATE_PROVIDER).detail(error.to_string())
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ServiceError::new(codes::CONTROLLER_UPDATE_PROVIDER)
                .detail(format!("HTTP {}", response.status())))
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProxyNode {
    pub name: String,
    pub delay_ms: i64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProxyGroup {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub now: String,
    pub nodes: Vec<ProxyNode>,
}

fn parse_groups(body: &Value) -> Vec<ProxyGroup> {
    let Some(proxies) = body.get("proxies").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut groups = Vec::new();
    for (name, value) in proxies {
        let Some(all) = value.get("all").and_then(Value::as_array) else {
            continue;
        };
        if all.is_empty() {
            continue;
        }
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("Selector")
            .to_string();
        let now = value
            .get("now")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let nodes = all
            .iter()
            .filter_map(Value::as_str)
            .map(|node_name| ProxyNode {
                delay_ms: proxies.get(node_name).map(delay_of).unwrap_or(-1),
                name: node_name.to_string(),
            })
            .collect();
        groups.push(ProxyGroup {
            name: name.clone(),
            kind,
            now,
            nodes,
        });
    }
    groups.sort_by(|left, right| left.name.cmp(&right.name));
    groups
}

fn delay_of(value: &Value) -> i64 {
    value
        .get("history")
        .and_then(Value::as_array)
        .and_then(|history| history.last())
        .and_then(|entry| entry.get("delay"))
        .and_then(Value::as_i64)
        .filter(|delay| *delay >= 0)
        .unwrap_or(-1)
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ConnectionRow {
    pub id: String,
    pub process: String,
    pub process_path: String,
    pub host: String,
    pub port: String,
    pub destination_ip: String,
    pub source_ip: String,
    pub source_port: String,
    pub rule: String,
    pub rule_payload: String,
    pub upload: u64,
    pub download: u64,
    pub network: String,
}

#[derive(Clone, Copy, Debug)]
enum ProviderKind {
    Proxy,
    Rule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveProvider {
    pub name: String,
    pub updated_at: String,
    pub count: i64,
}

pub fn parse_connections(body: &Value) -> Vec<ConnectionRow> {
    let Some(list) = body.get("connections").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter().filter_map(connection_row).collect()
}

fn connection_row(value: &Value) -> Option<ConnectionRow> {
    let id = value.get("id").and_then(Value::as_str).unwrap_or("").trim();
    if id.is_empty() {
        return None;
    }
    let metadata = value.get("metadata");
    let host = clean_host(&meta_text(metadata, "host"));
    let process_path = meta_text(metadata, "processPath");
    let mut process = meta_text(metadata, "process");
    if process.is_empty() && !process_path.is_empty() {
        process = process_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .to_string();
    }
    Some(ConnectionRow {
        id: id.to_string(),
        process,
        process_path,
        host,
        port: meta_text(metadata, "destinationPort"),
        destination_ip: meta_text(metadata, "destinationIP"),
        source_ip: meta_text(metadata, "sourceIP"),
        source_port: meta_text(metadata, "sourcePort"),
        rule: text_field(value, "rule"),
        rule_payload: text_field(value, "rulePayload"),
        upload: number_field(value, "upload"),
        download: number_field(value, "download"),
        network: meta_text(metadata, "network"),
    })
}

fn meta_text(metadata: Option<&Value>, key: &str) -> String {
    metadata
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn text_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn number_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

pub fn clean_host(text: &str) -> String {
    let text = text.trim();
    let text = text.split(['?', '#']).next().unwrap_or("");
    let text = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(text);
    if text.starts_with('[')
        && let Some(end) = text.find(']')
    {
        return text[1..end].to_string();
    }
    let text = text.split('/').next().unwrap_or("");
    if let Some((host, port)) = text.rsplit_once(':')
        && !host.is_empty()
        && !host.contains(':')
        && port.chars().all(|ch| ch.is_ascii_digit())
    {
        return host.to_string();
    }
    text.to_string()
}

fn parse_providers(body: &Value, kind: ProviderKind) -> Vec<LiveProvider> {
    let Some(map) = body.get("providers").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for (name, value) in map {
        let named = value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(name)
            .trim();
        if named.is_empty() {
            continue;
        }
        rows.push(LiveProvider {
            name: named.to_string(),
            updated_at: tidy_time(text_field(value, "updatedAt")),
            count: provider_count(value, kind),
        });
    }
    rows.sort_by(|left, right| left.name.cmp(&right.name));
    rows
}

fn provider_count(value: &Value, kind: ProviderKind) -> i64 {
    match kind {
        ProviderKind::Proxy => value
            .get("proxies")
            .and_then(Value::as_array)
            .map(|items| items.len() as i64)
            .unwrap_or(0),
        ProviderKind::Rule => value.get("ruleCount").and_then(Value::as_i64).unwrap_or(0),
    }
}

fn tidy_time(text: String) -> String {
    let Some(dot) = text.find('.') else {
        return text;
    };
    let rest = &text[dot + 1..];
    let zone = rest.find(['+', '-', 'Z']).unwrap_or(rest.len());
    let digits: String = rest[..zone].chars().take(3).collect();
    format!("{}{digits}{}", &text[..=dot], &rest[zone..])
}

pub fn path_segment(name: &str) -> String {
    let mut out = String::new();
    for byte in name.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct SubscriptionUsage {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire_unix: i64,
}

pub struct SubscriptionDocument {
    pub body: String,
    pub usage: SubscriptionUsage,
}

pub fn parse_subscription_userinfo(header: &str) -> SubscriptionUsage {
    let mut usage = SubscriptionUsage::default();
    for part in header.split(';') {
        let mut pair = part.splitn(2, '=');
        let key = pair.next().unwrap_or("").trim();
        let value = pair.next().unwrap_or("").trim();
        let Ok(number) = value.parse::<u64>() else {
            continue;
        };
        match key {
            "upload" => usage.upload = number,
            "download" => usage.download = number,
            "total" => usage.total = number,
            "expire" => usage.expire_unix = number as i64,
            _ => {}
        }
    }
    usage
}

pub async fn fetch_subscription(url: &str) -> Result<SubscriptionDocument, ServiceError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("clash.meta")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client.get(url).send().await.map_err(|error| {
        ServiceError::new(codes::CONTROLLER_SUBSCRIPTION_DOWNLOAD).detail(error.to_string())
    })?;
    if !response.status().is_success() {
        return Err(ServiceError::new(codes::CONTROLLER_SUBSCRIPTION_DOWNLOAD)
            .detail(format!("HTTP {}", response.status())));
    }
    let usage = response
        .headers()
        .get("subscription-userinfo")
        .and_then(|value| value.to_str().ok())
        .map(parse_subscription_userinfo)
        .unwrap_or_default();
    let body = response.text().await.map_err(|error| {
        ServiceError::new(codes::CONTROLLER_SUBSCRIPTION_READ).detail(error.to_string())
    })?;
    Ok(SubscriptionDocument { body, usage })
}

const REGION_URLS: [&str; 4] = [
    "https://ipwho.is/",
    "https://api.ip.sb/geoip",
    "https://ipapi.co/json/",
    "https://ipinfo.io/json",
];

pub fn describe_region(body: &Value) -> Option<String> {
    if body.get("success").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let code = country_code(body);
    let city = first_text(body, &["city"]);
    let ip = first_text(body, &["ip", "query"]);
    if !code.is_empty() {
        return Some(format!("{code}\t{city}\t{ip}"));
    }
    let country = first_text(body, &["country_name", "country"]);
    let place = match (country.is_empty(), city.is_empty()) {
        (true, true) => String::new(),
        (false, true) => country,
        (true, false) => city,
        (false, false) => format!("{country} · {city}"),
    };
    if place.is_empty() && ip.is_empty() {
        None
    } else if ip.is_empty() {
        Some(place)
    } else if place.is_empty() {
        Some(ip)
    } else {
        Some(format!("{place} ({ip})"))
    }
}

fn country_code(body: &Value) -> String {
    for key in ["country_code", "countryCode", "country"] {
        let Some(text) = body.get(key).and_then(Value::as_str) else {
            continue;
        };
        let code = text.trim();
        if code.len() == 2 && code.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return code.to_ascii_uppercase();
        }
    }
    String::new()
}

fn first_text(body: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| body.get(*key).and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionProbe {
    Direct,
    Proxy(u16),
}

/// 系统代理正在接管时，检测请求自己不会读桌面的代理设置，所以要显式走混合端口。
/// TUN 生效时直接访问，连接会被网卡接走。两边都没接管时也直接访问。
pub fn region_probe(system_proxy_applied: bool, tun_active: bool, port: u16) -> RegionProbe {
    if system_proxy_applied && !tun_active {
        RegionProbe::Proxy(port)
    } else {
        RegionProbe::Direct
    }
}

fn region_client(probe: RegionProbe) -> Result<Client, ServiceError> {
    let builder = Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        );
    let builder = match probe {
        RegionProbe::Direct => builder.no_proxy(),
        RegionProbe::Proxy(port) => {
            let proxy =
                reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).map_err(|error| {
                    ServiceError::new(codes::CONTROLLER_REGION_PROXY).detail(error.to_string())
                })?;
            builder.proxy(proxy)
        }
    };
    builder.build().map_err(|error| {
        ServiceError::new(codes::CONTROLLER_REGION_PROXY).detail(error.to_string())
    })
}

pub async fn lookup_exit_region(probe: RegionProbe) -> Result<String, ServiceError> {
    let client = region_client(probe)?;
    let mut tasks = tokio::task::JoinSet::new();
    for url in REGION_URLS {
        let client = client.clone();
        tasks.spawn(async move { fetch_region(&client, url).await });
    }
    let mut last_error = ServiceError::new(codes::CONTROLLER_REGION_FAILED);
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok(Ok(region)) => {
                tasks.abort_all();
                return Ok(region);
            }
            Ok(Err(error)) => last_error = error,
            Err(_) => {}
        }
    }
    Err(last_error)
}

async fn fetch_region(client: &Client, url: &str) -> Result<String, ServiceError> {
    let response = client.get(url).send().await.map_err(|error| {
        ServiceError::new(codes::CONTROLLER_REGION_FAILED).detail(error.to_string())
    })?;
    if !response.status().is_success() {
        return Err(ServiceError::new(codes::CONTROLLER_REGION_HTTP)
            .detail(format!("HTTP {}", response.status())));
    }
    let body: Value = response.json().await.map_err(|error| {
        ServiceError::new(codes::CONTROLLER_REGION_PARSE).detail(error.to_string())
    })?;
    describe_region(&body).ok_or_else(|| ServiceError::new(codes::CONTROLLER_REGION_EMPTY))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selector_groups_and_delay_history() {
        let body = serde_json::json!({
            "proxies": {
                "节点 A": {"type": "Shadowsocks", "history": [{"delay": 42}]},
                "PROXY": {"type": "Selector", "now": "节点 A", "all": ["节点 A", "DIRECT"]},
                "DIRECT": {"type": "Direct"}
            }
        });
        let groups = parse_groups(&body);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "PROXY");
        assert_eq!(groups[0].now, "节点 A");
        assert_eq!(groups[0].nodes[0].delay_ms, 42);
        assert_eq!(groups[0].nodes[1].delay_ms, -1);
    }

    #[test]
    fn path_segment_encodes_spaces_and_unicode() {
        assert_eq!(path_segment("a b"), "a%20b");
        assert_eq!(path_segment("香港"), "%E9%A6%99%E6%B8%AF");
    }

    #[test]
    fn parses_subscription_userinfo() {
        let usage =
            parse_subscription_userinfo("upload=10; download=20; total=100; expire=1700000000");
        assert_eq!(usage.upload, 10);
        assert_eq!(usage.download, 20);
        assert_eq!(usage.total, 100);
        assert_eq!(usage.expire_unix, 1_700_000_000);
    }

    #[test]
    fn describes_region_from_common_services() {
        let ipwho = serde_json::json!({"success": true, "country": "Japan", "country_code": "JP", "city": "Tokyo", "ip": "1.1.1.1"});
        assert_eq!(
            describe_region(&ipwho).as_deref(),
            Some("JP\tTokyo\t1.1.1.1")
        );
        let named = serde_json::json!({"success": true, "country": "Japan", "city": "Tokyo", "ip": "1.1.1.1"});
        assert_eq!(
            describe_region(&named).as_deref(),
            Some("Japan · Tokyo (1.1.1.1)")
        );
        let ipinfo = serde_json::json!({"ip": "8.8.8.8", "city": "Tokyo", "country": "JP"});
        assert_eq!(
            describe_region(&ipinfo).as_deref(),
            Some("JP\tTokyo\t8.8.8.8")
        );
        let failed = serde_json::json!({"success": false});
        assert!(describe_region(&failed).is_none());
    }

    #[test]
    fn parses_connections_without_keeping_a_query_token() {
        let body = serde_json::json!({
            "connections": [{
                "id": "abc",
                "metadata": {
                    "network": "tcp",
                    "host": "https://example.com/path?token=secret",
                    "destinationPort": "443",
                    "destinationIP": "1.2.3.4",
                    "sourceIP": "127.0.0.1",
                    "sourcePort": "40000",
                    "process": "firefox",
                    "processPath": "/usr/bin/firefox"
                },
                "upload": 10,
                "download": 20,
                "rule": "DomainSuffix",
                "rulePayload": "example.com"
            }]
        });
        let rows = parse_connections(&body);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].host, "example.com");
        assert_eq!(rows[0].process, "firefox");
        assert_eq!(rows[0].download, 20);
        let text = serde_json::to_string(&rows).unwrap();
        assert!(!text.contains("token"));
        let path_only = serde_json::json!({
            "connections": [{
                "id": "path-only",
                "metadata": {
                    "host": "example.com",
                    "process": "",
                    "processPath": "/usr/bin/firefox"
                }
            }]
        });
        let named = parse_connections(&path_only);
        assert_eq!(named[0].process, "firefox");
        assert_eq!(named[0].process_path, "/usr/bin/firefox");
    }

    #[test]
    fn parses_provider_counts_without_the_url() {
        let body = serde_json::json!({
            "providers": {
                "alpha": {
                    "name": "alpha",
                    "vehicleType": "HTTP",
                    "updatedAt": "2024-01-02T03:04:05.123456789Z",
                    "subscriptionInfo": {"Upload": 1},
                    "proxies": [{}, {}]
                }
            }
        });
        let rows = parse_providers(&body, ProviderKind::Proxy);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "alpha");
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[0].updated_at, "2024-01-02T03:04:05.123Z");
        let text = format!("{:?}", rows[0]);
        assert!(!text.contains("subscription"));
    }

    #[test]
    fn region_probe_follows_the_live_exit() {
        assert_eq!(region_probe(true, false, 7890), RegionProbe::Proxy(7890));
        assert_eq!(region_probe(true, true, 7890), RegionProbe::Direct);
        assert_eq!(region_probe(false, true, 7890), RegionProbe::Direct);
        assert_eq!(region_probe(false, false, 7890), RegionProbe::Direct);
    }
}
