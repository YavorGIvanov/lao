use lao_client_api::Status;
use serde_json::{Map, Value};
use std::{error::Error, fmt};

pub const OBSERVED: Version = Version(2, 1, 251);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version(pub u16, pub u16, pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Support {
    Observed,
    Untested,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Conflict {
    Auth(&'static str),
    Base,
    Helper,
    Managed,
    Provider(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preview {
    pub base_url: String,
    pub caller_header: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviewError;

#[derive(Debug, Eq, PartialEq)]
pub enum ConfigError {
    Conflict,
    Invalid,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Conflict => "conflicting Claude provider configuration",
            Self::Invalid => "invalid Claude settings",
        })
    }
}

impl Error for ConfigError {}

pub fn status() -> Status {
    Status::Active
}

pub fn support(output: &str) -> Support {
    let raw = output
        .trim()
        .strip_suffix(" (Claude Code)")
        .unwrap_or_default();
    match parse(raw) {
        Some(version) if version == OBSERVED => Support::Observed,
        Some(_) => Support::Untested,
        None => Support::Invalid,
    }
}

pub fn conflicts<'a>(
    keys: impl IntoIterator<Item = &'a str>,
    helper: bool,
    managed: bool,
) -> Vec<Conflict> {
    let mut found: Vec<_> = keys.into_iter().filter_map(classify).collect();
    found.extend(helper.then_some(Conflict::Helper));
    found.extend(managed.then_some(Conflict::Managed));
    found.sort();
    found.dedup();
    found
}

pub fn preview(port: u16) -> Result<Preview, PreviewError> {
    if port == 0 {
        return Err(PreviewError);
    }
    Ok(Preview {
        base_url: format!("http://127.0.0.1:{port}/ant"),
        caller_header: "X-LAO-Key: <redacted>",
    })
}

pub fn configure(original: Option<&[u8]>, port: u16, caller: &str) -> Result<Vec<u8>, ConfigError> {
    if port == 0 || !valid_caller(caller) {
        return Err(ConfigError::Invalid);
    }
    let mut root = match original {
        Some(bytes) => serde_json::from_slice::<Value>(bytes).map_err(|_| ConfigError::Invalid)?,
        None => Value::Object(Map::new()),
    };
    let object = root.as_object_mut().ok_or(ConfigError::Invalid)?;
    if object.contains_key("apiKeyHelper") {
        return Err(ConfigError::Conflict);
    }
    let env = object
        .entry("env")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or(ConfigError::Invalid)?;
    if env.keys().any(|key| classify(key).is_some()) {
        return Err(ConfigError::Conflict);
    }
    env.insert(
        "ANTHROPIC_BASE_URL".into(),
        Value::String(format!("http://127.0.0.1:{port}/ant")),
    );
    env.insert(
        "ANTHROPIC_CUSTOM_HEADERS".into(),
        Value::String(format!("X-LAO-Key: {caller}")),
    );
    serde_json::to_vec_pretty(&root).map_err(|_| ConfigError::Invalid)
}

pub fn verify(current: &[u8], installed: &[u8]) -> Result<(), ConfigError> {
    let current = object(current)?;
    let installed = object(installed)?;
    let current_env = current
        .get("env")
        .and_then(Value::as_object)
        .ok_or(ConfigError::Invalid)?;
    let installed_env = installed
        .get("env")
        .and_then(Value::as_object)
        .ok_or(ConfigError::Invalid)?;
    for key in ["ANTHROPIC_BASE_URL", "ANTHROPIC_CUSTOM_HEADERS"] {
        if current_env.get(key) != installed_env.get(key) {
            return Err(ConfigError::Conflict);
        }
    }
    Ok(())
}

pub fn restore(
    current: &[u8],
    installed: &[u8],
    original: Option<&[u8]>,
) -> Result<Vec<u8>, ConfigError> {
    let original = original.unwrap_or_default();
    if current == installed {
        return Ok(original.to_vec());
    }
    verify(current, installed)?;
    let original_had_env = !original.is_empty() && object(original)?.contains_key("env");
    let mut current = serde_json::from_slice::<Value>(current).map_err(|_| ConfigError::Invalid)?;
    let root = current.as_object_mut().ok_or(ConfigError::Invalid)?;
    let env = root
        .get_mut("env")
        .and_then(Value::as_object_mut)
        .ok_or(ConfigError::Invalid)?;
    env.remove("ANTHROPIC_BASE_URL");
    env.remove("ANTHROPIC_CUSTOM_HEADERS");
    if !original_had_env && env.is_empty() {
        root.remove("env");
    }
    serde_json::to_vec_pretty(&current).map_err(|_| ConfigError::Invalid)
}

fn object(bytes: &[u8]) -> Result<Map<String, Value>, ConfigError> {
    serde_json::from_slice::<Value>(bytes)
        .map_err(|_| ConfigError::Invalid)?
        .as_object()
        .cloned()
        .ok_or(ConfigError::Invalid)
}

fn valid_caller(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse(raw: &str) -> Option<Version> {
    let mut parts = raw.split('.').map(str::parse);
    let version = Version(
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    parts.next().is_none().then_some(version)
}

fn classify(key: &str) -> Option<Conflict> {
    Some(match key {
        "ANTHROPIC_AUTH_TOKEN" => Conflict::Auth("ANTHROPIC_AUTH_TOKEN"),
        "ANTHROPIC_API_KEY" => Conflict::Auth("ANTHROPIC_API_KEY"),
        "CLAUDE_CODE_OAUTH_TOKEN" => Conflict::Auth("CLAUDE_CODE_OAUTH_TOKEN"),
        "ANTHROPIC_PROFILE" => Conflict::Auth("ANTHROPIC_PROFILE"),
        "ANTHROPIC_FEDERATION_RULE_ID" => Conflict::Auth("ANTHROPIC_FEDERATION_RULE_ID"),
        "ANTHROPIC_ORGANIZATION_ID" => Conflict::Auth("ANTHROPIC_ORGANIZATION_ID"),
        "ANTHROPIC_WORKSPACE_ID" => Conflict::Auth("ANTHROPIC_WORKSPACE_ID"),
        "ANTHROPIC_BASE_URL" => Conflict::Base,
        "ANTHROPIC_CUSTOM_HEADERS" => Conflict::Provider("ANTHROPIC_CUSTOM_HEADERS"),
        "CLAUDE_CODE_USE_BEDROCK" => Conflict::Provider("CLAUDE_CODE_USE_BEDROCK"),
        "CLAUDE_CODE_USE_VERTEX" => Conflict::Provider("CLAUDE_CODE_USE_VERTEX"),
        "CLAUDE_CODE_USE_FOUNDRY" => Conflict::Provider("CLAUDE_CODE_USE_FOUNDRY"),
        "CLAUDE_CODE_USE_MANTLE" => Conflict::Provider("CLAUDE_CODE_USE_MANTLE"),
        "ANTHROPIC_BEDROCK_BASE_URL" => Conflict::Provider("ANTHROPIC_BEDROCK_BASE_URL"),
        "ANTHROPIC_VERTEX_BASE_URL" => Conflict::Provider("ANTHROPIC_VERTEX_BASE_URL"),
        "ANTHROPIC_FOUNDRY_BASE_URL" => Conflict::Provider("ANTHROPIC_FOUNDRY_BASE_URL"),
        "ANTHROPIC_AWS_BASE_URL" => Conflict::Provider("ANTHROPIC_AWS_BASE_URL"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owns_support_window() {
        assert_eq!(support("2.1.251 (Claude Code)"), Support::Observed);
        assert_eq!(support("2.1.252 (Claude Code)"), Support::Untested);
        assert_eq!(support("dev"), Support::Invalid);
    }

    #[test]
    fn classifies_known_conflict_keys() {
        assert_eq!(
            conflicts(
                [
                    "ANTHROPIC_BASE_URL",
                    "ANTHROPIC_API_KEY",
                    "ANTHROPIC_AUTH_TOKEN",
                    "ANTHROPIC_FEDERATION_RULE_ID",
                    "CLAUDE_CODE_USE_BEDROCK",
                    "IGNORED",
                ],
                true,
                true,
            ),
            vec![
                Conflict::Auth("ANTHROPIC_API_KEY"),
                Conflict::Auth("ANTHROPIC_AUTH_TOKEN"),
                Conflict::Auth("ANTHROPIC_FEDERATION_RULE_ID"),
                Conflict::Base,
                Conflict::Helper,
                Conflict::Managed,
                Conflict::Provider("CLAUDE_CODE_USE_BEDROCK"),
            ]
        );
    }

    #[test]
    fn previews_base_and_caller_header_without_secrets() {
        assert_eq!(
            preview(8765).unwrap(),
            Preview {
                base_url: "http://127.0.0.1:8765/ant".into(),
                caller_header: "X-LAO-Key: <redacted>",
            }
        );
        assert_eq!(preview(0), Err(PreviewError));

        let configured = configure(
            Some(br#"{"permissions":{"defaultMode":"default"}}"#),
            8765,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        let configured: serde_json::Value = serde_json::from_slice(&configured).unwrap();
        assert_eq!(configured["permissions"]["defaultMode"], "default");
        assert_eq!(
            configured["env"]["ANTHROPIC_BASE_URL"],
            "http://127.0.0.1:8765/ant"
        );
        assert_eq!(
            configure(
                Some(br#"{"env":{"ANTHROPIC_API_KEY":"existing"}}"#),
                8765,
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ),
            Err(ConfigError::Conflict)
        );
    }

    #[test]
    fn preserves_unrelated_edits_when_restoring_managed_settings() {
        let original = br#"{"permissions":{"defaultMode":"default"}}"#;
        let installed = configure(
            Some(original),
            8765,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        let mut current: Value = serde_json::from_slice(&installed).unwrap();
        current["theme"] = Value::String("dark".into());
        let current = serde_json::to_vec(&current).unwrap();

        verify(&current, &installed).unwrap();
        let restored: Value =
            serde_json::from_slice(&restore(&current, &installed, Some(original)).unwrap())
                .unwrap();
        assert_eq!(restored["theme"], "dark");
        assert_eq!(restored["permissions"]["defaultMode"], "default");
        assert!(restored.get("env").is_none());

        let mut changed: Value = serde_json::from_slice(&installed).unwrap();
        changed["env"]["ANTHROPIC_BASE_URL"] = Value::String("http://127.0.0.1:9999".into());
        assert_eq!(
            restore(
                &serde_json::to_vec(&changed).unwrap(),
                &installed,
                Some(original)
            ),
            Err(ConfigError::Conflict)
        );
        assert_eq!(
            restore(&installed, &installed, Some(original)).unwrap(),
            original
        );
    }
}
