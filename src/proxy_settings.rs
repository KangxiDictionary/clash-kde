// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::paths::Paths;

pub fn system_proxy_should_apply(enabled: bool, running: bool, tun_enabled: bool) -> bool {
    enabled && running && !tun_enabled
}

pub fn apply_if_needed(paths: &Paths, port: u16) -> std::io::Result<()> {
    fs::create_dir_all(&paths.config_dir)?;
    if !paths.proxy_flag().exists() {
        let original = fs::read_to_string(&paths.kioslaverc).unwrap_or_default();
        let backup = match extract_group_body(&original, "Proxy Settings") {
            Some(body) => format!("present\n{body}"),
            None => "absent\n".to_string(),
        };
        crate::state::write_private(&paths.proxy_backup(), backup.as_bytes())?;
        backup_gnome(paths)?;
        fs::write(paths.proxy_flag(), b"1")?;
    }
    write_manual_proxy(&paths.kioslaverc, port)?;
    apply_gnome(port);
    notify_kio();
    let _ = fs::remove_file(paths.proxy_left());
    Ok(())
}

pub fn kde_proxy_label(paths: &Paths, port: u16) -> &'static str {
    let Ok(text) = fs::read_to_string(&paths.kioslaverc) else {
        return "off";
    };
    let (proxy_type, http_proxy) = proxy_fields(&text);
    classify_manual_proxy(&proxy_type, &http_proxy, "", port)
}

pub fn gnome_proxy_label(port: u16) -> &'static str {
    if cfg!(test) {
        return "off";
    }
    let mode = gsettings_get("org.gnome.system.proxy", "mode").unwrap_or_default();
    let host = gsettings_get("org.gnome.system.proxy.http", "host").unwrap_or_default();
    let current = gsettings_get("org.gnome.system.proxy.http", "port").unwrap_or_default();
    let mode = mode.trim().trim_matches('\'');
    if mode.is_empty() || mode == "none" {
        return "off";
    }
    if mode != "manual" {
        return "other";
    }
    classify_manual_proxy("1", &host, current.trim().trim_matches('\''), port)
}

fn classify_manual_proxy(
    proxy_type: &str,
    host: &str,
    explicit_port: &str,
    port: u16,
) -> &'static str {
    let ours = if explicit_port.is_empty() {
        proxy_type == "1" && endpoint_is_local(host) && endpoint_port(host) == Some(port)
    } else {
        proxy_type == "1" && endpoint_is_local(host) && explicit_port == port.to_string()
    };
    if ours {
        "ours"
    } else if proxy_type.is_empty() || proxy_type == "0" {
        "off"
    } else {
        "other"
    }
}

pub fn still_points_at_us(paths: &Paths, port: u16) -> bool {
    let Ok(text) = fs::read_to_string(&paths.kioslaverc) else {
        return false;
    };
    let (proxy_type, http_proxy) = proxy_fields(&text);
    proxy_type == "1" && endpoint_is_local(&http_proxy) && endpoint_port(&http_proxy) == Some(port)
}

pub fn release_ownership(paths: &Paths, port: u16) -> std::io::Result<()> {
    let _ = fs::remove_file(paths.proxy_flag());
    let _ = fs::remove_file(paths.proxy_backup());
    // 用户改过 KDE 代理之后不再代为恢复。仍指向本程序的 GNOME 代理要关掉，
    // 否则 Firefox 一类程序还会继续走刚才的端口。
    finish_gnome(paths, port, false);
    let _ = fs::remove_file(paths.proxy_left());
    Ok(())
}

pub fn restore_if_applied(paths: &Paths, port: u16) -> std::io::Result<()> {
    let had_flag = paths.proxy_flag().exists();
    if had_flag {
        let backup =
            fs::read_to_string(paths.proxy_backup()).unwrap_or_else(|_| "absent\n".to_string());
        let original = fs::read_to_string(&paths.kioslaverc).unwrap_or_default();
        let restored = if let Some(body) = backup.strip_prefix("present\n") {
            replace_group(&original, "Proxy Settings", Some(body))
        } else {
            replace_group(&original, "Proxy Settings", None)
        };
        if let Some(parent) = paths.kioslaverc.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&paths.kioslaverc, restored)?;
        let _ = fs::remove_file(paths.proxy_flag());
        let _ = fs::remove_file(paths.proxy_backup());
    }
    // 标记文件丢了之后，KDE 或 GNOME 仍可能指着本程序。关闭必须把这两处都关掉。
    let kde_ours = still_points_at_us(paths, port);
    if kde_ours {
        force_kde_proxy_off(paths, port)?;
    }
    let gnome_ours = finish_gnome(paths, port, true);
    if had_flag || kde_ours || gnome_ours {
        notify_kio();
        remember_left_proxy(paths)?;
    }
    Ok(())
}

