//! Isolated worker-only gate for the public workflow collector.
use serde_json::{Value, json};
use std::{
    env,
    io::{self, BufRead, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Cold(Mutex<Option<(lao_run::Direct, Arc<lao_run_api::Endpoint>)>>);

impl lao_run_api::Local for Cold {
    fn endpoint(&self) -> io::Result<Arc<lao_run_api::Endpoint>> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| io::Error::other("runtime state"))?;
        if state.is_none() {
            let home = PathBuf::from(env::var_os("HOME").ok_or_else(|| io::Error::other("home"))?);
            let cache = home.join("Library/Caches/lao");
            let model = lao_model::open(&cache.join("models"))?;
            let bin = lao_run::binary(&cache.join("runtimes"));
            let (runtime, endpoint) = lao_run::Direct::start(lao_run::Config {
                bin: &bin,
                model: &model.path,
                mode: lao_run::Mode::Light,
                working_set: model.artifact.working_set,
                context: model.artifact.context,
                threads: 2,
            })?;
            *state = Some((runtime, Arc::new(endpoint)));
        }
        Ok(state.as_ref().expect("runtime started").1.clone())
    }
}

fn main() {
    if run().is_err() {
        eprintln!("workflow gate failed");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    if line.len() > 4096 {
        return Err("request too large".into());
    }
    let request: Value = serde_json::from_str(&line)?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let local: Arc<dyn lao_run_api::Local> = if request["cache"] == "cold" {
        Arc::new(Cold::default())
    } else {
        let addr = request["addr"].as_str().ok_or("runtime address")?.parse()?;
        Arc::new(lao_run::External::new(
            addr,
            request["bearer"].as_str().ok_or("runtime key")?,
        )?)
    };
    let caller: [u8; 64] = request["caller"]
        .as_str()
        .ok_or("worker key")?
        .as_bytes()
        .try_into()?;
    println!("{}", json!({"port": port}));
    io::stdout().flush()?;
    lao_gate::installed(
        listener,
        Arc::new(lao_route::Router),
        local,
        [0; 64],
        [0; 64],
        caller,
        lao_gate::CodexCloud::Api,
    )
    .map_err(|_| "gate failed".into())
}
