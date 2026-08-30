mod supervisor;

use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use lao_optimize_api::{Optimize, Plan};
use lao_run_api::{Endpoint, Local};

const PRESSURE_POLL: Duration = Duration::from_secs(5);

pub fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = (lao_model::status(), lao_run::status());
    let listener = supervisor::listener("gate")?;
    if let Some(path) = env::var_os("LAO_ADOPTED_FILE") {
        let address = listener.local_addr()?.to_string();
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => write!(file, "{address}")?,
            Err(error)
                if error.kind() == io::ErrorKind::AlreadyExists
                    && fs::read_to_string(path)? == address => {}
            Err(error) => return Err(error.into()),
        }
    }
    match env::var("LAO_LOCAL_CANARY").as_deref() {
        Err(env::VarError::NotPresent) => lao_gate::closed(listener, lao_route::Router),
        Ok("1") => {
            let port = listener.local_addr()?.port();
            let codex = caller("LAO_CODEX_CALLER")?;
            let claude = caller("LAO_CLAUDE_CALLER")?;
            let codex_cloud = match env::var("LAO_CODEX_CLOUD").as_deref() {
                Ok("openai") => lao_gate::CodexCloud::Api,
                Ok("chatgpt") => lao_gate::CodexCloud::ChatGpt,
                _ => return Err("LAO_CODEX_CLOUD".into()),
            };
            let local = runtime()?;
            let policy: Arc<dyn lao_route_api::Policy> = match env::var("LAO_ROUTER").as_deref() {
                Ok("semantic") => {
                    let root = env::var_os("LAO_ROUTER_DIR").ok_or("LAO_ROUTER_DIR")?;
                    Arc::new(lao_route::Semantic::open(std::path::Path::new(&root))?)
                }
                Ok("safe") | Err(env::VarError::NotPresent) => Arc::new(lao_route::Router),
                Ok("vllm-semantic") => Arc::new(vllm()?),
                _ => return Err("LAO_ROUTER".into()),
            };
            warm(port, codex, claude);
            lao_gate::installed(listener, policy, local, codex, claude, codex_cloud)
        }
        _ => Err("LAO_LOCAL_CANARY".into()),
    }
}

fn runtime() -> Result<Arc<dyn Local>, Box<dyn Error + Send + Sync>> {
    Ok(match env::var("LAO_RUNTIME").as_deref() {
        Ok("external") => {
            let address = env::var("LAO_EXTERNAL_ADDR")?.parse::<SocketAddr>()?;
            Arc::new(lao_run::External::new(
                address,
                key("LAO_EXTERNAL_KEY_FILE")?.into_boxed_str(),
            )?)
        }
        Ok("llama-cpp") | Err(env::VarError::NotPresent) => Arc::new(Lazy::new()?),
        _ => return Err("LAO_RUNTIME".into()),
    })
}

fn vllm() -> Result<lao_route::VllmSemantic, Box<dyn Error + Send + Sync>> {
    let address = match env::var("LAO_VLLM_ROUTER_ADDR") {
        Ok(value) => value.parse::<SocketAddrV4>()?,
        Err(env::VarError::NotPresent) => SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080),
        Err(error) => return Err(error.into()),
    };
    if !address.ip().is_loopback() {
        return Err("LAO_VLLM_ROUTER_ADDR".into());
    }
    let bearer = match env::var("LAO_VLLM_ROUTER_KEY_FILE") {
        Ok(_) => Some(key("LAO_VLLM_ROUTER_KEY_FILE")?),
        Err(env::VarError::NotPresent) => None,
        Err(error) => return Err(error.into()),
    };
    Ok(lao_route::VllmSemantic::new(address, bearer))
}

fn key(name: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let path = env::var_os(name).ok_or(name)?;
    let metadata = fs::symlink_metadata(&path)?;
    let home = env::var_os("HOME").ok_or("HOME")?;
    if !metadata.file_type().is_file()
        || metadata.mode() & 0o7777 != 0o600
        || metadata.uid() != fs::symlink_metadata(home)?.uid()
    {
        return Err(name.into());
    }
    let mut value = fs::read_to_string(path)?;
    while value.ends_with(['\r', '\n']) {
        value.pop();
    }
    if value.is_empty()
        || value.len() > 4096
        || !value.bytes().all(|byte| byte > b' ' && byte < 0x7f)
    {
        return Err(name.into());
    }
    Ok(value)
}

