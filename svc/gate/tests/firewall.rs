use std::{
    cell::Cell,
    io,
    net::{SocketAddr, TcpListener},
};

const HOST: &str = "127.0.0.1:8765";
const OAI_KEY: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
const ANT_KEY: &str = "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD";
const RESP: &str = "/oai/responses";
const MSG: &str = "/ant/v1/messages";

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

#[derive(Clone, Copy)]
enum Route {
    OpenAi,
    ChatGpt,
    AnthropicBearer,
    AnthropicKey,
    Local,
    Other,
}

#[derive(Debug, Eq, PartialEq)]
enum Deny {
    Auth,
    Body,
    Cors,
    Host,
    Path,
    Route,
}

struct Head<'a> {
    method: &'a str,
    path: &'a str,
    host: &'a str,
    headers: &'a [(&'a str, &'a str)],
    length: usize,
}

struct Req<'a> {
    client: Client,
    op: Op,
    headers: &'a [(&'a str, &'a str)],
    body: Vec<u8>,
}

struct Frozen<'a>(Req<'a>, Route);

enum In<'a> {
    Hello,
    Req(Req<'a>),
}

struct Out<'a> {
    target: &'static str,
    headers: Vec<(&'a str, &'a str)>,
    body: Vec<u8>,
}

impl<'a> Req<'a> {
    fn freeze(self, route: Route) -> Frozen<'a> {
        Frozen(self, route)
    }
}

impl<'a> Frozen<'a> {
    fn send(self) -> Result<Out<'a>, Deny> {
        let Frozen(req, route) = self;
        let native = matches!(
            route,
            Route::OpenAi | Route::ChatGpt | Route::AnthropicBearer | Route::AnthropicKey
        );
        if native && !valid_auth(req.client, route, req.headers) {
            return Err(Deny::Route);
        }
        let headers = req
            .headers
            .iter()
            .copied()
            .filter(|(name, _)| {
                !is(name, "x-lao-key")
                    && !is(name, "host")
                    && !is(name, "content-length")
                    && (native || is(name, "accept") || is(name, "content-type"))
            })
            .collect();
        Ok(Out {
            target: target(route, req.op)?,
            headers,
            body: req.body,
        })
    }
}

fn admit<'a>(head: Head<'a>, read: impl FnOnce() -> Vec<u8>) -> Result<In<'a>, Deny> {
    if head.host != HOST {
        return Err(Deny::Host);
    }
    if head.headers.iter().any(|(name, _)| {
        is(name, "origin")
            || is(name, "access-control-request-method")
            || is(name, "access-control-request-headers")
    }) {
        return Err(Deny::Cors);
    }
    if has(head.headers, "transfer-encoding") || values(head.headers, "content-length").count() > 1
    {
        return Err(Deny::Body);
    }
    let (client, op, hello) = operation(head.method, head.path)?;
    let callers: Vec<_> = values(head.headers, "x-lao-key").collect();
    if hello {
        let secret = head
            .headers
            .iter()
            .any(|(name, _)| is(name, "authorization") || is(name, "x-api-key"));
        return if head.length == 0 && callers.is_empty() && !secret {
            Ok(In::Hello)
        } else {
            Err(Deny::Auth)
        };
    }
    let key = if client == Client::Codex {
        OAI_KEY
    } else {
        ANT_KEY
    };
    if callers.len() != 1 || !constant_time(callers[0], key) {
        return Err(Deny::Auth);
    }
    let body = read();
    if body.len() != head.length {
        return Err(Deny::Body);
    }
    Ok(In::Req(Req {
        client,
        op,
        headers: head.headers,
        body,
    }))
}

fn operation(method: &str, path: &str) -> Result<(Client, Op, bool), Deny> {
    Ok(match (method, path) {
        ("POST", "/oai/responses") => (Client::Codex, Op::Responses, false),
        ("POST", "/oai/responses/compact") => (Client::Codex, Op::Compact, false),
        ("GET", "/oai/models") => (Client::Codex, Op::Models, false),
        ("HEAD", "/ant/api/hello") => (Client::Claude, Op::Messages, true),
        ("POST", "/ant/v1/messages" | "/ant/v1/messages?beta=true") => {
            (Client::Claude, Op::Messages, false)
        }
        ("POST", "/ant/v1/messages/count_tokens") => (Client::Claude, Op::Count, false),
        _ => return Err(Deny::Path),
    })
}

