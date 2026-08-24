use std::{
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const RESPONSES_REQUEST: &[u8] = include_bytes!("fixtures/responses.request.json");
const RESPONSES_RESPONSE: &[u8] = include_bytes!("fixtures/responses.response.sse");
const MESSAGES_REQUEST: &[u8] = include_bytes!("fixtures/messages.request.json");
const MESSAGES_RESPONSE: &[u8] = include_bytes!("fixtures/messages.response.sse");
const WAIT: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct Exchange {
    request: Vec<u8>,
    response: Vec<u8>,
}

fn responses(keep_alive: bool) -> Exchange {
    exchange(
        "/v1/responses",
        "Authorization: Bearer synthetic-openai-token\r\nOpenAI-Beta: fixture\r\n",
        RESPONSES_REQUEST,
        RESPONSES_RESPONSE,
        keep_alive,
    )
}

fn messages(keep_alive: bool) -> Exchange {
    exchange(
        "/v1/messages",
        "X-Api-Key: synthetic-anthropic-token\r\nAnthropic-Version: 2023-06-01\r\nAnthropic-Beta: fixture\r\n",
        MESSAGES_REQUEST,
        MESSAGES_RESPONSE,
        keep_alive,
    )
}

fn exchange(
    path: &str,
    provider_headers: &str,
    request_body: &[u8],
    response_body: &[u8],
    keep_alive: bool,
) -> Exchange {
    let connection = if keep_alive { "keep-alive" } else { "close" };
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: api.fixture.invalid\r\nContent-Type: application/json\r\nAccept: text/event-stream\r\nContent-Length: {}\r\nConnection: {connection}\r\nX-Fixture-Unknown: preserve-me\r\n{provider_headers}\r\n",
        request_body.len()
    );
    let mut request = head.into_bytes();
    request.extend_from_slice(request_body);

    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: {connection}\r\nX-Request-Id: req_fixture\r\nX-Fixture-Unknown: preserve-me\r\n\r\n"
    )
    .into_bytes();
    for chunk in response_body.chunks(53) {
        response.extend_from_slice(format!("{:X}\r\n", chunk.len()).as_bytes());
        response.extend_from_slice(chunk);
        response.extend_from_slice(b"\r\n");
    }
    response.extend_from_slice(b"0\r\n\r\n");
    Exchange { request, response }
}

fn proxy(upstream: SocketAddr) -> io::Result<(SocketAddr, thread::JoinHandle<io::Result<()>>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    let handle = thread::spawn(move || {
        let (downstream, _) = listener.accept()?;
        bridge(downstream, TcpStream::connect(upstream)?)
    });
    Ok((address, handle))
}

fn bridge(downstream: TcpStream, upstream: TcpStream) -> io::Result<()> {
    downstream.set_nodelay(true)?;
    upstream.set_nodelay(true)?;
    let cancel_downstream = downstream.try_clone()?;
    let cancel_upstream = upstream.try_clone()?;
    let down_read = downstream.try_clone()?;
    let up_read = upstream.try_clone()?;
    thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        let request_tx = tx.clone();
        scope.spawn(move || {
            request_tx
                .send(pump(down_read, upstream))
                .expect("request result");
        });
        scope.spawn(move || {
            tx.send(pump(up_read, downstream)).expect("response result");
        });
        let first = rx.recv().expect("first pump result");
        let _ = cancel_downstream.shutdown(Shutdown::Both);
        let _ = cancel_upstream.shutdown(Shutdown::Both);
        let _ = rx.recv().expect("second pump result");
        first.map(|_| ())
    })
}

fn pump(mut reader: TcpStream, mut writer: TcpStream) -> io::Result<u64> {
    io::copy(&mut reader, &mut writer)
}

fn finish(handle: thread::JoinHandle<io::Result<()>>) {
    handle.join().expect("proxy thread").expect("first pump");
}

