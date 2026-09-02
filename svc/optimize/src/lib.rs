use lao_optimize_api::{Optimize, Plan, Probe, Start, State, StateStore};
use std::{
    env,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const IDLE: u8 = 0;
const WARMING: u8 = 1;
const READY: u8 = 2;
const FAILED: u8 = 3;
const CALLER_ENV: &str = "LAO_OPTIMIZE_CODEX_CALLER";
const SELECTOR_ENV: &str = "LAO_OPTIMIZE_LOCAL";
const LOCAL_AUTH: &[u8] = b"{\"auth_mode\":\"apikey\",\"OPENAI_API_KEY\":\"lao-local-only\"}\n";
const OUTPUT_LIMIT: u64 = 1 << 20;
const TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn save(&self, state: State) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| invalid("optimize state path"))?;
        if !fs::symlink_metadata(parent)?.file_type().is_dir() {
            return Err(invalid("optimize state parent"));
        }
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) if !metadata.file_type().is_file() || metadata.mode() & 0o777 != 0o600 => {
                return Err(invalid("optimize state file"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let pending = pending(&self.path)?;
        remove_state(&pending)?;
        let mut cleanup = Pending(Some(pending.clone()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending)?;
        file.write_all(encoded(state))?;
        file.sync_all()?;
        fs::rename(&pending, &self.path)?;
        File::open(parent)?.sync_all()?;
        cleanup.0 = None;
        Ok(())
    }
}

impl StateStore for Store {
    fn load(&self) -> io::Result<Option<State>> {
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if !metadata.file_type().is_file() || metadata.mode() & 0o777 != 0o600 || metadata.len() > 8
        {
            return Err(invalid("optimize state file"));
        }
        match fs::read(&self.path)?.as_slice() {
            b"idle\n" => Ok(Some(State::Idle)),
            b"warming\n" => Ok(Some(State::Warming)),
            b"ready\n" => Ok(Some(State::Ready)),
            b"failed\n" => Ok(Some(State::Failed)),
            _ => Err(invalid("optimize state file")),
        }
    }

    fn remove(&self) -> io::Result<()> {
        remove_state(&self.path)?;
        remove_state(&pending(&self.path)?)
    }
}

pub struct Optimizer {
    state: Arc<AtomicU8>,
    store: Store,
}

impl Optimizer {
    pub fn new(store: Store) -> io::Result<Self> {
        store.save(State::Idle)?;
        Ok(Self {
            state: Arc::new(AtomicU8::new(IDLE)),
            store,
        })
    }
}

impl Optimize for Optimizer {
    fn start(&self, plan: Plan) -> io::Result<Start> {
        let previous = loop {
            let state = self.state.load(Ordering::Acquire);
            if state == WARMING {
                return Ok(Start::Busy);
            }
            if self
                .state
                .compare_exchange(state, WARMING, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break state;
            }
        };
        if let Err(error) = self.store.save(State::Warming) {
            self.state.store(previous, Ordering::Release);
            return Err(error);
        }

        let state = Arc::clone(&self.state);
        let store = self.store.clone();
        match thread::Builder::new()
            .name("lao-warm".into())
            .spawn(move || warm(state, store, plan))
        {
            Ok(_) => Ok(Start::Started),
            Err(error) => {
                self.state.store(previous, Ordering::Release);
                self.store.save(decoded(previous))?;
                Err(error)
            }
        }
    }

    fn state(&self) -> State {
        match self.state.load(Ordering::Acquire) {
            IDLE => State::Idle,
            WARMING => State::Warming,
            READY => State::Ready,
            FAILED => State::Failed,
            _ => unreachable!(),
        }
    }
}

fn warm(state: Arc<AtomicU8>, store: Store, plan: Plan) {
    let (claude, codex) = plan.into_probes();
    let claude = probe(claude);
    let codex = probe(codex);
    let complete = if claude && codex {
        State::Ready
    } else {
        State::Failed
    };
    if store.save(complete).is_ok() {
        state.store(encoded_state(complete), Ordering::Release);
    } else {
        state.store(FAILED, Ordering::Release);
        let _ = store.save(State::Failed);
    }
}

fn probe(probe: Probe) -> bool {
    panic::catch_unwind(AssertUnwindSafe(probe)).is_ok_and(|result| result.is_ok())
}

pub fn codex(
    bin: impl AsRef<OsStr>,
    catalog: impl AsRef<Path>,
    port: u16,
    caller: &str,
    instructions: &str,
) -> io::Result<Duration> {
    valid(port, caller)?;
    if instructions.is_empty() || instructions.len() > 4096 {
        return Err(invalid("Codex developer instructions"));
    }
    let scratch = Scratch::new("codex")?;
    let catalog_path = scratch.0.join("models_cache.json");
    let catalog_source = fs::symlink_metadata(catalog.as_ref())?;
    if !catalog_source.file_type().is_file()
        || catalog_source.len() == 0
        || catalog_source.len() > OUTPUT_LIMIT
    {
        return Err(invalid("Codex model catalog"));
    }
    let catalog_bytes = fs::read(catalog.as_ref())?;
    if catalog_bytes.is_empty() || catalog_bytes.len() as u64 > OUTPUT_LIMIT {
        return Err(invalid("Codex model catalog"));
    }
    write_private(&catalog_path, &catalog_bytes, 0o600)?;
    write_private(&scratch.0.join("auth.json"), LOCAL_AUTH, 0o600)?;
    let catalog_path = catalog_path
        .to_str()
        .filter(|value| {
            !value
                .bytes()
                .any(|byte| byte.is_ascii_control() || matches!(byte, b'"' | b'\\'))
        })
        .ok_or_else(|| invalid("Codex model catalog path"))?;
    let config = format!(
        "model_catalog_json = \"{catalog_path}\"\n\
         model_provider = \"lao\"\n\
         developer_instructions = {instructions:?}\n\
         [model_providers.lao]\n\
         name = \"LAO\"\n\
         base_url = \"http://127.0.0.1:{port}/oai\"\n\
         requires_openai_auth = true\n\
         supports_websockets = false\n\
         env_http_headers = {{ X-LAO-Key = \"{CALLER_ENV}\", X-LAO-Local = \"{SELECTOR_ENV}\" }}\n",
        instructions = instructions,
    );
    write_private(&scratch.0.join("config.toml"), config.as_bytes(), 0o600)?;
    let mut command = Command::new(bin);
    direct(&mut command);
    command
        .env("CODEX_HOME", &scratch.0)
        .env(CALLER_ENV, caller)
        .env(SELECTOR_ENV, "canary")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("CODEX_API_KEY")
        .env_remove("LAO_LOCAL_SELECTOR")
        .current_dir(&scratch.0)
        .args([
            "-c",
            "model_reasoning_effort=\"low\"",
            "-c",
            "mcp_servers={}",
            "-c",
            "web_search=\"disabled\"",
            "exec",
            "--strict-config",
            "--ephemeral",
            "--skip-git-repo-check",
            "--color",
            "never",
            "--sandbox",
            "read-only",
            "--model",
            "lao-local",
            "Reply exactly 42. Do not use tools.",
        ]);
    client(&mut command, caller, TIMEOUT)
}

pub fn claude(bin: impl AsRef<OsStr>, port: u16, caller: &str) -> io::Result<Duration> {
    valid(port, caller)?;
    let scratch = Scratch::new("claude")?;
    let settings = scratch.0.join("settings.json");
    let bytes = format!(
        r#"{{"env":{{"ANTHROPIC_BASE_URL":"http://127.0.0.1:{port}/ant","ANTHROPIC_CUSTOM_HEADERS":"X-LAO-Key: {caller}\nX-LAO-Local: canary","CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":"1"}}}}"#
    );
    write_private(&settings, bytes.as_bytes(), 0o600)?;
    let settings = settings
        .to_str()
        .ok_or_else(|| invalid("local probe settings"))?;
    let mut command = Command::new(bin);
    direct(&mut command);
    command
        .current_dir(&scratch.0)
        .env_remove(CALLER_ENV)
        .env_remove("LAO_LOCAL_SELECTOR");
    for key in [
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_API_KEY",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_PROFILE",
        "ANTHROPIC_FEDERATION_RULE_ID",
        "ANTHROPIC_ORGANIZATION_ID",
        "ANTHROPIC_WORKSPACE_ID",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_CUSTOM_HEADERS",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
        "CLAUDE_CODE_USE_MANTLE",
        "ANTHROPIC_BEDROCK_BASE_URL",
        "ANTHROPIC_VERTEX_BASE_URL",
        "ANTHROPIC_FOUNDRY_BASE_URL",
        "ANTHROPIC_AWS_BASE_URL",
    ] {
        command.env_remove(key);
    }
    command.args([
        "--safe-mode",
        "--settings",
        settings,
        "--setting-sources",
        "",
        "--no-session-persistence",
        "--disable-slash-commands",
        "--strict-mcp-config",
        "--tools",
        "",
        "--effort",
        "low",
        "-p",
        "--model",
        "lao-local",
        "Reply exactly 42. Do not use tools.",
    ]);
    client(&mut command, caller, TIMEOUT)
}

fn direct(command: &mut Command) {
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        command.env_remove(key);
    }
}

fn client(command: &mut Command, caller: &str, timeout: Duration) -> io::Result<Duration> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let started = Instant::now();
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| invalid("local probe output"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| invalid("local probe output"))?;
    let stdout = thread::spawn(move || read_bounded(stdout));
    let stderr = thread::spawn(move || read_bounded(stderr));
    let deadline = started + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout.join();
            let _ = stderr.join();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "local probe"));
        }
        thread::sleep(Duration::from_millis(20));
    };
    let stdout = join(stdout)?;
    let stderr = join(stderr)?;
    check(status, stdout, stderr, caller)?;
    Ok(started.elapsed())
}