pub fn left_proxy_was_edited(paths: &Paths) -> bool {
    let Ok(expected) = fs::read_to_string(paths.proxy_left()) else {
        return false;
    };
    current_proxy_fingerprint(&paths.kioslaverc) != expected
}

fn remember_left_proxy(paths: &Paths) -> std::io::Result<()> {
    let fingerprint = current_proxy_fingerprint(&paths.kioslaverc);
    crate::state::write_private(paths.proxy_left().as_path(), fingerprint.as_bytes())
}

fn current_proxy_fingerprint(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    let body = extract_group_body(&text, "Proxy Settings").unwrap_or_default();
    const KEYS: &[&str] = &[
        "Authmode",
        "ftpProxy",
        "httpProxy",
        "httpsProxy",
        "NoProxyFor",
        "ProxyType",
        "ReversedException",
        "socksProxy",
    ];
    let mut values = std::collections::BTreeMap::new();
    for line in body.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let key = key.trim();
        if KEYS.contains(&key) {
            values.insert(key.to_string(), value.trim().to_string());
        }
    }
    values
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_manual_proxy(path: &Path, port: u16) -> std::io::Result<()> {
    let original = fs::read_to_string(path).unwrap_or_default();
    // KDE stores the port after a space. A colon is not the value kwriteconfig6 writes.
    let body = format!(
        "\
ProxyType=1
httpProxy=http://127.0.0.1 {port}
httpsProxy=http://127.0.0.1 {port}
ftpProxy=http://127.0.0.1 {port}
socksProxy=socks://127.0.0.1 {port}
NoProxyFor=localhost,127.0.0.1,::1
Authmode=0
ReversedException=false
"
    );
    let updated = replace_group(&original, "Proxy Settings", Some(&body));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, updated)
}

pub fn replace_group(text: &str, group: &str, new_body: Option<&str>) -> String {
    let mut out = String::new();
    let mut lines = text.split_inclusive('\n').peekable();
    let mut found = false;
    while let Some(line) = lines.next() {
        if is_group_header(line, group) {
            found = true;
            if let Some(body) = new_body {
                push_header(&mut out, group);
                out.push_str(body);
                if !body.is_empty() && !body.ends_with('\n') {
                    out.push('\n');
                }
            }
            while let Some(next) = lines.peek() {
                if is_any_group_header(next) {
                    break;
                }
                lines.next();
            }
            continue;
        }
        out.push_str(line);
    }
    if !found && let Some(body) = new_body {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        push_header(&mut out, group);
        out.push_str(body);
        if !body.is_empty() && !body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

fn extract_group_body(text: &str, group: &str) -> Option<String> {
    let mut lines = text.split_inclusive('\n').peekable();
    let mut found = false;
    let mut body = String::new();
    while let Some(line) = lines.next() {
        if is_group_header(line, group) {
            found = true;
            while let Some(next) = lines.peek() {
                if is_any_group_header(next) {
                    break;
                }
                body.push_str(lines.next().expect("peeked"));
            }
            break;
        }
    }
    found.then_some(body)
}

fn push_header(out: &mut String, group: &str) {
    out.push('[');
    out.push_str(group);
    out.push(']');
    out.push('\n');
}

fn is_group_header(line: &str, group: &str) -> bool {
    group_name(line).is_some_and(|name| name == group)
}

fn is_any_group_header(line: &str) -> bool {
    group_name(line).is_some()
}

fn group_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let inside = trimmed.strip_prefix('[')?.split(']').next()?;
    let name = inside.trim();
    if name.is_empty() { None } else { Some(name) }
}

const GNOME_KEYS: &[(&str, &str)] = &[
    ("org.gnome.system.proxy", "mode"),
    ("org.gnome.system.proxy", "ignore-hosts"),
    ("org.gnome.system.proxy.http", "enabled"),
    ("org.gnome.system.proxy.http", "host"),
    ("org.gnome.system.proxy.http", "port"),
    ("org.gnome.system.proxy.https", "host"),
    ("org.gnome.system.proxy.https", "port"),
    ("org.gnome.system.proxy.socks", "host"),
    ("org.gnome.system.proxy.socks", "port"),
    ("org.gnome.system.proxy.ftp", "host"),
    ("org.gnome.system.proxy.ftp", "port"),
];

fn backup_gnome(paths: &Paths) -> std::io::Result<()> {
    if cfg!(test) {
        return Ok(());
    }
    let mut lines = Vec::new();
    for (schema, key) in GNOME_KEYS {
        if let Some(value) = gsettings_get(schema, key) {
            lines.push(format!("{schema}\t{key}\t{value}"));
        }
    }
    if lines.is_empty() {
        return Ok(());
    }
    crate::state::write_private(&paths.gnome_proxy_backup(), lines.join("\n").as_bytes())
}

fn apply_gnome(port: u16) {
    if cfg!(test) || gsettings_get("org.gnome.system.proxy", "mode").is_none() {
        return;
    }
    let address = "'127.0.0.1'".to_string();
    let port = port.to_string();
    gsettings_set("org.gnome.system.proxy", "mode", "'manual'");
    gsettings_set("org.gnome.system.proxy.http", "enabled", "true");
    gsettings_set("org.gnome.system.proxy.http", "host", &address);
    gsettings_set("org.gnome.system.proxy.http", "port", &port);
    gsettings_set("org.gnome.system.proxy.https", "host", &address);
    gsettings_set("org.gnome.system.proxy.https", "port", &port);
    gsettings_set("org.gnome.system.proxy.socks", "host", &address);
    gsettings_set("org.gnome.system.proxy.socks", "port", &port);
    gsettings_set("org.gnome.system.proxy.ftp", "host", &address);
    gsettings_set("org.gnome.system.proxy.ftp", "port", &port);
    if gsettings_get("org.gnome.system.proxy", "ignore-hosts")
        .is_none_or(|value| value == "@as []" || value == "[]")
    {
        gsettings_set(
            "org.gnome.system.proxy",
            "ignore-hosts",
            "['localhost', '127.0.0.1', '::1']",
        );
    }
}

fn gnome_backup_is_ours(text: &str, port: u16) -> bool {
    let mut mode = "";
    let mut host = "";
    let mut saved_port = "";
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(schema), Some(key), Some(value)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if schema == "org.gnome.system.proxy" && key == "mode" {
            mode = value;
        } else if schema == "org.gnome.system.proxy.http" && key == "host" {
            host = value;
        } else if schema == "org.gnome.system.proxy.http" && key == "port" {
            saved_port = value;
        }
    }
    mode.contains("manual") && endpoint_is_local(host) && saved_port.trim() == port.to_string()
}

