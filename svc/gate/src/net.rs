use std::{error::Error, io, net::SocketAddr};

use hyper::{
    Request, Response, body::Incoming, client::conn::http1 as client,
    server::conn::http1 as server, service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

#[cfg(not(test))]
use crate::policy::Target;
use crate::policy::{Gate, Route, admit, clean_response};

type Err = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct Plan {
    gate: Gate,
    route: Route,
    #[cfg(test)]
    fixture: SocketAddr,
}

async fn serve(stream: TcpStream, plan: Plan) -> Result<(), Err> {
    let service = service_fn(move |request| send(request, plan.clone()));
    server::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await?;
    Ok(())
}

async fn send(request: Request<Incoming>, plan: Plan) -> Result<Response<Incoming>, Err> {
    let task = admit(request, &plan.gate)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "hello"))?;
    let (target, request) = task.freeze(plan.route)?.take();
    #[cfg(test)]
    let stream = TcpStream::connect(plan.fixture).await?;
    #[cfg(not(test))]
    let stream = connect(target).await?;
    #[cfg(test)]
    let _ = target;
    let (mut sender, connection) = client::handshake(TokioIo::new(stream)).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let mut response = sender.send_request(request).await?;
    if response.status().is_redirection() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "redirect").into());
    }
    clean_response(response.headers_mut())?;
    Ok(response)
}

#[cfg(not(test))]
async fn connect(target: Target) -> Result<TcpStream, Err> {
    if target.tls {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "tls").into());
    }
    let addr: SocketAddr = target.host.parse()?;
    if !addr.ip().is_loopback() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "target").into());
    }
    Ok(TcpStream::connect(addr).await?)
}

#[cfg(test)]
mod tests {
    use std::{future::Future, net::TcpListener as StdListener, time::Duration};

    use hyper::header::HeaderValue;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        runtime::Builder,
    };

    use super::*;

    const BODY: &[u8] = br#"{"model":"fixture","input":"hello"}"#;
    const SSE: &[u8] = b"event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";

    #[test]
    fn codex_cloud_streams_after_policy() {
        run(async {
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
        run(async {
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
        run(async {
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
        run(async {
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

    fn run(test: impl Future<Output = ()>) {
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
            fixture: addr,
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
}
