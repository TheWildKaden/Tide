use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::{unsupported, PackageBackend, RunOptions};
use crate::error::{Result, TideError};
use crate::exec::CommandRunner;
use crate::model::{
    BackendCapabilities, CheckLevel, DoctorFinding, OperationReport, PackageSummary, RepoEntry,
};

/// Conservative AppImage integration.
///
/// AppImages are copied into a Tide-managed directory and exposed through a desktop
/// entry. Tide never executes an AppImage during installation, and it clearly reports
/// that AppImages do not provide repository metadata or automatic signature
/// verification.
#[derive(Debug, Clone)]
pub struct AppImageBackend {
    managed_dir: PathBuf,
    desktop_dir: PathBuf,
}

impl AppImageBackend {
    pub fn new(managed_dir: PathBuf, desktop_dir: PathBuf) -> Self {
        Self {
            managed_dir,
            desktop_dir,
        }
    }

    fn locate(&self, name: &str) -> Result<PathBuf> {
        validate_app_name(name)?;
        let filename = if name.ends_with(".AppImage") {
            name.to_string()
        } else {
            format!("{name}.AppImage")
        };
        Ok(self.managed_dir.join(filename))
    }

    fn entries(&self) -> Result<Vec<PackageSummary>> {
        let mut packages = Vec::new();
        let read_dir = match fs::read_dir(&self.managed_dir) {
            Ok(read_dir) => read_dir,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(packages),
            Err(error) => return Err(TideError::io(&self.managed_dir, error)),
        };
        for entry in read_dir {
            let entry = entry.map_err(|error| TideError::io(&self.managed_dir, error))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("AppImage") {
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            packages.push(PackageSummary {
                backend: "appimage".to_string(),
                name,
                version: None,
                description: Some(path.display().to_string()),
                installed: Some(true),
            });
        }
        packages.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(packages)
    }
}

impl PackageBackend for AppImageBackend {
    fn key(&self) -> &'static str {
        "appimage"
    }

