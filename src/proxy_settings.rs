use std::fs;
use std::path::Path;
use std::process::Command;

use crate::paths::Paths;

pub fn apply_if_needed(paths: &Paths, port: u16) -> std::io::Result<()> {
    fs::create_dir_all(&paths.config_dir)?;
    if !paths.proxy_flag().exists() {
        let original = fs::read_to_string(&paths.kioslaverc).unwrap_or_default();
        let backup = match extract_group_body(&original, "Proxy Settings") {
            Some(body) => format!("present\n{body}"),
            None => "absent\n".to_string(),
        };
        crate::state::write_private(&paths.proxy_backup(), backup.as_bytes())?;
        fs::write(&paths.proxy_flag(), b"1")?;
    }
    write_manual_proxy(&paths.kioslaverc, port)?;
    notify_kio();
    Ok(())
}

pub fn restore_if_applied(paths: &Paths) -> std::io::Result<()> {
    if !paths.proxy_flag().exists() {
        return Ok(());
    }
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
    notify_kio();
    Ok(())
}

fn write_manual_proxy(path: &Path, port: u16) -> std::io::Result<()> {
    let original = fs::read_to_string(path).unwrap_or_default();
    let body = format!(
        "\
ProxyType=1
httpProxy=http://127.0.0.1:{port}
httpsProxy=http://127.0.0.1:{port}
ftpProxy=http://127.0.0.1:{port}
socksProxy=socks://127.0.0.1:{port}
NoProxyFor=localhost,127.0.0.1,::1
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
    if !found {
        if let Some(body) = new_body {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            push_header(&mut out, group);
            out.push_str(body);
            if !body.is_empty() && !body.ends_with('\n') {
                out.push('\n');
            }
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
        assert!(applied.contains("http://127.0.0.1:7890"));
        assert!(applied.contains("[Other]\nFoo=1\n"));
        assert!(applied.contains("[Later]\nBar=2\n"));
        assert!(!applied.contains("http://old:1"));

        restore_if_applied(&paths).unwrap();
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
        restore_if_applied(&paths).unwrap();
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
        assert!(applied.contains("127.0.0.1:7891"));
        restore_if_applied(&paths).unwrap();
        assert_eq!(
            fs::read_to_string(&paths.kioslaverc).unwrap(),
            "[Proxy Settings]\nProxyType=4\n"
        );
    }
}