fn finish_cancel(handle: thread::JoinHandle<io::Result<()>>) {
    if let Err(error) = handle.join().expect("proxy thread") {
        assert!(matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::NotConnected
        ));
    }
}

fn expect_bytes(socket: &mut TcpStream, expected: &[u8]) {
    let mut actual = vec![0; expected.len()];
    socket.read_exact(&mut actual).expect("bytes");
    assert_eq!(actual, expected);
}

fn stream(exchange: Exchange) {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("upstream bind");
    let upstream_address = upstream.local_addr().expect("upstream address");
    let (first_request_tx, first_request_rx) = mpsc::channel();
    let (finish_response_tx, finish_response_rx) = mpsc::channel();
    let server_request = exchange.request.clone();
    let server_response = exchange.response.clone();
    let upstream_handle = thread::spawn(move || {
        let (mut socket, _) = upstream.accept().expect("upstream accept");
        socket.set_read_timeout(Some(WAIT)).expect("read timeout");
        let request_cut = server_request.len() / 2;
        let mut received = vec![0; server_request.len()];
        socket
            .read_exact(&mut received[..request_cut])
            .expect("first request chunk");
        first_request_tx.send(()).expect("request signal");
        socket
            .read_exact(&mut received[request_cut..])
            .expect("last request chunk");
        assert_eq!(received, server_request);

        let response_cut = server_response.len() / 2;
        socket
            .write_all(&server_response[..response_cut])
            .expect("first response chunk");
        socket.flush().expect("flush response");
        finish_response_rx
            .recv_timeout(WAIT)
            .expect("response release");
        socket
            .write_all(&server_response[response_cut..])
            .expect("last response chunk");
    });

    let (address, proxy_handle) = proxy(upstream_address).expect("start proxy");
    let mut client = TcpStream::connect(address).expect("connect proxy");
    client.set_read_timeout(Some(WAIT)).expect("read timeout");
    let request_cut = exchange.request.len() / 2;
    client
        .write_all(&exchange.request[..request_cut])
        .expect("first request write");
    client.flush().expect("flush request");
    first_request_rx
        .recv_timeout(WAIT)
        .expect("request streamed before complete");
    client
        .write_all(&exchange.request[request_cut..])
        .expect("last request write");

    let response_cut = exchange.response.len() / 2;
    let mut actual = vec![0; exchange.response.len()];
    client
        .read_exact(&mut actual[..response_cut])
        .expect("response streamed before complete");
    finish_response_tx.send(()).expect("release response");
    client
        .read_exact(&mut actual[response_cut..])
        .expect("last response read");
    assert_eq!(actual, exchange.response);
    drop(client);
    upstream_handle.join().expect("upstream thread");
    finish(proxy_handle);
}

#[test]
fn responses_and_messages_stream_complete_http_byte_exact() {
    stream(responses(false));
    stream(messages(false));
}

#[test]
fn persistent_connection_carries_two_exchanges() {
    let exchanges = [responses(true), messages(false)];
    let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("upstream bind");
    let address = upstream.local_addr().expect("upstream address");
    let server = exchanges.clone();
    let upstream_handle = thread::spawn(move || {
        let (mut socket, _) = upstream.accept().expect("upstream accept");
        socket.set_read_timeout(Some(WAIT)).expect("read timeout");
        for exchange in server {
            expect_bytes(&mut socket, &exchange.request);
            socket.write_all(&exchange.response).expect("response");
        }
    });
    let (proxy_address, proxy_handle) = proxy(address).expect("start proxy");
    let mut client = TcpStream::connect(proxy_address).expect("connect proxy");
    client.set_read_timeout(Some(WAIT)).expect("read timeout");
    for exchange in exchanges {
        client.write_all(&exchange.request).expect("request");
        expect_bytes(&mut client, &exchange.response);
    }
    drop(client);
    upstream_handle.join().expect("upstream thread");
    finish(proxy_handle);
}

