use std::{
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener as StdListener},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full, combinators::UnsyncBoxBody};
use hyper::{
    Request, Response,
    body::Incoming,
    client::conn::http1 as client,
    header::{CONTENT_LENGTH, HeaderValue},
    server::conn::http1 as server,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use lao_route_api::{Client, Decision, Policy};
use lao_run_api::{Endpoint, Local};
use rustls::{ClientConfig, pki_types::ServerName};
use rustls_platform_verifier::BuilderVerifierExt;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream, lookup_host},
    time::timeout,
};
use tokio_rustls::TlsConnector;

use crate::CodexCloud;
use crate::policy::{Cloud, Gate, Target, Task, admit, clean_response};

type Err = Box<dyn Error + Send + Sync>;
type Body = UnsyncBoxBody<Bytes, Err>;
const WAIT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct Plan {
    gate: Gate,
    policy: Arc<dyn Policy>,
    #[cfg(test)]
    fixture: Option<SocketAddr>,
    #[cfg(test)]
    status: Option<Arc<std::sync::atomic::AtomicU16>>,
}

async fn serve(stream: TcpStream, plan: Plan) -> Result<(), Err> {
    let service = service_fn(move |request| send(request, plan.clone()));
    server::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await?;
    Ok(())
}

pub(super) async fn configured(
    listener: StdListener,
    policy: impl Policy + 'static,
    local: Option<Arc<dyn Local>>,
    codex: [u8; 64],
    claude: [u8; 64],
    worker: [u8; 64],
    codex_cloud: CodexCloud,
) -> Result<(), Err> {
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let plan = Plan {
        gate: Gate {
            host: address.to_string().parse()?,
            codex,
            codex_cloud: match codex_cloud {
                CodexCloud::Api => Cloud::OpenAi,
                CodexCloud::ChatGpt => Cloud::ChatGpt,
            },
            claude,
            claude_cloud: Cloud::AnthropicBearer,
            worker,
            local,
        },
        policy: Arc::new(policy),
        #[cfg(test)]
        fixture: None,
        #[cfg(test)]
        status: None,
    };
    let listener = TcpListener::from_std(listener)?;
    let mut failures = 0;
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                failures = 0;
                let plan = plan.clone();
                tokio::spawn(async move {
                    let _ = serve(stream, plan).await;
                });
            }
            Err(_) if failures < 16 => failures += 1,
            Err(error) => return Err(error.into()),
        }
    }
}

async fn send(request: Request<Incoming>, plan: Plan) -> Result<Response<Body>, Err> {
    let request = request.map(|body| body.map_err(|error| -> Err { error.into() }).boxed_unsync());
    let Some(task) = admit(request, &plan.gate)? else {
        return Ok(hello());
    };
    task.validate_auth(&plan.gate)?;
    let task = if task.client == Client::Worker {
        prepare_worker(task).await?
    } else {
        task
    };
    let (mut task, query, local_body) = if plan.policy.requires_query() && !task.canary {
        inspect(task).await?
    } else {
        (task, None, None)
    };
    let mut decision = match query {
        Some(query) => decide(plan.policy.clone(), task.context(), query).await,
        None => plan.policy.decide(task.context()),
    };
    let mut endpoint = None;
    let mut sender = None;
    if decision == Decision::Local {
        let local_body = if task.canary {
            None
        } else {
            match local_body {
                Some(bytes) => rebuild(bytes, task.client).await,
                None => None,
            }
        };
        let resolved = resolve(&plan.gate.local).await;
        let connection = match &resolved {
            Ok(endpoint) => {
                let address = endpoint.addr();
                match local_addr(address).await {
                    Ok(stream) => handshake(stream).await,
                    Err(error) => Err(error),
                }
            }
            Err(_) => Err(deny("local")),
        };
        match (task.canary, local_body, resolved, connection) {
            (_, _, Ok(ready), Ok(connected)) if task.canary => {
                endpoint = Some(ready);
                sender = Some(connected);
            }
            (false, Some(body), Ok(ready), Ok(connected)) => {
                task.automatic = true;
                task.request.headers_mut().insert(
                    CONTENT_LENGTH,
                    HeaderValue::from_str(&body.len().to_string())?,
                );
                *task.request.body_mut() = full(body);
                endpoint = Some(ready);
                sender = Some(connected);
            }
            (true, _, _, _) => return Err(deny("local")),
            _ => decision = Decision::Cloud,
        }
    }
    let (target, request) = task
        .freeze(decision, &plan.gate, endpoint.as_deref())?
        .take();
    #[cfg(test)]
    if let Some(fixture) = plan.fixture {
        let _ = target;
        return match sender {
            Some(sender) => send_ready(request, sender, endpoint).await,
            None => relay(request, TcpStream::connect(fixture).await?, endpoint).await,
        };
    }
    if target.tls {
        let response = relay(request, native(&target).await?, None).await;
        #[cfg(test)]
        if let (Ok(response), Some(status)) = (&response, plan.status) {
            status.store(
                response.status().as_u16(),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        response
    } else {
        send_ready(request, sender.ok_or_else(|| deny("local"))?, endpoint).await
    }
}

async fn prepare_worker(task: Task<Body>) -> Result<Task<Body>, Err> {
    let Task {
        mut request,
        client,
        op,
        beta,
        canary,
        automatic,
    } = task;
    let (parts, body) = request.into_parts();
    let bytes = body.collect().await?.to_bytes();
    let bytes = tokio::task::spawn_blocking(move || worker_body(&bytes))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| deny("worker body"))?;
    request = Request::from_parts(parts, full(bytes.clone()));
    request.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())?,
    );
    Ok(Task {
        request,
        client,
        op,
        beta,
        canary,
        automatic,
    })
}

