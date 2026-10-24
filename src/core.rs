// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::paths::Paths;

pub struct CoreProcess {
    child: Option<Child>,
    pid: u32,
}

impl CoreProcess {
    pub fn spawn(paths: &Paths, binary: &Path) -> Result<Self, String> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(paths.core_log())
            .map_err(|error| format!("无法打开内核日志: {error}"))?;
        let stderr = log
            .try_clone()
            .map_err(|error| format!("无法打开内核日志: {error}"))?;
        let child = Command::new(binary)
            .arg("-d")
            .arg(paths.runtime_dir())
            .arg("-f")
            .arg(paths.runtime_config())
            .current_dir(paths.runtime_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("无法启动 mihomo: {error} ({})", binary.display()))?;
        let pid = child.id().ok_or_else(|| "mihomo 没有进程号".to_string())?;
        Ok(Self {
            child: Some(child),
            pid,
        })
    }

    pub fn adopt(pid: u32) -> Self {
        Self { child: None, pid }
    }

    pub async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        if let Some(child) = self.child.as_mut() {
            return child.wait().await;
        }
        loop {
            if !process_alive(self.pid) {
                return Ok(std::process::ExitStatus::from_raw(1 << 8));
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        if let Some(child) = self.child.as_mut() {
            return child.try_wait();
        }
        if process_alive(self.pid) {
            Ok(None)
        } else {
            Ok(Some(std::process::ExitStatus::from_raw(1 << 8)))
        }
    }

    pub async fn terminate(&mut self) {
        let pid = self.pid as i32;
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        // mihomo 用 SIGTERM 拆掉自己的路由。只留这一小段，到点就强杀，避免关内核时干等数秒。
        tokio::select! {
            _ = self.wait() => {}
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                unsafe { libc::kill(pid, libc::SIGKILL); }
                let _ = tokio::time::timeout(Duration::from_millis(200), self.wait()).await;
            }
        }
    }
}

pub(crate) fn process_alive(pid: u32) -> bool {
    let rc = unsafe { libc::kill(pid as i32, 0) };
    if rc == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

pub fn resolve_binary(requested: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(requested);
    if requested.contains('/') {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        return Err(format!("找不到 mihomo：{requested}"));
    }
    let path_env = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string());
    for dir in path_env.split(':').filter(|dir| !dir.is_empty()) {
        let candidate = Path::new(dir).join(requested);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err("找不到 mihomo，请先安装 mihomo 包".to_string())
}

pub fn log_tail(path: &Path) -> String {
    log_tail_lines(path, 8)
}

pub fn log_tail_lines(path: &Path, count: usize) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().rev().take(count).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}

pub fn startup_downloading(path: &Path) -> bool {
    let tail = log_tail(path).to_lowercase();
    tail.contains("download")
        || tail.contains("can't find mmdb")
        || tail.contains("can't find geosite")
}

pub fn startup_hint(path: &Path) -> String {
    if startup_downloading(path) {
        "正在下载地理数据".to_string()
    } else {
        "正在启动内核".to_string()
    }
}