fn valid_auth(client: Client, route: Route, headers: &[(&str, &str)]) -> bool {
    let auth: Vec<_> = values(headers, "authorization").collect();
    let bearer = auth.len() == 1 && is_bearer(auth[0]);
    let key = values(headers, "x-api-key").count();
    let account = values(headers, "chatgpt-account-id").count();
    let anthropic = headers
        .iter()
        .any(|(name, _)| starts(name, "anthropic-") || starts(name, "x-claude-"));
    let openai = headers
        .iter()
        .any(|(name, _)| starts(name, "openai-") || starts(name, "x-openai-"));
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

fn target(route: Route, op: Op) -> Result<&'static str, Deny> {
    Ok(match (route, op) {
        (Route::OpenAi, Op::Responses) => "https://api.openai.com/v1/responses",
        (Route::OpenAi, Op::Compact) => "https://api.openai.com/v1/responses/compact",
        (Route::OpenAi, Op::Models) => "https://api.openai.com/v1/models",
        (Route::ChatGpt, Op::Responses) => "https://chatgpt.com/backend-api/codex/responses",
        (Route::ChatGpt, Op::Compact) => "https://chatgpt.com/backend-api/codex/responses/compact",
        (Route::ChatGpt, Op::Models) => "https://chatgpt.com/backend-api/codex/models",
        (Route::AnthropicBearer | Route::AnthropicKey, Op::Messages) => {
            "https://api.anthropic.com/v1/messages?beta=true"
        }
        (Route::AnthropicBearer | Route::AnthropicKey, Op::Count) => {
            "https://api.anthropic.com/v1/messages/count_tokens"
        }
        (Route::Local, _) => "http://127.0.0.1:10000",
        (Route::Other, _) => "https://third.fixture.invalid",
        _ => return Err(Deny::Route),
    })
}

fn values<'a>(headers: &'a [(&str, &'a str)], name: &'a str) -> impl Iterator<Item = &'a str> {
    headers
        .iter()
        .filter(move |(header, _)| is(header, name))
        .map(|(_, value)| *value)
}

fn is(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn starts(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|value| value.eq_ignore_ascii_case(prefix))
}

fn is_bearer(value: &str) -> bool {
    value
        .split_once(' ')
        .is_some_and(|(scheme, token)| is(scheme, "bearer") && !token.is_empty())
}

fn constant_time(value: &str, expected: &str) -> bool {
    let mut different = value.len() ^ expected.len();
    for (index, byte) in expected.bytes().enumerate() {
        different |= usize::from(value.as_bytes().get(index).copied().unwrap_or_default() ^ byte);
    }
    different == 0
}

fn req(input: In<'_>) -> Req<'_> {
    match input {
        In::Req(req) => req,
        In::Hello => panic!("expected request"),
    }
}

fn has(headers: &[(&str, &str)], name: &str) -> bool {
    headers.iter().any(|(header, _)| is(header, name))
}

fn head<'a>(method: &'a str, path: &'a str, headers: &'a [(&'a str, &'a str)]) -> Head<'a> {
    Head {
        method,
        path,
        host: HOST,
        headers,
        length: usize::from(method == "POST") * 2,
    }
}

fn send<'a>(
    method: &'a str,
    path: &'a str,
    headers: &'a [(&'a str, &'a str)],
    route: Route,
) -> Result<Out<'a>, Deny> {
    let body = if method == "POST" {
        b"{}".to_vec()
    } else {
        Vec::new()
    };
    req(admit(head(method, path, headers), || body).unwrap())
        .freeze(route)
        .send()
}

fn activate(address: SocketAddr, write: impl FnOnce()) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(address)?;
    write();
    Ok(listener)
}

#[test]
fn caller_auth_precedes_body() {
    for caller in [None, Some("wrong"), Some(ANT_KEY)] {
        let headers = caller
            .map(|key| vec![("X-LAO-Key", key)])
            .unwrap_or_default();
        let reads = Cell::new(0);
        assert!(
            admit(head("POST", RESP, &headers), || {
                reads.set(reads.get() + 1);
                b"{}".to_vec()
            })
            .is_err()
        );
        assert_eq!(reads.get(), 0);
    }
    let duplicate = [("X-LAO-Key", OAI_KEY); 2];
    assert!(admit(head("POST", RESP, &duplicate), || panic!()).is_err());
}

#[test]
fn hello_is_the_only_unauthenticated_request() {
    assert!(matches!(
        admit(head("HEAD", "/ant/api/hello", &[]), || panic!()),
        Ok(In::Hello)
    ));
    for (method, path) in [("POST", "/ant/api/hello"), ("HEAD", "/ant/api/hello?x=1")] {
        assert!(admit(head(method, path, &[]), || panic!()).is_err());
    }
    let mut body = head("HEAD", "/ant/api/hello", &[]);
    body.length = 1;
    assert!(admit(body, || panic!()).is_err());
    for headers in [
        vec![("Transfer-Encoding", "chunked")],
        vec![("Content-Length", "0"), ("Content-Length", "0")],
    ] {
        assert!(admit(head("HEAD", "/ant/api/hello", &headers), || panic!()).is_err());
    }
}

