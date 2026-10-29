use std::path::PathBuf;

/// Well-known system paths. These are defaults, not hard-coded build-machine paths;
/// callers may override them in tests or packaged deployments.
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"))
}

pub fn xdg_data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home_dir().join(".local/share"))
}

pub fn default_appimage_dir() -> PathBuf {
    xdg_data_home().join("tide/appimages")
}

pub fn default_appimage_desktop_dir() -> PathBuf {
    xdg_data_home().join("applications")
}

pub fn system_data_dir() -> PathBuf {
    PathBuf::from("/usr/share/tide")
}

pub fn system_config_dir() -> PathBuf {
    PathBuf::from("/etc/tide")
}