/// 关掉仍由本程序接管的 GNOME 代理。返回值表示这次有没有改动。
///
/// Clash Party 在 KDE 上关闭系统代理时，除了把 ProxyType 写成 0，还会把
/// `org.gnome.system.proxy` 的 mode 和 dconf 的 `/system/proxy/mode` 写成 none。
/// 只恢复备份不够：备份可能就是上一次没关掉的手动代理，Firefox 会继续使用它。
fn finish_gnome(paths: &Paths, port: u16, restore_foreign: bool) -> bool {
    if cfg!(test) {
        let _ = fs::remove_file(paths.gnome_proxy_backup());
        return false;
    }
    let backup = fs::read_to_string(paths.gnome_proxy_backup()).ok();
    let live_ours = gnome_still_ours(port);
    let backup_ours = backup
        .as_deref()
        .is_some_and(|text| gnome_backup_is_ours(text, port));
    let changed = if live_ours && (backup.is_none() || backup_ours || !restore_foreign) {
        disable_gnome_mode();
        true
    } else if restore_foreign && let Some(text) = backup.as_ref().filter(|_| !backup_ours) {
        for line in text.lines() {
            let mut parts = line.splitn(3, '\t');
            let Some(schema) = parts.next() else { continue };
            let Some(key) = parts.next() else { continue };
            let Some(value) = parts.next() else { continue };
            gsettings_set(schema, key, value);
        }
        true
    } else {
        false
    };
    let _ = fs::remove_file(paths.gnome_proxy_backup());
    changed
}

fn disable_gnome_mode() {
    gsettings_set("org.gnome.system.proxy", "mode", "'none'");
    gsettings_set("org.gnome.system.proxy.http", "enabled", "false");
    dconf_write("/system/proxy/mode", "'none'");
}

