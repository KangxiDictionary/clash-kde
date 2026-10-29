// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

use serde_yaml::Value;

use crate::error::{ServiceError, codes};
use crate::state::write_private;

pub const SYSTEM_MIHOMO: &str = "/usr/bin/mihomo";
pub const STAMP_DIR: &str = "/var/lib/clash-kde/tun";

pub struct Account {
    pub name: String,
    pub gid: u32,
    pub home: PathBuf,
}

pub fn lookup_account(uid: u32) -> Result<Account, ServiceError> {
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
        return Err(ServiceError::new(codes::ACCOUNT_NOT_FOUND).param("uid", uid.to_string()));
    }
    let name = unsafe { std::ffi::CStr::from_ptr(pwd.pw_name) }
        .to_string_lossy()
        .into_owned();
    let home = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) }
        .to_string_lossy()
        .into_owned();
    if name.is_empty() || home.is_empty() {
        return Err(ServiceError::new(codes::ACCOUNT_NOT_FOUND).param("uid", uid.to_string()));
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

pub fn judge_binary(canonical: &Path, uid: u32, mode: u32, allowed: &Path) -> Result<(), ServiceError> {
    if canonical != allowed || uid != 0 || mode & 0o022 != 0 {
        return Err(ServiceError::new(codes::TUN_BINARY_NOT_SYSTEM));
    }
    Ok(())
}

pub fn ensure_system_binary(path: &Path) -> Result<(), ServiceError> {
    let allowed = fs::canonicalize(SYSTEM_MIHOMO)
        .map_err(|_| ServiceError::new(codes::TUN_SYSTEM_BINARY_MISSING))?;
    let canonical = fs::canonicalize(path).map_err(|error| {
        ServiceError::new(codes::TUN_BINARY_MISSING).detail(error.to_string())
    })?;
    let meta = fs::metadata(&canonical).map_err(ServiceError::from_io)?;
    judge_binary(&canonical, meta.uid(), meta.mode(), &allowed)
}

pub fn judge_runtime(
    config: &Path,
    runtime: &Path,
    expected_runtime: &Path,
    owner: u32,
) -> Result<(), ServiceError> {
    if runtime != expected_runtime
        || config.parent() != Some(runtime)
        || config.file_name() != Some(std::ffi::OsStr::new("config.yaml"))
    {
        return Err(ServiceError::new(codes::TUN_CONFIG_NOT_OWNED));
    }
    let meta = fs::metadata(config).map_err(ServiceError::from_io)?;
    if !meta.is_file() || meta.uid() != owner {
        return Err(ServiceError::new(codes::TUN_CONFIG_NOT_OWNED));
    }
    Ok(())
}

pub fn assess_tun_document(text: &str, uid: u32, scope: &str) -> Result<(), ServiceError> {
    let value: Value = serde_yaml::from_str(text)
        .map_err(|error| ServiceError::new(codes::TUN_CONFIG_PARSE).detail(error.to_string()))?;
    let tun = value
        .get("tun")
        .ok_or_else(|| ServiceError::new(codes::TUN_CONFIG_MISSING))?;
    if tun.get("enable").and_then(Value::as_bool) != Some(true) {
        return Err(ServiceError::new(codes::TUN_CONFIG_DISABLED));
    }
    if scope == "machine" {
        if tun.get("include-uid").is_some() {
            return Err(ServiceError::new(codes::TUN_MACHINE_INCLUDE_UID));
        }
        let excludes_us = tun
            .get("exclude-uid")
            .and_then(Value::as_sequence)
            .is_some_and(|ids| ids.iter().any(|id| id.as_u64() == Some(u64::from(uid))));
        if excludes_us {
            return Err(ServiceError::new(codes::TUN_MACHINE_EXCLUDE_UID));
        }
        return Ok(());
    }
    let ids = tun
        .get("include-uid")
        .and_then(Value::as_sequence)
        .ok_or_else(|| ServiceError::new(codes::TUN_INCLUDE_UID_INVALID))?;
    if ids.len() != 1 || ids[0].as_u64() != Some(u64::from(uid)) {
        return Err(ServiceError::new(codes::TUN_INCLUDE_UID_INVALID).param("uid", uid.to_string()));
    }
    Ok(())
}

pub fn personal_tun_in(text: &str) -> bool {
    let Ok(settings) = toml::from_str::<crate::state::Settings>(text) else {
        return false;
    };
    settings.tun_enabled && !settings.tun_machine
}

pub fn other_personal_tun(caller: u32) -> bool {
    signed_in_uids()
        .into_iter()
        .filter(|uid| *uid != caller)
        .any(|uid| {
            let Ok(account) = lookup_account(uid) else {
                return false;
            };
            let path = account.home.join(".config/clash-kde/config.toml");
            fs::read_to_string(path).is_ok_and(|text| personal_tun_in(&text))
        })
}

pub fn ensure_tun_launch(
    binary: &Path,
    runtime: &Path,
    config: &Path,
    uid: u32,
) -> Result<(), ServiceError> {
    ensure_system_binary(binary)?;
    let account = lookup_account(uid)?;
    let expected = fs::canonicalize(runtime_dir_for(&account))
        .map_err(|_| ServiceError::new(codes::TUN_CONFIG_NOT_OWNED))?;
    let runtime =
        fs::canonicalize(runtime).map_err(|_| ServiceError::new(codes::TUN_CONFIG_NOT_OWNED))?;
    let config_path =
        fs::canonicalize(config).map_err(|_| ServiceError::new(codes::TUN_CONFIG_NOT_OWNED))?;
    judge_runtime(&config_path, &runtime, &expected, uid)?;
    let scope = grant_scope(Path::new(STAMP_DIR), uid)?;
    if scope == "machine" && other_personal_tun(uid) {
        return Err(ServiceError::new(codes::TUN_ALREADY_OTHER_USER));
    }
    let text = fs::read_to_string(&config_path).map_err(ServiceError::from_io)?;
    assess_tun_document(&text, uid, scope)
}

pub fn log_path_is_runtime(log: &Path, runtime: &Path) -> Result<(), ServiceError> {
    let runtime = fs::canonicalize(runtime).map_err(ServiceError::from_io)?;
    let parent = log
        .parent()
        .ok_or_else(|| ServiceError::new(codes::TUN_CONFIG_NOT_OWNED))?;
    let parent =
        fs::canonicalize(parent).map_err(|_| ServiceError::new(codes::TUN_CONFIG_NOT_OWNED))?;
    if parent != runtime || log.file_name() != Some(std::ffi::OsStr::new("mihomo.log")) {
        return Err(ServiceError::new(codes::TUN_CONFIG_NOT_OWNED));
    }
    match fs::symlink_metadata(log) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(ServiceError::new(codes::TUN_LOG_SYMLINK));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(ServiceError::new(codes::TUN_LOG_CHECK).detail(error.to_string()));
        }
    }
    Ok(())
}

