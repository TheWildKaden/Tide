use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub search: bool,
    pub info: bool,
    pub list: bool,
    pub install: bool,
    pub remove: bool,
    pub update: bool,
    pub upgrade: bool,
    pub outdated: bool,
    pub repositories: bool,
}

impl BackendCapabilities {
    pub fn names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.search {
            names.push("search");
        }
        if self.info {
            names.push("info");
        }
        if self.list {
            names.push("list");
        }
        if self.install {
            names.push("install");
        }
        if self.remove {
            names.push("remove");
        }
        if self.update {
            names.push("update");
        }
        if self.upgrade {
            names.push("upgrade");
        }
        if self.outdated {
            names.push("outdated");
        }
        if self.repositories {
            names.push("repositories");
        }
        names
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageSummary {
    pub backend: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoEntry {
    pub id: String,
    pub manager: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suites: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub capabilities: BackendCapabilities,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckLevel {
    Ok,
    Info,
    Warning,
    Error,
}

impl CheckLevel {
    pub fn label(self) -> &'static str {
        match self {
            CheckLevel::Ok => "ok",
            CheckLevel::Info => "info",
            CheckLevel::Warning => "warning",
            CheckLevel::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorFinding {
    pub level: CheckLevel,
    pub area: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl DoctorFinding {
    pub fn new(level: CheckLevel, area: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level,
            area: area.into(),
            message: message.into(),
            remediation: None,
        }
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationReport {
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
    pub changed: bool,
}

impl OperationReport {
    pub fn completed(summary: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            summary: summary.into(),
            details,
            changed: true,
        }
    }

    pub fn planned(summary: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            details: vec![format!("would run: {}", command.into())],
            changed: false,
        }
    }
}