fn force_kde_proxy_off(paths: &Paths, port: u16) -> std::io::Result<()> {
    if !cfg!(test) {
        let path = paths.kioslaverc.display().to_string();
        let wrote = Command::new("kwriteconfig6")
            .args([
                "--file",
                &path,
                "--group",
                "Proxy Settings",
                "--key",
                "ProxyType",
                "0",
            ])
            .status()
            .is_ok_and(|status| status.success());
        if wrote && !still_points_at_us(paths, port) {
            return Ok(());
        }
    }
    let original = fs::read_to_string(&paths.kioslaverc).unwrap_or_default();
    let Some(body) = extract_group_body(&original, "Proxy Settings") else {
        return Ok(());
    };
    let mut next = String::new();
    let mut replaced = false;
    for line in body.lines() {
        if line
            .split_once('=')
            .is_some_and(|(key, _)| key.trim() == "ProxyType")
        {
            next.push_str("ProxyType=0\n");
            replaced = true;
        } else {
            next.push_str(line);
            next.push('\n');
        }
    }
    if !replaced {
        next.push_str("ProxyType=0\n");
    }
    if let Some(parent) = paths.kioslaverc.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &paths.kioslaverc,
        replace_group(&original, "Proxy Settings", Some(&next)),
    )
}

fn proxy_fields(text: &str) -> (String, String) {
    let Some(body) = extract_group_body(text, "Proxy Settings") else {
        return (String::new(), String::new());
    };
    let mut proxy_type = String::new();
    let mut http_proxy = String::new();
    for line in body.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "ProxyType" => proxy_type = value.trim().to_string(),
            "httpProxy" => http_proxy = value.trim().to_string(),
            _ => {}
        }
    }
    (proxy_type, http_proxy)
}

fn endpoint_is_local(value: &str) -> bool {
    value.contains("127.0.0.1") || value.contains("localhost") || value.contains("[::1]")
}

fn endpoint_port(value: &str) -> Option<u16> {
    value
        .trim()
        .trim_matches('"')
        .rsplit([' ', ':'])
        .next()?
        .trim()
        .parse()
        .ok()
}

fn gnome_still_ours(port: u16) -> bool {
    if cfg!(test) {
        return false;
    }
    let mode = gsettings_get("org.gnome.system.proxy", "mode").unwrap_or_default();
    let host = gsettings_get("org.gnome.system.proxy.http", "host").unwrap_or_default();
    let current = gsettings_get("org.gnome.system.proxy.http", "port").unwrap_or_default();
    mode == "'manual'" && endpoint_is_local(&host) && current == port.to_string()
}

fn gsettings_get(schema: &str, key: &str) -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn gsettings_set(schema: &str, key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, value])
        .status()
        .is_ok_and(|status| status.success())
}

fn dconf_write(path: &str, value: &str) {
    let _ = Command::new("dconf").args(["write", path, value]).status();
}

fn notify_kio() {
    let _ = Command::new("dbus-send")
        .args([
            "--session",
            "--type=signal",
            "/KIO/Scheduler",
            "org.kde.KIO.Scheduler.reparseSlaveConfiguration",
            "string:",
        ])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_labels_distinguish_off_ours_and_other() {
        assert_eq!(classify_manual_proxy("0", "", "", 7890), "off");
        assert_eq!(classify_manual_proxy("", "", "", 7890), "off");
        assert_eq!(
            classify_manual_proxy("1", "http://127.0.0.1 7890", "", 7890),
            "ours"
        );
        assert_eq!(
            classify_manual_proxy("1", "127.0.0.1", "7890", 7890),
            "ours"
        );
        assert_eq!(
            classify_manual_proxy("1", "http://10.0.0.1 8080", "", 7890),
            "other"
        );
        assert_eq!(classify_manual_proxy("4", "", "", 7890), "other");
    }

    #[test]
    fn spaced_kde_keys_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            data_dir: dir.path().join("data"),
            kioslaverc: dir.path().join("kioslaverc"),
        };
        fs::write(
            &paths.kioslaverc,
            "[Proxy Settings]\nProxyType = 1\nhttpProxy = http://127.0.0.1:7890\n",
        )
        .unwrap();
        assert_eq!(kde_proxy_label(&paths, 7890), "ours");
        assert!(still_points_at_us(&paths, 7890));
        fs::write(
            &paths.kioslaverc,
            "[Proxy Settings]\nProxyType = 0\nhttpProxy = http://127.0.0.1:7890\n",
        )
        .unwrap();
        assert_eq!(kde_proxy_label(&paths, 7890), "off");
        assert!(!still_points_at_us(&paths, 7890));
    }

    #[test]
    fn a_manual_kde_change_no_longer_points_at_us() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            data_dir: dir.path().join("data"),
            kioslaverc: dir.path().join("kioslaverc"),
        };
        fs::write(
            &paths.kioslaverc,
            "[Proxy Settings]\nProxyType=1\nhttpProxy=http://127.0.0.1 7890\n",
        )
        .unwrap();
        assert!(still_points_at_us(&paths, 7890));
        fs::write(&paths.kioslaverc, "[Proxy Settings]\nProxyType=0\n").unwrap();
        assert!(!still_points_at_us(&paths, 7890));
        fs::write(
            &paths.kioslaverc,
            "[Proxy Settings]\nProxyType=1\nhttpProxy=http://127.0.0.1 8080\n",
        )
        .unwrap();
        assert!(!still_points_at_us(&paths, 7890));
        remember_left_proxy(&paths).unwrap();
        assert!(!left_proxy_was_edited(&paths));
        fs::write(&paths.kioslaverc, "[Proxy Settings]\nProxyType=0\n").unwrap();
        assert!(left_proxy_was_edited(&paths));
    }

    #[test]
    fn tun_withdraws_the_system_proxy() {
        assert!(system_proxy_should_apply(true, true, false));
        assert!(!system_proxy_should_apply(true, true, true));
        assert!(!system_proxy_should_apply(true, false, false));
        assert!(!system_proxy_should_apply(false, true, false));
    }

    #[test]
    fn apply_and_restore_round_trip_keeps_other_groups() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            data_dir: dir.path().join("data"),
            kioslaverc: dir.path().join("kioslaverc"),
        };
        let original = "\
[Other]
Foo=1