fn worker_body(bytes: &[u8]) -> Option<Bytes> {
    let mut value = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    let root = value.as_object_mut()?;
    let tools = root.get("tools").and_then(serde_json::Value::as_array)?;
    if tools.is_empty() {
        return None;
    }
    let messages = root.get("messages").and_then(serde_json::Value::as_array)?;
    let edited = messages
        .iter()
        .filter_map(|message| message.get("tool_calls")?.as_array())
        .flatten()
        .filter_map(|call| call.pointer("/function/name")?.as_str())
        .any(|name| matches!(name, "edit" | "write"));
    root.insert(
        "tool_choice".into(),
        serde_json::Value::String(if edited { "auto" } else { "required" }.into()),
    );
    root.insert("parallel_tool_calls".into(), serde_json::Value::Bool(false));
    serde_json::to_vec(&value).ok().map(Bytes::from)
}

async fn decide(
    policy: Arc<dyn Policy>,
    context: lao_route_api::Context,
    query: String,
) -> Decision {
    match timeout(
        WAIT,
        tokio::task::spawn_blocking(move || policy.decide_query(context, &query)),
    )
    .await
    {
        Ok(Ok(decision)) => decision,
        _ => Decision::Cloud,
    }
}

async fn inspect(task: Task<Body>) -> Result<(Task<Body>, Option<String>, Option<Bytes>), Err> {
    let length = task
        .request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok());
    let content_type = task.request.headers().get_all(hyper::header::CONTENT_TYPE);
    let json = content_type.iter().count() == 1
        && content_type
            .iter()
            .next()
            .is_some_and(|value| value == "application/json");
    if !json
        || !matches!(
            task.op,
            lao_route_api::Op::Responses | lao_route_api::Op::Messages
        )
        || !length.is_some_and(|length| (1..=2 * 1024 * 1024).contains(&length))
    {
        return Ok((task, None, None));
    }
    let Task {
        request,
        client,
        op,
        beta,
        canary,
        automatic,
    } = task;
    let (parts, body) = request.into_parts();
    let bytes = body.collect().await?.to_bytes();
    let parsed = bytes.clone();
    let query = tokio::task::spawn_blocking(move || query(&parsed, client))
        .await
        .ok()
        .flatten();
    let local = query.as_ref().map(|_| bytes.clone());
    let request = Request::from_parts(parts, full(bytes));
    Ok((
        Task {
            request,
            client,
            op,
            beta,
            canary,
            automatic,
        },
        query,
        local,
    ))
}

async fn rebuild(bytes: Bytes, client: lao_route_api::Client) -> Option<Bytes> {
    tokio::task::spawn_blocking(move || local_body(&bytes, client))
        .await
        .ok()
        .flatten()
}

fn query(bytes: &[u8], client: lao_route_api::Client) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    value
        .get("model")
        .is_some_and(serde_json::Value::is_string)
        .then_some(())?;
    user_text(&value, client).and_then(bound)
}

fn local_body(bytes: &[u8], client: lao_route_api::Client) -> Option<Bytes> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    let query = user_text(&value, client).and_then(bound)?;
    value
        .get("model")
        .is_some_and(serde_json::Value::is_string)
        .then_some(())?;
    let local = match client {
        lao_route_api::Client::Codex => serde_json::json!({
            "model": "lao-local",
            "input": query,
            "max_output_tokens": value
                .get("max_output_tokens")
                .and_then(serde_json::Value::as_u64)
                .filter(|tokens| *tokens > 0)
                .unwrap_or(512)
                .min(512),
            "stream": true,
            "tools": [],
            "tool_choice": "none"
        }),
        lao_route_api::Client::Claude => serde_json::json!({
            "model": "lao-local",
            "messages": [{ "role": "user", "content": query }],
            "max_tokens": value
                .get("max_tokens")
                .and_then(serde_json::Value::as_u64)
                .filter(|tokens| *tokens > 0)
                .unwrap_or(512)
                .min(512),
            "stream": true,
            "tools": []
        }),
        lao_route_api::Client::Worker => return None,
    };
    serde_json::to_vec(&local).ok().map(Bytes::from)
}

