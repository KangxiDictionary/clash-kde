// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

//! TUN configuration and route health.
//!
//! mihomo creates the TUN device before it installs the `ip rule` and the
//! table-2022 default route. Tailscale, firewalls, and other VPN tools can also
//! rewrite those rules later. This module owns the route layout and the probe
//! used at startup and while the core is running.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_yaml::{Mapping, Value};

use crate::state::Settings;

pub const DEFAULT_ROUTE_TABLE: u32 = 2022;
pub const DEFAULT_RULE_INDEX: u32 = 9000;
pub const DEFAULT_HEALTH_SECS: u32 = 5;
pub const STARTUP_TIMEOUT_SECS: u64 = 30;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

/// Keep the tailnet, the LAN, and link-local traffic out of the proxy.
/// Tailscale owns 100.64.0.0/10 and fd7a:115c:a1e0::/48; routing those into
/// mihomo makes tailscaled rewrite the policy rules.
const EXCLUDED_NETWORKS: [&str; 8] = [
    "100.64.0.0/10",
    "169.254.0.0/16",
    "192.168.0.0/16",
    "10.0.0.0/8",
    "172.16.0.0/12",
    "fd7a:115c:a1e0::/48",
    "fc00::/7",
    "fe80::/10",
];

#[derive(Clone, Debug)]
pub struct TunOptions {
    pub route_table: u32,
    pub rule_index: u32,
    pub exclude_private: bool,
    pub health_secs: u32,
    pub device: String,
}

impl Default for TunOptions {
    fn default() -> Self {
        Self {
            route_table: DEFAULT_ROUTE_TABLE,
            rule_index: DEFAULT_RULE_INDEX,
            exclude_private: true,
            health_secs: DEFAULT_HEALTH_SECS,
            device: String::new(),
        }
    }
}

impl From<&Settings> for TunOptions {
    fn from(settings: &Settings) -> Self {
        Self {
            route_table: settings.tun_route_table,
            rule_index: settings.tun_rule_index,
            exclude_private: settings.tun_exclude_private,
            health_secs: settings.tun_health_secs,
            device: settings.tun_device.clone(),
        }
    }
}

impl TunOptions {
    /// mihomo's default TUN name is Meta. If it changes, users can pin the
    /// real name with `tun_device` instead of relying on a heuristic.
    pub fn expected_device(&self) -> &str {
        if self.device.trim().is_empty() {
            "Meta"
        } else {
            self.device.trim()
        }
    }
}

/// Build the managed `tun` block. Values that could point at another
/// account's runtime are supplied by the caller, not by the subscription.
pub fn config(uid: u32, device: &str, machine: bool, options: &TunOptions) -> Value {
    let hijack = vec![
        Value::String("any:53".into()),
        Value::String("tcp://any:53".into()),
    ];

    let mut tun = Mapping::new();
    tun.insert("enable".into(), Value::Bool(true));
    tun.insert("stack".into(), Value::String("mixed".into()));
    // 1500 works with common client paths and avoids oversized handshake
    // packets. The rule-index value is configured separately below.
    tun.insert("mtu".into(), Value::Number(1500.into()));
    tun.insert("auto-route".into(), Value::Bool(true));
    tun.insert("auto-redirect".into(), Value::Bool(true));
    tun.insert("auto-detect-interface".into(), Value::Bool(true));
    tun.insert("strict-route".into(), Value::Bool(true));
    tun.insert(
        "iproute2-table-index".into(),
        Value::from(options.route_table),
    );
    tun.insert(
        "iproute2-rule-index".into(),
        Value::from(options.rule_index),
    );
    if options.exclude_private {
        tun.insert(
            "route-exclude-address".into(),
            Value::Sequence(
                EXCLUDED_NETWORKS
                    .iter()
                    .map(|network| Value::String((*network).into()))
                    .collect(),
            ),
        );
    }
    tun.insert("dns-hijack".into(), Value::Sequence(hijack));
    if !machine {
        tun.insert(
            "include-uid".into(),
            Value::Sequence(vec![Value::from(uid)]),
        );
    }
    if !device.is_empty() {
        tun.insert("device".into(), Value::String(device.into()));
    }
    Value::Mapping(tun)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteHealth {
    /// True when the route is present, or when iproute2 cannot be queried.
    pub ready: bool,
    /// False when the probe could not run; the UI should not alert in that case.
    pub known: bool,
    pub detail: String,
}

/// Locate iproute2 even when the service PATH does not include /sbin.
pub fn ip_program() -> &'static str {
    for path in ["/usr/bin/ip", "/usr/sbin/ip", "/sbin/ip"] {
        if Path::new(path).is_file() {
            return path;
        }
    }
    "ip"
}

pub fn probe(options: &TunOptions) -> RouteHealth {
    let ip = ip_program();
    let rules = command_text(ip, &["-4", "rule", "show"]);
    let table = options.route_table.to_string();
    let routes = if rules.is_some() {
        command_text(ip, &["-4", "route", "show", "table", &table])
    } else {
        None
    };
    health_from_output(rules.as_deref(), routes.as_deref(), options)
}

