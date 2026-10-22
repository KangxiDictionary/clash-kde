use std::collections::BTreeSet;
use std::fs;
use std::net::IpAddr;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

use crate::paths::Paths;
use crate::state::write_private;

pub const KINDS: [&str; 9] = [
    "DOMAIN",
    "DOMAIN-SUFFIX",
    "DOMAIN-KEYWORD",
    "IP-CIDR",
    "GEOSITE",
    "GEOIP",
    "PROCESS-NAME",
    "DST-PORT",
    "MATCH",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnRule {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub payload: String,
    pub target: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ShownRule {
    #[serde(rename = "type")]
    pub kind: String,
    pub payload: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProviderRow {
    pub name: String,
    pub updated_at: String,
    pub count: i64,
}

#[derive(Serialize, Deserialize)]
struct RulesFile {
    #[serde(default)]
    rules: Vec<OwnRule>,
}

pub fn rules_file(paths: &Paths) -> std::path::PathBuf {
    paths.config_dir.join("rules.yaml")
}

pub fn load_rules(path: &Path) -> Result<Vec<OwnRule>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|error| format!("无法读取自己的规则: {error}"))?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    match serde_yaml::from_str::<RulesFile>(&text) {
        Ok(file) => Ok(file.rules),
        Err(error) => Err(format!("自己的规则无法解析: {error}")),
    }
}

pub fn save_rules(path: &Path, rules: &[OwnRule]) -> Result<(), String> {
    let text = serde_yaml::to_string(&RulesFile {
        rules: rules.to_vec(),
    })
    .map_err(|error| format!("无法写入自己的规则: {error}"))?;
    write_private(path, text.as_bytes()).map_err(|error| format!("无法写入自己的规则: {error}"))
}

pub fn rules_from_json(text: &str) -> Result<Vec<OwnRule>, String> {
    serde_json::from_str(text).map_err(|_| "规则读不出来".to_string())
}

pub fn insert_own_rules(config: &mut Value, rules: &[OwnRule]) {
    let enabled: Vec<&OwnRule> = rules.iter().filter(|rule| rule.enabled).collect();
    if enabled.is_empty() {
        return;
    }
    let Some(map) = config.as_mapping_mut() else {
        return;
    };
    let mut profile = map
        .get(Value::String("rules".into()))
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let trailing = peel_trailing_match(&mut profile);
    let mut own_match = None;
    let mut merged = Vec::new();
    for rule in enabled {
        if rule.kind == "MATCH" {
            if own_match.is_none() {
                own_match = Some(Value::String(clash_line(rule)));
            }
            continue;
        }
        merged.push(Value::String(clash_line(rule)));
    }
    merged.extend(profile);
    if let Some(own_match) = own_match {
        merged.push(own_match);
    } else if let Some(trailing) = trailing {
        merged.push(trailing);
    }
    map.insert(Value::String("rules".into()), Value::Sequence(merged));
}

fn peel_trailing_match(rules: &mut Vec<Value>) -> Option<Value> {
    let last = rules.last()?;
    if rule_kind(last) == "MATCH" {
        rules.pop()
    } else {
        None
    }
}

fn rule_kind(value: &Value) -> String {
    match value {
        Value::String(text) => text
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_uppercase(),
        Value::Mapping(map) => text_field(map, "type").to_ascii_uppercase(),
        _ => String::new(),
    }
}

fn text_field(map: &Mapping, key: &str) -> String {
    map.get(Value::String(key.into()))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn clash_line(rule: &OwnRule) -> String {
    if rule.kind == "MATCH" {
        format!("MATCH,{}", rule.target)
    } else {
        format!("{},{},{}", rule.kind, rule.payload, rule.target)
    }
}

pub fn profile_rules(profile: &Value) -> Vec<ShownRule> {
    let Some(list) = profile.get("rules").and_then(Value::as_sequence) else {
        return Vec::new();
    };
    list.iter().filter_map(shown_from_value).collect()
}

fn shown_from_value(value: &Value) -> Option<ShownRule> {
    match value {
        Value::String(text) => shown_from_text(text),
        Value::Mapping(map) => {
            let kind = text_field(map, "type").to_ascii_uppercase();
            if kind.is_empty() {
                return None;
            }
            let payload = text_field(map, "payload");
            let target = {
                let named = text_field(map, "target");
                if named.is_empty() {
                    text_field(map, "proxy")
                } else {
                    named
                }
            };
            if kind == "MATCH" {
                Some(ShownRule {
                    kind,
                    payload: String::new(),
                    target,
                })
            } else {
                Some(ShownRule {
                    kind,
                    payload,
                    target,
                })
            }
        }
        _ => None,
    }
}

fn shown_from_text(text: &str) -> Option<ShownRule> {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let kind = parts.first()?.to_ascii_uppercase();
    if kind.is_empty() {
        return None;
    }
    if kind == "MATCH" {
        return Some(ShownRule {
            kind,
            payload: String::new(),
            target: parts.get(1).unwrap_or(&"").to_string(),
        });
    }
    if parts.len() < 3 {
        return None;
    }
    Some(ShownRule {
        kind,
        payload: parts[1].to_string(),
        target: parts[2].to_string(),
    })
}

pub fn provider_names(profile: &Value, key: &str) -> Vec<String> {
    let Some(map) = profile.get(key).and_then(Value::as_mapping) else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(name, _)| name.as_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn merge_provider_lists(
    file_names: &[String],
    live: &[(String, String, i64)],
) -> Vec<ProviderRow> {
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for name in file_names {
        seen.insert(name.clone());
        let found = live.iter().find(|(live_name, _, _)| live_name == name);
        rows.push(ProviderRow {
            name: name.clone(),
            updated_at: found.map(|(_, time, _)| time.clone()).unwrap_or_default(),
            count: found.map(|(_, _, count)| *count).unwrap_or(-1),
        });
    }
    let mut extras: Vec<&(String, String, i64)> = live
        .iter()
        .filter(|(name, _, _)| !seen.contains(name))
        .collect();
    extras.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, time, count) in extras {
        rows.push(ProviderRow {
            name: name.clone(),
            updated_at: time.clone(),
            count: *count,
        });
    }
    rows
}

pub fn policy_names(profile: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for builtin in ["DIRECT", "REJECT", "GLOBAL"] {
        names.insert(builtin.to_string());
    }
    if let Some(list) = profile.get("proxies").and_then(Value::as_sequence) {
        for item in list {
            if let Some(name) = item.get("name").and_then(Value::as_str) {
                let name = name.trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    if let Some(list) = profile.get("proxy-groups").and_then(Value::as_sequence) {
        for item in list {
            if let Some(name) = item.get("name").and_then(Value::as_str) {
                let name = name.trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
            if let Some(members) = item.get("proxies").and_then(Value::as_sequence) {
                for member in members {
                    if let Some(name) = member.as_str() {
                        let name = name.trim();
                        if !name.is_empty() {
                            names.insert(name.to_string());
                        }
                    }
                }
            }
        }
    }
    names
}

pub fn validate_rules(rules: &[OwnRule], names: &BTreeSet<String>) -> Result<(), String> {
    for (index, rule) in rules.iter().enumerate() {
        check_rule(rule, names).map_err(|error| format!("第 {} 行：{error}", index + 1))?;
    }
    Ok(())
}

fn check_rule(rule: &OwnRule, names: &BTreeSet<String>) -> Result<(), String> {
    check_shape(rule)?;
    if rule.target.is_empty() {
        return Err("目标不能为空".to_string());
    }
    if !names.contains(&rule.target) {
        return Err("没有这个目标".to_string());
    }
    Ok(())
}

fn check_shape(rule: &OwnRule) -> Result<(), String> {
    if !KINDS.contains(&rule.kind.as_str()) {
        return Err("不认识的类型".to_string());
    }
    if rule.payload.contains([',', '\n', '\r']) || rule.target.contains([',', '\n', '\r']) {
        return Err("匹配内容不能包含逗号".to_string());
    }
    if rule.kind == "MATCH" {
        if rule.target.is_empty() {
            return Err("目标不能为空".to_string());
        }
        return Ok(());
    }
    if rule.payload.is_empty() {
        return Err("匹配内容不能为空".to_string());
    }
    if rule.target.is_empty() {
        return Err("目标不能为空".to_string());
    }
    match rule.kind.as_str() {
        "IP-CIDR" if !valid_cidr(&rule.payload) => Err("IP 段不合法".to_string()),
        "DST-PORT" if !valid_port(&rule.payload) => Err("端口不合法".to_string()),
        "GEOIP" if !valid_geoip(&rule.payload) => Err("国家代码不合法".to_string()),
        _ => Ok(()),
    }
}

fn valid_cidr(text: &str) -> bool {
    let Some((address, prefix)) = text.split_once('/') else {
        return false;
    };
    let Ok(address) = address.parse::<IpAddr>() else {
        return false;
    };
    let Ok(prefix) = prefix.parse::<u8>() else {
        return false;
    };
    match address {
        IpAddr::V4(_) => prefix <= 32,
        IpAddr::V6(_) => prefix <= 128,
    }
}

fn valid_port(text: &str) -> bool {
    let mut parts = text.split('-');
    let Some(start) = parts.next().and_then(|part| part.parse::<u16>().ok()) else {
        return false;
    };
    let end = match parts.next() {
        Some(part) => match part.parse::<u16>() {
            Ok(value) => value,
            Err(_) => return false,
        },
        None => start,
    };
    parts.next().is_none() && start >= 1 && end >= start
}

fn valid_geoip(text: &str) -> bool {
    let length = text.chars().count();
    (2..=16).contains(&length) && text.chars().all(|ch| ch.is_ascii_alphabetic())
}

pub fn rules_to_csv(rules: &[OwnRule]) -> String {
    let mut text = String::from("type,payload,target,enabled\n");
    for rule in rules {
        text.push_str(&csv_field(&rule.kind));
        text.push(',');
        text.push_str(&csv_field(&rule.payload));
        text.push(',');
        text.push_str(&csv_field(&rule.target));
        text.push(',');
        text.push_str(if rule.enabled { "true" } else { "false" });
        text.push('\n');
    }
    text
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

pub fn rules_from_csv(text: &str) -> Result<Vec<OwnRule>, String> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("").trim();
    let delimiter = if first == "type,payload,target,enabled" {
        ','
    } else if first == "type;payload;target;enabled" {
        ';'
    } else {
        return Err("表头不对".to_string());
    };
    let rows = parse_table(text, delimiter)?;
    if rows.is_empty() || rows[0] != ["type", "payload", "target", "enabled"] {
        return Err("表头不对".to_string());
    }
    let mut rules = Vec::new();
    for (index, row) in rows.iter().skip(1).enumerate() {
        if row.iter().all(|field| field.is_empty()) {
            continue;
        }
        let row = if row.len() == 5 && row[4].is_empty() {
            &row[..4]
        } else {
            row.as_slice()
        };
        if row.len() != 4 {
            return Err(format!("第 {} 行无法拆分", index + 1));
        }
        let kind = row[0].trim().to_ascii_uppercase();
        if !KINDS.contains(&kind.as_str()) {
            return Err(format!("第 {} 行：不认识的类型", index + 1));
        }
        let enabled = match row[3].trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => true,
            "false" | "no" | "0" => false,
            _ => return Err(format!("第 {} 行：启用一列只能是 true 或 false", index + 1)),
        };
        let rule = OwnRule {
            payload: if kind == "MATCH" {
                String::new()
            } else {
                row[1].trim().to_string()
            },
            kind,
            target: row[2].trim().to_string(),
            enabled,
        };
        rules.push(rule);
    }
    Ok(rules)
}

fn parse_table(text: &str, delimiter: char) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        if ch == '"' {
            if !field.is_empty() || quoted {
                return Err(format!("第 {} 行无法拆分", rows.len()));
            }
            in_quotes = true;
            quoted = true;
            continue;
        }
        if ch == delimiter {
            row.push(std::mem::take(&mut field));
            quoted = false;
            continue;
        }
        if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            quoted = false;
            rows.push(std::mem::take(&mut row));
            continue;
        }
        field.push(ch);
    }
    if in_quotes {
        return Err(format!("第 {} 行无法拆分", rows.len().max(1)));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

pub fn rules_json(rules: &[OwnRule]) -> String {
    serde_json::to_string(rules).expect("rules json")
}

pub fn provider_json(rows: &[ProviderRow]) -> String {
    serde_json::to_string(rows).expect("provider json")
}

pub fn rule_view_json(
    own: &[OwnRule],
    profile: &[ShownRule],
    providers: &[ProviderRow],
    targets: &BTreeSet<String>,
) -> String {
    let targets: Vec<&str> = targets.iter().map(String::as_str).collect();
    serde_json::json!({
        "own": own,
        "profile": profile,
        "providers": providers,
        "targets": targets,
    })
    .to_string()
}

pub fn csv_from_json(text: &str) -> Result<String, String> {
    Ok(rules_to_csv(&rules_from_json(text)?))
}

pub fn json_from_csv(text: &str) -> Result<String, String> {
    Ok(rules_json(&rules_from_csv(text)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_rules_go_before_the_profile_and_leave_match_last() {
        let mut config = serde_yaml::from_str(
            "rules:\n- DOMAIN,a.com,PROXY\n- MATCH,DIRECT\nproxy-groups:\n- {name: PROXY, type: select, proxies: [DIRECT]}\n",
        )
        .unwrap();
        let rules = vec![
            OwnRule {
                kind: "DOMAIN-SUFFIX".into(),
                payload: "b.com".into(),
                target: "REJECT".into(),
                enabled: true,
            },
            OwnRule {
                kind: "DOMAIN".into(),
                payload: "skip.example".into(),
                target: "DIRECT".into(),
                enabled: false,
            },
        ];
        insert_own_rules(&mut config, &rules);
        let lines: Vec<String> = config["rules"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            lines,
            vec![
                "DOMAIN-SUFFIX,b.com,REJECT".to_string(),
                "DOMAIN,a.com,PROXY".to_string(),
                "MATCH,DIRECT".to_string(),
            ]
        );
    }

    #[test]
    fn an_own_match_replaces_the_profile_match() {
        let mut config =
            serde_yaml::from_str("rules:\n- DOMAIN,a.com,PROXY\n- MATCH,DIRECT\n").unwrap();
        insert_own_rules(
            &mut config,
            &[OwnRule {
                kind: "MATCH".into(),
                payload: String::new(),
                target: "REJECT".into(),
                enabled: true,
            }],
        );
        let lines: Vec<&str> = config["rules"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(lines, vec!["DOMAIN,a.com,PROXY", "MATCH,REJECT"]);
    }

    #[test]
    fn csv_round_trip_keeps_commas_and_quotes() {
        let rules = vec![OwnRule {
            kind: "DOMAIN-KEYWORD".into(),
            payload: "say \"hi\", there".into(),
            target: "DIRECT".into(),
            enabled: true,
        }];
        let again = rules_from_csv(&rules_to_csv(&rules)).unwrap();
        assert_eq!(again, rules);
    }

    #[test]
    fn a_bad_csv_is_rejected() {
        assert_eq!(rules_from_csv("a,b\n").unwrap_err(), "表头不对");
        assert!(rules_from_csv("type,payload,target,enabled\nNOPE,a,DIRECT,true\n").is_err());
        assert!(rules_from_csv("type,payload,target,enabled\n\"DOMAIN,DIRECT,true\n").is_err());
    }

    #[test]
    fn provider_names_come_from_the_profile_without_the_url() {
        let profile = serde_yaml::from_str(
            "proxy-providers:\n  alpha:\n    type: http\n    url: https://example.test/sub?token=secret\n  beta:\n    type: file\n    path: beta.yaml\n",
        )
        .unwrap();
        let names = provider_names(&profile, "proxy-providers");
        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
        assert!(names.iter().all(|name| !name.contains("token")));
    }

    #[test]
    fn apply_rejects_a_bad_cidr_without_writing() {
        let names = policy_names(&Value::Mapping(Mapping::new()));
        let error = validate_rules(
            &[OwnRule {
                kind: "IP-CIDR".into(),
                payload: "1.2.3.4".into(),
                target: "DIRECT".into(),
                enabled: true,
            }],
            &names,
        )
        .unwrap_err();
        assert!(error.contains("第 1 行"));
        assert!(error.contains("IP 段"));
    }

    #[test]
    fn an_empty_field_names_the_row() {
        let names = policy_names(&Value::Mapping(Mapping::new()));
        let error = validate_rules(
            &[OwnRule {
                kind: "DOMAIN".into(),
                payload: String::new(),
                target: "DIRECT".into(),
                enabled: true,
            }],
            &names,
        )
        .unwrap_err();
        assert_eq!(error, "第 1 行：匹配内容不能为空");
        let error = validate_rules(
            &[OwnRule {
                kind: "MATCH".into(),
                payload: String::new(),
                target: String::new(),
                enabled: true,
            }],
            &names,
        )
        .unwrap_err();
        assert_eq!(error, "第 1 行：目标不能为空");
    }
}
