use candle_semantic_router::BertSimilarity;
use lao_route_api::{Client, Context, Decision, Policy};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

const WAIT: Duration = Duration::from_secs(2);
const REQUEST_LIMIT: usize = 2 * 1024 * 1024;
const RESPONSE_LIMIT: usize = 256 * 1024;
const REVISION: &str = "1110a243fdf4706b3f48f1d95db1a4f5529b4d41";
const FILES: [Asset; 3] = [
    Asset::new(
        "config.json",
        612,
        "953f9c0d463486b10a6871cc2fd59f223b2c70184f49815e7efbcab5d8908b41",
    ),
    Asset::new(
        "tokenizer.json",
        466_247,
        "be50c3628f2bf5bb5e3a7f17b1f74611b2561a3a27eeab05e5aa30f411572037",
    ),
    Asset::new(
        "model.safetensors",
        90_868_376,
        "53aa51172d142c89d9012cce15ae4d6cc0ca6895895114379cacb4fab128d9db",
    ),
];

pub struct Router;

impl Policy for Router {
    fn decide(&self, context: Context) -> Decision {
        if context.is_canary() {
            Decision::Local
        } else {
            Decision::Cloud
        }
    }
}

pub struct VllmSemantic {
    address: SocketAddrV4,
    bearer: Option<String>,
}

pub struct Semantic {
    state: Mutex<State>,
}

enum State {
    Cold(PathBuf),
    Ready(Box<Loaded>),
    Failed,
}

struct Loaded {
    model: BertSimilarity,
    easy: Vec<Vec<f32>>,
    hard: Vec<Vec<f32>>,
    work_easy: Vec<Vec<f32>>,
}

impl Semantic {
    pub fn open(root: &Path) -> io::Result<Self> {
        if !root.is_absolute() {
            return Err(invalid("router model"));
        }
        Ok(Self {
            state: Mutex::new(State::Cold(root.to_owned())),
        })
    }

    fn query(&self, context: Context, query: &str) -> Option<Decision> {
        if query.len() > 4096 || risky(query) {
            return Some(Decision::Cloud);
        }
        let mut state = self.state.try_lock().ok()?;
        if let State::Cold(root) = &*state {
            *state = match Loaded::open(root) {
                Some(loaded) => State::Ready(Box::new(loaded)),
                None => State::Failed,
            };
        }
        let State::Ready(loaded) = &*state else {
            return Some(Decision::Cloud);
        };
        let query = vector(&loaded.model, query)?;
        let easy = score(
            &query,
            if context.client() == Client::Worker {
                &loaded.work_easy
            } else {
                &loaded.easy
            },
        )?;
        let hard = score(&query, &loaded.hard)?;
        Some(if hard - easy < -0.08 {
            Decision::Local
        } else {
            Decision::Cloud
        })
    }
}

impl Loaded {
    fn open(root: &Path) -> Option<Self> {
        verify(root).ok()?;
        let model = BertSimilarity::new(root.to_str()?, true).ok()?;
        let easy = embed(
            &model,
            &[
                "Reply with one exact short value.",
                "Answer a simple factual question directly.",
                "Make no changes and call no tools.",
            ],
        )
        .ok()?;
        let hard = embed(
            &model,
            &[
                "Implement architecture changes across multiple source files and run end-to-end tests.",
                "Investigate and debug an unknown concurrency or security root cause.",
                "Research, plan, and build a long multi-step software feature.",
            ],
        )
        .ok()?;
        let work_easy = embed(
            &model,
            &[
                "Make one small mechanical code change in one named file and run one existing test.",
                "Fix a narrow typo or formatting issue and verify the exact result.",
                "Add one bounded test for already understood behavior.",
            ],
        )
        .ok()?;
        Some(Self {
            model,
            easy,
            hard,
            work_easy,
        })
    }
}

impl Policy for Semantic {
    fn decide(&self, context: Context) -> Decision {
        if context.is_canary() {
            Decision::Local
        } else {
            Decision::Cloud
        }
    }

    fn requires_query(&self) -> bool {
        true
    }