fn health_from_output(
    rules: Option<&str>,
    routes: Option<&str>,
    options: &TunOptions,
) -> RouteHealth {
    let Some(rules) = rules else {
        return RouteHealth {
            ready: true,
            known: false,
            detail: "iproute2 is not available; IPv4 route was not checked".to_string(),
        };
    };
    let Some(routes) = routes else {
        return RouteHealth {
            ready: true,
            known: false,
            detail: format!("could not read IPv4 route table {}", options.route_table),
        };
    };

    let rule_prefix = format!("{}:", options.rule_index);
    let rule_present = rules
        .lines()
        .any(|line| line.trim_start().starts_with(&rule_prefix));
    let entries = parse_routes(routes);
    let (route_present, route_detail) = route_matches(&entries, options.expected_device());
    let detail = match (rule_present, route_present) {
        (true, true) => format!(
            "IPv4 table {} ({route_detail}), rule {}",
            options.route_table, options.rule_index
        ),
        (false, true) => format!("IPv4 {route_detail}, but rule {rule_prefix} is missing"),
        (true, false) => format!("IPv4 rule exists, but {route_detail}"),
        (false, false) => format!("IPv4 rule {rule_prefix} is missing and {route_detail}"),
    };
    RouteHealth {
        ready: rule_present && route_present,
        known: true,
        detail,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouteKind {
    Default,
    Low,
    High,
}

#[derive(Clone, Copy, Debug)]
struct RouteEntry<'a> {
    kind: RouteKind,
    dev: Option<&'a str>,
}

fn parse_routes(text: &str) -> Vec<RouteEntry<'_>> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let kind = if line.starts_with("default") {
                RouteKind::Default
            } else if line.starts_with("0.0.0.0/1") {
                RouteKind::Low
            } else if line.starts_with("128.0.0.0/1") {
                RouteKind::High
            } else {
                return None;
            };
            Some(RouteEntry {
                kind,
                dev: dev_of(line),
            })
        })
        .collect()
}

