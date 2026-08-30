mod supervisor;

use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use lao_run_api::{Endpoint, Local};

const IDLE: Duration = Duration::from_secs(5 * 60);
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
            let codex = caller("LAO_CODEX_CALLER")?;
            let claude = caller("LAO_CLAUDE_CALLER")?;
            let codex_cloud = match env::var("LAO_CODEX_CLOUD").as_deref() {
                Ok("openai") => lao_gate::CodexCloud::Api,
                Ok("chatgpt") => lao_gate::CodexCloud::ChatGpt,
                _ => return Err("LAO_CODEX_CLOUD".into()),
            };
            lao_gate::installed(
                listener,
                lao_route::Router,
                Arc::new(Lazy::new()?),
                codex,
                claude,
                codex_cloud,
            )
        }
        _ => Err("LAO_LOCAL_CANARY".into()),
    }
}

/// The runtime starts on the first Local request, so cloud work never waits for a model load
/// and an install that never routes Local never loads one.
struct Lazy {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    started: Option<(lao_run::Direct, Arc<Endpoint>)>,
    idle: Option<Instant>,
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
        state.idle = None;
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
    let bin = env::var_os("LAO_LLAMA_SERVER")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/opt/homebrew/bin/llama-server".into());
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
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some((_, endpoint)) = state.started.as_ref() else {
                continue;
            };
            // The manager owns one reference; the gate holds one more per active response.
            if Arc::strong_count(endpoint) != 1 {
                state.idle = None;
                continue;
            }
            *state.idle.get_or_insert_with(Instant::now)
        };
        let pressure = lao_run::pressured().unwrap_or(true);
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.idle == Some(idle)
            && state
                .started
                .as_ref()
                .is_some_and(|(_, endpoint)| Arc::strong_count(endpoint) == 1)
            && due(idle, Instant::now(), pressure)
        {
            state.idle = None;
            if let Some((runtime, _)) = state.started.take() {
                let _ = runtime.stop();
            }
        }
    }
}

fn due(idle: Instant, now: Instant, pressure: bool) -> bool {
    pressure || now.duration_since(idle) >= IDLE
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

    // R1: pressure or five observed idle minutes selects eviction.
    #[test]
    fn residency_waits_for_idle_and_then_evicts() {
        let idle = Instant::now();
        assert!(!due(idle, idle + IDLE - Duration::from_secs(1), false));
        assert!(due(idle, idle + IDLE, false));
        assert!(due(idle, idle, true));
    }
}
