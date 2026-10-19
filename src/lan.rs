use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn policy_dir() -> PathBuf {
    PathBuf::from("/var/lib/clash-kde/lan")
}

pub fn current_uid() -> u32 {
    unsafe { libc::geteuid() as u32 }
}

/// Missing files mean the administrator has not forbidden this.
pub fn permitted(dir: &Path, uid: u32) -> bool {
    global_allowed(dir) && user_allowed(dir, uid)
}

pub fn global_allowed(dir: &Path) -> bool {
    flag(dir, "global") != Some(false)
}

pub fn user_allowed(dir: &Path, uid: u32) -> bool {
    flag(dir, &uid.to_string()) != Some(false)
}

pub fn set_global(dir: &Path, allow: bool) -> io::Result<()> {
    write_flag(&dir.join("global"), allow)
}

pub fn set_user(dir: &Path, uid: u32, allow: bool) -> io::Result<()> {
    write_flag(&dir.join(uid.to_string()), allow)
}

pub fn normalize_auth(username: &str, password: &str) -> Result<(String, String), String> {
    let username = username.trim();
    let password = password.trim();
    if username.is_empty() && password.is_empty() {
        return Ok((String::new(), String::new()));
    }
    if username.is_empty() || password.is_empty() {
        return Err("局域网账号和密码要一起填写".to_string());
    }
    if username.contains(':') || password.contains(':') {
        return Err("局域网账号和密码不能包含冒号".to_string());
    }
    Ok((username.to_string(), password.to_string()))
}

fn flag(dir: &Path, name: &str) -> Option<bool> {
    let text = fs::read_to_string(dir.join(name)).ok()?;
    match text.trim() {
        "deny" => Some(false),
        "allow" => Some(true),
        _ => None,
    }
}

fn write_flag(path: &Path, allow: bool) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o755))?;
            if let Some(grand) = parent.parent() {
                let _ = fs::set_permissions(grand, fs::Permissions::from_mode(0o755));
            }
        }
    }
    fs::write(path, if allow { "allow\n" } else { "deny\n" })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_policy_allows_and_a_deny_file_forbids() {
        let dir = std::env::temp_dir().join(format!("clash-kde-lan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(permitted(&dir, 1000));
        set_user(&dir, 1000, false).unwrap();
        assert!(!permitted(&dir, 1000));
        assert!(permitted(&dir, 1001));
        set_global(&dir, false).unwrap();
        assert!(!permitted(&dir, 1001));
        set_global(&dir, true).unwrap();
        set_user(&dir, 1000, true).unwrap();
        assert!(permitted(&dir, 1000));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lan_auth_is_both_or_neither_and_has_no_colon() {
        assert!(normalize_auth("", "").is_ok());
        assert!(normalize_auth("a", "").is_err());
        assert!(normalize_auth("", "b").is_err());
        assert!(normalize_auth("a:b", "c").is_err());
        assert!(normalize_auth("a", "b:c").is_err());
        assert_eq!(
            normalize_auth(" a ", " b ").unwrap(),
            ("a".to_string(), "b".to_string())
        );
    }
}
