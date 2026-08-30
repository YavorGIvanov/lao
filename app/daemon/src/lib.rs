mod supervisor;

use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use lao_run_api::{Endpoint, Local};

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
                Arc::new(Lazy::default()),
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
#[derive(Default)]
struct Lazy(Mutex<Option<(lao_run::Direct, Arc<Endpoint>)>>);

impl Local for Lazy {
    fn endpoint(&self) -> io::Result<Arc<Endpoint>> {
        let mut started = self.0.lock().map_err(|_| io::Error::other("local"))?;
        if let Some((_, endpoint)) = started.as_ref() {
            return Ok(endpoint.clone());
        }
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
        let endpoint = Arc::new(endpoint);
        *started = Some((runtime, endpoint.clone()));
        Ok(endpoint)
    }
}

fn caller(name: &str) -> Result<[u8; 64], Box<dyn Error + Send + Sync>> {
    let value = env::var(name)?;
    value
        .as_bytes()
        .try_into()
        .map_err(|_| format!("{name} must be 64 bytes").into())
}
