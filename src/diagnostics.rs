// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

//! Read-only report used when the core refuses to start.
//!
//! `clash-kde --diagnose` deliberately does not talk to the session service:
//! the interesting failures often happen while that service is stopped or
//! being restarted. Everything here is derived from files, the mihomo binary
//! and journalctl, so it also works from a TTY after a failed login start.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core;
use crate::paths::Paths;
use crate::profile;
use crate::rules;
use crate::state::{self, Settings};
use crate::tun::{self, TunOptions};

pub fn run() -> Result<(), String> {
    let paths = Paths::from_env();
    let settings = Settings::load(&paths).map_err(|error| format!("无法读取配置: {error}"))?;
    print!("{}", report(&paths, &settings));
    Ok(())
}

fn report(paths: &Paths, settings: &Settings) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "clash-kde {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "generated: {}", now_text());
    let _ = writeln!(out);
    let _ = writeln!(out, "== paths ==");
    let _ = writeln!(out, "config dir:   {}", paths.config_dir.display());
    let _ = writeln!(out, "data dir:     {}", paths.data_dir.display());
    let _ = writeln!(out, "settings:     {}", existence(&paths.settings_file()));
    let _ = writeln!(out, "override:     {}", effective_override(paths, settings));
    let _ = writeln!(out, "profiles dir: {}", paths.profiles_dir().display());
    let _ = writeln!(out, "runtime dir:  {}", paths.runtime_dir().display());
    let _ = writeln!(out, "runtime cfg:  {}", existence(&paths.runtime_config()));
    let _ = writeln!(out, "mihomo log:   {}", existence(&paths.core_log()));
    let _ = writeln!(out);

    let _ = writeln!(out, "== preferences ==");
    let _ = writeln!(out, "core_desired:        {}", settings.core_desired);
    let _ = writeln!(
        out,
        "system_proxy:        {}",
        settings.system_proxy_enabled
    );
    let _ = writeln!(out, "tun_enabled:         {}", settings.tun_enabled);
    let _ = writeln!(out, "tun_machine:         {}", settings.tun_machine);
    let _ = writeln!(out, "mode:                {}", settings.mode);
    let _ = writeln!(out, "mixed_port:          {}", settings.mixed_port);
    let _ = writeln!(out, "controller_port:     {}", settings.controller_port);
    let _ = writeln!(out, "mihomo_path:         {}", settings.mihomo_path);
    let _ = writeln!(out, "active_profile:      {}", settings.active_profile);
    let _ = writeln!(out);

    let _ = writeln!(out, "== profiles ==");
    let profiles = profile::load_profiles(paths);
    let _ = writeln!(out, "count: {}", profiles.len());
    for item in &profiles {
        let marker = if item.id == settings.active_profile {
            "*"
        } else {
            " "
        };
        let _ = writeln!(
            out,
            "{} {}  {}  url={}",
            marker,
            item.id,
            item.name,
            if item.url.is_empty() {
                "imported"
            } else {
                "subscription"
            }
        );
    }
    if !settings.active_profile.is_empty() {
        match profile::read_profile_body(paths, &settings.active_profile) {
            Ok(body) => match profile::decode_subscription(&body) {
                Ok(value) => {
                    let rules = value
                        .get("rules")
                        .and_then(serde_yaml::Value::as_sequence)
                        .map_or(0, Vec::len);
                    let proxies = value
                        .get("proxies")
                        .and_then(serde_yaml::Value::as_sequence)
                        .map_or(0, Vec::len);
                    let _ = writeln!(
                        out,
                        "active body: {} bytes, YAML ok, {} proxies, {} rules",
                        body.len(),
                        proxies,
                        rules
                    );
                }
                Err(error) => {
                    let _ = writeln!(out, "active body: YAML ERROR: {error}");
                }
            },
            Err(error) => {
                let _ = writeln!(out, "active body: MISSING: {error}");
            }
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "== generated files ==");
    match std::fs::read_to_string(paths.runtime_config()) {
        Ok(text) => match serde_yaml::from_str::<serde_yaml::Value>(&text) {
            Ok(value) => {
                let _ = writeln!(
                    out,
                    "runtime config: {} bytes, YAML ok, mixed-port={:?}",
                    text.len(),
                    value.get("mixed-port").and_then(serde_yaml::Value::as_i64)
                );
            }
            Err(error) => {
                let _ = writeln!(out, "runtime config: YAML ERROR: {error}");
            }
        },
        Err(error) => {
            let _ = writeln!(out, "runtime config: cannot read: {error}");
        }
    }
    let override_path = state::effective_override(paths, &settings.override_path);
    let _ = writeln!(
        out,
        "override: {}",
        match profile::load_override(&override_path) {
            Ok(_) => "ok".to_string(),
            Err(error) => error,
        }
    );
    let _ = writeln!(
        out,
        "own rules: {}",
        match rules::load_rules(&rules::rules_file(paths)) {
            Ok(items) => format!("{} parsed", items.len()),
            Err(error) => error,
        }
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "== mihomo ==");
    match core::resolve_binary(&settings.mihomo_path) {
        Ok(binary) => {
            let _ = writeln!(out, "resolved: {}", binary.display());
            match Command::new(&binary).arg("-v").output() {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let text = if text.is_empty() {
                        String::from_utf8_lossy(&output.stderr).trim().to_string()
                    } else {
                        text
                    };
                    let _ = writeln!(out, "version: {text}");
                }
                Err(error) => {
                    let _ = writeln!(out, "version: cannot run: {error}");
                }
            }
        }
        Err(error) => {
            let _ = writeln!(out, "resolved: NOT FOUND: {error}");
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "== geodata (copy sources) ==");
    let geodata = core::geodata_sources();
    let _ = writeln!(
        out,
        "mode: {}",
        match core::geodata_mode(&geodata) {
            "mmdb" => "country.mmdb/geoip.metadb",
            "dat" => "geoip.dat/geosite.dat (geodata-mode: true)",
            _ => "download (no local MMDB or v2ray dat files)",
        }
    );
    for (name, found) in &geodata {
        match found {
            Some(path) => {
                let size = std::fs::metadata(path).map_or(0, |meta| meta.len());
                let _ = writeln!(out, "{name}: {} ({size} bytes)", path.display());
            }
            None => {
                let _ = writeln!(out, "{name}: not found");
            }
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "== tun ==");
    let options = TunOptions::from(settings);
    let _ = writeln!(
        out,
        "enabled: {}   machine: {}   table: {}   rule: {}   exclude_private: {}   health: {}s",
        settings.tun_enabled,
        settings.tun_machine,
        options.route_table,
        options.rule_index,
        options.exclude_private,
        options.health_secs
    );
    let _ = writeln!(
        out,
        "links: {}",
        command_text(
            tun::ip_program(),
            &["-brief", "link", "show", "type", "tun"]
        )
    );
    let _ = writeln!(
        out,
        "expected device: {}{}",
        options.expected_device(),
        if options.device.trim().is_empty() {
            " (default; set tun_device if mihomo uses another name)"
        } else {
            " (from tun_device)"
        }
    );
    let health = settings.tun_enabled.then(|| tun::probe(&options));
    let _ = writeln!(
        out,
        "route ready: {}",
        match &health {
            None => "n/a (TUN off)".to_string(),
            Some(health) if !health.known => format!("unknown ({})", health.detail),
            Some(health) if health.ready => format!("yes ({})", health.detail),
            Some(health) => format!("no ({})", health.detail),
        }
    );
    let table = options.route_table.to_string();
    let rule_prefix = format!("{}:", options.rule_index);
    let rules = command_text(tun::ip_program(), &["-4", "rule", "show"])
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            let lookup_table = line
                .split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .any(|pair| pair[0] == "lookup" && pair[1] == table.as_str());
            line.starts_with(&rule_prefix) || lookup_table
        })
        .collect::<Vec<_>>()
        .join(" | ");
    let _ = writeln!(
        out,
        "rules: {}",
        if rules.is_empty() { "(none)" } else { &rules }
    );
    let _ = writeln!(out, "probe: live IPv4 check at {}", now_text());
    let _ = writeln!(
        out,
        "table {table}: {}",
        command_text(tun::ip_program(), &["-4", "route", "show", "table", &table])
    );
    let _ = writeln!(
        out,
        "firewalld: {}   ufw: {}   tailscale0: {}",
        command_state("systemctl", &["is-active", "firewalld"]),
        command_state("systemctl", &["is-active", "ufw"]),
        if Path::new("/sys/class/net/tailscale0").exists() {
            "present"
        } else {
            "absent"
        }
    );
    if settings.tun_enabled && !health.as_ref().is_some_and(|health| health.ready) {
        let _ = writeln!(
            out,
            "  (TUN is on but its routes are missing; check UFW/firewalld and Tailscale)"
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "== ports ==");
    let lan_open = crate::lan::permitted(&crate::lan::policy_dir(), crate::lan::current_uid())
        && settings.allow_lan;
    let mixed_host = if lan_open { "0.0.0.0" } else { "127.0.0.1" };
    let mixed_free = core::port_available(mixed_host, settings.mixed_port);
    let controller_free = core::port_available("127.0.0.1", settings.controller_port);
    let _ = writeln!(
        out,
        "mixed {} ({}): {}",
        settings.mixed_port,
        mixed_host,
        if mixed_free { "free" } else { "IN USE" }
    );
    let _ = writeln!(
        out,
        "controller {}: {}",
        settings.controller_port,
        if controller_free { "free" } else { "IN USE" }
    );
    if !mixed_free || !controller_free {
        let _ = writeln!(
            out,
            "  (the running core normally owns these ports; stop it before starting another copy)"
        );
    }
    let system_mihomo = command_state("systemctl", &["is-active", "mihomo.service"]);
    let _ = writeln!(out, "system mihomo.service: {system_mihomo}");
    if system_mihomo == "active" {
        let _ = writeln!(
            out,
            "  (it normally uses 7890/9090; run: sudo systemctl disable --now mihomo, or choose other ports)"
        );
    }
    let user_service = command_state("systemctl", &["--user", "is-active", "clash-kde.service"]);
    let _ = writeln!(out, "user clash-kde.service: {user_service}");
    let _ = writeln!(out);

    let _ = writeln!(out, "== proxy ==");
    let _ = writeln!(
        out,
        "KDE:   {}",
        crate::proxy_settings::kde_proxy_label(paths, settings.mixed_port)
    );
    let _ = writeln!(
        out,
        "GNOME: {}",
        crate::proxy_settings::gnome_proxy_label(settings.mixed_port)
    );
    let _ = writeln!(out, "ownership flag: {}", paths.proxy_flag().exists());
    let _ = writeln!(out, "KDE backup:     {}", paths.proxy_backup().exists());
    let _ = writeln!(
        out,
        "GNOME backup:   {}",
        paths.gnome_proxy_backup().exists()
    );
    let _ = writeln!(out, "left marker:    {}", paths.proxy_left().exists());
    let _ = writeln!(out);

    let _ = writeln!(out, "== recent mihomo log ==");
    let log = core::log_tail_lines(&paths.core_log(), 80);
    if log.is_empty() {
        let _ = writeln!(out, "(no log yet)");
    } else {
        let _ = writeln!(out, "{log}");
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "== service journal (last 120 lines) ==");
    let journal = journal_tail();
    if journal.is_empty() {
        let _ = writeln!(out, "(journalctl returned nothing)");
    } else {
        let _ = writeln!(out, "{journal}");
    }
    out
}

