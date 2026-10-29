use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Result, TideError};

/// Tide's on-disk configuration. Every field has a safe default so a missing
/// configuration file never blocks read-only diagnostics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TideConfig {
    pub package: PackageConfig,
    pub apt: AptConfig,
    pub flatpak: FlatpakConfig,
    pub appimage: AppImageConfig,
    pub repositories: RepositoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PackageConfig {
    /// `auto`, or a backend id such as `apt`, `dnf5`, `pacman`, `apk`, `flatpak`, `appimage`.
    pub preferred_backend: String,
    pub assume_yes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AptConfig {
    pub install_recommends: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FlatpakConfig {
    /// `user` or `system`. User scope avoids elevating the package manager.
    pub scope: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppImageConfig {
    /// Optional override for the managed AppImage directory.
    pub managed_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RepositoryConfig {
    /// Tide-native repositories are not active yet. Adapters must never disable
    /// native signature validation, regardless of this value.
    pub native_tide_repos: bool,
}

impl Default for PackageConfig {
    fn default() -> Self {
        Self {
            preferred_backend: "auto".to_string(),
            assume_yes: false,
        }
    }
}

impl Default for AptConfig {
    fn default() -> Self {
        Self {
            install_recommends: true,
        }
    }
}

impl Default for FlatpakConfig {
    fn default() -> Self {
        Self {
            scope: "user".to_string(),
        }
    }
}

impl TideConfig {
    pub fn default_path() -> PathBuf {
        std::env::var_os("TIDE_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/etc/tide/tide.toml"))
    }

    /// Load configuration if present, otherwise return defaults.
    pub fn load_optional(path: Option<&PathBuf>) -> Result<Self> {
        let path = path.cloned().unwrap_or_else(Self::default_path);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content =
            std::fs::read_to_string(&path).map_err(|error| TideError::io(&path, error))?;
        let config: TideConfig = toml::from_str(&content)
            .map_err(|error| TideError::Config(format!("{path:?}: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        let preferred = self.package.preferred_backend.as_str();
        let allowed = [
            "auto", "apt", "dnf5", "dnf", "pacman", "apk", "flatpak", "appimage",
        ];
        if !allowed.contains(&preferred) {
            return Err(TideError::Config(format!(
                "package.preferred_backend must be one of {}, got '{preferred}'",
                allowed.join(", ")
            )));
        }
        if !matches!(self.flatpak.scope.as_str(), "user" | "system") {
            return Err(TideError::Config(
                "flatpak.scope must be 'user' or 'system'".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_valid() {
        let config = TideConfig::default();
        config.validate().unwrap();
        assert!(!config.repositories.native_tide_repos);
        assert!(config.apt.install_recommends);
    }

    #[test]
    fn rejects_unknown_backend() {
        let mut config = TideConfig::default();
        config.package.preferred_backend = "portage".into();
        assert!(config.validate().is_err());
    }
}
