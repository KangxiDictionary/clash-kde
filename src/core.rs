use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::paths::Paths;

pub struct CoreProcess {
    child: Child,
}

impl CoreProcess {
    pub fn spawn(paths: &Paths, binary: &Path) -> Result<Self, String> {
        let log = OpenOptions::new()
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
            .map_err(|error| format!("无法启动 mihomo: {error}"))?;
        Ok(Self { child })
    }

    pub async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait().await
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    pub async fn terminate(&mut self) {
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
        }
        tokio::select! {
            _ = self.child.wait() => {}
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                let _ = self.child.start_kill();
                let _ = self.child.wait().await;
            }
        }
    }
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
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().rev().take(8).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}

pub fn startup_hint(path: &Path) -> String {
    let tail = log_tail(path).to_lowercase();
    if tail.contains("download")
        || tail.contains("can't find mmdb")
        || tail.contains("can't find geosite")
    {
        "正在下载地理数据".to_string()
    } else {
        "正在启动内核".to_string()
    }
}

const GEODATA_FILES: [&str; 4] = ["country.mmdb", "geoip.metadb", "geosite.dat", "geoip.dat"];

pub fn seed_geodata(runtime: &Path) {
    seed_geodata_from(runtime, &geodata_roots());
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
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        file.write_all(&bytes)?;
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
}