fn user_text(value: &serde_json::Value, client: lao_route_api::Client) -> Option<String> {
    match client {
        lao_route_api::Client::Codex => {
            if !value
                .get("previous_response_id")
                .is_none_or(serde_json::Value::is_null)
            {
                return None;
            }
            let input = value.get("input")?;
            if let Some(text) = input.as_str() {
                return Some(text.to_owned());
            }
            let input = input.as_array()?;
            if input.is_empty()
                || input.iter().any(|item| {
                    item.get("type").and_then(serde_json::Value::as_str) != Some("message")
                        || !matches!(
                            item.get("role").and_then(serde_json::Value::as_str),
                            Some("developer" | "user")
                        )
                        || item
                            .get("content")
                            .and_then(|content| text_content(content, &["input_text"]))
                            .is_none()
                })
            {
                return None;
            }
            let current = input.last()?;
            if current.get("role").and_then(serde_json::Value::as_str) != Some("user") {
                return None;
            }
            text_content(current.get("content")?, &["input_text"])
        }
        lao_route_api::Client::Claude => {
            let messages = value.get("messages")?.as_array()?;
            if messages.len() != 1
                || messages[0].get("role").and_then(serde_json::Value::as_str) != Some("user")
            {
                return None;
            }
            text_content(messages[0].get("content")?, &["text"])
        }
        lao_route_api::Client::Worker => None,
    }
}

fn text_content(value: &serde_json::Value, allowed: &[&str]) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_owned());
    }
    let parts = value.as_array()?;
    if parts.is_empty()
        || parts.iter().any(|part| {
            !part
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| allowed.contains(&kind))
                || !part.get("text").is_some_and(serde_json::Value::is_string)
        })
    {
        return None;
    }
    let text = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn bound(text: String) -> Option<String> {
    const MAX: usize = 4096;
    if text.len() > MAX {
        return None;
    }
    (!text.trim().is_empty()).then_some(text)
}

fn hello() -> Response<Body> {
    let mut response = Response::new(empty());
    response
        .headers_mut()
        .insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
    response
}

// Starting the runtime blocks, so it must never run on the gate's single thread.
async fn resolve(local: &Option<Arc<dyn Local>>) -> Result<Arc<Endpoint>, Err> {
    let local = local.clone().ok_or_else(|| deny("local"))?;
    match tokio::task::spawn_blocking(move || local.endpoint()).await {
        Ok(Ok(endpoint)) => Ok(endpoint),
        _ => Err(deny("local")),
    }
}

async fn relay<I>(
    request: Request<Body>,
    stream: I,
    endpoint: Option<Arc<Endpoint>>,
) -> Result<Response<Body>, Err>
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let sender = handshake(stream).await?;
    send_ready(request, sender, endpoint).await
}

async fn handshake<I>(stream: I) -> Result<client::SendRequest<Body>, Err>
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (sender, connection) = client::handshake(TokioIo::new(stream)).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(sender)
}

async fn send_ready(
    request: Request<Body>,
    mut sender: client::SendRequest<Body>,
    endpoint: Option<Arc<Endpoint>>,
) -> Result<Response<Body>, Err> {
    let mut response = sender.send_request(request).await?;
    if response.status().is_redirection() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "redirect").into());
    }
    clean_response(response.headers_mut())?;
    Ok(response.map(|body| {
        Held {
            body,
            _endpoint: endpoint,
        }
        .map_err(|error| -> Err { error.into() })
        .boxed_unsync()
    }))
}

struct Held {
    body: Incoming,
    _endpoint: Option<Arc<Endpoint>>,
}

impl hyper::body::Body for Held {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.get_mut().body).poll_frame(context)
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        self.body.size_hint()
    }
}

async fn native(target: &Target) -> Result<tokio_rustls::client::TlsStream<TcpStream>, Err> {
    if !target.tls {
        return Err(deny("target"));
    }
    let addresses: Vec<_> = timeout(WAIT, lookup_host((target.host.as_ref(), 443)))
        .await
        .map_err(|_| deny("resolve"))??
        .collect();
    if addresses.is_empty() || addresses.iter().any(|addr| !public(addr.ip())) {
        return Err(deny("address"));
    }
    let tcp = timeout(WAIT, TcpStream::connect(addresses.as_slice()))
        .await
        .map_err(|_| deny("connect"))??;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_platform_verifier()?
        .with_no_client_auth();
    let name = ServerName::try_from(target.host.clone().into_owned())?;
    timeout(
        WAIT,
        TlsConnector::from(Arc::new(config)).connect(name, tcp),
    )
    .await
    .map_err(|_| deny("tls"))?
    .map_err(|_| deny("tls"))
}

async fn local_addr(addr: SocketAddr) -> Result<TcpStream, Err> {
    if !addr.ip().is_loopback() {
        return Err(deny("target"));
    }
    timeout(WAIT, TcpStream::connect(addr))
        .await
        .map_err(|_| deny("connect"))?
        .map_err(Into::into)
}

fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or_else(|| public_v6(ip), public_v4),
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 88 && c == 99)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn public_v6(ip: Ipv6Addr) -> bool {
    let segment = ip.segments();
    (segment[0] & 0xe000) == 0x2000
        && !(segment[0] == 0x2001 && segment[1] <= 0x01ff)
        && !(segment[0] == 0x2001 && segment[1] == 0x0db8)
        && segment[0] != 0x2002
        && !((segment[0] & 0xfff0) == 0x3ff0)
}

