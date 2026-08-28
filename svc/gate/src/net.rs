use std::{
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use http_body_util::{BodyExt, Empty, combinators::UnsyncBoxBody};
use hyper::{
    Request, Response, body::Incoming, client::conn::http1 as client,
    server::conn::http1 as server, service::service_fn,
};
use hyper_util::rt::TokioIo;
use rustls::{ClientConfig, pki_types::ServerName};
use rustls_platform_verifier::BuilderVerifierExt;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, lookup_host},
    time::timeout,
};
use tokio_rustls::TlsConnector;

use crate::policy::Target;
use crate::policy::{Gate, Route, admit, clean_response};

type Err = Box<dyn Error + Send + Sync>;
type Body = UnsyncBoxBody<Bytes, Err>;
const WAIT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct Plan {
    gate: Gate,
    route: Route,
    #[cfg(test)]
    fixture: Option<SocketAddr>,
    #[cfg(test)]
    native: Option<Arc<std::sync::atomic::AtomicBool>>,
}

async fn serve(stream: TcpStream, plan: Plan) -> Result<(), Err> {
    let service = service_fn(move |request| send(request, plan.clone()));
    server::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await?;
    Ok(())
}

async fn send(request: Request<Incoming>, plan: Plan) -> Result<Response<Body>, Err> {
    let Some(task) = admit(request, &plan.gate)? else {
        return Ok(Response::new(empty()));
    };
    let (target, request) = task.freeze(plan.route)?.take();
    #[cfg(test)]
    if let Some(fixture) = plan.fixture {
        let _ = target;
        return relay(request, TcpStream::connect(fixture).await?).await;
    }
    if target.tls {
        let response = relay(request, native(&target).await?).await;
        #[cfg(test)]
        if response.is_ok()
            && let Some(native) = plan.native
        {
            native.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        response
    } else {
        relay(request, local(&target).await?).await
    }
}

async fn relay<I>(request: Request<Incoming>, stream: I) -> Result<Response<Body>, Err>
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, connection) = client::handshake(TokioIo::new(stream)).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let mut response = sender.send_request(request).await?;
    if response.status().is_redirection() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "redirect").into());
    }
    clean_response(response.headers_mut())?;
    Ok(response.map(|body| body.map_err(Into::into).boxed_unsync()))
}

async fn native(target: &Target) -> Result<tokio_rustls::client::TlsStream<TcpStream>, Err> {
    if !target.tls {
        return Err(deny("target"));
    }
    let addresses: Vec<_> = timeout(WAIT, lookup_host((target.host, 443)))
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
    let name = ServerName::try_from(target.host)?;
    timeout(
        WAIT,
        TlsConnector::from(Arc::new(config)).connect(name, tcp),
    )
    .await
    .map_err(|_| deny("tls"))?
    .map_err(|_| deny("tls"))
}