    fn name(&self) -> &'static str {
        "AppImage (managed local applications)"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            search: false,
            info: true,
            list: true,
            install: true,
            remove: true,
            update: false,
            upgrade: false,
            outdated: false,
            repositories: false,
        }
    }

    fn detect(&self, _runner: &dyn CommandRunner) -> bool {
        true
    }

    fn version(&self, _runner: &dyn CommandRunner) -> Option<String> {
        Some("managed".to_string())
    }

    fn notes(&self) -> Vec<String> {
        vec![
            "AppImages are installed from local files only; there is no app store or repository index.".into(),
            "AppImage files are not executed by Tide during installation.".into(),
            "AppImages do not have Debian/RPM-style package signatures; verify checksums before installing.".into(),
        ]
    }

    fn search(&self, _runner: &dyn CommandRunner, _query: &str) -> Result<Vec<PackageSummary>> {
        Err(unsupported("appimage", "search"))
    }

    fn info(&self, _runner: &dyn CommandRunner, package: &str) -> Result<PackageSummary> {
        let path = self.locate(package)?;
        if !path.exists() {
            return Err(TideError::Message(format!(
                "AppImage '{}' is not managed by Tide",
                path.display()
            )));
        }
        Ok(PackageSummary {
            backend: "appimage".to_string(),
            name: path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(package)
                .to_string(),
            version: None,
            description: Some(path.display().to_string()),
            installed: Some(true),
        })
    }

    fn list_installed(&self, _runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        self.entries()
    }

    fn install(
        &self,
        _runner: &dyn CommandRunner,
        packages: &[String],
        options: &RunOptions,
    ) -> Result<OperationReport> {
        if packages.is_empty() {
            return Err(TideError::InvalidInput {
                kind: "AppImage path",
                message: "at least one path to an AppImage file is required".into(),
            });
        }
        let mut details = Vec::new();
        let mut planned_details = Vec::new();
        for source in packages {
            if source.starts_with('-') {
                return Err(TideError::InvalidInput {
                    kind: "AppImage path",
                    message: format!("'{source}' may not start with '-'"),
                });
            }
            let source_path = PathBuf::from(source);
            let metadata = fs::metadata(&source_path)
                .map_err(|error| TideError::io(&source_path, error))?;
            if !metadata.is_file() {
                return Err(TideError::InvalidInput {
                    kind: "AppImage path",
                    message: format!("'{}' is not a regular file", source_path.display()),
                });
            }
            verify_elf_magic(&source_path)?;
            let filename = source_path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| TideError::InvalidInput {
                    kind: "AppImage path",
                    message: format!("'{}' has no valid file name", source_path.display()),
                })?;
            if filename.starts_with('-') || !filename.ends_with(".AppImage") {
                return Err(TideError::InvalidInput {
                    kind: "AppImage path",
                    message: "file name must end in .AppImage and may not start with '-'".into(),
                });
            }
            let destination = self.managed_dir.join(filename);
            planned_details.push(format!(
                "copy {} -> {}",
                source_path.display(),
                destination.display()
            ));
            if options.dry_run {
                continue;
            }
            fs::create_dir_all(&self.managed_dir)
                .map_err(|error| TideError::io(&self.managed_dir, error))?;
            if source_path != destination {
                fs::copy(&source_path, &destination)
                    .map_err(|error| TideError::io(&destination, error))?;
            }
            set_executable(&destination)?;
            let desktop = self.desktop_dir.join(format!("tide-appimage-{filename}.desktop"));
            write_desktop_entry(&desktop, filename, &destination)?;
            details.push(format!("installed {}", destination.display()));
        }
        if options.dry_run {
            return Ok(OperationReport {
                summary: format!("would install {} AppImage(s)", packages.len()),
                details: planned_details,
                changed: false,
            });
        }
        Ok(OperationReport::completed(
            format!("installed {} AppImage(s)", packages.len()),
            details,
        ))
    }

    fn remove(
        &self,
        _runner: &dyn CommandRunner,
        packages: &[String],
        options: &RunOptions,
    ) -> Result<OperationReport> {
        if packages.is_empty() {
            return Err(TideError::InvalidInput {
                kind: "application name",
                message: "at least one managed AppImage name is required".into(),
            });
        }
        let mut details = Vec::new();
        for package in packages {
            let path = self.locate(package)?;
            if !path.exists() {
                return Err(TideError::Message(format!(
                    "'{}' is not managed by Tide",
                    path.display()
                )));
            }
            let filename = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            let desktop = self.desktop_dir.join(format!("tide-appimage-{filename}.desktop"));
            if options.dry_run {
                details.push(format!("would remove {}", path.display()));
                continue;
            }
            fs::remove_file(&path).map_err(|error| TideError::io(&path, error))?;
            if desktop.exists() {
                fs::remove_file(&desktop).map_err(|error| TideError::io(&desktop, error))?;
            }
            details.push(format!("removed {}", path.display()));
        }
        if options.dry_run {
            return Ok(OperationReport {
                summary: format!("would remove {} AppImage(s)", packages.len()),
                details,
                changed: false,
            });
        }
        Ok(OperationReport::completed(
            format!("removed {} AppImage(s)", packages.len()),
            details,
        ))
    }

    fn update(&self, _runner: &dyn CommandRunner) -> Result<OperationReport> {
        Err(unsupported("appimage", "update"))
    }

    fn upgrade(&self, _runner: &dyn CommandRunner) -> Result<OperationReport> {
        Err(unsupported("appimage", "upgrade"))
    }

    fn outdated(&self, _runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        Err(unsupported("appimage", "outdated"))
    }

    fn repositories(&self, _runner: &dyn CommandRunner) -> Result<Vec<RepoEntry>> {
        Err(unsupported("appimage", "repositories"))
    }

    fn doctor(&self, _runner: &dyn CommandRunner) -> Result<Vec<DoctorFinding>> {
        let mut findings = Vec::new();
        match fs::metadata(&self.managed_dir) {
            Ok(metadata) if metadata.is_dir() => findings.push(DoctorFinding::new(
                CheckLevel::Ok,
                "appimage",
                format!("managed directory {} exists", self.managed_dir.display()),
            )),
            Ok(_) => findings.push(DoctorFinding::new(
                CheckLevel::Error,
                "appimage",
                format!("{} is not a directory", self.managed_dir.display()),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => findings.push(
                DoctorFinding::new(
                    CheckLevel::Info,
                    "appimage",
                    format!(
                        "managed directory {} does not exist yet; it will be created on first install",
                        self.managed_dir.display()
                    ),
                )
                .with_remediation("Run: tide --backend appimage install ./application.AppImage"),
            ),
            Err(error) => findings.push(DoctorFinding::new(
                CheckLevel::Error,
                "appimage",
                format!("cannot inspect {}: {error}", self.managed_dir.display()),
            )),
        }
        findings.push(DoctorFinding::new(
            CheckLevel::Warning,
            "appimage",
            "AppImages are not signed by a package repository; verify the publisher checksum before installing",
        ));
        Ok(findings)
    }
}

fn validate_app_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.starts_with('-')
        || name.contains('/')
        || name.contains('\\')
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '+'))
    {
        return Err(TideError::InvalidInput {
            kind: "AppImage name",
            message: format!("'{name}' is not a valid managed AppImage name"),
        });
    }
    Ok(())
}

