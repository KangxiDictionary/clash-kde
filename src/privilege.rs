use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

use serde_yaml::Value;

use crate::state::write_private;

pub const SYSTEM_MIHOMO: &str = "/usr/bin/mihomo";
pub const STAMP_DIR: &str = "/var/lib/clash-kde/tun";

pub struct Account {
    pub name: String,
    pub gid: u32,
    pub home: PathBuf,
}

pub fn lookup_account(uid: u32) -> Result<Account, String> {
    let mut pwd = unsafe { std::mem::zeroed::<libc::passwd>() };
    let mut buf = vec![0u8; 16 * 1024];
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    let rc = unsafe {
        libc::getpwuid_r(
            uid,
            &mut pwd,
            buf.as_mut_ptr().cast(),
            buf.len(),
            &mut result,
        )
    };
    if rc != 0 || result.is_null() {
        return Err(format!("找不到账户 {uid}"));
    }
    let name = unsafe { std::ffi::CStr::from_ptr(pwd.pw_name) }
        .to_string_lossy()
        .into_owned();
    let home = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) }
        .to_string_lossy()
        .into_owned();
    if name.is_empty() || home.is_empty() {
        return Err(format!("找不到账户 {uid}"));
    }
    Ok(Account {
        gid: pwd.pw_gid,
        name,
        home: PathBuf::from(home),
    })
}

pub fn runtime_dir_for(account: &Account) -> PathBuf {
    account.home.join(".local/share/clash-kde/runtime")
}

pub fn judge_binary(canonical: &Path, uid: u32, mode: u32, allowed: &Path) -> Result<(), String> {
    if canonical != allowed || uid != 0 || mode & 0o022 != 0 {
        return Err("TUN 只能使用系统里的 /usr/bin/mihomo".to_string());
    }
    Ok(())
}

pub fn ensure_system_binary(path: &Path) -> Result<(), String> {
    let allowed =
        fs::canonicalize(SYSTEM_MIHOMO).map_err(|_| "系统里没有 /usr/bin/mihomo".to_string())?;
    let canonical = fs::canonicalize(path).map_err(|error| format!("找不到 mihomo：{error}"))?;
    let meta = fs::metadata(&canonical).map_err(|error| error.to_string())?;
    judge_binary(&canonical, meta.uid(), meta.mode(), &allowed)
}

pub fn judge_runtime(
    config: &Path,
    runtime: &Path,
    expected_runtime: &Path,
    owner: u32,
) -> Result<(), String> {
    if runtime != expected_runtime
        || config.parent() != Some(runtime)
        || config.file_name() != Some(std::ffi::OsStr::new("config.yaml"))
    {
        return Err("TUN 只能使用这个账户自己的运行配置".to_string());
    }
    let meta = fs::metadata(config).map_err(|error| error.to_string())?;
    if !meta.is_file() || meta.uid() != owner {
        return Err("TUN 只能使用这个账户自己的运行配置".to_string());
    }
    Ok(())
}

pub fn assess_tun_document(text: &str, uid: u32) -> Result<(), String> {
    let value: Value =
        serde_yaml::from_str(text).map_err(|error| format!("运行配置无法解析: {error}"))?;
    let tun = value.get("tun").ok_or("运行配置里没有 TUN")?;
    if tun.get("enable").and_then(Value::as_bool) != Some(true) {
        return Err("运行配置没有打开 TUN".to_string());
    }
    let ids = tun
        .get("include-uid")
        .and_then(Value::as_sequence)
        .ok_or("TUN 只能包含这个账户")?;
    if ids.len() != 1 || ids[0].as_u64() != Some(u64::from(uid)) {
        return Err("TUN 只能包含这个账户".to_string());
    }
    Ok(())
}

