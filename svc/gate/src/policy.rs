use std::io;

use hyper::{
    HeaderMap, Method, Request, Uri,
    header::{
        ACCEPT, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderName, HeaderValue,
        PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER, TRANSFER_ENCODING, UPGRADE,
    },
};

const OAI: &str = "api.openai.com";
const CHATGPT: &str = "chatgpt.com";
const ANT: &str = "api.anthropic.com";
const LOCAL: &str = "127.0.0.1:10000";

type Err = io::Error;

#[derive(Clone)]
pub(super) struct Gate {
    pub host: HeaderValue,
    pub codex: [u8; 32],
    pub claude: [u8; 32],
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Route {
    OpenAi,
    ChatGpt,
    AnthropicBearer,
    AnthropicKey,
    Local,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Client {
    Codex,
    Claude,
}

#[derive(Clone, Copy)]
enum Op {
    Responses,
    Compact,
    Models,
    Messages,
    Count,
}

pub(super) struct Task<B> {
    request: Request<B>,
    client: Client,
    op: Op,
}

pub(super) struct Frozen<B> {
    target: Target,
    request: Request<B>,
}

impl<B> Frozen<B> {
    pub fn take(self) -> (Target, Request<B>) {
        (self.target, self.request)
    }
}

pub(super) struct Target {
    pub host: &'static str,
    path: &'static str,
    pub tls: bool,
}

impl<B> Task<B> {
    pub fn freeze(mut self, route: Route) -> Result<Frozen<B>, Err> {
        if native(route) && !valid_auth(self.client, route, self.request.headers()) {
            return Err(deny("route"));
        }
        clean(self.request.headers_mut(), native(route))?;
        let target = target(route, self.op)?;
        self.request
            .headers_mut()
            .insert(HOST, HeaderValue::from_static(target.host));
        *self.request.uri_mut() = Uri::from_static(target.path);
        Ok(Frozen {
            target,
            request: self.request,
        })
    }
}

pub(super) fn admit<B>(mut request: Request<B>, gate: &Gate) -> Result<Option<Task<B>>, Err> {
    if !one(request.headers(), HOST, |value| {
        value == gate.host.as_bytes()
    }) {
        return Err(deny("host"));
    }
    if request.headers().contains_key("origin")
        || request
            .headers()
            .contains_key("access-control-request-method")
        || request
            .headers()
            .contains_key("access-control-request-headers")
    {
        return Err(deny("cors"));
    }
    if request.headers().contains_key(TRANSFER_ENCODING)
        || request.headers().get_all(CONTENT_LENGTH).iter().count() > 1
        || request.headers().get(CONTENT_LENGTH).is_some_and(|value| {
            value
                .to_str()
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .is_none()
        })
    {
        return Err(deny("body"));
    }
    let (client, op, hello) = operation(&request)?;
    let caller = request.headers().get_all("x-lao-key");
    if hello {
        let body = request
            .headers()
            .get(CONTENT_LENGTH)
            .is_some_and(|value| value != "0");
        return if caller.iter().next().is_none()
            && !body
            && !request.headers().contains_key("authorization")
            && !request.headers().contains_key("x-api-key")
        {
            Ok(None)
        } else {
            Err(deny("auth"))
        };
    }
    let key = if client == Client::Codex {
        &gate.codex
    } else {
        &gate.claude
    };
    if !one(request.headers(), "x-lao-key", |value| {
        constant_time(value, key)
    }) {
        return Err(deny("caller"));
    }
    request.headers_mut().remove("x-lao-key");
    Ok(Some(Task {
        request,
        client,
        op,
    }))
}

fn operation<B>(request: &Request<B>) -> Result<(Client, Op, bool), Err> {
    if request.uri().scheme().is_some() || request.uri().authority().is_some() {
        return Err(deny("path"));
    }
    let path = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_default();
    Ok(match (request.method(), path) {
        (&Method::POST, "/oai/responses") => (Client::Codex, Op::Responses, false),
        (&Method::POST, "/oai/responses/compact") => (Client::Codex, Op::Compact, false),
        (&Method::GET, "/oai/models") => (Client::Codex, Op::Models, false),
        (&Method::HEAD, "/ant/api/hello") => (Client::Claude, Op::Messages, true),
        (&Method::POST, "/ant/v1/messages" | "/ant/v1/messages?beta=true") => {
            (Client::Claude, Op::Messages, false)
        }
        (&Method::POST, "/ant/v1/messages/count_tokens") => (Client::Claude, Op::Count, false),
        _ => return Err(deny("path")),
    })
}

fn valid_auth(client: Client, route: Route, headers: &HeaderMap) -> bool {
    let auth: Vec<_> = headers.get_all("authorization").iter().collect();
    let bearer = auth.len() == 1 && is_bearer(auth[0]);
    let key = headers.get_all("x-api-key").iter().count();
    let account = headers.get_all("chatgpt-account-id").iter().count();
    let anthropic = headers
        .keys()
        .any(|name| starts(name, "anthropic-") || starts(name, "x-claude-"));
    let openai = headers
        .keys()
        .any(|name| starts(name, "openai-") || starts(name, "x-openai-"));
    match (client, route) {
        (Client::Codex, Route::OpenAi) => bearer && key == 0 && account == 0 && !anthropic,
        (Client::Codex, Route::ChatGpt) => bearer && key == 0 && account <= 1 && !anthropic,
        (Client::Claude, Route::AnthropicBearer) => bearer && key == 0 && account == 0 && !openai,
        (Client::Claude, Route::AnthropicKey) => {
            auth.is_empty() && key == 1 && account == 0 && !openai
        }
        _ => false,
    }
}

fn target(route: Route, op: Op) -> Result<Target, Err> {
    let (host, path, tls) = match (route, op) {
        (Route::OpenAi, Op::Responses) => (OAI, "/v1/responses", true),
        (Route::OpenAi, Op::Compact) => (OAI, "/v1/responses/compact", true),
        (Route::OpenAi, Op::Models) => (OAI, "/v1/models", true),
        (Route::ChatGpt, Op::Responses) => (CHATGPT, "/backend-api/codex/responses", true),
        (Route::ChatGpt, Op::Compact) => (CHATGPT, "/backend-api/codex/responses/compact", true),
        (Route::ChatGpt, Op::Models) => (CHATGPT, "/backend-api/codex/models", true),
        (Route::AnthropicBearer | Route::AnthropicKey, Op::Messages) => {
            (ANT, "/v1/messages?beta=true", true)
        }
        (Route::AnthropicBearer | Route::AnthropicKey, Op::Count) => {
            (ANT, "/v1/messages/count_tokens", true)
        }
        (Route::Local, Op::Responses) => (LOCAL, "/v1/responses", false),
        (Route::Local, Op::Compact) => (LOCAL, "/v1/responses/compact", false),
        (Route::Local, Op::Models) => (LOCAL, "/v1/models", false),
        (Route::Local, Op::Messages) => (LOCAL, "/v1/messages", false),
        (Route::Local, Op::Count) => (LOCAL, "/v1/messages/count_tokens", false),
        _ => return Err(deny("route")),
    };
    Ok(Target { host, path, tls })
}

fn native(route: Route) -> bool {
    matches!(
        route,
        Route::OpenAi | Route::ChatGpt | Route::AnthropicBearer | Route::AnthropicKey
    )
}

fn clean(headers: &mut HeaderMap, native: bool) -> Result<(), Err> {
    if !native {
        let mut clean = HeaderMap::new();
        for name in [ACCEPT, CONTENT_TYPE] {
            for value in headers.get_all(&name) {
                clean.append(name.clone(), value.clone());
            }
        }
        *headers = clean;
        return Ok(());
    }
    strip_hop(headers)?;
    for name in [HOST, CONTENT_LENGTH] {
        headers.remove(name);
    }
    Ok(())
}

pub(super) fn strip_hop(headers: &mut HeaderMap) -> Result<(), Err> {
    let mut remove = vec![
        CONNECTION,
        PROXY_AUTHENTICATE,
        PROXY_AUTHORIZATION,
        TE,
        TRAILER,
        TRANSFER_ENCODING,
        UPGRADE,
    ];
    for value in headers.get_all(CONNECTION) {
        for name in value.as_bytes().split(|byte| *byte == b',') {
            let name = HeaderName::from_bytes(trim(name)).map_err(|_| deny("hop"))?;
            remove.push(name);
        }
    }
    for name in remove {
        headers.remove(name);
    }
    Ok(())
}

pub(super) fn clean_response(headers: &mut HeaderMap) -> Result<(), Err> {
    strip_hop(headers)?;
    for name in ["authorization", "x-api-key", "x-lao-key"] {
        headers.remove(name);
    }
    Ok(())
}

fn one(
    headers: &HeaderMap,
    name: impl hyper::header::AsHeaderName,
    accept: impl FnOnce(&[u8]) -> bool,
) -> bool {
    let mut values = headers.get_all(name).iter();
    matches!((values.next(), values.next()), (Some(value), None) if accept(value.as_bytes()))
}

fn starts(name: &HeaderName, prefix: &str) -> bool {
    name.as_str().starts_with(prefix)
}

fn is_bearer(value: &HeaderValue) -> bool {
    value.to_str().ok().is_some_and(|value| {
        let mut parts = value.split_ascii_whitespace();
        parts
            .next()
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("bearer"))
            && parts.next().is_some()
            && parts.next().is_none()
    })
}

fn constant_time(value: &[u8], expected: &[u8; 32]) -> bool {
    let mut different = value.len() ^ expected.len();
    for (index, byte) in expected.iter().enumerate() {
        different |= usize::from(value.get(index).copied().unwrap_or_default() ^ byte);
    }
    different == 0
}

fn trim(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn deny(name: &'static str) -> Err {
    io::Error::new(io::ErrorKind::PermissionDenied, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OAI_KEY: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
    const ANT_KEY: &str = "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD";

    #[test]
    fn ingress_is_exact_and_hello_is_inert() {
        assert!(matches!(
            admit(request(Method::HEAD, "/ant/api/hello", &[]), &gate()),
            Ok(None)
        ));
        for (method, path) in [
            (Method::POST, "/ant/api/hello"),
            (Method::HEAD, "/ant/api/hello?x=1"),
            (Method::POST, "/OAI/responses"),
            (Method::POST, "/oai/%72esponses"),
            (Method::POST, "//oai/responses"),
            (Method::POST, "http://lao.local/oai/responses"),
        ] {
            assert!(admit(request(method, path, &[]), &gate()).is_err());
        }
        for headers in [
            vec![("X-LAO-Key", ANT_KEY)],
            vec![("Authorization", "Bearer native")],
            vec![("Content-Length", "1")],
            vec![("Transfer-Encoding", "chunked")],
        ] {
            assert!(admit(request(Method::HEAD, "/ant/api/hello", &headers), &gate()).is_err());
        }
    }

    #[test]
    fn callers_and_browser_inputs_fail_closed() {
        for headers in [
            vec![],
            vec![("X-LAO-Key", "wrong")],
            vec![("X-LAO-Key", ANT_KEY)],
            vec![("X-LAO-Key", OAI_KEY), ("X-LAO-Key", OAI_KEY)],
            vec![("X-LAO-Key", OAI_KEY), ("Origin", "https://bad.invalid")],
        ] {
            assert!(admit(request(Method::POST, "/oai/responses", &headers), &gate()).is_err());
        }
        let mut bad_host = request(Method::POST, "/oai/responses", &[("X-LAO-Key", OAI_KEY)]);
        bad_host
            .headers_mut()
            .insert(HOST, HeaderValue::from_static("api.openai.com"));
        assert!(admit(bad_host, &gate()).is_err());
    }

    #[test]
    fn native_routes_keep_only_matching_credentials() {
        let codex = frozen(
            request(
                Method::POST,
                "/oai/responses",
                &[
                    ("X-LAO-Key", OAI_KEY),
                    ("Authorization", "Bearer native"),
                    ("X-Unknown", "keep"),
                    ("Connection", "X-Hop"),
                    ("X-Hop", "drop"),
                ],
            ),
            Route::OpenAi,
        );
        assert_eq!(codex.uri(), "/v1/responses");
        assert_eq!(codex.headers()[HOST], OAI);
        assert!(codex.headers().contains_key("authorization"));
        assert!(codex.headers().contains_key("x-unknown"));
        assert!(!codex.headers().contains_key("x-lao-key"));
        assert!(!codex.headers().contains_key("x-hop"));

        let account = request(
            Method::POST,
            "/oai/responses",
            &[
                ("X-LAO-Key", OAI_KEY),
                ("Authorization", "Bearer native"),
                ("ChatGPT-Account-Id", "account"),
            ],
        );
        assert!(task(account).freeze(Route::OpenAi).is_err());

        let claude = frozen(
            request(
                Method::POST,
                "/ant/v1/messages?beta=true",
                &[
                    ("X-LAO-Key", ANT_KEY),
                    ("X-Api-Key", "native-key"),
                    ("Anthropic-Version", "2023-06-01"),
                ],
            ),
            Route::AnthropicKey,
        );
        assert_eq!(claude.uri(), "/v1/messages?beta=true");
        assert_eq!(claude.headers()[HOST], ANT);
        assert!(claude.headers().contains_key("anthropic-version"));

        let mixed = request(
            Method::POST,
            "/ant/v1/messages",
            &[
                ("X-LAO-Key", ANT_KEY),
                ("Authorization", "Bearer native"),
                ("X-Api-Key", "native-key"),
            ],
        );
        assert!(task(mixed).freeze(Route::AnthropicBearer).is_err());
        for auth in ["Basic native", "Bearer", "Bearer native extra"] {
            let bad = request(
                Method::POST,
                "/ant/v1/messages",
                &[("X-LAO-Key", ANT_KEY), ("Authorization", auth)],
            );
            assert!(task(bad).freeze(Route::AnthropicBearer).is_err());
        }
        let cross = request(
            Method::POST,
            "/ant/v1/messages",
            &[
                ("X-LAO-Key", ANT_KEY),
                ("Authorization", "Bearer native"),
                ("X-OpenAI-Client", "cross-provider"),
            ],
        );
        assert!(task(cross).freeze(Route::AnthropicBearer).is_err());
    }

    #[test]
    fn local_route_rebuilds_headers() {
        let local = frozen(
            request(
                Method::POST,
                "/ant/v1/messages",
                &[
                    ("X-LAO-Key", ANT_KEY),
                    ("Authorization", "Bearer native-secret"),
                    ("Anthropic-Beta", "oauth-capability"),
                    ("X-Claude-Code-Session-Id", "session"),
                    ("Content-Type", "application/json"),
                    ("Accept", "text/event-stream"),
                    ("X-Unknown", "drop"),
                ],
            ),
            Route::Local,
        );
        assert_eq!(local.uri(), "/v1/messages");
        assert_eq!(local.headers()[HOST], LOCAL);
        assert_eq!(local.headers().len(), 3);
        assert!(local.headers().contains_key(CONTENT_TYPE));
        assert!(local.headers().contains_key(ACCEPT));
    }

    #[test]
    fn operation_and_route_fix_the_target() {
        for (method, path, route, host, target) in [
            (
                Method::POST,
                "/oai/responses/compact",
                Route::ChatGpt,
                CHATGPT,
                "/backend-api/codex/responses/compact",
            ),
            (Method::GET, "/oai/models", Route::OpenAi, OAI, "/v1/models"),
        ] {
            let ready = frozen(
                request(
                    method,
                    path,
                    &[("X-LAO-Key", OAI_KEY), ("Authorization", "Bearer native")],
                ),
                route,
            );
            assert_eq!(ready.headers()[HOST], host);
            assert_eq!(ready.uri(), target);
        }
    }

    fn request(method: Method, path: &str, headers: &[(&str, &str)]) -> Request<Vec<u8>> {
        let body = if method == Method::POST {
            b"{}".to_vec()
        } else {
            Vec::new()
        };
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(HOST, "lao.local")
            .body(body)
            .unwrap();
        for (name, value) in headers {
            request.headers_mut().append(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        request
    }

    fn gate() -> Gate {
        Gate {
            host: HeaderValue::from_static("lao.local"),
            codex: *OAI_KEY.as_bytes().first_chunk().unwrap(),
            claude: *ANT_KEY.as_bytes().first_chunk().unwrap(),
        }
    }

    fn task(request: Request<Vec<u8>>) -> Task<Vec<u8>> {
        admit(request, &gate()).unwrap().expect("task")
    }

    fn frozen(request: Request<Vec<u8>>, route: Route) -> Request<Vec<u8>> {
        task(request).freeze(route).unwrap().take().1
    }
}