    fn decide_query(&self, context: Context, query: &str) -> Decision {
        if context.is_canary() {
            return Decision::Local;
        }
        self.query(context, query).unwrap_or(Decision::Cloud)
    }
}

pub const fn semantic_bytes() -> u64 {
    FILES[0].bytes + FILES[1].bytes + FILES[2].bytes
}

pub fn prepare(root: &Path) -> io::Result<()> {
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    for asset in FILES {
        let path = root.join(asset.name);
        if valid(&path, asset)? {
            continue;
        }
        let part = root.join(format!(".{}.part", asset.name));
        let mut pending = Pending(Some(part.clone()));
        let _ = fs::remove_file(&part);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&part)?;
        let url = format!(
            "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/{REVISION}/{}",
            asset.name
        );
        let status = Command::new("/usr/bin/curl")
            .args(["--fail", "--location", "--silent", "--show-error"])
            .args(["--proto", "=https", "--proto-redir", "=https"])
            .args(["--max-filesize", &asset.bytes.to_string()])
            .args(["--connect-timeout", "30", "--max-time", "600"])
            .arg("--output")
            .arg(&part)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() || !valid(&part, asset)? {
            return Err(invalid("router model download"));
        }
        fs::rename(&part, path)?;
        pending.0 = None;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Asset {
    name: &'static str,
    bytes: u64,
    sha256: &'static str,
}

impl Asset {
    const fn new(name: &'static str, bytes: u64, sha256: &'static str) -> Self {
        Self {
            name,
            bytes,
            sha256,
        }
    }
}

struct Pending(Option<PathBuf>);

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

fn verify(root: &Path) -> io::Result<()> {
    for asset in FILES {
        if !valid(&root.join(asset.name), asset)? {
            return Err(invalid("router model"));
        }
    }
    Ok(())
}

fn valid(path: &Path, asset: Asset) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() || metadata.len() != asset.bytes {
        return Ok(false);
    }
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()?;
    let hash = String::from_utf8(output.stdout).map_err(|_| invalid("router model"))?;
    Ok(output.status.success() && hash.split_whitespace().next() == Some(asset.sha256))
}

fn embed(model: &BertSimilarity, candidates: &[&str]) -> io::Result<Vec<Vec<f32>>> {
    candidates
        .iter()
        .map(|candidate| vector(model, candidate).ok_or_else(|| invalid("router model")))
        .collect()
}

fn vector(model: &BertSimilarity, text: &str) -> Option<Vec<f32>> {
    model
        .get_embedding(text, Some(128))
        .ok()?
        .squeeze(0)
        .ok()?
        .to_vec1::<f32>()
        .ok()
}

fn score(query: &[f32], candidates: &[Vec<f32>]) -> Option<f32> {
    let mut best = f32::NEG_INFINITY;
    let mut second = f32::NEG_INFINITY;
    let mut count = 0;
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.len() == query.len())
    {
        let value = query
            .iter()
            .zip(candidate)
            .map(|(left, right)| left * right)
            .sum::<f32>();
        if value > best {
            second = best;
            best = value;
        } else if value > second {
            second = value;
        }
        count += 1;
    }
    if count == 0 {
        return None;
    }
    let support = if count == 1 {
        best
    } else {
        (best + second) / 2.0
    };
    Some(0.75 * best + 0.25 * support)
}

fn risky(query: &str) -> bool {
    let query = query.to_ascii_lowercase();
    [
        "password",
        "credential",
        "secret",
        "api key",
        "authentication",
        "authorization",
        "security",
        "cryptograph",
        "delete",
        "destroy",
        "drop database",
        "production",
        "deploy",
        "publish",
        "upload",
        "download",
        "install",
        "sudo",
        "permission",
        "git push",
        "release",
        "browse",
        "web search",
        "current news",
        "latest version",
    ]
    .iter()
    .any(|marker| query.contains(marker))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl VllmSemantic {
    pub fn new(address: SocketAddrV4, bearer: Option<String>) -> Self {
        Self { address, bearer }
    }

    fn query(&self, query: &str) -> Option<Decision> {
        let body = serde_json::to_vec(&serde_json::json!({ "text": query })).ok()?;
        decision(post(
            self.address,
            self.bearer.as_deref(),
            "/api/v1/eval",
            &body,
        )?)
    }
}

impl Policy for VllmSemantic {
    fn decide(&self, context: Context) -> Decision {
        if context.is_canary() {
            Decision::Local
        } else {
            Decision::Cloud
        }
    }

    fn requires_query(&self) -> bool {
        true
    }

    fn decide_query(&self, context: Context, query: &str) -> Decision {
        if context.is_canary() {
            return Decision::Local;
        }
        self.query(query).unwrap_or(Decision::Cloud)
    }
}

fn remaining(deadline: Instant) -> Option<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
}