[Proxy Settings]
ProxyType=0
httpProxy=http://old:1

[Later]
Bar=2
";
        fs::write(&paths.kioslaverc, original).unwrap();
        apply_if_needed(&paths, 7890).unwrap();
        let applied = fs::read_to_string(&paths.kioslaverc).unwrap();
        assert!(applied.contains("ProxyType=1"));
        assert!(applied.contains("http://127.0.0.1 7890"));
        assert!(applied.contains("[Other]\nFoo=1\n"));
        assert!(applied.contains("[Later]\nBar=2\n"));
        assert!(!applied.contains("http://old:1"));

        restore_if_applied(&paths, 7890).unwrap();
        assert_eq!(fs::read_to_string(&paths.kioslaverc).unwrap(), original);
        assert!(!paths.proxy_flag().exists());
    }

    #[test]
    fn restore_removes_group_that_did_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            data_dir: dir.path().join("data"),
            kioslaverc: dir.path().join("kioslaverc"),
        };
        let original = "[Other]\nFoo=1\n";
        fs::write(&paths.kioslaverc, original).unwrap();
        apply_if_needed(&paths, 7890).unwrap();
        assert!(
            fs::read_to_string(&paths.kioslaverc)
                .unwrap()
                .contains("[Proxy Settings]")
        );
        restore_if_applied(&paths, 7890).unwrap();
        assert_eq!(fs::read_to_string(&paths.kioslaverc).unwrap(), original);
    }

    #[test]
    fn second_apply_does_not_overwrite_backup() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            data_dir: dir.path().join("data"),
            kioslaverc: dir.path().join("kioslaverc"),
        };
        fs::write(&paths.kioslaverc, "[Proxy Settings]\nProxyType=4\n").unwrap();
        apply_if_needed(&paths, 7890).unwrap();
        apply_if_needed(&paths, 7891).unwrap();
        let applied = fs::read_to_string(&paths.kioslaverc).unwrap();
        assert!(applied.contains("127.0.0.1 7891"));
        restore_if_applied(&paths, 7891).unwrap();
        assert_eq!(
            fs::read_to_string(&paths.kioslaverc).unwrap(),
            "[Proxy Settings]\nProxyType=4\n"
        );
    }

    #[test]
    fn disable_clears_our_kde_proxy_when_the_flag_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            data_dir: dir.path().join("data"),
            kioslaverc: dir.path().join("kioslaverc"),
        };
        fs::write(
            &paths.kioslaverc,
            "[Proxy Settings]\nProxyType=1\nhttpProxy=http://127.0.0.1 7890\n",
        )
        .unwrap();
        restore_if_applied(&paths, 7890).unwrap();
        let text = fs::read_to_string(&paths.kioslaverc).unwrap();
        assert!(text.contains("ProxyType=0"));
        assert!(!still_points_at_us(&paths, 7890));
    }

    #[test]
    fn a_backup_of_our_manual_proxy_counts_as_ours() {
        let text = "\
org.gnome.system.proxy\tmode\t'manual'
org.gnome.system.proxy.http\thost\t'127.0.0.1'
org.gnome.system.proxy.http\tport\t7890
";
        assert!(gnome_backup_is_ours(text, 7890));
        assert!(!gnome_backup_is_ours(text, 8080));
        assert!(!gnome_backup_is_ours(
            "org.gnome.system.proxy\tmode\t'none'\n",
            7890
        ));
    }
}