#[test]
fn downstream_cancel_midstream_closes_upstream() {
    let exchange = responses(false);
    let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("upstream bind");
    let address = upstream.local_addr().expect("upstream address");
    let (partial_tx, partial_rx) = mpsc::channel();
    let request = exchange.request.clone();
    let response = exchange.response.clone();
    let upstream_handle = thread::spawn(move || {
        let (mut socket, _) = upstream.accept().expect("upstream accept");
        socket.set_read_timeout(Some(WAIT)).expect("read timeout");
        expect_bytes(&mut socket, &request);
        socket
            .write_all(&response[..response.len() / 2])
            .expect("partial response");
        partial_tx.send(()).expect("partial signal");
        let mut byte = [0];
        assert_eq!(socket.read(&mut byte).expect("upstream close"), 0);
    });
    let (proxy_address, proxy_handle) = proxy(address).expect("start proxy");
    let mut client = TcpStream::connect(proxy_address).expect("connect proxy");
    client.write_all(&exchange.request).expect("request");
    partial_rx.recv_timeout(WAIT).expect("partial response");
    let mut byte = [0];
    client.read_exact(&mut byte).expect("response started");
    drop(client);
    upstream_handle.join().expect("upstream thread");
    finish_cancel(proxy_handle);
}

#[test]
fn upstream_cancel_closes_downstream() {
    let exchange = messages(false);
    let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("upstream bind");
    let address = upstream.local_addr().expect("upstream address");
    let request = exchange.request.clone();
    let upstream_handle = thread::spawn(move || {
        let (mut socket, _) = upstream.accept().expect("upstream accept");
        expect_bytes(&mut socket, &request);
    });
    let (proxy_address, proxy_handle) = proxy(address).expect("start proxy");
    let mut client = TcpStream::connect(proxy_address).expect("connect proxy");
    client.write_all(&exchange.request).expect("request");
    let mut byte = [0];
    assert_eq!(client.read(&mut byte).expect("downstream close"), 0);
    upstream_handle.join().expect("upstream thread");
    finish(proxy_handle);
}

fn first_byte(exchange: &Exchange, via_proxy: bool) -> Duration {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("upstream bind");
    let upstream_address = upstream.local_addr().expect("upstream address");
    let request = exchange.request.clone();
    let response = exchange.response.clone();
    let upstream_handle = thread::spawn(move || {
        let (mut socket, _) = upstream.accept().expect("upstream accept");
        expect_bytes(&mut socket, &request);
        socket.write_all(&response).expect("response");
    });
    let (address, proxy_handle) = if via_proxy {
        let (address, handle) = proxy(upstream_address).expect("start proxy");
        (address, Some(handle))
    } else {
        (upstream_address, None)
    };
    let mut client = TcpStream::connect(address).expect("connect");
    let start = Instant::now();
    client.write_all(&exchange.request).expect("request");
    let mut actual = vec![0; exchange.response.len()];
    client.read_exact(&mut actual[..1]).expect("first byte");
    let elapsed = start.elapsed();
    client.read_exact(&mut actual[1..]).expect("response");
    assert_eq!(actual, exchange.response);
    drop(client);
    upstream_handle.join().expect("upstream thread");
    if let Some(handle) = proxy_handle {
        finish(handle);
    }
    elapsed
}

#[test]
fn paired_fixture_first_byte_overhead_is_below_twenty_ms() {
    let exchange = responses(false);
    let _ = first_byte(&exchange, false);
    let _ = first_byte(&exchange, true);
    let mut overheads = Vec::new();
    for _ in 0..21 {
        let direct = first_byte(&exchange, false);
        let proxied = first_byte(&exchange, true);
        overheads.push(proxied.saturating_sub(direct));
    }
    overheads.sort();
    let median = overheads[10];
    let p95 = overheads[19];
    eprintln!("P0-01 paired first-byte overhead: median {median:?}, p95 {p95:?}");
    assert!(median < Duration::from_millis(20));
    assert!(p95 < Duration::from_millis(20));
}