fn empty() -> Body {
    Empty::new().map_err(|never| match never {}).boxed_unsync()
}

fn full(bytes: Bytes) -> Body {
    Full::new(bytes)
        .map_err(|never| match never {})
        .boxed_unsync()
}

fn deny(stage: &'static str) -> Err {
    io::Error::new(io::ErrorKind::PermissionDenied, stage).into()
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        future::Future,
        net::TcpListener as StdListener,
        path::PathBuf,
        process::{Command, Output, Stdio},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering},
        },
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use lao_route_api::{Context, Decision};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        runtime::Builder,
    };

    use super::*;

    const BODY: &[u8] = br#"{"model":"fixture","input":"hello"}"#;
    const ERROR: &[u8] = br#"{"error":{"type":"rate_limit","message":"fixture"}}"#;
    const SSE: &[u8] = b"event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";
    const CODEX: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
    const CLAUDE: &str = "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD";

    #[test]
    fn hello_response_is_explicitly_empty() {
        let response = hello();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers().get(CONTENT_LENGTH).unwrap(), "0");
    }

    #[test]
    fn codex_cloud_streams_after_policy() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
            let plan = plan_with(
                upstream.local_addr().unwrap(),
                Arc::new(Spy {
                    decision: Decision::Cloud,
                    seen: seen.clone(),
                }),
            );
            let upstream_task = tokio::spawn(async move {
                let (mut stream, _) = upstream.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                assert!(request.starts_with(b"POST /v1/responses HTTP/1.1\r\n"));
                assert!(find(&request, b"host: api.openai.com"));
                assert!(find(&request, b"authorization: Bearer synthetic"));
                assert!(!find(&request, b"x-lao-key"));
                assert!(request.ends_with(BODY));
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nX-Fixture: keep\r\nConnection: X-Hop\r\nX-Hop: drop\r\nX-LAO-Key: reflected\r\nAuthorization: Bearer reflected\r\n\r\n",
                    SSE.len()
                );
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(SSE).await.unwrap();
            });
            let gate_task = tokio::spawn(async move {
                let (stream, _) = gate.accept().await.unwrap();
                serve(stream, plan).await.unwrap();
            });

            let mut client = TcpStream::connect(gate_addr).await.unwrap();
            let request = format!(
                "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nAuthorization: Bearer synthetic\r\nX-Unknown: keep\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                BODY.len()
            );
            client.write_all(request.as_bytes()).await.unwrap();
            client.write_all(BODY).await.unwrap();
            let mut response = Vec::new();
            client.read_to_end(&mut response).await.unwrap();

            upstream_task.await.unwrap();
            gate_task.await.unwrap();
            assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
            assert!(find(&response, b"x-fixture: keep"));
            assert!(!find(&response, b"x-hop"));
            assert!(!find(&response, b"x-lao-key"));
            assert!(!find(&response, b"authorization"));
            assert!(response.ends_with(SSE));
            assert_eq!(
                *seen.lock().unwrap(),
                [Context::new(
                    lao_route_api::Client::Codex,
                    lao_route_api::Op::Responses
                )]
            );
        });
    }

    #[test]
    fn native_error_is_preserved() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let plan = plan(upstream.local_addr().unwrap(), Decision::Cloud);
            let upstream_task = tokio::spawn(async move {
                let (mut stream, _) = upstream.accept().await.unwrap();
                let _ = read_request(&mut stream).await;
                let head = format!(
                    "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nRetry-After: 7\r\nX-Request-Id: req_fixture\r\nConnection: close\r\n\r\n",
                    ERROR.len()
                );
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(ERROR).await.unwrap();
            });
            let gate_task = tokio::spawn(async move {
                let (stream, _) = gate.accept().await.unwrap();
                serve(stream, plan).await.unwrap();
            });

            let mut client = TcpStream::connect(gate_addr).await.unwrap();
            let request = format!(
                "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: {CODEX}\r\nAuthorization: Bearer synthetic\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                BODY.len()
            );
            client.write_all(request.as_bytes()).await.unwrap();
            client.write_all(BODY).await.unwrap();
            let mut response = Vec::new();
            client.read_to_end(&mut response).await.unwrap();

            upstream_task.await.unwrap();
            gate_task.await.unwrap();
            assert!(response.starts_with(b"HTTP/1.1 429 Too Many Requests\r\n"));
            assert!(find(&response, b"retry-after: 7"));
            assert!(find(&response, b"x-request-id: req_fixture"));
            assert!(response.ends_with(ERROR));
        });
    }

    #[test]
    fn downstream_cancel_closes_upstream() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let plan = plan(upstream.local_addr().unwrap(), Decision::Cloud);
            let upstream_task = tokio::spawn(async move {
                let (mut stream, _) = upstream.accept().await.unwrap();
                let _ = read_request(&mut stream).await;
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\nx")
                    .await
                    .unwrap();
                let mut byte = [0];
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(1), stream.read(&mut byte))
                        .await
                        .unwrap()
                        .unwrap(),
                    0
                );
            });
            let gate_task = tokio::spawn(async move {
                let (stream, _) = gate.accept().await.unwrap();
                serve(stream, plan).await
            });

            let mut client = TcpStream::connect(gate_addr).await.unwrap();
            let request = format!(
                "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: {CODEX}\r\nAuthorization: Bearer synthetic\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                BODY.len()
            );
            client.write_all(request.as_bytes()).await.unwrap();
            client.write_all(BODY).await.unwrap();
            let mut byte = [0];
            client.read_exact(&mut byte).await.unwrap();
            drop(client);

            upstream_task.await.unwrap();
            let _ = gate_task.await.unwrap();
        });
    }

    #[test]
    fn claude_local_never_sends_secrets() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let upstream_addr = upstream.local_addr().unwrap();
            let mut plan = plan(upstream_addr, Decision::Local);
            plan.fixture = None;
            plan.gate.local = Some(ready(upstream_addr));
            let upstream_task = tokio::spawn(async move {
                let (mut stream, _) = upstream.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                assert!(request.starts_with(b"POST /v1/messages HTTP/1.1\r\n"));
                assert!(find(&request, format!("host: {upstream_addr}").as_bytes()));
                assert!(find(&request, b"content-type: application/json"));
                assert!(find(&request, b"authorization: Bearer runtime"));
                for secret in [
                    b"x-lao-key".as_slice(),
                    b"x-lao-local",
                    b"anthropic-beta",
                    b"x-claude-code-session-id",
                    b"native-secret",
                    b"DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
                ] {
                    assert!(!find(&request, secret));
                }
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", SSE.len());
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(SSE).await.unwrap();
            });
            let gate_task = tokio::spawn(async move {
                let (stream, _) = gate.accept().await.unwrap();
                serve(stream, plan).await.unwrap();
            });
            let mut client = TcpStream::connect(gate_addr).await.unwrap();
            let request = format!(
                "POST /ant/v1/messages?beta=true HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD\r\nX-LAO-Local: canary\r\nAuthorization: Bearer native-secret\r\nAnthropic-Beta: oauth-capability\r\nX-Claude-Code-Session-Id: session-secret\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                BODY.len()
            );
            client.write_all(request.as_bytes()).await.unwrap();
            client.write_all(BODY).await.unwrap();
            let mut response = Vec::new();
            client.read_to_end(&mut response).await.unwrap();
            upstream_task.await.unwrap();
            gate_task.await.unwrap();
            assert!(response.ends_with(SSE));
        });
    }

    #[test]
    fn local_runtime_is_acquired_only_for_local_route_and_released() {
        rt(async {
            for (decision, selector, expected) in [
                (Decision::Cloud, "", 0),
                (Decision::Local, "X-LAO-Local: canary\r\n", 1),
            ] {
                let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let upstream_addr = upstream.local_addr().unwrap();
                let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let gate_addr = gate.local_addr().unwrap();
                let starts = Arc::new(AtomicUsize::new(0));
                let endpoint = Arc::new(Endpoint::new(upstream_addr, "runtime"));
                let held = Arc::downgrade(&endpoint);
                let mut plan = plan(upstream_addr, decision);
                plan.gate.local = Some(Arc::new(Started(starts.clone(), endpoint.clone())));
                let streaming = held.clone();
                let upstream_task = tokio::spawn(async move {
                    let (mut stream, _) = upstream.accept().await.unwrap();
                    let _ = read_request(&mut stream).await;
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", SSE.len());
                    stream.write_all(head.as_bytes()).await.unwrap();
                    tokio::task::yield_now().await;
                    assert_eq!(streaming.strong_count(), expected + 2);
                    stream.write_all(SSE).await.unwrap();
                });
                let gate_task = tokio::spawn(async move {
                    let (stream, _) = gate.accept().await.unwrap();
                    serve(stream, plan).await.unwrap();
                });
                let mut client = TcpStream::connect(gate_addr).await.unwrap();
                let request = format!(
                    "POST /ant/v1/messages HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: {CLAUDE}\r\n{selector}Authorization: Bearer synthetic\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    BODY.len()
                );
                client.write_all(request.as_bytes()).await.unwrap();
                client.write_all(BODY).await.unwrap();
                let mut response = Vec::new();
                client.read_to_end(&mut response).await.unwrap();
                upstream_task.await.unwrap();
                gate_task.await.unwrap();
                assert!(response.ends_with(SSE));
                assert_eq!(starts.load(Ordering::Relaxed), expected);
                assert_eq!(held.strong_count(), 1);
            }
        });
    }

    #[test]
    fn automatic_local_unavailable_preserves_cloud_request() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let mut plan = plan_with(upstream.local_addr().unwrap(), Arc::new(QueryLocal));
            let stale = StdListener::bind(("127.0.0.1", 0)).unwrap();
            let stale_addr = stale.local_addr().unwrap();
            drop(stale);
            plan.gate.local = Some(ready(stale_addr));
            let upstream_task = tokio::spawn(async move {
                let (mut stream, _) = upstream.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                assert!(request.starts_with(b"POST /v1/responses HTTP/1.1\r\n"));
                assert!(find(&request, b"host: api.openai.com"));
                assert!(find(&request, b"authorization: Bearer synthetic"));
                assert!(request.ends_with(BODY));
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", SSE.len());
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(SSE).await.unwrap();
            });
            let gate_task = tokio::spawn(async move {
                let (stream, _) = gate.accept().await.unwrap();
                serve(stream, plan).await.unwrap();
            });
            let mut client = TcpStream::connect(gate_addr).await.unwrap();
            let request = format!(
                "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: {CODEX}\r\nAuthorization: Bearer synthetic\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                BODY.len()
            );
            client.write_all(request.as_bytes()).await.unwrap();
            client.write_all(BODY).await.unwrap();
            let mut response = Vec::new();
            client.read_to_end(&mut response).await.unwrap();
            upstream_task.await.unwrap();
            gate_task.await.unwrap();
            assert!(response.ends_with(SSE));
        });
    }

    #[test]
    fn automatic_routing_requires_text_only_first_turn() {
        let valid = br#"{"model":"fixture","max_output_tokens":12,"input":[{"type":"message","role":"developer","content":[{"type":"input_text","text":"harness"}]},{"type":"message","role":"user","content":[{"type":"input_text","text":"context"}]},{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}]}"#;
        assert_eq!(
            query(valid, lao_route_api::Client::Codex).as_deref(),
            Some("hello")
        );
        let local: serde_json::Value =
            serde_json::from_slice(&local_body(valid, lao_route_api::Client::Codex).unwrap())
                .unwrap();
        assert_eq!(local["input"], "hello");
        assert_eq!(local["max_output_tokens"], 12);
        assert!(!local.to_string().contains("harness"));
        assert!(!local.to_string().contains("context"));
        for body in [
            br#"{"model":"fixture","previous_response_id":"prior","input":"hello"}"#.as_slice(),
            br#"{"model":"fixture","input":[{"type":"function_call_output","call_id":"1","output":"done"}]}"#,
            br#"{"model":"fixture","input":[{"type":"message","role":"assistant","content":[{"type":"input_text","text":"hello"}]}]}"#,
            br#"{"model":"fixture","input":[{"type":"message","role":"user","content":[{"type":"input_image","image_url":"private"}]}]}"#,
        ] {
            assert_eq!(query(body, lao_route_api::Client::Codex), None);
        }
        let tool_result = br#"{"model":"fixture","messages":[{"role":"user","content":[{"type":"tool_result","content":"private"}]}]}"#;
        assert_eq!(query(tool_result, lao_route_api::Client::Claude), None);
    }

    #[test]
    fn worker_requires_native_tools_until_the_edit() {
        let first = worker_body(
            br#"{"model":"lao-local","messages":[{"role":"user","content":"edit"}],"tools":[{"type":"function","function":{"name":"read"}}]}"#,
        )
        .unwrap();
        let first: serde_json::Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(first["tool_choice"], "required");
        assert_eq!(first["parallel_tool_calls"], false);

        let edited = worker_body(
            br#"{"model":"lao-local","messages":[{"role":"assistant","tool_calls":[{"type":"function","function":{"name":"edit","arguments":"{}"}}]}],"tools":[{"type":"function","function":{"name":"edit"}}]}"#,
        )
        .unwrap();
        let edited: serde_json::Value = serde_json::from_slice(&edited).unwrap();
        assert_eq!(edited["tool_choice"], "auto");
        assert!(worker_body(br#"{"messages":[],"tools":[]}"#).is_none());
    }

    #[test]
    fn rejected_requests_never_connect_upstream() {
        rt(async {
            for (line, length, expected) in [
                ("", 999, 0),
                ("X-LAO-Key: wrong\r\n", 0, 0),
                (
                    "X-LAO-Key: DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD\r\n",
                    0,
                    0,
                ),
                (
                    "X-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nX-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\n",
                    0,
                    0,
                ),
                (
                    "X-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nAuthorization: Basic wrong\r\n",
                    0,
                    0,
                ),
            ] {
                let upstream = StdListener::bind(("127.0.0.1", 0)).unwrap();
                upstream.set_nonblocking(true).unwrap();
                let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let gate_addr = gate.local_addr().unwrap();
                let calls = Arc::new(AtomicUsize::new(0));
                let plan = plan_with(
                    upstream.local_addr().unwrap(),
                    Arc::new(Count(calls.clone())),
                );
                let task = tokio::spawn(async move {
                    let (stream, _) = gate.accept().await.unwrap();
                    serve(stream, plan).await
                });
                let mut client = TcpStream::connect(gate_addr).await.unwrap();
                let request = format!(
                    "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\n{line}Content-Length: {length}\r\nConnection: close\r\n\r\n"
                );
                client.write_all(request.as_bytes()).await.unwrap();
                let mut response = Vec::new();
                let _ = client.read_to_end(&mut response).await;
                assert!(task.await.unwrap().is_err());
                assert!(matches!(
                    upstream.accept(),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock
                ));
                assert_eq!(calls.load(Ordering::Relaxed), expected);
            }
        });
    }

    #[test]
    fn redirect_is_not_relayed() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let plan = plan(upstream.local_addr().unwrap(), Decision::Cloud);
            let upstream_task = tokio::spawn(async move {
                let (mut stream, _) = upstream.accept().await.unwrap();
                let _ = read_request(&mut stream).await;
                stream
                    .write_all(b"HTTP/1.1 302 Found\r\nLocation: https://bad.invalid\r\nContent-Length: 0\r\n\r\n")
                    .await
                    .unwrap();
            });
            let gate_task = tokio::spawn(async move {
                let (stream, _) = gate.accept().await.unwrap();
                serve(stream, plan).await
            });
            let mut client = TcpStream::connect(gate_addr).await.unwrap();
            let request = format!(
                "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nAuthorization: Bearer synthetic\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                BODY.len()
            );
            client.write_all(request.as_bytes()).await.unwrap();
            client.write_all(BODY).await.unwrap();
            let mut response = Vec::new();
            let _ = client.read_to_end(&mut response).await;
            upstream_task.await.unwrap();
            assert!(gate_task.await.unwrap().is_err());
            assert!(!find(&response, b"location"));
        });
    }

    #[test]
    fn dns_policy_rejects_non_public_addresses() {
        for ip in [
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.1.1",
            "172.16.0.1",
            "192.0.2.1",
            "192.168.0.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
            "fc00::1",
            "fe80::1",
            "ff00::1",
        ] {
            assert!(!public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(public(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    #[ignore = "uses the installed Codex login for one cheap native request"]
    fn installed_codex_reaches_chatgpt_through_gate() {
        version("codex", "codex-cli 0.151.0");
        let live = Live::start(Cloud::ChatGpt);
        let output = codex(
            &live,
            "gpt-5.4",
            "Reply exactly LAO_E2E_OK. Do not use tools.",
        );
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("LAO_E2E_OK"));
        clean_output(&output, CODEX);
        live.assert_status(200);
    }

    #[test]
    #[ignore = "uses the installed Codex login for one cheap native error"]
    fn installed_codex_preserves_native_error() {
        version("codex", "codex-cli 0.151.0");
        let live = Live::start(Cloud::ChatGpt);
        let output = codex(&live, "lao-invalid-model", "Reply exactly LAO_E2E_OK.");
        assert!(!output.status.success());
        clean_output(&output, CODEX);
        live.assert_status(400);
    }

    #[test]
    #[ignore = "uses the installed Claude login for one cheap native request"]
    fn installed_claude_reaches_anthropic_through_gate() {
        version("claude", "2.1.251 (Claude Code)");
        let live = Live::start(Cloud::AnthropicBearer);
        let settings = format!(
            "{{\"env\":{{\"ANTHROPIC_BASE_URL\":\"http://127.0.0.1:{}/ant\",\"ANTHROPIC_CUSTOM_HEADERS\":\"X-LAO-Key: {CLAUDE}\",\"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC\":\"1\"}}}}",
            live.port
        );
        let temp = Temp::new("claude");
        let output = exec(
            Command::new("claude")
                .env_remove("ANTHROPIC_API_KEY")
                .env_remove("ANTHROPIC_AUTH_TOKEN")
                .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
                .current_dir(&temp.0)
                .args([
                    "--safe-mode",
                    "--settings",
                    &settings,
                    "--no-session-persistence",
                    "--disable-slash-commands",
                    "--tools",
                    "",
                    "--effort",
                    "low",
                    "-p",
                    "--model",
                    "haiku",
                    "Reply exactly LAO_E2E_OK. Do not use tools.",
                ]),
        );
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("LAO_E2E_OK"));
        clean_output(&output, CLAUDE);
        live.assert_status(200);
    }

    fn rt(test: impl Future<Output = ()>) {
        Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(2), test)
                    .await
                    .unwrap();
            });
    }

    fn plan(addr: SocketAddr, decision: Decision) -> Plan {
        plan_with(addr, Arc::new(Fixed(decision)))
    }

    fn plan_with(addr: SocketAddr, policy: Arc<dyn Policy>) -> Plan {
        Plan {
            gate: Gate {
                host: HeaderValue::from_static("lao.local"),
                codex: *b"CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                codex_cloud: Cloud::OpenAi,
                claude: *b"DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
                claude_cloud: Cloud::AnthropicBearer,
                worker: [0; 64],
                local: Some(ready("127.0.0.1:10000".parse().unwrap())),
            },
            policy,
            fixture: Some(addr),
            status: None,
        }
    }

    fn ready(addr: SocketAddr) -> Arc<dyn Local> {
        Arc::new(crate::Ready(Arc::new(Endpoint::new(addr, "runtime"))))
    }

    async fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buf = [0; 1024];
        loop {
            let read = stream.read(&mut buf).await.unwrap();
            assert!(read > 0);
            request.extend_from_slice(&buf[..read]);
            if let Some(head) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let body = head + 4;
                if request.len() >= body + BODY.len() {
                    return request;
                }
            }
        }
    }

    fn find(bytes: &[u8], needle: &[u8]) -> bool {
        bytes.windows(needle.len()).any(|part| part == needle)
    }

    struct Live {
        port: u16,
        status: Arc<AtomicU16>,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Live {
        fn start(cloud: Cloud) -> Self {
            let listener = StdListener::bind(("127.0.0.1", 0)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let stop = Arc::new(AtomicBool::new(false));
            let done = stop.clone();
            let status = Arc::new(AtomicU16::new(0));
            let observed = status.clone();
            let thread = thread::spawn(move || {
                Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()
                    .unwrap()
                    .block_on(async move {
                        let listener = TcpListener::from_std(listener).unwrap();
                        let plan = Plan {
                            gate: Gate {
                                host: HeaderValue::from_str(&format!("127.0.0.1:{port}")).unwrap(),
                                codex: *CODEX.as_bytes().first_chunk().unwrap(),
                                codex_cloud: cloud,
                                claude: *CLAUDE.as_bytes().first_chunk().unwrap(),
                                claude_cloud: cloud,
                                worker: [0; 64],
                                local: None,
                            },
                            policy: Arc::new(Fixed(Decision::Cloud)),
                            fixture: None,
                            status: Some(observed),
                        };
                        while !done.load(Ordering::Relaxed) {
                            if let Ok(Ok((stream, _))) =
                                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                                    .await
                            {
                                let _ = serve(stream, plan.clone()).await;
                            }
                        }
                    });
            });
            Self {
                port,
                status,
                stop,
                thread: Some(thread),
            }
        }

        fn assert_status(&self, expected: u16) {
            assert_eq!(self.status.load(Ordering::Relaxed), expected);
        }
    }

    impl Drop for Live {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.thread.take().unwrap().join().unwrap();
        }
    }

    struct Fixed(Decision);

    impl Policy for Fixed {
        fn decide(&self, _: Context) -> Decision {
            self.0
        }
    }

    struct Spy {
        decision: Decision,
        seen: Arc<std::sync::Mutex<Vec<Context>>>,
    }

    impl Policy for Spy {
        fn decide(&self, context: Context) -> Decision {
            self.seen.lock().unwrap().push(context);
            self.decision
        }
    }

    struct Started(Arc<AtomicUsize>, Arc<Endpoint>);

    impl Local for Started {
        fn endpoint(&self) -> io::Result<Arc<Endpoint>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(self.1.clone())
        }
    }

    struct QueryLocal;

    impl Policy for QueryLocal {
        fn decide(&self, _: Context) -> Decision {
            Decision::Cloud
        }

        fn requires_query(&self) -> bool {
            true
        }

        fn decide_query(&self, _: Context, _: &str) -> Decision {
            Decision::Local
        }
    }

    struct Count(Arc<AtomicUsize>);

    impl Policy for Count {
        fn decide(&self, _: Context) -> Decision {
            self.0.fetch_add(1, Ordering::Relaxed);
            Decision::Cloud
        }
    }

    struct Temp(PathBuf);

    impl Temp {
        fn new(name: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                env::temp_dir().join(format!("lao-gate-{name}-{}-{stamp}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn exec(command: &mut Command) -> Output {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let until = Instant::now() + Duration::from_secs(60);
        while Instant::now() < until {
            if child.try_wait().unwrap().is_some() {
                return child.wait_with_output().unwrap();
            }
            thread::sleep(Duration::from_millis(20));
        }
        child.kill().unwrap();
        let _ = child.wait();
        panic!("client timed out")
    }

    fn clean_output(output: &Output, caller: &str) {
        assert!(!String::from_utf8_lossy(&output.stdout).contains(caller));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(caller));
    }

    fn codex(live: &Live, model: &str, prompt: &str) -> Output {
        let base = format!("http://127.0.0.1:{}/oai", live.port);
        let provider = format!(
            "{{ name = \"LAO\", base_url = \"{base}\", requires_openai_auth = true, supports_websockets = false, http_headers = {{ X-LAO-Key = \"{CODEX}\" }}, request_max_retries = 0, stream_max_retries = 0 }}"
        );
        let temp = Temp::new("codex");
        exec(
            Command::new("codex")
                .env_remove("OPENAI_API_KEY")
                .env_remove("CODEX_API_KEY")
                .env_remove("CODEX_ACCESS_TOKEN")
                .current_dir(&temp.0)
                .args([
                    "-c",
                    "model_provider=\"lao_e2e\"",
                    "-c",
                    &format!("model_providers.lao_e2e={provider}"),
                    "-c",
                    "model_reasoning_effort=\"low\"",
                    "exec",
                    "--ignore-user-config",
                    "--ignore-rules",
                    "--ephemeral",
                    "--skip-git-repo-check",
                    "--color",
                    "never",
                    "--sandbox",
                    "read-only",
                    "--model",
                    model,
                    prompt,
                ]),
        )
    }

    fn version(bin: &str, expected: &str) {
        let output = exec(Command::new(bin).arg("--version"));
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}