fn valid_bearer(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && value.bytes().all(|byte| byte > b' ' && byte < 0x7f)
}

fn post(address: SocketAddrV4, bearer: Option<&str>, path: &str, body: &[u8]) -> Option<Value> {
    if *address.ip() != Ipv4Addr::LOCALHOST || body.len() > REQUEST_LIMIT {
        return None;
    }
    let bearer = match bearer {
        Some(value) if valid_bearer(value) => format!("Authorization: Bearer {value}\r\n"),
        Some(_) => return None,
        None => String::new(),
    };
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{bearer}\r\n",
        body.len()
    );
    let deadline = Instant::now() + WAIT;
    let mut stream =
        TcpStream::connect_timeout(&SocketAddr::V4(address), remaining(deadline)?).ok()?;
    stream.set_write_timeout(Some(remaining(deadline)?)).ok()?;
    stream.write_all(request.as_bytes()).ok()?;
    stream.write_all(body).ok()?;
    stream.flush().ok()?;
    let mut response = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        stream.set_read_timeout(Some(remaining(deadline)?)).ok()?;
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        if response.len() + read > RESPONSE_LIMIT {
            return None;
        }
        response.extend_from_slice(&chunk[..read]);
    }
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&response[..split]).ok()?;
    let status = head.lines().next()?;
    if status != "HTTP/1.1 200 OK" && status != "HTTP/1.0 200 OK" {
        return None;
    }
    let body = &response[split + 4..];
    if head.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
    }) {
        serde_json::from_slice(&unchunk(body)?).ok()
    } else {
        serde_json::from_slice(body).ok()
    }
}

fn unchunk(mut bytes: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line = bytes.windows(2).position(|window| window == b"\r\n")?;
        let size =
            usize::from_str_radix(std::str::from_utf8(&bytes[..line]).ok()?.trim(), 16).ok()?;
        bytes = &bytes[line + 2..];
        if size == 0 {
            return bytes.starts_with(b"\r\n").then_some(body);
        }
        if size > bytes.len() || body.len() + size > RESPONSE_LIMIT {
            return None;
        }
        body.extend_from_slice(&bytes[..size]);
        bytes = bytes.get(size..)?;
        if !bytes.starts_with(b"\r\n") {
            return None;
        }
        bytes = &bytes[2..];
    }
}

