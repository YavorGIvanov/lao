use lao_client_api::Status;

pub const OBSERVED: Version = Version(0, 146, 0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version(pub u16, pub u16, pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Support {
    Observed,
    Untested,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Auth {
    ApiKey,
    ChatGpt,
    Missing,
    Unsupported,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Conflict {
    Auth(&'static str),
    Base,
    Managed,
    Provider(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preview {
    pub provider: &'static str,
    pub base_url: String,
    pub caller_header: &'static str,
    pub native_auth: bool,
    pub websockets: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviewError;

pub fn status() -> Status {
    Status::Stub
}

pub fn support(output: &str) -> Support {
    match output.trim().strip_prefix("codex-cli ").and_then(parse) {
        Some(version) if version == OBSERVED => Support::Observed,
        Some(_) => Support::Untested,
        None => Support::Invalid,
    }
}

pub fn auth(output: &str) -> Auth {
    let output = output.trim();
    if output.contains('\n') || output.contains('\r') {
        return Auth::Invalid;
    }
    if let Some(masked) = output.strip_prefix("Logged in using an API key - ") {
        return if !masked.is_empty() && !masked.chars().any(char::is_whitespace) {
            Auth::ApiKey
        } else {
            Auth::Invalid
        };
    }
    match output {
        "Logged in using ChatGPT" => Auth::ChatGpt,
        "Not logged in" => Auth::Missing,
        value if value.starts_with("Logged in using ") => Auth::Unsupported,
        _ => Auth::Invalid,
    }
}

pub fn conflicts<'a>(keys: impl IntoIterator<Item = &'a str>, managed: bool) -> Vec<Conflict> {
    let mut found: Vec<_> = keys.into_iter().filter_map(classify).collect();
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
        provider: "lao",
        base_url: format!("http://127.0.0.1:{port}/oai"),
        caller_header: "X-LAO-Key: <redacted>",
        native_auth: true,
        websockets: false,
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
        "OPENAI_API_KEY" => Conflict::Auth("OPENAI_API_KEY"),
        "CODEX_API_KEY" => Conflict::Auth("CODEX_API_KEY"),
        "CODEX_ACCESS_TOKEN" => Conflict::Auth("CODEX_ACCESS_TOKEN"),
        "openai_base_url" => Conflict::Base,
        "model_provider" => Conflict::Provider("model_provider"),
        "model_providers" => Conflict::Provider("model_providers"),
        "profile" => Conflict::Provider("profile"),
        "profiles" => Conflict::Provider("profiles"),
        "CODEX_AUTHAPI_BASE_URL" => Conflict::Provider("CODEX_AUTHAPI_BASE_URL"),
        "CODEX_REFRESH_TOKEN_URL_OVERRIDE" => {
            Conflict::Provider("CODEX_REFRESH_TOKEN_URL_OVERRIDE")
        }
        "CODEX_REVOKE_TOKEN_URL_OVERRIDE" => Conflict::Provider("CODEX_REVOKE_TOKEN_URL_OVERRIDE"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owns_support_window() {
        assert_eq!(support("codex-cli 0.146.0\n"), Support::Observed);
        assert_eq!(support("codex-cli 0.147.0"), Support::Untested);
        assert_eq!(support("codex 0.146.0"), Support::Invalid);
    }

    #[test]
    fn classifies_status_without_retaining_secrets() {
        assert_eq!(auth("Logged in using ChatGPT"), Auth::ChatGpt);
        assert_eq!(
            auth("Logged in using an API key - sk-...1234"),
            Auth::ApiKey
        );
        assert_eq!(auth("Not logged in"), Auth::Missing);
        assert_eq!(auth("Logged in using an access token"), Auth::Unsupported);
        assert_eq!(
            auth("Logged in using an API key - sk-...1234\nwarning"),
            Auth::Invalid
        );
        assert_eq!(auth("error"), Auth::Invalid);
    }

    #[test]
    fn classifies_known_conflict_keys() {
        assert_eq!(
            conflicts(
                [
                    "openai_base_url",
                    "OPENAI_API_KEY",
                    "CODEX_ACCESS_TOKEN",
                    "model_provider",
                    "CODEX_AUTHAPI_BASE_URL",
                    "IGNORED",
                ],
                true,
            ),
            vec![
                Conflict::Auth("CODEX_ACCESS_TOKEN"),
                Conflict::Auth("OPENAI_API_KEY"),
                Conflict::Base,
                Conflict::Managed,
                Conflict::Provider("CODEX_AUTHAPI_BASE_URL"),
                Conflict::Provider("model_provider"),
            ]
        );
    }

    #[test]
    fn previews_custom_provider_without_secrets() {
        assert_eq!(
            preview(8765).unwrap(),
            Preview {
                provider: "lao",
                base_url: "http://127.0.0.1:8765/oai".into(),
                caller_header: "X-LAO-Key: <redacted>",
                native_auth: true,
                websockets: false,
            }
        );
        assert_eq!(preview(0), Err(PreviewError));
    }
}