pub fn stamp_path(root: &Path, uid: u32) -> PathBuf {
    root.join(uid.to_string())
}

pub fn write_stamp(root: &Path, uid: u32) -> io::Result<()> {
    write_grant(root, uid, "user")
}

pub fn write_machine_stamp(root: &Path, uid: u32) -> io::Result<()> {
    write_grant(root, uid, "machine")
}

fn write_grant(root: &Path, uid: u32, scope: &str) -> io::Result<()> {
    fs::create_dir_all(root)?;
    write_private(&stamp_path(root, uid), scope.as_bytes())
}

pub fn grant_scope(root: &Path, uid: u32) -> Result<&'static str, ServiceError> {
    let text =
        fs::read_to_string(stamp_path(root, uid))
        .map_err(|_| ServiceError::new(codes::TUN_NOT_AUTHORIZED))?;
    match text.trim() {
        "machine" => Ok("machine"),
        "user" | "" => Ok("user"),
        _ => Err(ServiceError::new(codes::TUN_NOT_AUTHORIZED)),
    }
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
        eprintln!(
            "{}",
            serde_json::to_string(&error).unwrap_or_else(|_| error.code.clone())
        );
        return 1;
    }
    unreachable!("prepare_exec returned without replacing the process")
}

fn prepare_exec(
    uid: u32,
    binary: &Path,
    runtime: &Path,
    config: &Path,
    log: &Path,
) -> Result<(), ServiceError> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(ServiceError::new(codes::TUN_SYSTEM_ROOT));
    }
    if !stamp_exists(Path::new(STAMP_DIR), uid) {
        return Err(ServiceError::new(codes::TUN_NOT_AUTHORIZED));
    }
    ensure_tun_launch(binary, runtime, config, uid)?;
    log_path_is_runtime(log, runtime)?;
    let account = lookup_account(uid)?;
    enter_user(uid, account.gid, &account.name)
        .map_err(|error| ServiceError::new(codes::TUN_SWITCH_ACCOUNT).detail(error.to_string()))?;
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(log)
        .map_err(|error| ServiceError::new(codes::CORE_LOG_OPEN).detail(error.to_string()))?;
    let stderr = log_file
        .try_clone()
        .map_err(|error| ServiceError::new(codes::CORE_LOG_OPEN).detail(error.to_string()))?;
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
    Err(ServiceError::new(codes::CORE_SPAWN_FAILED)
        .param("binary", binary.display().to_string())
        .detail(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Simulate a user-owned file at the expected path. Using a fixed uid
        // keeps this test valid when the suite runs as root in CI.
        let error = judge_binary(
            Path::new("/usr/bin/mihomo"),
            1000,
            0o755,
            Path::new("/usr/bin/mihomo"),
        );
        assert!(error.is_err());
    }

    #[test]
    fn accepts_a_root_owned_unwritable_system_binary() {
        let meta = fs::metadata("/bin/sh").unwrap();
        if meta.uid() != 0 {
            // Unprivileged test containers map root-owned files to nobody.
            eprintln!("skipping: /bin/sh is not root-owned in this environment");
            return;
        }
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
            .truncate(true)
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
        assert!(assess_tun_document(&fs::read_to_string(&config).unwrap(), 1, "user").is_ok());
        assert!(
            assess_tun_document(&fs::read_to_string(&config).unwrap(), uid, "user").is_err()
                || uid == 1
        );
        assert!(
            assess_tun_document("tun:\n  enable: true\n  include-uid: [1, 0]\n", 1, "user")
                .is_err()
        );
        assert!(assess_tun_document("tun:\n  enable: true\n", 1, "user").is_err());
        assert!(assess_tun_document("tun:\n  enable: true\n", 1, "machine").is_ok());
        assert!(
            assess_tun_document("tun:\n  enable: true\n  include-uid: [1]\n", 1, "machine")
                .is_err()
        );
        assert!(
            assess_tun_document("tun:\n  enable: true\n  exclude-uid: [1]\n", 1, "machine")
                .is_err()
        );
        assert!(personal_tun_in("tun_enabled = true\n"));
        assert!(!personal_tun_in("tun_enabled = true\ntun_machine = true\n"));
        assert!(!personal_tun_in("tun_enabled = false\n"));
    }

    #[test]
    fn stamp_can_be_removed() {
        let dir = tempfile::tempdir().unwrap();
        write_stamp(dir.path(), 7).unwrap();
        assert!(stamp_exists(dir.path(), 7));
        assert_eq!(grant_scope(dir.path(), 7).unwrap(), "user");
        write_machine_stamp(dir.path(), 7).unwrap();
        assert_eq!(grant_scope(dir.path(), 7).unwrap(), "machine");
        let mode = fs::metadata(stamp_path(dir.path(), 7)).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        remove_stamp(dir.path(), 7).unwrap();
        assert!(!stamp_exists(dir.path(), 7));
    }
    #[test]
    fn rejects_a_symlinked_tun_log() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("runtime");
        fs::create_dir_all(&runtime).unwrap();
        let target = dir.path().join("target.log");
        fs::write(&target, b"x").unwrap();
        std::os::unix::fs::symlink(&target, runtime.join("mihomo.log")).unwrap();
        let error = log_path_is_runtime(&runtime.join("mihomo.log"), &runtime).unwrap_err();
        assert_eq!(error.code, codes::TUN_LOG_SYMLINK);
    }
}