async fn local(target: &Target) -> Result<TcpStream, Err> {
    if target.tls {
        return Err(deny("target"));
    }
    let addr: SocketAddr = target.host.parse()?;
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
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use hyper::header::HeaderValue;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        runtime::Builder,
    };

    use super::*;

    const BODY: &[u8] = br#"{"model":"fixture","input":"hello"}"#;
    const SSE: &[u8] = b"event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";
    const CODEX: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
    const CLAUDE: &str = "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD";

    #[test]
    fn codex_cloud_streams_after_policy() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let plan = plan(upstream.local_addr().unwrap(), Route::OpenAi);
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
                "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nAuthorization: Bearer synthetic\r\nX-Unknown: keep\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
        });
    }

    #[test]
    fn claude_local_never_sends_secrets() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let plan = plan(upstream.local_addr().unwrap(), Route::Local);
            let upstream_task = tokio::spawn(async move {
                let (mut stream, _) = upstream.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                assert!(request.starts_with(b"POST /v1/messages HTTP/1.1\r\n"));
                assert!(find(&request, b"host: 127.0.0.1:10000"));
                assert!(find(&request, b"content-type: application/json"));
                for secret in [
                    b"x-lao-key".as_slice(),
                    b"authorization",
                    b"anthropic-beta",
                    b"x-claude-code-session-id",
                    b"native-secret",
                    b"DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
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
                "POST /ant/v1/messages?beta=true HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD\r\nAuthorization: Bearer native-secret\r\nAnthropic-Beta: oauth-capability\r\nX-Claude-Code-Session-Id: session-secret\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
    fn rejected_requests_never_connect_upstream() {
        rt(async {
            for (line, route, length) in [
                ("", Route::OpenAi, 999),
                ("X-LAO-Key: wrong\r\n", Route::OpenAi, 0),
                (
                    "X-LAO-Key: DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD\r\n",
                    Route::OpenAi,
                    0,
                ),
                (
                    "X-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nX-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\n",
                    Route::OpenAi,
                    0,
                ),
                (
                    "X-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nAuthorization: Basic wrong\r\n",
                    Route::OpenAi,
                    0,
                ),
            ] {
                let upstream = StdListener::bind(("127.0.0.1", 0)).unwrap();
                upstream.set_nonblocking(true).unwrap();
                let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let gate_addr = gate.local_addr().unwrap();
                let plan = plan(upstream.local_addr().unwrap(), route);
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
            }
        });
    }

    #[test]
    fn redirect_is_not_relayed() {
        rt(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let plan = plan(upstream.local_addr().unwrap(), Route::OpenAi);
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
                "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nAuthorization: Bearer synthetic\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
    fn listener_is_owned_before_configuration() {
        let occupied = StdListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = occupied.local_addr().unwrap();
        let mut configured = false;
        assert!(activate(addr, || configured = true).is_err());
        assert!(!configured);
        drop(occupied);
        let held = activate(addr, || configured = true).unwrap();
        assert!(configured);
        drop(held.try_clone().unwrap());
        assert!(StdListener::bind(addr).is_err());
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
        version("codex", "codex-cli 0.146.0");
        let live = Live::start(Route::ChatGpt);
        let base = format!("http://127.0.0.1:{}/oai", live.port);
        let provider = format!(
            "{{ name = \"LAO\", base_url = \"{base}\", requires_openai_auth = true, supports_websockets = false, http_headers = {{ X-LAO-Key = \"{CODEX}\" }}, request_max_retries = 0, stream_max_retries = 0 }}"
        );
        let temp = Temp::new("codex");
        let output = exec(
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
                    "gpt-5.4",
                    "Reply exactly LAO_E2E_OK. Do not use tools.",
                ]),
        );
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("LAO_E2E_OK"));
        clean_output(&output, CODEX);
        live.assert_native();
    }

    #[test]
    #[ignore = "uses the installed Claude login for one cheap native request"]
    fn installed_claude_reaches_anthropic_through_gate() {
        version("claude", "2.1.223 (Claude Code)");
        let live = Live::start(Route::AnthropicBearer);
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
        live.assert_native();
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

    fn plan(addr: SocketAddr, route: Route) -> Plan {
        Plan {
            gate: Gate {
                host: HeaderValue::from_static("lao.local"),
                codex: *b"CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                claude: *b"DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
            },
            route,
            fixture: Some(addr),
            native: None,
        }
    }

    fn activate(addr: SocketAddr, configure: impl FnOnce()) -> io::Result<StdListener> {
        let listener = StdListener::bind(addr)?;
        configure();
        Ok(listener)
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
        native: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Live {
        fn start(route: Route) -> Self {
            let listener = StdListener::bind(("127.0.0.1", 0)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let stop = Arc::new(AtomicBool::new(false));
            let done = stop.clone();
            let native = Arc::new(AtomicBool::new(false));
            let observed = native.clone();
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
                                claude: *CLAUDE.as_bytes().first_chunk().unwrap(),
                            },
                            route,
                            fixture: None,
                            native: Some(observed),
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
                native,
                stop,
                thread: Some(thread),
            }
        }

        fn assert_native(&self) {
            assert!(self.native.load(Ordering::Relaxed));
        }
    }

    impl Drop for Live {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.thread.take().unwrap().join().unwrap();
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

    fn version(bin: &str, expected: &str) {
        let output = exec(Command::new(bin).arg("--version"));
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}
