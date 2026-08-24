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
    let down_read = downstream.try_clone()?;
    let up_read = upstream.try_clone()?;
    let (request, response) = thread::scope(|scope| {
        let request = scope.spawn(|| pump(down_read, upstream));
        let response = scope.spawn(|| pump(up_read, downstream));
        (
            request.join().expect("request pump"),
            response.join().expect("response pump"),
        )
    });
    if request.is_ok() || response.is_ok() {
        Ok(())
    } else {
        request.and(response).map(|_| ())
    }
}

fn pump(mut reader: TcpStream, mut writer: TcpStream) -> io::Result<u64> {
    let result = io::copy(&mut reader, &mut writer);
    let _ = reader.shutdown(Shutdown::Both);
    let _ = writer.shutdown(Shutdown::Both);
    result
}

fn stream_fixture(request: &'static [u8], response: &'static [u8]) {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("upstream bind");
    let upstream_address = upstream.local_addr().expect("upstream address");
    let (first_request_tx, first_request_rx) = mpsc::channel();
    let (finish_response_tx, finish_response_rx) = mpsc::channel();
    let (received_tx, received_rx) = mpsc::channel();
    let upstream_handle = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().expect("upstream accept");
        stream.set_read_timeout(Some(WAIT)).expect("read timeout");
        let request_cut = request.len() / 2;
        let mut received = vec![0; request.len()];
        stream
            .read_exact(&mut received[..request_cut])
            .expect("first request chunk");
        first_request_tx.send(()).expect("first request signal");
        stream
            .read_exact(&mut received[request_cut..])
            .expect("last request chunk");
        received_tx.send(received).expect("received request");

        let response_cut = response.len() / 2;
        stream
            .write_all(&response[..response_cut])
            .expect("first response chunk");
        stream.flush().expect("flush response");
        finish_response_rx
            .recv_timeout(WAIT)
            .expect("response release");
        stream
            .write_all(&response[response_cut..])
            .expect("last response chunk");
    });

    let (address, proxy_handle) = proxy(upstream_address).expect("start proxy");
    let mut client = TcpStream::connect(address).expect("connect proxy");
    client.set_read_timeout(Some(WAIT)).expect("read timeout");
    let request_cut = request.len() / 2;
    client
        .write_all(&request[..request_cut])
        .expect("first request write");
    client.flush().expect("flush request");
    first_request_rx
        .recv_timeout(WAIT)
        .expect("request streamed before complete");
    client
        .write_all(&request[request_cut..])
        .expect("last request write");

    let response_cut = response.len() / 2;
    let mut actual = vec![0; response.len()];
    client
        .read_exact(&mut actual[..response_cut])
        .expect("response streamed before complete");
    finish_response_tx.send(()).expect("release response");
    client
        .read_exact(&mut actual[response_cut..])
        .expect("last response read");

    assert_eq!(
        received_rx.recv_timeout(WAIT).expect("request bytes"),
        request
    );
    assert_eq!(actual, response);
    drop(client);
    upstream_handle.join().expect("upstream thread");
    proxy_handle
        .join()
        .expect("proxy thread")
        .expect("proxy result");
}

#[test]
fn responses_and_messages_stream_byte_exact() {
    stream_fixture(RESPONSES_REQUEST, RESPONSES_RESPONSE);
    stream_fixture(MESSAGES_REQUEST, MESSAGES_RESPONSE);
}

#[test]
fn downstream_cancel_closes_upstream() {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("upstream bind");
    let address = upstream.local_addr().expect("upstream address");
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let upstream_handle = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().expect("upstream accept");
        stream.set_read_timeout(Some(WAIT)).expect("read timeout");
        let mut byte = [0];
        stream.read_exact(&mut byte).expect("sentinel");
        accepted_tx.send(()).expect("accepted signal");
        assert_eq!(stream.read(&mut byte).expect("upstream close"), 0);
    });
    let (proxy_address, proxy_handle) = proxy(address).expect("start proxy");
    let mut client = TcpStream::connect(proxy_address).expect("connect proxy");
    client.write_all(b"x").expect("write sentinel");
    accepted_rx.recv_timeout(WAIT).expect("upstream accepted");
    drop(client);
    upstream_handle.join().expect("upstream thread");
    proxy_handle
        .join()
        .expect("proxy thread")
        .expect("proxy result");
}

fn round_trip(via_proxy: bool) -> Duration {
    let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("upstream bind");
    let upstream_address = upstream.local_addr().expect("upstream address");
    let upstream_handle = thread::spawn(move || {
        let (mut stream, _) = upstream.accept().expect("upstream accept");
        let mut byte = [0];
        stream.read_exact(&mut byte).expect("request byte");
        stream.write_all(&byte).expect("response byte");
    });
    let (address, proxy_handle) = if via_proxy {
        let (address, handle) = proxy(upstream_address).expect("start proxy");
        (address, Some(handle))
    } else {
        (upstream_address, None)
    };
    let mut client = TcpStream::connect(address).expect("connect");
    let start = Instant::now();
    client.write_all(b"x").expect("write");
    let mut byte = [0];
    client.read_exact(&mut byte).expect("read");
    let elapsed = start.elapsed();
    drop(client);
    upstream_handle.join().expect("upstream thread");
    if let Some(handle) = proxy_handle {
        handle.join().expect("proxy thread").expect("proxy result");
    }
    elapsed
}

#[test]
fn median_loopback_overhead_is_below_twenty_ms() {
    let mut direct: Vec<_> = (0..21).map(|_| round_trip(false)).collect();
    let mut proxied: Vec<_> = (0..21).map(|_| round_trip(true)).collect();
    direct.sort();
    proxied.sort();
    let overhead = proxied[10].saturating_sub(direct[10]);
    eprintln!("P0-01 median loopback overhead: {overhead:?}");
    assert!(overhead < Duration::from_millis(20));
}
