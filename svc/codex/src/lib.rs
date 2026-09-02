use lao_client_api::Status;
use std::{error::Error, fmt, path::Path};

pub const OBSERVED: Version = Version(0, 151, 0);
pub const DELEGATION_INSTRUCTIONS: &str = "Delegate one small mechanical implementation to lao.execute before editing when you can name every writable path. Call it once with a bounded objective and exact repository-relative paths. If it returns Cloud, continue yourself. If Local completes, review changed paths and verify without repeating the edit. Keep planning, broad, ambiguous, security-sensitive, or multi-area work in this cloud harness.";

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

#[derive(Debug, Eq, PartialEq)]
pub enum ConfigError {
    Conflict,
    Invalid,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Conflict => "conflicting Codex configuration",
            Self::Invalid => "invalid Codex configuration",
        })
    }
}

impl Error for ConfigError {}

pub fn status() -> Status {
    Status::Active
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

pub fn configure(
    original: Option<&[u8]>,
    port: u16,
    caller: &str,
    catalog: &str,
) -> Result<Vec<u8>, ConfigError> {
    if port == 0 || !valid_caller(caller) || !Path::new(catalog).is_absolute() {
        return Err(ConfigError::Invalid);
    }
    let raw = original.unwrap_or_default();
    let text = std::str::from_utf8(raw).map_err(|_| ConfigError::Invalid)?;
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| ConfigError::Invalid)?;
    if [
        "openai_base_url",
        "model_provider",
        "model_providers",
        "profile",
        "profiles",
    ]
    .iter()
    .any(|key| document.contains_key(key))
    {
        return Err(ConfigError::Conflict);
    }

    document["model_provider"] = toml_edit::value("lao");
    if !document.contains_key("model_catalog_json") {
        document["model_catalog_json"] = toml_edit::value(catalog);
    }
    let mut headers = toml_edit::InlineTable::new();
    headers.insert("X-LAO-Key", caller.into());
    let mut environment_headers = toml_edit::InlineTable::new();
    environment_headers.insert("X-LAO-Local", "LAO_LOCAL_SELECTOR".into());
    let mut provider = toml_edit::Table::new();
    provider["name"] = toml_edit::value("LAO");
    provider["base_url"] = toml_edit::value(format!("http://127.0.0.1:{port}/oai"));
    provider["requires_openai_auth"] = toml_edit::value(true);
    provider["supports_websockets"] = toml_edit::value(false);
    provider["http_headers"] = toml_edit::Item::Value(headers.into());
    provider["env_http_headers"] = toml_edit::Item::Value(environment_headers.into());
    let mut providers = toml_edit::Table::new();
    providers["lao"] = toml_edit::Item::Table(provider);
    document["model_providers"] = toml_edit::Item::Table(providers);
    Ok(document.to_string().into_bytes())
}

