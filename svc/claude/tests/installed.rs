use lao_claude::{Support, preview, support};
use std::{
    env, fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PREFIX: &str = "ant";
const NATIVE: &str = "native-sentinel";
const CALLER: &str = "claude-caller-sentinel";
const SSE: &str = concat!(include_str!("fixtures/message.sse"), "\n");

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("lao-claude-{}-{stamp}", std::process::id()));
        fs::create_dir_all(path.join("home")).unwrap();
        fs::create_dir_all(path.join("config")).unwrap();
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[test]
#[ignore = "requires the probed Claude Code binary; uses only a fake loopback gateway"]
fn installed_claude_contract_opt_in() {
    let binary = binary();
    let temp = Temp::new();
    let version = run(isolated(&binary, &temp).arg("--version"));
    assert!(version.status.success());
    assert_eq!(
        support(&String::from_utf8(version.stdout).unwrap()),
        Support::Observed
    );

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || serve(listener));
    let plan = preview(port).unwrap();
    let settings = format!(
        r#"{{"env":{{"ANTHROPIC_BASE_URL":"{}","ANTHROPIC_AUTH_TOKEN":"{NATIVE}","ANTHROPIC_CUSTOM_HEADERS":"X-LAO-Key: {CALLER}"}}}}"#,
        plan.base_url
    );
    let output = run(isolated(&binary, &temp).args([
        "--bare",
        "--settings",
        &settings,
        "-p",
        "--model",
        "claude-sonnet-4-6",
        "Reply only ok",
    ]));
    let (head, post) = server.join().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stdout.trim(), "ok");
    for secret in [NATIVE, CALLER] {
        assert!(!stdout.contains(secret));
        assert!(!stderr.contains(secret));
    }
    assert_eq!(head.method, "HEAD");
    assert_eq!(head.path, format!("/{PREFIX}/api/hello"));
    assert_eq!(head.header("authorization"), None);
    assert_eq!(head.header("x-lao-key"), None);
    assert!(head.body.is_empty());
    assert_eq!(head.header("transfer-encoding"), None);
    assert_eq!(post.method, "POST");
    assert_eq!(post.path, format!("/{PREFIX}/v1/messages?beta=true"));
    assert_eq!(post.header("authorization"), Some("Bearer native-sentinel"));
    assert_eq!(post.header("x-api-key"), None);
    assert_eq!(post.header("x-lao-key"), Some(CALLER));
    assert_eq!(post.header("anthropic-version"), Some("2023-06-01"));
    assert!(post.header("anthropic-beta").is_some());
    assert!(post.header("x-claude-code-session-id").is_some());
    assert!(
        String::from_utf8(post.body)
            .unwrap()
            .contains(r#""stream":true"#)
    );
}

fn binary() -> PathBuf {
    if let Some(path) = env::var_os("LAO_CLAUDE_BIN") {
        return path.into();
    }
    env::split_paths(&env::var_os("PATH").expect("PATH"))
        .map(|path| path.join("claude"))
        .find(|path| path.is_file())
        .expect("claude is not installed")
}

fn isolated(binary: &PathBuf, temp: &Temp) -> Command {
    let mut command = Command::new(binary);
    command
        .env_clear()
        .env("PATH", env::var_os("PATH").unwrap())
        .env("HOME", temp.0.join("home"))
        .env("CLAUDE_CONFIG_DIR", temp.0.join("config"))
        .env("ANTHROPIC_BASE_URL", "http://127.0.0.1:1/wrong")
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        .current_dir(&temp.0);
    command
}

fn run(command: &mut Command) -> Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    while Instant::now() < until {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    panic!(
        "Claude timed out: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn serve(listener: TcpListener) -> (Request, Request) {
    listener.set_nonblocking(true).unwrap();
    let mut first = accept(&listener);
    let head = read(&mut first);
    first
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .unwrap();

    let mut second = accept(&listener);
    let post = read(&mut second);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{SSE}",
        SSE.len()
    );
    second.write_all(response.as_bytes()).unwrap();
    (head, post)
}

fn accept(listener: &TcpListener) -> TcpStream {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                return stream;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < until, "Claude did not connect");
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("accept: {error}"),
        }
    }
}

fn read(stream: &mut TcpStream) -> Request {
    let mut raw = Vec::new();
    let mut chunk = [0; 4096];
    let split = loop {
        let size = stream.read(&mut chunk).unwrap();
        assert_ne!(size, 0);
        raw.extend_from_slice(&chunk[..size]);
        assert!(raw.len() <= 64 * 1024, "oversized request headers");
        if let Some(index) = raw.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let head = String::from_utf8(raw[..split].to_vec()).unwrap();
    let mut lines = head.lines();
    let mut request = lines.next().unwrap().split_whitespace();
    let method = request.next().unwrap().into();
    let path = request.next().unwrap().into();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.into(), value.trim().into()))
        .collect();
    let length = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    assert!(length <= 2 * 1024 * 1024, "oversized request body");
    while raw.len() - split < length {
        let size = stream.read(&mut chunk).unwrap();
        assert_ne!(size, 0);
        raw.extend_from_slice(&chunk[..size]);
    }
    Request {
        method,
        path,
        headers,
        body: raw[split..split + length].to_vec(),
    }
}
