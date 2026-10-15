use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub kioslaverc: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::config_dir())
            .unwrap_or_else(|| PathBuf::from(".config"));
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::data_dir())
            .unwrap_or_else(|| PathBuf::from(".local/share"));
        Self {
            kioslaverc: config_home.join("kioslaverc"),
            config_dir: config_home.join("clash-kde"),
            data_dir: data_home.join("clash-kde"),
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn override_file(&self) -> PathBuf {
        self.config_dir.join("override.yaml")
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.data_dir.join("profiles")
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.data_dir.join("runtime")
    }

    pub fn runtime_config(&self) -> PathBuf {
        self.runtime_dir().join("config.yaml")
    }

    pub fn core_log(&self) -> PathBuf {
        self.runtime_dir().join("mihomo.log")
    }

    pub fn proxy_flag(&self) -> PathBuf {
        self.config_dir.join("proxy-applied")
    }

    pub fn proxy_backup(&self) -> PathBuf {
        self.config_dir.join("proxy-group.backup")
    }
}