fn verify_elf_magic(path: &Path) -> Result<()> {
    let bytes = fs::read(path).map_err(|error| TideError::io(path, error))?;
    if bytes.len() < 4 || &bytes[..4] != b"\x7fELF" {
        return Err(TideError::InvalidInput {
            kind: "AppImage",
            message: format!("'{}' does not look like an ELF executable", path.display()),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|error| TideError::io(path, error))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|error| TideError::io(path, error))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn write_desktop_entry(path: &Path, filename: &str, executable: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| TideError::io(parent, error))?;
    }
    let name = filename.trim_end_matches(".AppImage").replace('_', " ");
    let content = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec={}\nTerminal=false\nCategories=Utility;\nX-Tide-Managed=true\n",
        desktop_escape(&name),
        desktop_exec_quote(executable),
    );
    fs::write(path, content).map_err(|error| TideError::io(path, error))
}

fn desktop_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', " ")
}

fn desktop_exec_quote(path: &Path) -> String {
    let rendered = path.display().to_string();
    let escaped = rendered
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "tide-appimage-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        (base.join("apps"), base.join("applications"))
    }

    #[test]
    fn rejects_traversal_names() {
        assert!(validate_app_name("../../etc/passwd").is_err());
        assert!(validate_app_name("--help").is_err());
        assert!(validate_app_name("My.AppImage").is_ok());
    }

    #[test]
    fn installs_and_removes_managed_appimage() {
        let (apps, desktop) = dirs("lifecycle");
        let backend = AppImageBackend::new(apps.clone(), desktop.clone());
        let source_dir = apps.parent().unwrap();
        fs::create_dir_all(source_dir).unwrap();
        let source = source_dir.join("Example.AppImage");
        fs::write(&source, b"\x7fELFfake").unwrap();
        let runner = crate::exec::MockRunner::default();
        backend
            .install(&runner, &[source.display().to_string()], &RunOptions::default())
            .unwrap();
        assert!(apps.join("Example.AppImage").exists());
        assert!(desktop.join("tide-appimage-Example.AppImage.desktop").exists());
        let listed = backend.list_installed(&runner).unwrap();
        assert_eq!(listed[0].name, "Example");
        backend
            .remove(&runner, &["Example".to_string()], &RunOptions::default())
            .unwrap();
        assert!(!apps.join("Example.AppImage").exists());
        let _ = fs::remove_dir_all(source_dir);
    }

    #[test]
    fn refuses_non_elf_file() {
        let (apps, desktop) = dirs("non-elf");
        let backend = AppImageBackend::new(apps.clone(), desktop);
        let source_dir = apps.parent().unwrap();
        fs::create_dir_all(source_dir).unwrap();
        let source = source_dir.join("Bad.AppImage");
        fs::write(&source, b"#!/bin/sh\necho nope\n").unwrap();
        let runner = crate::exec::MockRunner::default();
        assert!(backend
            .install(&runner, &[source.display().to_string()], &RunOptions::default())
            .is_err());
        let _ = fs::remove_dir_all(source_dir);
    }
}
