#![cfg(target_os = "macos")]

use std::{
    env, fs,
    net::TcpListener,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CODEX: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
const CLAUDE: &str = "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD";

#[test]
#[ignore = "uses the cached model and installed Codex and Claude saved logins"]
fn installed_clients_are_semantically_routed_to_real_local_inference() {
    version("codex", "codex-cli 0.151.0");
    version("claude", "2.1.251 (Claude Code)");

    let root = env::var_os("LAO_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").expect("HOME")).join("Library/Caches/lao/models")
        });
    let model = lao_model::open(&root).expect("verified cached model");
    let bin = env::var_os("LAO_LLAMA_SERVER")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(|| {
            let root = PathBuf::from(env::var_os("HOME").expect("HOME"))
                .join("Library/Caches/lao/runtimes");
            lao_run::prepare(&root)
        })
        .expect("verified local runtime");
    let (runtime, endpoint) = lao_run::Direct::start(lao_run::Config {
        bin: &bin,
        model: &model.path,
        mode: lao_run::Mode::Light,
        working_set: model.artifact.working_set,
        context: model.artifact.context,
        threads: 2,
    })
    .expect("local runtime");

    let resolved = Arc::new(AtomicUsize::new(0));
    let local: Arc<dyn lao_run_api::Local> = Arc::new(Ready(Arc::new(endpoint), resolved.clone()));
    let router = env::var_os("LAO_ROUTER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").expect("HOME"))
                .join("Library/Caches/lao/routers/minilm")
        });
    lao_route::prepare(&router).expect("verified semantic router asset");
    let policy = lao_route::Semantic::open(&router).expect("verified semantic router");
    let classified = Arc::new(AtomicUsize::new(0));
    let selected = Arc::new(AtomicUsize::new(0));
    let policy = Observed {
        inner: policy,
        classified: classified.clone(),
        selected: selected.clone(),
    };
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let _ = lao_gate::installed(
            listener,
            policy,
            local,
            *CODEX.as_bytes().first_chunk().unwrap(),
            *CLAUDE.as_bytes().first_chunk().unwrap(),
            lao_gate::CodexCloud::ChatGpt,
        );
    });

    check(codex(port), CODEX, &classified, &selected, &resolved);
    assert_eq!(
        classified.load(Ordering::Relaxed),
        1,
        "Codex first turn is classified"
    );
    check(claude(port), CLAUDE, &classified, &selected, &resolved);
    assert_eq!(classified.load(Ordering::Relaxed), 2, "both are classified");
    assert_eq!(selected.load(Ordering::Relaxed), 2, "both select Local");
    assert_eq!(resolved.load(Ordering::Relaxed), 2, "both routes are local");
    runtime.stop().unwrap();
}

fn codex(port: u16) -> Output {
    let base = format!("http://127.0.0.1:{port}/oai");
    let provider = format!(
        "{{ name = \"LAO\", base_url = \"{base}\", requires_openai_auth = true, supports_websockets = false, http_headers = {{ X-LAO-Key = \"{CODEX}\" }}, request_max_retries = 0, stream_max_retries = 0 }}"
    );
    let temp = Temp::new("codex");
    exec(
        Command::new("codex")
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove("CODEX_ACCESS_TOKEN")
            .current_dir(&temp.0)
            .args([
                "-c",
                "model_provider=\"lao_e2e\"",
                "-c",
                &format!("model_providers.lao_e2e={provider}"),
                "-c",
                "model_reasoning_effort=\"low\"",
                "exec",
                "--ignore-user-config",
                "--ignore-rules",
                "--ephemeral",
                "--skip-git-repo-check",
                "--color",
                "never",
                "--sandbox",
                "read-only",
                "--model",
                "gpt-5.4",
                "Correct the spelling error in this one word: teh. Reply with only the corrected word.",
            ]),
    )
}

fn claude(port: u16) -> Output {
    let settings = format!(
        "{{\"env\":{{\"ANTHROPIC_BASE_URL\":\"http://127.0.0.1:{port}/ant\",\"ANTHROPIC_CUSTOM_HEADERS\":\"X-LAO-Key: {CLAUDE}\",\"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC\":\"1\"}}}}"
    );
    let temp = Temp::new("claude");
    exec(
        Command::new("claude")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("ANTHROPIC_AUTH_TOKEN")
            .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
            .current_dir(&temp.0)
            .args([
                "--safe-mode",
                "--settings",
                &settings,
                "--no-session-persistence",
                "--disable-slash-commands",
                "--tools",
                "",
                "--effort",
                "low",
                "-p",
                "--model",
                "claude-sonnet-4-5",
                "Correct the spelling error in this one word: teh. Reply with only the corrected word.",
            ]),
    )
}

fn check(
    output: Output,
    caller: &str,
    classified: &AtomicUsize,
    selected: &AtomicUsize,
    resolved: &AtomicUsize,
) {
    assert!(
        output.status.success(),
        "client failed after {} classifications, {} Local decisions, and {} local resolutions",
        classified.load(Ordering::Relaxed),
        selected.load(Ordering::Relaxed),
        resolved.load(Ordering::Relaxed)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "the", "wrong client output");
    assert!(!stdout.contains(caller));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(caller));
}

struct Observed {
    inner: lao_route::Semantic,
    classified: Arc<AtomicUsize>,
    selected: Arc<AtomicUsize>,
}

impl lao_route_api::Policy for Observed {
    fn decide(&self, context: lao_route_api::Context) -> lao_route_api::Decision {
        self.inner.decide(context)
    }

    fn requires_query(&self) -> bool {
        true
    }

    fn decide_query(
        &self,
        context: lao_route_api::Context,
        query: &str,
    ) -> lao_route_api::Decision {
        self.classified.fetch_add(1, Ordering::Relaxed);
        let decision = self.inner.decide_query(context, query);
        if decision == lao_route_api::Decision::Local {
            self.selected.fetch_add(1, Ordering::Relaxed);
        }
        decision
    }
}

struct Ready(Arc<lao_run_api::Endpoint>, Arc<AtomicUsize>);

impl lao_run_api::Local for Ready {
    fn endpoint(&self) -> std::io::Result<Arc<lao_run_api::Endpoint>> {
        self.1.fetch_add(1, Ordering::Relaxed);
        Ok(self.0.clone())
    }
}

fn version(bin: &str, expected: &str) {
    let output = exec(Command::new(bin).arg("--version"));
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
}

fn exec(command: &mut Command) -> Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let until = Instant::now() + Duration::from_secs(60);
    while Instant::now() < until {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    let _ = child.wait();
    panic!("client timed out")
}

struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            env::temp_dir().join(format!("lao-daemon-{name}-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