pub fn ensure_tun_launch(
    binary: &Path,
    runtime: &Path,
    config: &Path,
    uid: u32,
) -> Result<(), String> {
    ensure_system_binary(binary)?;
    let account = lookup_account(uid)?;
    let expected = fs::canonicalize(runtime_dir_for(&account))
        .map_err(|_| "TUN 只能使用这个账户自己的运行配置".to_string())?;
    let runtime =
        fs::canonicalize(runtime).map_err(|_| "TUN 只能使用这个账户自己的运行配置".to_string())?;
    let config_path =
        fs::canonicalize(config).map_err(|_| "TUN 只能使用这个账户自己的运行配置".to_string())?;
    judge_runtime(&config_path, &runtime, &expected, uid)?;
    let text = fs::read_to_string(&config_path).map_err(|error| error.to_string())?;
    assess_tun_document(&text, uid)
}

pub fn log_path_is_runtime(log: &Path, runtime: &Path) -> Result<(), String> {
    let runtime = fs::canonicalize(runtime).map_err(|error| error.to_string())?;
    let parent = log
        .parent()
        .ok_or_else(|| "TUN 只能使用这个账户自己的运行配置".to_string())?;
    let parent =
        fs::canonicalize(parent).map_err(|_| "TUN 只能使用这个账户自己的运行配置".to_string())?;
    if parent != runtime || log.file_name() != Some(std::ffi::OsStr::new("mihomo.log")) {
        return Err("TUN 只能使用这个账户自己的运行配置".to_string());
    }
    Ok(())
}

pub fn stamp_path(root: &Path, uid: u32) -> PathBuf {
    root.join(uid.to_string())
}

pub fn write_stamp(root: &Path, uid: u32) -> io::Result<()> {
    fs::create_dir_all(root)?;
    write_private(&stamp_path(root, uid), b"")
}

pub fn stamp_exists(root: &Path, uid: u32) -> bool {
    stamp_path(root, uid).is_file()
}