fn read_bounded(mut pipe: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.by_ref()
        .take(OUTPUT_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn join(reader: thread::JoinHandle<io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    reader.join().map_err(|_| invalid("local probe output"))?
}

fn check(status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8>, caller: &str) -> io::Result<()> {
    if !status.success()
        || stdout.len() as u64 > OUTPUT_LIMIT
        || stderr.len() as u64 > OUTPUT_LIMIT
        || stdout.trim_ascii() != b"42"
        || contains(&stdout, caller.as_bytes())
        || contains(&stderr, caller.as_bytes())
    {
        return Err(invalid("local probe"));
    }
    Ok(())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|part| part == needle)
}

fn valid(port: u16, caller: &str) -> io::Result<()> {
    if port == 0 || caller.len() != 64 || !caller.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid("local probe configuration"));
    }
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    file.write_all(bytes)
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> io::Result<Self> {
        let mut random = [0_u8; 8];
        getrandom::getrandom(&mut random).map_err(|_| invalid("local probe random"))?;
        let path = env::temp_dir().join(format!("lao-{name}-{:016x}", u64::from_ne_bytes(random)));
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn encoded(state: State) -> &'static [u8] {
    match state {
        State::Idle => b"idle\n",
        State::Warming => b"warming\n",
        State::Ready => b"ready\n",
        State::Failed => b"failed\n",
    }
}

