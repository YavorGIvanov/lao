use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    time::Duration,
};

use lao_run::{Config, Direct};
use lao_run_api::Mode;

#[test]
#[ignore = "uses the pinned installed llama-server and verified local model"]
fn direct_llama_cpp_serves_and_stops() {
    let bin = env::var_os("LAO_LLAMA_SERVER")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/opt/homebrew/bin/llama-server".into());
    let model = env::var_os("LAO_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap())
                .join("Library/Caches/lao/models/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf")
        });
    let (runtime, endpoint) = Direct::start(Config {
        bin: &bin,
        model: &model,
        mode: Mode::Light,
        working_set: 3 * 1024 * 1024 * 1024,
        context: 32_768,
        threads: 2,
    })
    .unwrap();
    let addr = endpoint.addr();
    assert!(addr.ip().is_loopback());
    assert_eq!(endpoint.bearer().len(), 64);

    assert!(request(addr, None).starts_with("HTTP/1.1 401"));
    let props = get(addr, endpoint.bearer(), "/props");
    let body = props.split_once("\r\n\r\n").unwrap().1;
    let json: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(json["default_generation_settings"]["n_ctx"], 32_768);
    let response = request(addr, Some(endpoint.bearer()));
    assert!(response.starts_with("HTTP/1.1 200"));
    let body = response.split_once("\r\n\r\n").unwrap().1;
    let json: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(json["model"], "lao-local");
    assert_eq!(json["choices"][0]["message"]["content"], "42");

    runtime.stop().unwrap();
    TcpListener::bind(addr).unwrap();
}

fn get(addr: std::net::SocketAddr, bearer: &str, path: &str) -> String {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {bearer}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn request(addr: std::net::SocketAddr, bearer: Option<&str>) -> String {
    let body = r#"{"model":"local","messages":[{"role":"user","content":"Return only the number produced by 20 + 22."}],"temperature":0,"max_tokens":16}"#;
    let auth = bearer
        .map(|key| format!("Authorization: Bearer {key}\r\n"))
        .unwrap_or_default();
    let head = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}
