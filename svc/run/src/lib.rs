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

use lao_run_api::{Endpoint, Status};

pub use lao_run_api::Mode;

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
    log: Option<JoinHandle<()>>,
}

struct Key(std::path::PathBuf);

impl Key {
    // The name is independent of the bearer: a directory listing is readable by
    // any process, and a managed token must never appear in one.
    fn new(bearer: &str) -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!("lao-{}.key", hex::<8>()?));
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
    pub fn start(config: Config<'_>) -> io::Result<(Self, Endpoint)> {
        let budget = plan(config.bin, config.mode)?;
        if config.working_set == 0
            || config.working_set > budget.bytes
            || config.context == 0
            || config.threads == 0
            || config.threads > budget.threads
        {
            return Err(invalid("config"));
        }
        let bearer = hex::<32>()?;
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
                "--temp",
                "0",
                "--alias",
                "lao-local",
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
        let Ok(port) = receive.recv_timeout(Duration::from_secs(15)) else {
            return Err(fail(child, log, "startup"));
        };
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        if !matches!(health(addr), Ok(true)) {
            return Err(fail(child, log, "health"));
        }
        drop(key);
        if !matches!(rss(child.id()), Ok(bytes) if bytes <= budget.bytes) {
            return Err(fail(child, log, "memory"));
        }
        Ok((
            Self {
                child,
                log: Some(log),
            },
            Endpoint::new(addr, bearer),
        ))
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

// Loaded weights, KV cache, and compute buffers are resident, so RSS is the
// cheapest honest reading of what the child took from the shared Apple pool.
fn rss(pid: u32) -> io::Result<u64> {
    let observed = Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()?;
    String::from_utf8_lossy(&observed.stdout)
        .trim()
        .parse::<u64>()
        .map(|kib| kib << 10)
        .map_err(|_| invalid("memory"))
}

fn fail(mut child: Child, log: JoinHandle<()>, message: &'static str) -> io::Error {
    let _ = child.kill();
    let _ = child.wait();
    let _ = log.join();
    invalid(message)
}

fn hex<const N: usize>() -> io::Result<String> {
    let mut bytes = [0; N];
    getrandom::getrandom(&mut bytes).map_err(|_| invalid("random"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn status() -> Status {
    Status::Active
}

#[cfg(test)]
mod tests {
    use super::*;

    // S1-01 "stop leaves no key file" and vision 7.3: a managed token never
    // reaches a product-controlled name, and the file goes away on drop.
    #[test]
    fn key_hides_the_bearer_and_is_removed() {
        let bearer = hex::<32>().unwrap();
        let key = Key::new(&bearer).unwrap();
        let other = Key::new(&bearer).unwrap();
        assert_ne!(key.0, other.0);
        assert!(!key.0.to_string_lossy().contains(&bearer));
        let path = key.0.clone();
        drop(key);
        assert!(!path.exists());
    }
}
