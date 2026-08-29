use lao_client_api::Status;

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

pub fn status() -> Status {
    Status::Stub
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
    }
}