/// Check that a default (or split default) route actually points at the
/// configured TUN device. Multiple default entries are normal; any matching
/// one is enough.
fn route_matches(entries: &[RouteEntry<'_>], expected: &str) -> (bool, String) {
    let defaults: Vec<&RouteEntry<'_>> = entries
        .iter()
        .filter(|entry| entry.kind == RouteKind::Default)
        .collect();
    if defaults.iter().any(|entry| entry.dev == Some(expected)) {
        return (true, format!("default route on {expected}"));
    }
    if !defaults.is_empty() {
        let devices: Vec<&str> = defaults.iter().filter_map(|entry| entry.dev).collect();
        return (
            false,
            if devices.is_empty() {
                format!("default routes have no dev field (expected {expected})")
            } else {
                format!(
                    "default routes point at {} (expected {expected})",
                    devices.join(", ")
                )
            },
        );
    }

    let low: Vec<&RouteEntry<'_>> = entries
        .iter()
        .filter(|entry| entry.kind == RouteKind::Low)
        .collect();
    let high: Vec<&RouteEntry<'_>> = entries
        .iter()
        .filter(|entry| entry.kind == RouteKind::High)
        .collect();
    if low.is_empty() || high.is_empty() {
        return (false, "no default or split default route".to_string());
    }
    if low.iter().any(|entry| entry.dev == Some(expected))
        && high.iter().any(|entry| entry.dev == Some(expected))
    {
        return (
            true,
            format!("0.0.0.0/1 + 128.0.0.0/1 routes on {expected}"),
        );
    }
    let low_devices: Vec<&str> = low.iter().filter_map(|entry| entry.dev).collect();
    let high_devices: Vec<&str> = high.iter().filter_map(|entry| entry.dev).collect();
    let low_text = if low_devices.is_empty() {
        "no dev".to_string()
    } else {
        low_devices.join(", ")
    };
    let high_text = if high_devices.is_empty() {
        "no dev".to_string()
    } else {
        high_devices.join(", ")
    };
    (
        false,
        format!(
            "split routes: 0.0.0.0/1 on [{low_text}], 128.0.0.0/1 on [{high_text}], expected {expected}"
        ),
    )
}

fn dev_of(line: &str) -> Option<&str> {
    let mut tokens = line.split_whitespace();
    while let Some(token) = tokens.next() {
        if token == "dev" {
            return tokens.next();
        }
    }
    None
}

fn command_text(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut text = String::new();
                child.stdout.take()?.read_to_string(&mut text).ok()?;
                return Some(text);
            }
            Ok(None) if started.elapsed() < COMMAND_TIMEOUT => {
                thread::sleep(Duration::from_millis(25));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tun_value(uid: u32, machine: bool, options: &TunOptions) -> Value {
        config(uid, "", machine, options)
    }

    #[test]
    fn config_uses_the_configured_table_and_rule() {
        let options = TunOptions {
            route_table: 4040,
            rule_index: 9100,
            ..TunOptions::default()
        };
        let tun = tun_value(1000, false, &options);
        assert_eq!(
            tun.get("iproute2-table-index").and_then(Value::as_u64),
            Some(4040)
        );
        assert_eq!(
            tun.get("iproute2-rule-index").and_then(Value::as_u64),
            Some(9100)
        );
        assert_eq!(
            tun.get("include-uid")
                .and_then(Value::as_sequence)
                .map(|items| items.len()),
            Some(1)
        );
    }

    #[test]
    fn private_exclusions_are_optional() {
        let options = TunOptions {
            exclude_private: false,
            ..TunOptions::default()
        };
        let tun = tun_value(1000, false, &options);
        assert!(tun.get("route-exclude-address").is_none());

        let tun = tun_value(1000, false, &TunOptions::default());
        let excluded = tun
            .get("route-exclude-address")
            .and_then(Value::as_sequence)
            .expect("default exclusions");
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
    }

    #[test]
    fn machine_capture_omits_include_uid() {
        let tun = tun_value(1000, true, &TunOptions::default());
        assert!(tun.get("include-uid").is_none());
    }

    #[test]
    fn route_probe_requires_the_exact_rule_index() {
        let options = TunOptions::default();
        let rules = "9000: from all uidrange 65534-65534 goto 9010\n9001: from all to 198.18.0.0/30 lookup 2022\n";
        let routes = "default via 198.18.0.2 dev Meta\n";
        let health = health_from_output(Some(rules), Some(routes), &options);
        assert!(health.ready);
        assert!(health.known);

        let wrong = "90000: from all uidrange 65534-65534 goto 9010\n9001: from all to 198.18.0.0/30 lookup 20220\n";
        let health = health_from_output(Some(wrong), Some(routes), &options);
        assert!(!health.ready);
        assert!(health.detail.contains("rule 9000:"));
    }

    #[test]
    fn route_probe_accepts_split_default_routes() {
        let options = TunOptions::default();
        let rules = "9000: from all uidrange 65534-65534 goto 9010\n";
        let routes = "0.0.0.0/1 via 198.18.0.2 dev Meta\n128.0.0.0/1 via 198.18.0.2 dev Meta\n";
        let health = health_from_output(Some(rules), Some(routes), &options);
        assert!(health.ready);
        assert!(health.detail.contains("0.0.0.0/1"));
    }

    #[test]
    fn route_probe_rejects_a_default_route_on_another_device() {
        let options = TunOptions::default();
        let rules = "9000: from all uidrange 65534-65534 goto 9010\n";
        let routes = "default via 10.0.0.1 dev wg0\n";
        let health = health_from_output(Some(rules), Some(routes), &options);
        assert!(!health.ready);
        assert!(health.detail.contains("point at wg0 (expected Meta)"));
    }

    #[test]
    fn route_probe_accepts_any_matching_default_route() {
        let options = TunOptions::default();
        let rules = "9000: from all uidrange 65534-65534 goto 9010\n";
        let routes = "default via 10.0.0.1 dev wg0\ndefault via 198.18.0.2 dev Meta\n";
        let health = health_from_output(Some(rules), Some(routes), &options);
        assert!(health.ready);
        assert!(health.detail.contains("default route on Meta"));
    }

    #[test]
    fn route_probe_accepts_multiple_split_routes() {
        let options = TunOptions::default();
        let rules = "9000: from all uidrange 65534-65534 goto 9010\n";
        let routes = "0.0.0.0/1 via 198.18.0.2 dev Meta\n0.0.0.0/1 via 198.18.0.2 dev Meta metric 10\n128.0.0.0/1 via 198.18.0.2 dev Meta\n128.0.0.0/1 via 198.18.0.2 dev Meta metric 10\n";
        let health = health_from_output(Some(rules), Some(routes), &options);
        assert!(health.ready);
    }

    #[test]
    fn split_route_detail_lists_low_and_high_devices_separately() {
        let options = TunOptions::default();
        let rules = "9000: from all uidrange 65534-65534 goto 9010\n";
        let routes = "0.0.0.0/1 via 198.18.0.2 dev Meta\n0.0.0.0/1 via 10.0.0.1 dev wg0\n128.0.0.0/1 via 10.0.0.1 dev wg0\n";
        let health = health_from_output(Some(rules), Some(routes), &options);
        assert!(!health.ready);
        assert!(health.detail.contains("0.0.0.0/1 on [Meta, wg0]"));
        assert!(health.detail.contains("128.0.0.0/1 on [wg0]"));
    }

    #[test]
    fn route_probe_uses_the_configured_device_name() {
        let options = TunOptions {
            device: "clash0".to_string(),
            ..TunOptions::default()
        };
        let rules = "9000: from all uidrange 65534-65534 goto 9010\n";
        let good = "default via 198.18.0.2 dev clash0\n";
        assert!(health_from_output(Some(rules), Some(good), &options).ready);

        let wrong = "default via 198.18.0.2 dev Meta\n";
        let health = health_from_output(Some(rules), Some(wrong), &options);
        assert!(!health.ready);
        assert!(health.detail.contains("expected clash0"));
    }

    #[test]
    fn route_probe_reports_unknown_when_iproute2_is_missing() {
        let health = health_from_output(None, None, &TunOptions::default());
        assert!(health.ready);
        assert!(!health.known);
        assert!(health.detail.contains("not checked"));
    }
}