#[test]
fn cloud_keeps_native_auth_and_other_routes_are_clean() {
    let codex = [
        ("X-LAO-Key", OAI_KEY),
        ("Authorization", "Bearer native"),
        ("Content-Type", "application/json"),
        ("X-Unknown", "keep"),
    ];
    let cloud = send("POST", RESP, &codex, Route::OpenAi).unwrap();
    assert_eq!(cloud.target, "https://api.openai.com/v1/responses");
    assert!(has(&cloud.headers, "authorization"));
    assert!(has(&cloud.headers, "x-unknown"));
    assert!(!has(&cloud.headers, "x-lao-key"));
    assert_eq!(cloud.body, b"{}");

    let ant_cloud = [
        ("X-LAO-Key", ANT_KEY),
        ("Authorization", "Bearer native"),
        ("Anthropic-Beta", "oauth-capability"),
    ];
    let ant_cloud = send("POST", MSG, &ant_cloud, Route::AnthropicBearer).unwrap();
    assert!(has(&ant_cloud.headers, "anthropic-beta"));

    for route in [Route::Local, Route::Other] {
        let claude = [
            ("X-LAO-Key", ANT_KEY),
            ("Authorization", "Bearer native"),
            ("Anthropic-Beta", "oauth-capability"),
            ("X-Claude-Code-Session-Id", "session"),
            ("Content-Type", "application/json"),
            ("Accept", "text/event-stream"),
            ("X-Unknown", "drop"),
        ];
        let clean = send("POST", MSG, &claude, route).unwrap();
        assert_eq!(clean.headers.len(), 2);
        assert!(has(&clean.headers, "content-type"));
        assert!(has(&clean.headers, "accept"));
    }
}

#[test]
fn malformed_ingress_and_auth_confusion_fail_closed() {
    let key = [("X-LAO-Key", OAI_KEY)];
    for host in ["localhost:8765", "127.0.0.2:8765", "api.openai.com"] {
        let mut bad = head("POST", RESP, &key);
        bad.host = host;
        assert_eq!(admit(bad, Vec::new).err(), Some(Deny::Host));
    }
    for path in [
        "/OAI/responses",
        "/oai/%72esponses",
        "//oai/responses",
        "http://127.0.0.1:8765/oai/responses",
    ] {
        assert_eq!(
            admit(head("POST", path, &key), Vec::new).err(),
            Some(Deny::Path)
        );
    }
    let cors = [("X-LAO-Key", OAI_KEY), ("Origin", "https://bad.invalid")];
    assert_eq!(
        admit(head("POST", RESP, &cors), Vec::new).err(),
        Some(Deny::Cors)
    );
    let mixed = [
        ("X-LAO-Key", ANT_KEY),
        ("Authorization", "Bearer native"),
        ("X-Api-Key", "native-key"),
    ];
    assert!(matches!(
        send("POST", MSG, &mixed, Route::AnthropicBearer),
        Err(Deny::Route)
    ));
    for auth in ["Basic native", "Bearer"] {
        let bad = [("X-LAO-Key", ANT_KEY), ("Authorization", auth)];
        assert!(send("POST", MSG, &bad, Route::AnthropicBearer).is_err());
    }
    let cross = [
        ("X-LAO-Key", ANT_KEY),
        ("Authorization", "Bearer native"),
        ("ChatGPT-Account-Id", "account"),
    ];
    assert!(send("POST", MSG, &cross, Route::AnthropicBearer).is_err());
}

#[test]
fn listener_precedes_config_and_outlives_a_worker_handle() {
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = occupied.local_addr().unwrap();
    let wrote = Cell::new(false);
    assert!(activate(address, || wrote.set(true)).is_err());
    assert!(!wrote.get());
    drop(occupied);

    let held = activate(address, || wrote.set(true)).unwrap();
    assert!(wrote.get());
    drop(held.try_clone().unwrap());
    assert!(TcpListener::bind(address).is_err());
    drop(held);
    assert!(TcpListener::bind(address).is_ok());
}

#[test]
fn supported_native_targets_are_exact() {
    let codex = [("X-LAO-Key", OAI_KEY), ("Authorization", "Bearer native")];
    for (method, path, route, target) in [
        (
            "POST",
            "/oai/responses/compact",
            Route::ChatGpt,
            "https://chatgpt.com/backend-api/codex/responses/compact",
        ),
        (
            "GET",
            "/oai/models",
            Route::OpenAi,
            "https://api.openai.com/v1/models",
        ),
    ] {
        assert_eq!(send(method, path, &codex, route).unwrap().target, target);
    }

    let claude = [("X-LAO-Key", ANT_KEY), ("X-Api-Key", "native-key")];
    assert_eq!(
        send(
            "POST",
            "/ant/v1/messages/count_tokens",
            &claude,
            Route::AnthropicKey,
        )
        .unwrap()
        .target,
        "https://api.anthropic.com/v1/messages/count_tokens"
    );
}