fn journal_tail() -> String {
    let output = Command::new("journalctl")
        .args([
            "--user",
            "-u",
            "clash-kde.service",
            "-n",
            "120",
            "--no-pager",
            "-o",
            "short-iso",
        ])
        .output();
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                return stdout;
            }
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        }
        Err(error) => format!("cannot run journalctl: {error}"),
    }
}

fn existence(path: &std::path::Path) -> String {
    match std::fs::metadata(path) {
        Ok(meta) => format!("{} ({} bytes)", path.display(), meta.len()),
        Err(error) => format!("{} ({error})", path.display()),
    }
}

fn command_text(program: &str, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                stdout
            } else if output.status.success() {
                "(empty)".to_string()
            } else if let Some(code) = output.status.code() {
                format!("exit {code}")
            } else {
                format!("({})", output.status)
            }
        }
        Err(error) => format!("not available ({error})"),
    }
}

fn command_state(program: &str, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                stdout
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    format!("exit {}", output.status)
                } else {
                    stderr
                }
            }
        }
        Err(error) => format!("not available ({error})"),
    }
}

fn effective_override(paths: &Paths, settings: &Settings) -> String {
    state::effective_override(paths, &settings.override_path)
        .display()
        .to_string()
}

fn now_text() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let timestamp = seconds as libc::time_t;
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&timestamp, &mut local) }.is_null() {
        return format!("unix {seconds}");
    }
    let mut buffer = [0u8; 64];
    let format = b"%Y-%m-%d %H:%M:%S %z\0";
    let size = unsafe {
        libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            format.as_ptr().cast(),
            &local,
        )
    };
    if size == 0 {
        format!("unix {seconds}")
    } else {
        String::from_utf8_lossy(&buffer[..size]).into_owned()
    }
}