pub fn remove_stamp(root: &Path, uid: u32) -> io::Result<()> {
    let path = stamp_path(root, uid);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Drop to `uid` while leaving `CAP_NET_ADMIN` and `CAP_NET_RAW` for the next exec.
///
/// Called only from the single-threaded `--spawn-tun` helper, before `exec`.
pub fn enter_user(uid: u32, gid: u32, name: &str) -> io::Result<()> {
    let mut net = caps::CapsHashSet::new();
    net.insert(caps::Capability::CAP_NET_ADMIN);
    net.insert(caps::Capability::CAP_NET_RAW);

    caps::securebits::set_keepcaps(true).map_err(io::Error::other)?;
    // setuid clears ambient and effective, but not inheritable. The bits
    // have to be inheritable before the uid change so they can be raised
    // again afterwards.
    caps::set(None, caps::CapSet::Inheritable, &net).map_err(io::Error::other)?;

    let c_name = std::ffi::CString::new(name).map_err(|_| io::Error::other("账户名无效"))?;
    if unsafe { libc::initgroups(c_name.as_ptr(), gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::setgid(gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::setuid(uid) } != 0 {
        return Err(io::Error::last_os_error());
    }

    // setuid always clears the ambient set, even with keepcaps, and exec of
    // an ordinary program keeps only ambient capabilities. Raise them again
    // after the uid change, while the kept permitted set still contains them.
    caps::set(None, caps::CapSet::Effective, &net).map_err(io::Error::other)?;
    caps::set(None, caps::CapSet::Permitted, &net).map_err(io::Error::other)?;
    caps::raise(None, caps::CapSet::Ambient, caps::Capability::CAP_NET_ADMIN)
        .map_err(io::Error::other)?;
    caps::raise(None, caps::CapSet::Ambient, caps::Capability::CAP_NET_RAW)
        .map_err(io::Error::other)?;
    let ambient = caps::read(None, caps::CapSet::Ambient).map_err(io::Error::other)?;
    if !net.is_subset(&ambient) {
        return Err(io::Error::other("切换账户后没有留下网络能力"));
    }
    caps::securebits::set_keepcaps(false).map_err(io::Error::other)?;
    Ok(())
}

pub fn signed_in_uids() -> Vec<u32> {
    let mut uids = Vec::new();
    let Ok(entries) = fs::read_dir("/run/user") else {
        return uids;
    };
    for entry in entries.flatten() {
        let Ok(uid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if entry.path().join("bus").exists() {
            uids.push(uid);
        }
    }
    uids.sort_unstable();
    uids
}

pub fn exec_tun(uid: u32, binary: &Path, runtime: &Path, config: &Path, log: &Path) -> i32 {
    if let Err(error) = prepare_exec(uid, binary, runtime, config, log) {
        eprintln!("{error}");
        return 1;
    }
    1
}

fn prepare_exec(
    uid: u32,
    binary: &Path,
    runtime: &Path,
    config: &Path,
    log: &Path,
) -> Result<(), String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("TUN 需要系统服务".to_string());
    }
    if !stamp_exists(Path::new(STAMP_DIR), uid) {
        return Err("TUN 还没有得到授权".to_string());
    }
    ensure_tun_launch(binary, runtime, config, uid)?;
    log_path_is_runtime(log, runtime)?;
    let account = lookup_account(uid)?;
    enter_user(uid, account.gid, &account.name)
        .map_err(|error| format!("无法切换到这个账户: {error}"))?;
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|error| format!("无法打开内核日志: {error}"))?;
    let stderr = log_file
        .try_clone()
        .map_err(|error| format!("无法打开内核日志: {error}"))?;
    let error = std::process::Command::new(binary)
        .arg("-d")
        .arg(runtime)
        .arg("-f")
        .arg(config)
        .current_dir(runtime)
        .env_clear()
        .env("HOME", &account.home)
        .env("USER", &account.name)
        .env("LOGNAME", &account.name)
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(stderr))
        .exec();
    Err(format!("无法启动 mihomo: {error} ({})", binary.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::OpenOptionsExt as _;

    #[test]
    fn rejects_a_binary_that_is_not_the_system_one() {
        let meta = fs::metadata("/bin/sh").unwrap();
        let error = judge_binary(
            Path::new("/tmp/mihomo"),
            meta.uid(),
            meta.mode(),
            Path::new("/usr/bin/mihomo"),
        );
        assert!(error.is_err());
    }

    #[test]
    fn rejects_a_user_owned_binary_even_at_the_system_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mihomo");
        fs::write(&path, b"x").unwrap();
        let meta = fs::metadata(&path).unwrap();
        let error = judge_binary(
            Path::new("/usr/bin/mihomo"),
            meta.uid(),
            meta.mode(),
            Path::new("/usr/bin/mihomo"),
        );
        assert!(error.is_err());
    }

    #[test]
    fn accepts_a_root_owned_unwritable_system_binary() {
        let meta = fs::metadata("/bin/sh").unwrap();
        assert_eq!(meta.uid(), 0);
        judge_binary(
            Path::new("/usr/bin/mihomo"),
            0,
            meta.mode(),
            Path::new("/usr/bin/mihomo"),
        )
        .unwrap();
    }

    #[test]
    fn runtime_config_must_belong_to_that_account() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("runtime");
        fs::create_dir_all(&runtime).unwrap();
        let config = runtime.join("config.yaml");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o600)
            .open(&config)
            .unwrap();
        use std::io::Write;
        writeln!(file, "tun:\n  enable: true\n  include-uid: [1]\n").unwrap();
        drop(file);
        let uid = unsafe { libc::getuid() };
        judge_runtime(&config, &runtime, &runtime, uid).unwrap();
        assert!(judge_runtime(&config, &runtime, dir.path(), uid).is_err());
        assert!(judge_runtime(&runtime.join("other.yaml"), &runtime, &runtime, uid).is_err());
        assert!(assess_tun_document(&fs::read_to_string(&config).unwrap(), 1).is_ok());
        assert!(
            assess_tun_document(&fs::read_to_string(&config).unwrap(), uid).is_err() || uid == 1
        );
        assert!(assess_tun_document("tun:\n  enable: true\n  include-uid: [1, 0]\n", 1).is_err());
    }

    #[test]
    fn stamp_can_be_removed() {
        let dir = tempfile::tempdir().unwrap();
        write_stamp(dir.path(), 7).unwrap();
        assert!(stamp_exists(dir.path(), 7));
        let mode = fs::metadata(stamp_path(dir.path(), 7)).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        remove_stamp(dir.path(), 7).unwrap();
        assert!(!stamp_exists(dir.path(), 7));
    }
}