fn decision(value: Value) -> Option<Decision> {
    let route = match value.get("selected_model") {
        Some(value) => value.as_str()?,
        None => value.get("routing_decision")?.as_str()?,
    };
    match route {
        "lao-local" => Some(Decision::Local),
        "lao-cloud" => Some(Decision::Cloud),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lao_route_api::{Client, Op};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    #[test]
    fn only_canary_is_local() {
        assert!(!Router.requires_query());
        assert_eq!(
            Router.decide(Context::new(Client::Codex, Op::Responses)),
            Decision::Cloud
        );
        assert_eq!(
            Router.decide(Context::canary(Client::Claude, Op::Messages)),
            Decision::Local
        );
    }

    #[test]
    fn semantic_router_uses_only_the_bounded_decision_api() {
        let (address, server) = serve(2, |request, call| {
            assert!(request.starts_with("POST /api/v1/eval HTTP/1.1\r\n"));
            assert!(request.contains("Authorization: Bearer private-router-key\r\n"));
            let body = request.split_once("\r\n\r\n").unwrap().1;
            let query = &serde_json::from_str::<Value>(body).unwrap()["text"];
            if call == 0 {
                assert_eq!(query, "small task");
                chunked(r#"{"selected_model":"lao-local"}"#)
            } else {
                assert_eq!(query, "hard task");
                response(r#"{"routing_decision":"lao-cloud"}"#)
            }
        });
        let router = VllmSemantic::new(address, Some("private-router-key".into()));

        assert!(router.requires_query());
        assert_eq!(
            router.decide_query(Context::new(Client::Codex, Op::Responses), "small task"),
            Decision::Local
        );
        assert_eq!(
            router.decide_query(Context::canary(Client::Claude, Op::Messages), "canary"),
            Decision::Local
        );
        assert_eq!(
            router.decide_query(Context::new(Client::Claude, Op::Messages), "hard task"),
            Decision::Cloud
        );
        server.join().unwrap();
    }

    #[test]
    fn semantic_router_failure_is_cloud() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            SocketAddr::V4(address) => address,
            SocketAddr::V6(_) => unreachable!(),
        };
        drop(listener);
        let router = VllmSemantic::new(address, None);

        assert_eq!(
            router.decide_query(Context::new(Client::Claude, Op::Messages), "task"),
            Decision::Cloud
        );
    }

    #[test]
    fn failed_local_model_load_is_cached_until_restart() {
        let root = std::env::temp_dir().join(format!("lao-missing-router-{}", std::process::id()));
        let router = Semantic::open(&root).unwrap();
        let context = Context::new(Client::Codex, Op::Responses);

        assert_eq!(router.decide_query(context, "small task"), Decision::Cloud);
        assert!(matches!(*router.state.lock().unwrap(), State::Failed));
        assert_eq!(router.decide_query(context, "small task"), Decision::Cloud);
    }

    #[test]
    #[ignore = "downloads and runs the pinned MiniLM semantic model"]
    fn real_semantic_model_keeps_easy_local_and_hard_cloud() {
        let root = PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library/Caches/lao/routers/minilm");
        prepare(&root).unwrap();
        let router = Semantic::open(&root).unwrap();
        assert_eq!(
            router.decide_query(
                Context::new(Client::Codex, Op::Responses),
                "Correct the spelling error in this one word: teh. Reply with only the corrected word."
            ),
            Decision::Local
        );
        assert_eq!(
            router.decide_query(
                Context::new(Client::Codex, Op::Responses),
                "Research current dependencies, redesign the architecture across many files, and deploy it to production."
            ),
            Decision::Cloud
        );
        assert_eq!(
            router.decide_query(
                Context::new(Client::Worker, Op::Chat),
                "Change only word.txt from teh to the and run the existing ./verify.sh check."
            ),
            Decision::Local
        );
        assert_eq!(
            router.decide_query(
                Context::new(Client::Worker, Op::Chat),
                "Plan and implement a broad authentication architecture across many files."
            ),
            Decision::Cloud
        );
    }

    fn serve(
        calls: usize,
        mut handler: impl FnMut(String, usize) -> Vec<u8> + Send + 'static,
    ) -> (SocketAddrV4, thread::JoinHandle<()>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            SocketAddr::V4(address) => address,
            SocketAddr::V6(_) => unreachable!(),
        };
        let server = thread::spawn(move || {
            for call in 0..calls {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(WAIT)).unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0; 1024];
                loop {
                    let read = stream.read(&mut chunk).unwrap();
                    bytes.extend_from_slice(&chunk[..read]);
                    let Some(split) = bytes.windows(4).position(|window| window == b"\r\n\r\n")
                    else {
                        continue;
                    };
                    let head = std::str::from_utf8(&bytes[..split]).unwrap();
                    let length = head
                        .lines()
                        .find_map(|line| line.strip_prefix("Content-Length: "))
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    if bytes.len() >= split + 4 + length {
                        break;
                    }
                }
                let reply = handler(String::from_utf8(bytes).unwrap(), call);
                stream.write_all(&reply).unwrap();
            }
        });
        (address, server)
    }

    fn response(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn chunked(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
            body.len()
        )
        .into_bytes()
    }
}
