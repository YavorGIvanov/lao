use std::{borrow::Cow, io, sync::Arc};

use hyper::{
    HeaderMap, Method, Request, Uri,
    header::{
        ACCEPT, AUTHORIZATION, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderName,
        HeaderValue, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER, TRANSFER_ENCODING,
        UPGRADE,
    },
};
use lao_route_api::{Client, Context, Decision, Op};
use lao_run_api::{Endpoint, Local};

const OAI: &str = "api.openai.com";
const CHATGPT: &str = "chatgpt.com";
const ANT: &str = "api.anthropic.com";
type Err = io::Error;

#[derive(Clone)]
pub(super) struct Gate {
    pub host: HeaderValue,
    pub codex: [u8; 64],
    pub codex_cloud: Cloud,
    pub claude: [u8; 64],
    pub claude_cloud: Cloud,
    pub worker: [u8; 64],
    pub local: Option<Arc<dyn Local>>,
}

#[derive(Clone, Copy)]
pub(super) enum Cloud {
    OpenAi,
    ChatGpt,
    AnthropicBearer,
    AnthropicKey,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Ingress {
    Hello,
    Plain,
    Beta,
}

#[derive(Clone, Copy)]
enum Route {
    Cloud(Cloud),
    Local,
}

pub(super) struct Task<B> {
    pub request: Request<B>,
    pub client: Client,
    pub op: Op,
    pub beta: bool,
    pub canary: bool,
    pub automatic: bool,
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
    pub host: Cow<'static, str>,
    path: &'static str,
    pub tls: bool,
}

impl<B> Task<B> {
    pub fn context(&self) -> Context {
        if self.canary {
            Context::canary(self.client, self.op)
        } else {
            Context::new(self.client, self.op)
        }
    }

    pub fn validate_auth(&self, gate: &Gate) -> Result<(), Err> {
        if self.client == Client::Worker {
            return valid_auth(self.client, Route::Local, self.request.headers())
                .then_some(())
                .ok_or_else(|| deny("route"));
        }
        let cloud = Route::Cloud(match self.client {
            Client::Codex => gate.codex_cloud,
            Client::Claude => gate.claude_cloud,
            Client::Worker => unreachable!(),
        });
        valid_auth(self.client, cloud, self.request.headers())
            .then_some(())
            .ok_or_else(|| deny("route"))
    }