/// True when a listener can be created on the loopback port. Used before
/// spawning mihomo so a port conflict is reported instead of timing out.
pub fn port_available(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub fn tun_listen_error(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let start = text.rfind("Start initial configuration")?;
    let line = text[start..]
        .lines()
        .find(|line| line.contains("Start TUN listening error:"))?;
    let detail = line
        .split("Start TUN listening error:")
        .nth(1)
        .unwrap_or("")
        .trim()
        .trim_matches('"');
    if detail.is_empty() {
        Some("TUN 没有接上".to_string())
    } else {
        Some(format!("TUN 没有接上: {detail}"))
    }
}

const GEODATA_FILES: [&str; 4] = ["country.mmdb", "geoip.metadb", "geosite.dat", "geoip.dat"];

pub fn seed_geodata(runtime: &Path) {
    seed_geodata_from(runtime, &geodata_roots());
}

/// The geodata files that could be copied from another installation. The
/// diagnostics panel uses this to show why a first start may download.
pub fn geodata_sources() -> Vec<(&'static str, Option<PathBuf>)> {
    let roots = geodata_roots();
    GEODATA_FILES
        .iter()
        .map(|name| {
            let found = roots.iter().find_map(|root| {
                let candidate = root.join(name);
                let meta = std::fs::metadata(&candidate).ok()?;
                (meta.is_file() && meta.len() >= 1024).then_some(candidate)
            });
            (*name, found)
        })
        .collect()
}

/// How the current files can satisfy mihomo's geodata needs.
///
/// `mmdb` uses country.mmdb/geoip.metadb (mihomo's default), `dat` uses the
/// v2ray-style geoip.dat/geosite.dat with `geodata-mode: true`, and `download`
/// means mihomo will try to fetch them on first start.
pub fn geodata_mode(sources: &[(&'static str, Option<PathBuf>)]) -> &'static str {
    let has = |name: &str| {
        sources
            .iter()
            .any(|(item, path)| *item == name && path.is_some())
    };
    if has("country.mmdb") || has("geoip.metadb") {
        "mmdb"
    } else if has("geoip.dat") && has("geosite.dat") {
        "dat"
    } else {
        "download"
    }
}

fn geodata_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(config) = dirs::config_dir() {
        for relative in ["mihomo", "mihomo-party/work", "clash", "clash-meta"] {
            roots.push(config.join(relative));
        }
    }
    if let Some(data) = dirs::data_dir() {
        for relative in ["mihomo", "clash", "clash-meta"] {
            roots.push(data.join(relative));
        }
    }
    roots.push(PathBuf::from("/usr/share/mihomo"));
    roots.push(PathBuf::from("/usr/share/clash"));
    roots.push(PathBuf::from("/usr/share/v2ray"));
    roots
}

fn seed_geodata_from(runtime: &Path, roots: &[PathBuf]) {
    if let Err(error) = std::fs::create_dir_all(runtime) {
        tracing::warn!("无法准备地理数据目录: {error}");
        return;
    }
    for name in GEODATA_FILES {
        let dest = runtime.join(name);
        let dest_len = std::fs::metadata(&dest).map(|meta| meta.len()).unwrap_or(0);
        let mut best: Option<(u64, PathBuf)> = None;
        for root in roots {
            let candidate = root.join(name);
            if same_file(&candidate, &dest) {
                continue;
            }
            let Ok(meta) = std::fs::metadata(&candidate) else {
                continue;
            };
            if !meta.is_file() || meta.len() < 1024 {
                continue;
            }
            if best.as_ref().is_none_or(|(len, _)| meta.len() > *len) {
                best = Some((meta.len(), candidate));
            }
        }
        let Some((len, src)) = best else {
            continue;
        };
        if dest_len >= len {
            continue;
        }
        if let Err(error) = copy_file(&src, &dest) {
            tracing::warn!("无法复制地理数据 {name}: {error}");
        }
    }
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn copy_file(src: &Path, dest: &Path) -> std::io::Result<()> {
    let bytes = std::fs::read(src)?;
    let file_name = dest.file_name().unwrap_or_default().to_string_lossy();
    let tmp = dest.with_file_name(format!("{file_name}.partial"));
    {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        std::io::Write::write_all(&mut file, &bytes)?;
    }
    std::fs::rename(&tmp, dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_hint_reports_geodata_download() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("mihomo.log");
        std::fs::write(&log, "Can't find MMDB, start download\n").unwrap();
        assert_eq!(startup_hint(&log), "正在下载地理数据");
    }

    #[test]
    fn startup_hint_stays_generic_for_loader_messages() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("mihomo.log");
        std::fs::write(&log, "Geodata Loader mode: memconservative\n").unwrap();
        assert_eq!(startup_hint(&log), "正在启动内核");
    }

    #[test]
    fn tun_listen_error_uses_the_latest_start() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("mihomo.log");
        std::fs::write(
            &log,
            "Start initial configuration in progress\nmsg=\"Start TUN listening error: old\"\nStart initial configuration in progress\nmsg=\"Start TUN listening error: configure tun interface: operation not permitted\"\n",
        )
        .unwrap();
        let error = tun_listen_error(&log).expect("error");
        assert!(error.contains("operation not permitted"));
        assert!(!error.contains("old"));
    }

    #[test]
    fn seed_replaces_a_smaller_geodata_file() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("mihomo");
        let runtime = dir.path().join("runtime");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(source.join("country.mmdb"), vec![7; 2048]).unwrap();
        std::fs::write(runtime.join("country.mmdb"), vec![1; 100]).unwrap();
        seed_geodata_from(&runtime, &[source]);
        assert_eq!(
            std::fs::metadata(runtime.join("country.mmdb"))
                .unwrap()
                .len(),
            2048
        );
        assert!(!runtime.join("country.mmdb.partial").exists());
    }

    #[test]
    fn a_listening_port_is_reported_as_unavailable() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!port_available(port));
        drop(listener);
        assert!(port_available(port));
    }

    #[test]
    fn geodata_mode_prefers_mmdb_then_v2ray_dat() {
        let path = || Some(PathBuf::from("/tmp/geodata"));
        let mmdb = [
            ("country.mmdb", path()),
            ("geoip.metadb", None),
            ("geoip.dat", None),
            ("geosite.dat", None),
        ];
        assert_eq!(geodata_mode(&mmdb), "mmdb");
        let dat = [
            ("country.mmdb", None),
            ("geoip.metadb", None),
            ("geoip.dat", path()),
            ("geosite.dat", path()),
        ];
        assert_eq!(geodata_mode(&dat), "dat");
        let missing = [
            ("country.mmdb", None),
            ("geoip.metadb", None),
            ("geoip.dat", None),
            ("geosite.dat", None),
        ];
        assert_eq!(geodata_mode(&missing), "download");
        let partial = [
            ("country.mmdb", None),
            ("geoip.metadb", None),
            ("geoip.dat", None),
            ("geosite.dat", path()),
        ];
        assert_eq!(geodata_mode(&partial), "download");
    }
}
