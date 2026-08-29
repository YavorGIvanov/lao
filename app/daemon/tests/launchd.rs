#![cfg(target_os = "macos")]

use std::{
    env, fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[test]
fn direct_start_fails_closed() {
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_lao-daemon"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}

#[test]
#[ignore = "set LAO_LAUNCHD_E2E=1; may prompt for an ad-hoc local build"]
fn socket_survives_daemon_failure() {
    if env::var("LAO_LAUNCHD_E2E").as_deref() != Ok("1") {
        return;
    }
    let held = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = held.local_addr().unwrap().port();
    let mut blocked = Job::new(port, "blocked");
    assert!(!blocked.start());
    drop(held);
    assert!(blocked.stop(), "{}", blocked.error());
    wait_until_free(port);

    let mut active = Job::new(port, "active");
    assert!(active.start(), "{}", active.error());
    hello(port, &active);
    rejected_payload(port);
    assert!(TcpListener::bind(("127.0.0.1", port)).is_err());
    assert!(
        Command::new("/bin/launchctl")
            .args(["kill", "SIGKILL", &active.service])
            .status()
            .unwrap()
            .success()
    );
    thread::sleep(Duration::from_millis(250));
    assert!(TcpListener::bind(("127.0.0.1", port)).is_err());
    hello(port, &active);
    assert!(active.stop());
    wait_until_closed(port);
}

struct Job {
    domain: String,
    service: String,
    dir: PathBuf,
    port: u16,
    loaded: bool,
}

impl Job {
    fn new(port: u16, suffix: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let label = format!("com.lao.socket.{suffix}.{}.{stamp}", std::process::id());
        let dir = env::temp_dir().join(&label);
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let domain = format!("gui/{}", uid());
        let service = format!("{domain}/{label}");
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>{label}</string>\n<key>ProgramArguments</key><array><string>{bin}</string></array>\n<key>EnvironmentVariables</key><dict><key>LAO_ADOPTED_FILE</key><string>{adopted}</string></dict>\n<key>RunAtLoad</key><true/>\n<key>ThrottleInterval</key><integer>1</integer>\n<key>Sockets</key><dict><key>gate</key><dict><key>SockNodeName</key><string>127.0.0.1</string><key>SockServiceName</key><integer>{port}</integer><key>SockFamily</key><string>IPv4</string><key>SockType</key><string>stream</string><key>SockProtocol</key><string>TCP</string><key>SockPassive</key><true/></dict></dict>\n<key>StandardErrorPath</key><string>{err}</string>\n</dict></plist>\n",
            bin = escape(env!("CARGO_BIN_EXE_lao-daemon")),
            adopted = escape(&dir.join("adopted").to_string_lossy()),
            err = escape(&dir.join("err").to_string_lossy()),
        );
        let path = dir.join("job.plist");
        fs::write(&path, plist).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            Command::new("/usr/bin/plutil")
                .args(["-lint"])
                .arg(&path)
                .stdout(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        Self {
            domain,
            service,
            dir,
            port,
            loaded: false,
        }
    }

    fn start(&mut self) -> bool {
        let loaded = Command::new("/bin/launchctl")
            .args(["bootstrap", &self.domain])
            .arg(self.dir.join("job.plist"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        self.loaded = loaded;
        loaded && self.adopted()
    }

    fn adopted(&self) -> bool {
        let until = Instant::now() + Duration::from_secs(20);
        while Instant::now() < until {
            if fs::read_to_string(self.dir.join("adopted"))
                .is_ok_and(|value| value == format!("127.0.0.1:{}", self.port))
            {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }

    fn stop(&mut self) -> bool {
        if !self.loaded {
            return true;
        }
        let stopped = Command::new("/bin/launchctl")
            .args(["bootout", &self.service])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        self.loaded = !stopped;
        stopped
    }

    fn error(&self) -> String {
        fs::read_to_string(self.dir.join("err")).unwrap_or_default()
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if self.stop() {
            let _ = fs::remove_dir_all(&self.dir);
        } else {
            eprintln!(
                "launchd cleanup failed: {} ({})",
                self.service,
                self.dir.display()
            );
        }
    }
}

fn hello(port: u16, job: &Job) {
    let response = exchange(
        port,
        format!(
            "HEAD /ant/api/hello HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        )
        .as_bytes(),
    );
    assert!(
        response.starts_with(b"HTTP/1.1 200 OK\r\n"),
        "response={} error={}",
        String::from_utf8_lossy(&response),
        job.error()
    );
    assert!(response.ends_with(b"\r\n\r\n"));
}

fn rejected_payload(port: u16) {
    let response = exchange(
        port,
        format!(
            "POST /oai/responses HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
        )
        .as_bytes(),
    );
    assert!(!response.starts_with(b"HTTP/1.1 200"));
}

fn exchange(port: u16, request: &[u8]) -> Vec<u8> {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(mut stream) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream.write_all(request).unwrap();
                let mut response = Vec::new();
                let mut chunk = [0; 1024];
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(count) => {
                            response.extend_from_slice(&chunk[..count]);
                            if response.windows(4).any(|part| part == b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) =>
                        {
                            break;
                        }
                        Err(error) => panic!("read: {error}"),
                    }
                }
                return response;
            }
            Err(_) => {
                assert!(Instant::now() < until, "launchd did not accept");
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn wait_until_free(port: u16) {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(_) => return,
            Err(_) => {
                assert!(Instant::now() < until, "launchd did not release socket");
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn wait_until_closed(port: u16) {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_err() {
            return;
        }
        assert!(Instant::now() < until, "launchd did not close socket");
        thread::sleep(Duration::from_millis(20));
    }
}

fn uid() -> String {
    let output = Command::new("/usr/bin/id").arg("-u").output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
