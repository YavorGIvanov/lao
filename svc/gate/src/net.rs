use std::{error::Error, io, net::SocketAddr};

use hyper::{
    Request, Response, Uri,
    body::Incoming,
    client::conn::http1 as client,
    header::{HOST, HeaderValue},
    server::conn::http1 as server,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

type Err = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct Native {
    addr: SocketAddr,
    host: HeaderValue,
    path: Uri,
    caller: [u8; 32],
}

async fn serve(stream: TcpStream, target: Native) -> Result<(), Err> {
    let service = service_fn(move |request| send(request, target.clone()));
    server::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await?;
    Ok(())
}

async fn send(mut request: Request<Incoming>, target: Native) -> Result<Response<Incoming>, Err> {
    let valid = {
        let mut values = request.headers().get_all("x-lao-key").iter();
        matches!((values.next(), values.next()), (Some(value), None) if constant_time(value.as_bytes(), &target.caller))
    };
    if !valid {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "caller").into());
    }
    request.headers_mut().remove("x-lao-key");
    *request.uri_mut() = target.path;
    request.headers_mut().insert(HOST, target.host);
    let stream = TcpStream::connect(target.addr).await?;
    let (mut sender, connection) = client::handshake(TokioIo::new(stream)).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(sender.send_request(request).await?)
}

fn constant_time(value: &[u8], expected: &[u8; 32]) -> bool {
    let mut different = value.len() ^ expected.len();
    for (index, byte) in expected.iter().enumerate() {
        different |= usize::from(value.get(index).copied().unwrap_or_default() ^ byte);
    }
    different == 0
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        net::TcpListener as StdListener,
        time::{Duration, Instant},
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        runtime::Builder,
    };

    use super::*;

    const BODY: &[u8] = br#"{"model":"fixture","input":"hello"}"#;
    const SSE: &[u8] = b"event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";

    #[test]
    fn one_http_exchange_uses_hyper() {
        run(async {
            let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let gate_addr = gate.local_addr().unwrap();
            let target = native(upstream.local_addr().unwrap());
            let upstream_task = tokio::spawn(async move {
                let (mut stream, _) = upstream.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                assert!(request.starts_with(b"POST /v1/responses HTTP/1.1\r\n"));
                assert!(find(&request, b"host: api.fixture.invalid"));
                assert!(find(&request, b"authorization: Bearer synthetic"));
                assert!(!find(&request, b"x-lao-key"));
                assert!(request.ends_with(BODY));
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nX-Fixture: keep\r\n\r\n",
                    SSE.len()
                );
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(SSE).await.unwrap();
            });
            let gate_task = tokio::spawn(async move {
                let (stream, _) = gate.accept().await.unwrap();
                serve(stream, target).await.unwrap();
            });

            let mut client = TcpStream::connect(gate_addr).await.unwrap();
            let request = format!(
                "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\nX-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nAuthorization: Bearer synthetic\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                BODY.len()
            );
            let start = Instant::now();
            client.write_all(request.as_bytes()).await.unwrap();
            client.write_all(BODY).await.unwrap();
            let mut response = Vec::new();
            client.read_to_end(&mut response).await.unwrap();
            let elapsed = start.elapsed();

            upstream_task.await.unwrap();
            gate_task.await.unwrap();
            assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
            assert!(find(&response, b"x-fixture: keep"));
            assert!(response.ends_with(SSE));
            assert!(elapsed < Duration::from_millis(100));
            eprintln!("P0-04 Hyper loopback exchange: {elapsed:?}");
        });
    }

    #[test]
    fn bad_callers_never_connect_upstream() {
        run(async {
            for caller in [
                "",
                "X-LAO-Key: wrong\r\n",
                "X-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\nX-LAO-Key: CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC\r\n",
            ] {
                let upstream = StdListener::bind(("127.0.0.1", 0)).unwrap();
                upstream.set_nonblocking(true).unwrap();
                let gate = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let gate_addr = gate.local_addr().unwrap();
                let target = native(upstream.local_addr().unwrap());
                let task = tokio::spawn(async move {
                    let (stream, _) = gate.accept().await.unwrap();
                    serve(stream, target).await
                });
                let mut client = TcpStream::connect(gate_addr).await.unwrap();
                let request = format!(
                    "POST /oai/responses HTTP/1.1\r\nHost: lao.local\r\n{caller}Content-Length: 0\r\nConnection: close\r\n\r\n"
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

    fn native(addr: SocketAddr) -> Native {
        Native {
            addr,
            host: HeaderValue::from_static("api.fixture.invalid"),
            path: Uri::from_static("/v1/responses"),
            caller: *b"CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
        }
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