    pub fn freeze(
        mut self,
        decision: Decision,
        gate: &Gate,
        local: Option<&Endpoint>,
    ) -> Result<Frozen<B>, Err> {
        let route = match (decision, self.canary, self.automatic) {
            (Decision::Local, true, _) | (Decision::Local, false, true) => Route::Local,
            (Decision::Cloud, false, _) => Route::Cloud(match self.client {
                Client::Codex => gate.codex_cloud,
                Client::Claude => gate.claude_cloud,
                Client::Worker => return Err(deny("route")),
            }),
            _ => return Err(deny("route")),
        };
        let cloud = native(route);
        if cloud && !valid_auth(self.client, route, self.request.headers()) {
            return Err(deny("route"));
        }
        clean(self.request.headers_mut(), cloud)?;
        if !cloud {
            let endpoint = local.ok_or_else(|| deny("local"))?;
            if !endpoint.addr().ip().is_loopback() {
                return Err(deny("local"));
            }
            self.request.headers_mut().insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", endpoint.bearer()))
                    .map_err(|_| deny("local"))?,
            );
        }
        let target = target(route, self.op, self.beta, local)?;
        self.request.headers_mut().insert(
            HOST,
            HeaderValue::from_str(&target.host).map_err(|_| deny("host"))?,
        );
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
    let (client, op, ingress) = operation(&request)?;
    let caller = request.headers().get_all("x-lao-key");
    if ingress == Ingress::Hello {
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
    let key = match client {
        Client::Codex => &gate.codex,
        Client::Claude => &gate.claude,
        Client::Worker => &gate.worker,
    };
    if !one(request.headers(), "x-lao-key", |value| {
        constant_time(value, key)
    }) {
        return Err(deny("caller"));
    }
    request.headers_mut().remove("x-lao-key");
    let mut routes = request.headers().get_all("x-lao-local").iter();
    let canary = match (client, routes.next(), routes.next()) {
        (Client::Worker, None, None)
            if op == Op::Chat
                && request
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .is_some_and(|length| (1..=2 * 1024 * 1024).contains(&length))
                && one(request.headers(), CONTENT_TYPE, |value| {
                    value == b"application/json"
                }) =>
        {
            true
        }
        (_, None, None) => false,
        (_, Some(value), None)
            if value.as_bytes() == b"canary"
                && matches!(op, Op::Responses | Op::Messages)
                && request
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .is_some_and(|length| (1..=2 * 1024 * 1024).contains(&length))
                && one(request.headers(), CONTENT_TYPE, |value| {
                    value == b"application/json"
                }) =>
        {
            true
        }
        _ => return Err(deny("route")),
    };
    request.headers_mut().remove("x-lao-local");
    Ok(Some(Task {
        request,
        client,
        op,
        beta: ingress == Ingress::Beta,
        canary,
        automatic: false,
    }))
}

fn operation<B>(request: &Request<B>) -> Result<(Client, Op, Ingress), Err> {
    if request.uri().scheme().is_some() || request.uri().authority().is_some() {
        return Err(deny("path"));
    }
    let path = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_default();
    Ok(match (request.method(), path) {
        (&Method::POST, "/oai/responses") => (Client::Codex, Op::Responses, Ingress::Plain),
        (&Method::POST, "/oai/responses/compact") => (Client::Codex, Op::Compact, Ingress::Plain),
        (&Method::GET, "/oai/models") => (Client::Codex, Op::Models, Ingress::Plain),
        (&Method::HEAD, "/ant/api/hello") => (Client::Claude, Op::Messages, Ingress::Hello),
        (&Method::POST, "/ant/v1/messages") => (Client::Claude, Op::Messages, Ingress::Plain),
        (&Method::POST, "/ant/v1/messages?beta=true") => {
            (Client::Claude, Op::Messages, Ingress::Beta)
        }
        (&Method::POST, "/ant/v1/messages/count_tokens") => {
            (Client::Claude, Op::Count, Ingress::Plain)
        }
        (&Method::POST, "/wrk/v1/chat/completions") => (Client::Worker, Op::Chat, Ingress::Plain),
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
        (Client::Codex, Route::Cloud(Cloud::OpenAi)) => {
            bearer && key == 0 && account == 0 && !anthropic
        }
        (Client::Codex, Route::Cloud(Cloud::ChatGpt)) => {
            bearer && key == 0 && account <= 1 && !anthropic
        }
        (Client::Claude, Route::Cloud(Cloud::AnthropicBearer)) => {
            bearer && key == 0 && account == 0 && !openai
        }
        (Client::Claude, Route::Cloud(Cloud::AnthropicKey)) => {
            auth.is_empty() && key == 1 && account == 0 && !openai
        }
        (Client::Worker, Route::Local) => bearer && key == 0 && account == 0 && !anthropic,
        _ => false,
    }
}

fn target(route: Route, op: Op, beta: bool, local: Option<&Endpoint>) -> Result<Target, Err> {
    let (host, path, tls): (Cow<'static, str>, _, _) = match (route, op) {
        (Route::Cloud(Cloud::OpenAi), Op::Responses) => (OAI.into(), "/v1/responses", true),
        (Route::Cloud(Cloud::OpenAi), Op::Compact) => (OAI.into(), "/v1/responses/compact", true),
        (Route::Cloud(Cloud::OpenAi), Op::Models) => (OAI.into(), "/v1/models", true),
        (Route::Cloud(Cloud::ChatGpt), Op::Responses) => {
            (CHATGPT.into(), "/backend-api/codex/responses", true)
        }
        (Route::Cloud(Cloud::ChatGpt), Op::Compact) => {
            (CHATGPT.into(), "/backend-api/codex/responses/compact", true)
        }
        (Route::Cloud(Cloud::ChatGpt), Op::Models) => {
            (CHATGPT.into(), "/backend-api/codex/models", true)
        }
        (Route::Cloud(Cloud::AnthropicBearer | Cloud::AnthropicKey), Op::Messages) => (
            ANT.into(),
            if beta {
                "/v1/messages?beta=true"
            } else {
                "/v1/messages"
            },
            true,
        ),
        (Route::Cloud(Cloud::AnthropicBearer | Cloud::AnthropicKey), Op::Count) => {
            (ANT.into(), "/v1/messages/count_tokens", true)
        }
        (Route::Local, Op::Responses) => (
            local
                .ok_or_else(|| deny("local"))?
                .addr()
                .to_string()
                .into(),
            "/v1/responses",
            false,
        ),
        (Route::Local, Op::Messages) => (
            local
                .ok_or_else(|| deny("local"))?
                .addr()
                .to_string()
                .into(),
            "/v1/messages",
            false,
        ),
        (Route::Local, Op::Chat) => (
            local
                .ok_or_else(|| deny("local"))?
                .addr()
                .to_string()
                .into(),
            "/v1/chat/completions",
            false,
        ),
        _ => return Err(deny("route")),
    };
    Ok(Target { host, path, tls })
}

fn native(route: Route) -> bool {
    matches!(route, Route::Cloud(_))
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

fn constant_time(value: &[u8], expected: &[u8; 64]) -> bool {
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

    const OAI_KEY: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
    const ANT_KEY: &str = "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD";
    const WORKER_KEY: &str = "EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE";

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
        for headers in [
            vec![("X-LAO-Key", OAI_KEY), ("X-LAO-Local", "wrong")],
            vec![
                ("X-LAO-Key", OAI_KEY),
                ("X-LAO-Local", "canary"),
                ("X-LAO-Local", "canary"),
            ],
        ] {
            assert!(admit(request(Method::POST, "/oai/responses", &headers), &gate()).is_err());
        }
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
            Decision::Cloud,
            gate(),
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
        assert!(
            task(account)
                .freeze(Decision::Cloud, &gate(), None)
                .is_err()
        );

        let mut key_gate = gate();
        key_gate.claude_cloud = Cloud::AnthropicKey;
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
            Decision::Cloud,
            key_gate,
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
        assert!(task(mixed).freeze(Decision::Cloud, &gate(), None).is_err());
        for auth in ["Basic native", "Bearer", "Bearer native extra"] {
            let bad = request(
                Method::POST,
                "/ant/v1/messages",
                &[("X-LAO-Key", ANT_KEY), ("Authorization", auth)],
            );
            assert!(task(bad).freeze(Decision::Cloud, &gate(), None).is_err());
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
        assert!(task(cross).freeze(Decision::Cloud, &gate(), None).is_err());
    }

    #[test]
    fn local_route_rebuilds_headers() {
        let local = frozen(
            request(
                Method::POST,
                "/ant/v1/messages",
                &[
                    ("X-LAO-Key", ANT_KEY),
                    ("X-LAO-Local", "canary"),
                    ("Authorization", "Bearer native-secret"),
                    ("Anthropic-Beta", "oauth-capability"),
                    ("X-Claude-Code-Session-Id", "session"),
                    ("Content-Type", "application/json"),
                    ("Content-Length", "2"),
                    ("Accept", "text/event-stream"),
                    ("X-Unknown", "drop"),
                ],
            ),
            Decision::Local,
            gate(),
        );
        assert_eq!(local.uri(), "/v1/messages");
        assert_eq!(local.headers()[HOST], "127.0.0.1:10000");
        assert_eq!(local.headers().len(), 4);
        assert!(local.headers().contains_key(CONTENT_TYPE));
        assert!(local.headers().contains_key(ACCEPT));
        assert_eq!(local.headers()[AUTHORIZATION], "Bearer runtime");
    }

    #[test]
    fn operation_and_route_fix_the_target() {
        for (method, path, cloud, host, target) in [
            (
                Method::POST,
                "/oai/responses/compact",
                Cloud::ChatGpt,
                CHATGPT,
                "/backend-api/codex/responses/compact",
            ),
            (Method::GET, "/oai/models", Cloud::OpenAi, OAI, "/v1/models"),
        ] {
            let mut configured = gate();
            configured.codex_cloud = cloud;
            let ready = frozen(
                request(
                    method,
                    path,
                    &[("X-LAO-Key", OAI_KEY), ("Authorization", "Bearer native")],
                ),
                Decision::Cloud,
                configured,
            );
            assert_eq!(ready.headers()[HOST], host);
            assert_eq!(ready.uri(), target);
        }

        let mut codex = gate();
        codex.codex_cloud = Cloud::AnthropicBearer;
        assert!(
            task(request(
                Method::POST,
                "/oai/responses",
                &[("X-LAO-Key", OAI_KEY), ("Authorization", "Bearer native")],
            ))
            .freeze(Decision::Cloud, &codex, None)
            .is_err()
        );

        let mut claude = gate();
        claude.claude_cloud = Cloud::OpenAi;
        assert!(
            task(request(
                Method::POST,
                "/ant/v1/messages",
                &[("X-LAO-Key", ANT_KEY), ("Authorization", "Bearer native")],
            ))
            .freeze(Decision::Cloud, &claude, None)
            .is_err()
        );
    }

    #[test]
    fn anthropic_query_is_never_rewritten() {
        for (path, upstream) in [
            ("/ant/v1/messages", "/v1/messages"),
            ("/ant/v1/messages?beta=true", "/v1/messages?beta=true"),
        ] {
            let ready = frozen(
                request(
                    Method::POST,
                    path,
                    &[("X-LAO-Key", ANT_KEY), ("Authorization", "Bearer native")],
                ),
                Decision::Cloud,
                gate(),
            );
            assert_eq!(ready.headers()[HOST], ANT);
            assert_eq!(ready.uri(), upstream);
        }
    }

    #[test]
    fn worker_is_local_only_and_credential_clean() {
        let mut gate = gate();
        gate.worker = *WORKER_KEY.as_bytes().first_chunk().unwrap();
        let task = admit(
            request(
                Method::POST,
                "/wrk/v1/chat/completions",
                &[
                    ("X-LAO-Key", WORKER_KEY),
                    ("Authorization", "Bearer local"),
                    ("Content-Type", "application/json"),
                    ("Content-Length", "2"),
                ],
            ),
            &gate,
        )
        .unwrap()
        .unwrap();
        task.validate_auth(&gate).unwrap();
        let frozen = task
            .freeze(Decision::Local, &gate, Some(&endpoint()))
            .unwrap()
            .take()
            .1;
        assert_eq!(frozen.uri(), "/v1/chat/completions");
        assert_eq!(frozen.headers()[AUTHORIZATION], "Bearer runtime");
        assert!(!frozen.headers().contains_key("x-lao-key"));

        let cloud = admit(
            request(
                Method::POST,
                "/wrk/v1/chat/completions",
                &[
                    ("X-LAO-Key", WORKER_KEY),
                    ("Authorization", "Bearer local"),
                    ("Content-Type", "application/json"),
                    ("Content-Length", "2"),
                ],
            ),
            &gate,
        )
        .unwrap()
        .unwrap();
        assert!(cloud.freeze(Decision::Cloud, &gate, None).is_err());
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
            codex_cloud: Cloud::OpenAi,
            claude: *ANT_KEY.as_bytes().first_chunk().unwrap(),
            claude_cloud: Cloud::AnthropicBearer,
            worker: *WORKER_KEY.as_bytes().first_chunk().unwrap(),
            local: None,
        }
    }

    fn endpoint() -> Endpoint {
        Endpoint::new("127.0.0.1:10000".parse().unwrap(), "runtime")
    }

    fn task(request: Request<Vec<u8>>) -> Task<Vec<u8>> {
        admit(request, &gate()).unwrap().expect("task")
    }

    fn frozen(request: Request<Vec<u8>>, decision: Decision, gate: Gate) -> Request<Vec<u8>> {
        task(request)
            .freeze(decision, &gate, Some(&endpoint()))
            .unwrap()
            .take()
            .1
    }
}