fn encoded_state(state: State) -> u8 {
    match state {
        State::Idle => IDLE,
        State::Warming => WARMING,
        State::Ready => READY,
        State::Failed => FAILED,
    }
}

fn decoded(state: u8) -> State {
    match state {
        IDLE => State::Idle,
        WARMING => State::Warming,
        READY => State::Ready,
        FAILED => State::Failed,
        _ => unreachable!(),
    }
}

fn pending(path: &Path) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("optimize state path"))?;
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| invalid("optimize state path"))?;
    Ok(parent.join(format!(".{name}.pending")))
}

fn remove_state(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && metadata.mode() & 0o777 == 0o600 => {
            fs::remove_file(path)
        }
        Ok(_) => Err(invalid("optimize state file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
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

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Mutex, mpsc},
        time::{Duration, Instant},
    };

    #[test]
    fn warm_is_single_flight_and_claude_runs_first() {
        let fixture = Scratch::new("state-test").unwrap();
        let store = Store::new(fixture.0.join("optimize.state"));
        store.save(State::Warming).unwrap();
        let optimizer = Optimizer::new(store.clone()).unwrap();
        assert_eq!(store.load().unwrap(), Some(State::Idle));
        let order = Arc::new(Mutex::new(Vec::new()));
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let claude_order = Arc::clone(&order);
        let codex_order = Arc::clone(&order);
        let plan = Plan::new(
            move || {
                claude_order.lock().unwrap().push("claude");
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            },
            move || {
                codex_order.lock().unwrap().push("codex");
                Ok(())
            },
        );

        assert_eq!(optimizer.start(plan).unwrap(), Start::Started);
        assert_eq!(store.load().unwrap(), Some(State::Warming));
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            optimizer.start(Plan::new(|| Ok(()), || Ok(()))).unwrap(),
            Start::Busy
        );
        release_tx.send(()).unwrap();
        wait_for(&optimizer, State::Ready);
        assert_eq!(store.load().unwrap(), Some(State::Ready));
        assert_eq!(fs::metadata(&store.path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(*order.lock().unwrap(), ["claude", "codex"]);
    }

    #[test]
    fn a_failed_probe_does_not_skip_codex_or_block_retry() {
        let fixture = Scratch::new("retry-test").unwrap();
        let store = Store::new(fixture.0.join("optimize.state"));
        let optimizer = Optimizer::new(store.clone()).unwrap();
        let codex_runs = Arc::new(AtomicU8::new(0));
        let runs = Arc::clone(&codex_runs);
        optimizer
            .start(Plan::new(
                || Err(io::Error::other("failed")),
                move || {
                    runs.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                },
            ))
            .unwrap();
        wait_for(&optimizer, State::Failed);
        assert_eq!(store.load().unwrap(), Some(State::Failed));
        assert_eq!(codex_runs.load(Ordering::Relaxed), 1);

        assert_eq!(
            optimizer.start(Plan::new(|| Ok(()), || Ok(()))).unwrap(),
            Start::Started
        );
        wait_for(&optimizer, State::Ready);
        assert_eq!(store.load().unwrap(), Some(State::Ready));
        store.remove().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn client_probes_are_loopback_only_and_keep_callers_private() {
        let caller = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let fixture = Scratch::new("probe-test").unwrap();
        let codex_bin = fixture.0.join("codex");
        let claude_bin = fixture.0.join("claude");
        let catalog = fixture.0.join("models_cache.json");
        let codex_script = format!(
            r#"#!/bin/sh
for value in "$@"; do
    [ "$value" != "{caller}" ] || exit 10
done
[ "$LAO_OPTIMIZE_CODEX_CALLER" = "{caller}" ] || exit 11
[ "$LAO_OPTIMIZE_LOCAL" = "canary" ] || exit 12
grep -F 'base_url = "http://127.0.0.1:8765/oai"' "$CODEX_HOME/config.toml" >/dev/null || exit 13
grep -F 'requires_openai_auth = true' "$CODEX_HOME/config.toml" >/dev/null || exit 14
grep -F 'X-LAO-Key = "LAO_OPTIMIZE_CODEX_CALLER"' "$CODEX_HOME/config.toml" >/dev/null || exit 15
if grep -F '{caller}' "$CODEX_HOME/config.toml" >/dev/null; then exit 16; fi
[ "$(stat -f %Lp "$CODEX_HOME/models_cache.json")" = "600" ] || exit 17
grep -F 'model_catalog_json = ' "$CODEX_HOME/config.toml" >/dev/null || exit 18
grep -F 'developer_instructions = "delegate"' "$CODEX_HOME/config.toml" >/dev/null || exit 25
[ "$(stat -f %Lp "$CODEX_HOME/auth.json")" = "600" ] || exit 19
grep -F '"OPENAI_API_KEY":"lao-local-only"' "$CODEX_HOME/auth.json" >/dev/null || exit 20
printf '42\n'
"#
        );
        let claude_script = format!(
            r#"#!/bin/sh
settings=
previous=
for value in "$@"; do
    [ "$value" != "{caller}" ] || exit 20
    if [ "$previous" = "--settings" ]; then settings=$value; fi
    previous=$value
done
[ -n "$settings" ] || exit 21
grep -F 'http://127.0.0.1:8765/ant' "$settings" >/dev/null || exit 22
grep -F 'X-LAO-Key: {caller}' "$settings" >/dev/null || exit 23
if /usr/bin/env | grep -F '{caller}' >/dev/null; then exit 24; fi
printf '42\n'
"#
        );
        write_private(&codex_bin, codex_script.as_bytes(), 0o700).unwrap();
        write_private(&claude_bin, claude_script.as_bytes(), 0o700).unwrap();
        write_private(&catalog, b"{}\n", 0o600).unwrap();

        codex(&codex_bin, &catalog, 8765, caller, "delegate").unwrap();
        claude(&claude_bin, 8765, caller).unwrap();
    }

    #[test]
    fn client_runner_rejects_secret_output_and_times_out() {
        let caller = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let fixture = Scratch::new("runner-test").unwrap();
        let leak = fixture.0.join("leak");
        let stall = fixture.0.join("stall");
        write_private(
            &leak,
            format!("#!/bin/sh\nprintf '{caller}\\n'\n").as_bytes(),
            0o700,
        )
        .unwrap();
        write_private(&stall, b"#!/bin/sh\nwhile :; do :; done\n", 0o700).unwrap();

        assert_eq!(
            client(&mut Command::new(leak), caller, Duration::from_secs(1))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        let started = Instant::now();
        assert_eq!(
            client(&mut Command::new(stall), caller, Duration::from_millis(20))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    fn wait_for(optimizer: &Optimizer, state: State) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while optimizer.state() != state {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
    }
}