fn warm(port: u16, codex: [u8; 64], claude: [u8; 64]) {
    let (Some(codex_bin), Some(claude_bin)) =
        (env::var_os("LAO_CODEX_BIN"), env::var_os("LAO_CLAUDE_BIN"))
    else {
        return;
    };
    let (Ok(codex), Ok(claude)) = (
        String::from_utf8(codex.to_vec()),
        String::from_utf8(claude.to_vec()),
    ) else {
        return;
    };
    let plan = Plan::new(
        move || lao_optimize::claude(claude_bin, port, &claude).map(|_| ()),
        move || lao_optimize::codex(codex_bin, port, &codex).map(|_| ()),
    );
    let _ = lao_optimize::Optimizer::default().start(plan);
}

/// The optimizer may start the runtime in the background; requests never wait on that policy.
struct Lazy {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    started: Option<(lao_run::Direct, Arc<Endpoint>)>,
}

impl Lazy {
    fn new() -> io::Result<Self> {
        let state = Arc::new(Mutex::new(State::default()));
        let watched = Arc::downgrade(&state);
        drop(
            thread::Builder::new()
                .name("lao-residency".into())
                .spawn(move || watch(watched))?,
        );
        Ok(Self { state })
    }
}

impl Local for Lazy {
    fn endpoint(&self) -> io::Result<Arc<Endpoint>> {
        let mut state = self.state.lock().map_err(|_| io::Error::other("local"))?;
        if state.started.is_none() {
            state.started = Some(start()?);
        }
        state
            .started
            .as_ref()
            .map(|(_, endpoint)| endpoint.clone())
            .ok_or_else(|| io::Error::other("local"))
    }
}

fn start() -> io::Result<(lao_run::Direct, Arc<Endpoint>)> {
    let root = match env::var_os("LAO_MODEL_DIR") {
        Some(root) => PathBuf::from(root),
        None => PathBuf::from(env::var_os("HOME").ok_or_else(|| io::Error::other("HOME"))?)
            .join("Library/Caches/lao/models"),
    };
    let model = lao_model::open(&root)?;
    let bin = PathBuf::from(
        env::var_os("LAO_LLAMA_SERVER").ok_or_else(|| io::Error::other("local runtime"))?,
    );
    let (runtime, endpoint) = lao_run::Direct::start(lao_run::Config {
        bin: &bin,
        model: &model.path,
        mode: lao_run::Mode::Light,
        working_set: model.artifact.working_set,
        context: model.artifact.context,
        threads: 2,
    })?;
    Ok((runtime, Arc::new(endpoint)))
}

fn watch(state: std::sync::Weak<Mutex<State>>) {
    loop {
        thread::sleep(PRESSURE_POLL);
        let Some(state) = state.upgrade() else {
            return;
        };
        let idle = {
            let state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some((_, endpoint)) = state.started.as_ref() else {
                continue;
            };
            // The manager owns one reference; the gate holds one more per active response.
            Arc::strong_count(endpoint) == 1
        };
        if !idle {
            continue;
        }
        let pressure = lao_run::pressured().unwrap_or(true);
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state
            .started
            .as_ref()
            .is_some_and(|(_, endpoint)| evict(endpoint, pressure))
            && let Some((runtime, _)) = state.started.take()
        {
            let _ = runtime.stop();
        }
    }
}

fn evict(endpoint: &Arc<Endpoint>, pressure: bool) -> bool {
    pressure && Arc::strong_count(endpoint) == 1
}

fn caller(name: &str) -> Result<[u8; 64], Box<dyn Error + Send + Sync>> {
    let value = env::var(name)?;
    value
        .as_bytes()
        .try_into()
        .map_err(|_| format!("{name} must be 64 bytes").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Warm state remains available while healthy; pressure still cannot interrupt a response.
    #[test]
    fn residency_keeps_healthy_cache_and_evicts_idle_pressure() {
        let endpoint = Arc::new(Endpoint::new("127.0.0.1:1".parse().unwrap(), "key"));
        assert!(!evict(&endpoint, false));
        let response = endpoint.clone();
        assert!(!evict(&endpoint, true));
        drop(response);
        assert!(evict(&endpoint, true));
    }
}