pub fn verify(
    current: &[u8],
    installed: &[u8],
    original: Option<&[u8]>,
) -> Result<(), ConfigError> {
    let current = document(current)?;
    let installed = document(installed)?;
    let original = document(original.unwrap_or_default())?;
    let current_provider = provider(&current)?;
    let installed_provider = provider(&installed)?;
    let current_headers = current_provider["http_headers"]
        .as_inline_table()
        .ok_or(ConfigError::Invalid)?;
    let installed_headers = installed_provider["http_headers"]
        .as_inline_table()
        .ok_or(ConfigError::Invalid)?;
    let current_environment = current_provider["env_http_headers"]
        .as_inline_table()
        .ok_or(ConfigError::Invalid)?;
    let installed_environment = installed_provider["env_http_headers"]
        .as_inline_table()
        .ok_or(ConfigError::Invalid)?;
    if current["model_provider"].as_str() != installed["model_provider"].as_str()
        || current_provider.len() != 6
        || current_headers.len() != 1
        || current_environment.len() != 1
        || current_provider["name"].as_str() != installed_provider["name"].as_str()
        || current_provider["base_url"].as_str() != installed_provider["base_url"].as_str()
        || current_provider["requires_openai_auth"].as_bool()
            != installed_provider["requires_openai_auth"].as_bool()
        || current_provider["supports_websockets"].as_bool()
            != installed_provider["supports_websockets"].as_bool()
        || current_headers
            .get("X-LAO-Key")
            .and_then(toml_edit::Value::as_str)
            != installed_headers
                .get("X-LAO-Key")
                .and_then(toml_edit::Value::as_str)
        || current_environment
            .get("X-LAO-Local")
            .and_then(toml_edit::Value::as_str)
            != installed_environment
                .get("X-LAO-Local")
                .and_then(toml_edit::Value::as_str)
    {
        return Err(ConfigError::Conflict);
    }
    if !original.contains_key("model_catalog_json")
        && current["model_catalog_json"].as_str() != installed["model_catalog_json"].as_str()
    {
        return Err(ConfigError::Conflict);
    }
    if let Some(installed_worker) = worker(&installed)? {
        let current_worker = worker(&current)?.ok_or(ConfigError::Conflict)?;
        if current_worker.to_string() != installed_worker.to_string() {
            return Err(ConfigError::Conflict);
        }
    }
    if installed.contains_key("developer_instructions")
        && current["developer_instructions"].as_str()
            != installed["developer_instructions"].as_str()
    {
        return Err(ConfigError::Conflict);
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
    verify(current, installed, Some(original))?;
    let installed = document(installed)?;
    let original_document = document(original)?;
    let mut current = document(current)?;
    current.remove("model_provider");
    current.remove("model_providers");
    if !original_document.contains_key("model_catalog_json") {
        current.remove("model_catalog_json");
    }
    if worker(&installed)?.is_some() {
        let servers = current
            .get_mut("mcp_servers")
            .and_then(toml_edit::Item::as_table_mut)
            .ok_or(ConfigError::Conflict)?;
        servers.remove("lao");
        if servers.is_empty() && !original_document.contains_key("mcp_servers") {
            current.remove("mcp_servers");
        }
    }
    if installed.contains_key("developer_instructions")
        && !original_document.contains_key("developer_instructions")
    {
        current.remove("developer_instructions");
    }
    Ok(current.to_string().into_bytes())
}

fn document(bytes: &[u8]) -> Result<toml_edit::DocumentMut, ConfigError> {
    std::str::from_utf8(bytes)
        .map_err(|_| ConfigError::Invalid)?
        .parse()
        .map_err(|_| ConfigError::Invalid)
}

fn provider(document: &toml_edit::DocumentMut) -> Result<&toml_edit::Table, ConfigError> {
    let providers = document["model_providers"]
        .as_table()
        .ok_or(ConfigError::Invalid)?;
    if providers.len() != 1 {
        return Err(ConfigError::Conflict);
    }
    providers["lao"].as_table().ok_or(ConfigError::Invalid)
}

fn worker(document: &toml_edit::DocumentMut) -> Result<Option<&toml_edit::Item>, ConfigError> {
    let Some(servers) = document.get("mcp_servers") else {
        return Ok(None);
    };
    Ok(servers.as_table().ok_or(ConfigError::Invalid)?.get("lao"))
}

pub fn configure_worker(config: &[u8], command: &Path) -> Result<Vec<u8>, ConfigError> {
    if !command.is_absolute() {
        return Err(ConfigError::Invalid);
    }
    let text = std::str::from_utf8(config).map_err(|_| ConfigError::Invalid)?;
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| ConfigError::Invalid)?;
    if document.contains_key("developer_instructions")
        || document
            .get("mcp_servers")
            .and_then(toml_edit::Item::as_table)
            .is_some_and(|servers| servers.contains_key("lao"))
    {
        return Err(ConfigError::Conflict);
    }
    if !document.contains_key("mcp_servers") {
        document["mcp_servers"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    let command = command.to_str().ok_or(ConfigError::Invalid)?;
    let mut server = toml_edit::Table::new();
    server["command"] = toml_edit::value(command);
    server["args"] = toml_edit::value(toml_edit::Array::from_iter(["mcp"]));
    server["tool_timeout_sec"] = toml_edit::value(600);
    server["enabled_tools"] = toml_edit::value(toml_edit::Array::from_iter(["execute"]));
    let mut execute = toml_edit::Table::new();
    execute["approval_mode"] = toml_edit::value("approve");
    let mut tools = toml_edit::Table::new();
    tools["execute"] = toml_edit::Item::Table(execute);
    server["tools"] = toml_edit::Item::Table(tools);
    document["mcp_servers"]["lao"] = toml_edit::Item::Table(server);
    document["developer_instructions"] = toml_edit::value(DELEGATION_INSTRUCTIONS);
    Ok(document.to_string().into_bytes())
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
        "OPENAI_API_KEY" => Conflict::Auth("OPENAI_API_KEY"),
        "CODEX_API_KEY" => Conflict::Auth("CODEX_API_KEY"),
        "CODEX_ACCESS_TOKEN" => Conflict::Auth("CODEX_ACCESS_TOKEN"),
        "LAO_LOCAL_SELECTOR" => Conflict::Provider("LAO_LOCAL_SELECTOR"),
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
        assert_eq!(support("codex-cli 0.151.0\n"), Support::Observed);
        assert_eq!(support("codex-cli 0.152.0"), Support::Untested);
        assert_eq!(support("codex 0.151.0"), Support::Invalid);
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
                    "LAO_LOCAL_SELECTOR",
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
                Conflict::Provider("LAO_LOCAL_SELECTOR"),
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

        let original = b"model = \"gpt-5.4\"\n[mcp_servers.fixture]\ncommand = \"true\"\n";
        let configured = configure(
            Some(original),
            8765,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "/tmp/models.json",
        )
        .unwrap();
        let configured = String::from_utf8(configured).unwrap();
        let configured = configured.parse::<toml_edit::DocumentMut>().unwrap();
        assert_eq!(configured["model"].as_str(), Some("gpt-5.4"));
        assert_eq!(configured["model_provider"].as_str(), Some("lao"));
        assert_eq!(
            configured["model_catalog_json"].as_str(),
            Some("/tmp/models.json")
        );
        assert_eq!(
            configured["model_providers"]["lao"]["base_url"].as_str(),
            Some("http://127.0.0.1:8765/oai")
        );
        assert_eq!(
            configured["model_providers"]["lao"]["env_http_headers"]["X-LAO-Local"].as_str(),
            Some("LAO_LOCAL_SELECTOR")
        );
        assert_eq!(
            configured["mcp_servers"]["fixture"]["command"].as_str(),
            Some("true")
        );
        let configured =
            configure_worker(configured.to_string().as_bytes(), Path::new("/tmp/lao")).unwrap();
        let configured = String::from_utf8(configured)
            .unwrap()
            .parse::<toml_edit::DocumentMut>()
            .unwrap();
        assert_eq!(
            configured["mcp_servers"]["lao"]["command"].as_str(),
            Some("/tmp/lao")
        );
        assert_eq!(
            configured["mcp_servers"]["lao"]["enabled_tools"][0].as_str(),
            Some("execute")
        );
        assert_eq!(
            configured["mcp_servers"]["lao"]["tools"]["execute"]["approval_mode"].as_str(),
            Some("approve")
        );
        assert_eq!(
            configured["developer_instructions"].as_str(),
            Some(DELEGATION_INSTRUCTIONS)
        );
        assert_eq!(
            configure_worker(
                b"developer_instructions = \"keep mine\"\n",
                Path::new("/tmp/lao")
            ),
            Err(ConfigError::Conflict)
        );
        assert_eq!(
            configure(
                Some(b"model_provider = \"other\"\n"),
                8765,
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "/tmp/models.json",
            ),
            Err(ConfigError::Conflict)
        );
    }

    #[test]
    fn preserves_unrelated_edits_when_restoring_managed_settings() {
        let original = b"model = \"gpt-5.4\"\n[mcp_servers.fixture]\ncommand = \"true\"\n";
        let installed = configure(
            Some(original),
            8765,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "/tmp/models.json",
        )
        .unwrap();
        let installed = configure_worker(&installed, Path::new("/tmp/lao")).unwrap();
        let mut current = String::from_utf8(installed.clone()).unwrap();
        current.push_str("\n[projects.\"/tmp/new\"]\ntrust_level = \"trusted\"\n");

        verify(current.as_bytes(), &installed, Some(original)).unwrap();
        let restored =
            String::from_utf8(restore(current.as_bytes(), &installed, Some(original)).unwrap())
                .unwrap();
        assert!(restored.contains("model = \"gpt-5.4\""));
        assert!(restored.contains("[mcp_servers.fixture]"));
        assert!(restored.contains("[projects.\"/tmp/new\"]"));
        assert!(!restored.contains("[mcp_servers.lao]"));
        assert!(!restored.contains("developer_instructions"));
        assert!(!restored.contains("model_provider"));
        assert!(!restored.contains("model_catalog_json"));

        let changed = current.replace("127.0.0.1:8765", "127.0.0.1:9999");
        assert_eq!(
            restore(changed.as_bytes(), &installed, Some(original)),
            Err(ConfigError::Conflict)
        );
        assert_eq!(
            restore(&installed, &installed, Some(original)).unwrap(),
            original
        );
    }
}
