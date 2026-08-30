use std::{
    fs::{OpenOptions, remove_file},
    io::{self, BufRead, BufReader, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use lao_run_api::{Endpoint, Mode, Status};

mod resource;

pub use resource::plan;

pub const BUILD: &str = "version: 10280 (61881b1f7)";

pub struct Config<'a> {
    pub bin: &'a Path,
    pub model: &'a Path,
    pub mode: Mode,
    pub working_set: u64,
    pub context: u32,
    pub threads: u16,
}

pub struct Direct {
    child: Child,
    endpoint: Endpoint,
    log: Option<JoinHandle<()>>,
}

struct Key(std::path::PathBuf);

impl Key {
    fn new(bearer: &str) -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!("lao-{}.key", &bearer[..16]));
        let key = Self(path);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&key.0)?;
        file.write_all(bearer.as_bytes())?;
        Ok(key)
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        let _ = remove_file(&self.0);
    }
}

impl Direct {
    pub fn start(config: Config<'_>) -> io::Result<Self> {
        let budget = plan(config.bin, config.mode)?;
        if config.working_set == 0
            || config.working_set > budget.bytes
            || config.context == 0
            || config.threads == 0
            || config.threads > budget.threads
        {
            return Err(invalid("config"));
        }
        let mut bytes = [0; 32];
        getrandom::getrandom(&mut bytes).map_err(|_| invalid("random"))?;
        let bearer = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let key = Key::new(&bearer)?;

        let mut child = Command::new(config.bin)
            .args([
                "--model",
                config.model.to_str().ok_or_else(|| invalid("model"))?,
            ])
            .args(["--host", "127.0.0.1", "--port", "0"])
            .args([
                "--api-key-file",
                key.0.to_str().ok_or_else(|| invalid("key"))?,
            ])
            .args(["--ctx-size", &config.context.to_string()])
            .args(["--threads", &config.threads.to_string()])
            .args(["--threads-batch", &config.threads.to_string()])
            .args([
                "--parallel",
                "1",
                "--fit",
                "off",
                "--cache-ram",
                "0",
                "--no-webui",
                "--metrics",
                "--jinja",
                "--reasoning",
                "off",
                "--timeout",
                "30",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let Some(stderr) = child.stderr.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(invalid("stderr"));
        };
        let (send, receive) = mpsc::sync_channel(1);
        let log = thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Some((_, port)) = line.rsplit_once("listening on http://127.0.0.1:")
                    && let Ok(port) = port.parse()
                {
                    let _ = send.try_send(port);
                }
            }
        });
        let port = match receive.recv_timeout(Duration::from_secs(15)) {
            Ok(port) => port,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = log.join();
                return Err(invalid("startup"));
            }
        };
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        if !matches!(health(addr), Ok(true)) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = log.join();
            return Err(invalid("health"));
        }
        drop(key);
        Ok(Self {
            child,
            endpoint: Endpoint::new(addr, bearer),
            log: Some(log),
        })
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn stop(mut self) -> io::Result<()> {
        self.end()
    }

    fn end(&mut self) -> io::Result<()> {
        if self.log.is_none() {
            return Ok(());
        }
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
        }
        self.child.wait()?;
        if let Some(log) = self.log.take() {
            let _ = log.join();
        }
        Ok(())
    }
}

impl Drop for Direct {
    fn drop(&mut self) {
        let _ = self.end();
    }
}

fn health(addr: SocketAddr) -> io::Result<bool> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response.starts_with("HTTP/1.1 200"))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn status() -> Status {
    Status::Active
}
