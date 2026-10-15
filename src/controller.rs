use std::time::Duration;

use bytes::Bytes;
use futures_util::Stream;
use reqwest::Client;
use serde_json::{Value, json};

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

    pub async fn set_mode(&self, mode: &str) -> Result<(), String> {
        let response = self
            .client
            .patch(format!("{}/configs", self.base))
            .json(&json!({ "mode": mode }))
            .send()
            .await
            .map_err(|error| format!("无法切换模式: {error}"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("无法切换模式: HTTP {}", response.status()))
        }
    }

    pub async fn select_proxy(&self, group: &str, name: &str) -> Result<(), String> {
        let response = self
            .client
            .put(format!("{}/proxies/{}", self.base, path_segment(group)))
            .json(&json!({ "name": name }))
            .send()
            .await
            .map_err(|error| format!("无法选择节点: {error}"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("无法选择节点: HTTP {}", response.status()))
        }
    }

    pub async fn proxies(&self) -> Result<Vec<ProxyGroup>, String> {
        let response = self
            .client
            .get(format!("{}/proxies", self.base))
            .send()
            .await
            .map_err(|error| format!("无法读取策略组: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("无法读取策略组: HTTP {}", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|error| format!("策略组响应无法解析: {error}"))?;
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
            .unwrap_or(0)
    }

    pub async fn traffic_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<Bytes, reqwest::Error>>, String> {
        let response = self
            .traffic
            .get(format!("{}/traffic", self.base))
            .send()
            .await
            .map_err(|error| format!("无法读取流量: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("无法读取流量: HTTP {}", response.status()));
        }
        Ok(response.bytes_stream())
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

pub async fn fetch_subscription(url: &str) -> Result<SubscriptionDocument, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("clash.meta")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("无法下载订阅: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("无法下载订阅: HTTP {}", response.status()));
    }
    let usage = response
        .headers()
        .get("subscription-userinfo")
        .and_then(|value| value.to_str().ok())
        .map(parse_subscription_userinfo)
        .unwrap_or_default();
    let body = response
        .text()
        .await
        .map_err(|error| format!("无法读取订阅: {error}"))?;
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
    let country = first_text(body, &["country_name", "country"]);
    let city = first_text(body, &["city"]);
    let ip = first_text(body, &["ip", "query"]);
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

fn first_text(body: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| body.get(*key).and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_string()
}

pub async fn lookup_exit_region(port: u16) -> Result<String, String> {
    let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
        .map_err(|error| format!("无法设置出口检测代理: {error}"))?;
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(8))
        .user_agent(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        )
        .build()
        .map_err(|error| error.to_string())?;
    let mut tasks = tokio::task::JoinSet::new();
    for url in REGION_URLS {
        let client = client.clone();
        tasks.spawn(async move { fetch_region(&client, url).await });
    }
    let mut last_error = "出口地区检测失败".to_string();
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

async fn fetch_region(client: &Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("出口地区检测失败: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("出口地区检测失败: HTTP {}", response.status()));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|error| format!("出口地区检测失败: {error}"))?;
    describe_region(&body).ok_or_else(|| "出口地区检测没有返回结果".to_string())
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
        let ipwho = serde_json::json!({"success": true, "country": "Japan", "city": "Tokyo", "ip": "1.1.1.1"});
        assert_eq!(
            describe_region(&ipwho).as_deref(),
            Some("Japan · Tokyo (1.1.1.1)")
        );
        let ipinfo = serde_json::json!({"ip": "8.8.8.8", "city": "Tokyo", "country": "JP"});
        assert_eq!(
            describe_region(&ipinfo).as_deref(),
            Some("JP · Tokyo (8.8.8.8)")
        );
        let failed = serde_json::json!({"success": false});
        assert!(describe_region(&failed).is_none());
    }
}
