use lao_codex::{Auth, Support, auth, support};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PREFIX: &str = "oai";
const KEY: &str = "sk-lao-p0-02-only";
const CALLER: &str = "codex-caller-sentinel";
const SSE: &str = concat!(include_str!("fixtures/response.sse"), "\n");

#[test]
#[ignore = "opt-in probe for installed Codex 0.151.0"]
fn installed_codex_custom_provider_keeps_native_and_caller_auth_separate() {
    let home = Temp::new();
    let bin = std::env::var_os("LAO_CODEX_BIN").unwrap_or_else(|| "codex".into());
    let version = run(isolated(&bin, &home.0).arg("--version"), None);
    assert!(version.status.success());
    assert_eq!(
        support(&String::from_utf8_lossy(&version.stdout)),
        Support::Observed
    );
    assert_clean(&version);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    fs::write(
        home.0.join("config.toml"),
        format!(
            "cli_auth_credentials_store = \"file\"\nmodel_provider = \"lao\"\ncheck_for_update_on_startup = false\nweb_search = \"disabled\"\n[analytics]\nenabled = false\n[model_providers.lao]\nname = \"LAO\"\nbase_url = \"http://127.0.0.1:{port}/{PREFIX}\"\nrequires_openai_auth = true\nsupports_websockets = false\nhttp_headers = {{ X-LAO-Key = \"{CALLER}\" }}\n"
        ),
    )
    .unwrap();

    let login = run(
        isolated(&bin, &home.0).args(["login", "--with-api-key"]),
        Some(KEY),
    );
    assert!(
        login.status.success(),
        "{}",
        String::from_utf8_lossy(&login.stderr)
    );
    assert_clean(&login);
    let status = run(isolated(&bin, &home.0).args(["login", "status"]), None);
    assert!(status.status.success());
    let status_text = format!(
        "{}{}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(auth(&status_text), Auth::ApiKey, "{status_text}");
    assert_clean(&status);

    let (sent, received) = mpsc::channel();
    thread::spawn(move || sent.send(serve(listener)).unwrap());
    let output = run(
        isolated(&bin, &home.0).args([
            "exec",
            "--ephemeral",
            "--strict-config",
            "--skip-git-repo-check",
            "--ignore-rules",
            "--color",
            "never",
            "--sandbox",
            "read-only",
            "--model",
            "gpt-5.4",
            "Reply once.",
        ]),
        None,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("LAO_P0_02_OK"));
    assert_clean(&output);

    received
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .expect("fake gateway");
}

fn serve(listener: TcpListener) -> Result<(), String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let (mut stream, _) = match listener.accept() {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        let request = read_request(&mut stream).map_err(|error| error.to_string())?;
        if request.path != format!("/{PREFIX}/responses") {
            write_response(&mut stream, "404 Not Found", "application/json", "{}")
                .map_err(|error| error.to_string())?;
            continue;
        }
        if request.header("authorization") != Some(format!("Bearer {KEY}").as_str()) {
            return Err("request did not use the isolated synthetic credential".into());
        }
        if request.header("x-lao-key") != Some(CALLER) {
            return Err("request did not carry caller authentication".into());
        }
        if request.header("chatgpt-account-id").is_some()
            || request.header("x-openai-fedramp").is_some()
        {
            return Err("request carried unsupported account credentials".into());
        }
        if request.method != "POST" || request.header("upgrade").is_some() {
            return Err("custom provider did not use HTTP Responses".into());
        }
        write_response(&mut stream, "200 OK", "text/event-stream", SSE)
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    Err("Codex did not reach the fake gateway".into())
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut raw = Vec::new();
    let mut chunk = [0; 4096];
    let head_end = loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        raw.extend_from_slice(&chunk[..count]);
        if raw.len() > 64 * 1024 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        if let Some(end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = String::from_utf8_lossy(&raw[..head_end]);
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or_default().to_owned();
    let path = first.next().unwrap_or_default().to_owned();
    let headers: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let length = headers
        .iter()
        .find(|(key, _)| key == "content-length")
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    if length > 2 * 1024 * 1024 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    while raw.len() < head_end + length {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..count]);
    }
    Ok(Request {
        method,
        path,
        headers,
    })
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn assert_clean(output: &Output) {
    for secret in [KEY, CALLER] {
        assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));
    }
}

fn isolated(bin: &OsString, home: &Path) -> Command {
    let mut command = Command::new(bin);
    let path = std::env::var_os("PATH").unwrap_or_default();
    command
        .env_clear()
        .env("PATH", path)
        .env("HOME", home)
        .env("CODEX_HOME", home)
        .current_dir(home);
    command
}

fn run(command: &mut Command, input: Option<&str>) -> Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    panic!(
        "command timed out: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lao-codex-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
