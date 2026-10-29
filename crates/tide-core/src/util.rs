use crate::error::{Result, TideError};

/// Validate a package name or package spec before passing it to a native backend.
///
/// This blocks option injection (leading `-`), control characters, whitespace, and
/// shell metacharacters. Native package specs may contain `+._:-` and an optional
/// `=version` suffix for APT-style pinning.
pub fn validate_package_spec(spec: &str, allow_version: bool) -> Result<()> {
    if spec.is_empty() {
        return Err(TideError::InvalidInput {
            kind: "package name",
            message: "must not be empty".into(),
        });
    }
    if spec.len() > 256 {
        return Err(TideError::InvalidInput {
            kind: "package name",
            message: "is longer than 256 bytes".into(),
        });
    }
    if spec.starts_with('-') {
        return Err(TideError::InvalidInput {
            kind: "package name",
            message: format!("'{spec}' may not start with '-'"),
        });
    }
    let (name, version) = match spec.split_once('=') {
        Some((name, version)) => (name, Some(version)),
        None => (spec, None),
    };
    if version.is_some() && !allow_version {
        return Err(TideError::InvalidInput {
            kind: "package name",
            message: format!("version pins are not supported for '{spec}'"),
        });
    }
    if let Some(version) = version {
        if version.is_empty() || !version.chars().all(is_version_char) {
            return Err(TideError::InvalidInput {
                kind: "package version",
                message: format!("invalid version in '{spec}'"),
            });
        }
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => {
            return Err(TideError::InvalidInput {
                kind: "package name",
                message: format!("'{spec}' must start with an ASCII letter or digit"),
            })
        }
    }
    if !chars.all(is_package_char) {
        return Err(TideError::InvalidInput {
            kind: "package name",
            message: format!("'{spec}' contains unsupported characters"),
        });
    }
    Ok(())
}

fn is_package_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '+' | '.' | '_' | ':' | '-')
}

fn is_version_char(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '+' | '.' | '_' | ':' | '-' | '~' | '?')
}

/// Validate a systemd unit name used by `tidectl services`.
pub fn validate_unit_name(unit: &str) -> Result<()> {
    if unit.is_empty()
        || unit.len() > 255
        || unit.starts_with('-')
        || !unit.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '@' | ':')
        })
    {
        return Err(TideError::InvalidInput {
            kind: "systemd unit",
            message: format!("'{unit}' is not a valid unit name"),
        });
    }
    Ok(())
}

/// Quote a command for human display only. Never use the result for execution.
pub fn display_command(program: &str, args: &[String]) -> String {
    let mut rendered = String::new();
    rendered.push_str(&shell_quote(program));
    for arg in args {
        rendered.push(' ');
        rendered.push_str(&shell_quote(arg));
    }
    rendered
}

fn shell_quote(value: &str) -> String {
    if value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/' | ':' | '=')
    }) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// Parse KEY="value" pairs from an os-release style file.
pub fn parse_key_value_file(content: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or(value);
            map.insert(key.trim().to_string(), value.replace("\\\"", "\""));
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_package_names() {
        for spec in [
            "firefox",
            "libc6",
            "python3.12",
            "g++",
            "foo=1.2.3-1",
            "nvidia-driver",
        ] {
            validate_package_spec(spec, true).unwrap();
        }
    }

    #[test]
    fn rejects_injection_attempts() {
        for spec in ["-y", "", "foo bar", "foo;rm", "$(id)", "foo\nbar", "../etc"] {
            assert!(validate_package_spec(spec, true).is_err(), "{spec}");
        }
    }

    #[test]
    fn rejects_version_when_not_allowed() {
        assert!(validate_package_spec("foo=1.0", false).is_err());
    }

    #[test]
    fn validates_units() {
        assert!(validate_unit_name("sshd.service").is_ok());
        assert!(validate_unit_name("--now").is_err());
        assert!(validate_unit_name("sshd; rm").is_err());
    }

    #[test]
    fn parses_os_release() {
        let parsed = parse_key_value_file("NAME=\"Tide OS\"\nID=tide\n# comment\n");
        assert_eq!(parsed["NAME"], "Tide OS");
        assert_eq!(parsed["ID"], "tide");
    }
}
