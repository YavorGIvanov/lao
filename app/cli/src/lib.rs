use lao_optimize_api::{State as OptimizeState, StateStore};
use serde::{Deserialize, Serialize};
use std::{
    env,
    error::Error,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const LABEL: &str = "com.local-agent-optimizer.daemon";
const CODEX_BEFORE: &str = "codex.before";
const CODEX_AFTER: &str = "codex.after";
const CLAUDE_BEFORE: &str = "claude.before";
const CLAUDE_AFTER: &str = "claude.after";
const CODEX_RESTORE_FROM: &str = "codex.restore-from";
const CODEX_RESTORE_TO: &str = "codex.restore-to";
const CLAUDE_RESTORE_FROM: &str = "claude.restore-from";
const CLAUDE_RESTORE_TO: &str = "claude.restore-to";
const CLAUDE_MCP_BEFORE: &str = "claude-mcp.before";
const CLAUDE_MCP_AFTER: &str = "claude-mcp.after";
const CLAUDE_MCP_RESTORE_FROM: &str = "claude-mcp.restore-from";
const CLAUDE_MCP_RESTORE_TO: &str = "claude-mcp.restore-to";
const PLIST_AFTER: &str = "launchd.after";
const RECORD: &str = "install.json";
const DAEMON_ERROR: &str = "daemon.err";

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone)]
struct Paths {
    state: PathBuf,
    codex: PathBuf,
    claude: PathBuf,
    claude_mcp: PathBuf,
    plist: PathBuf,
    adopted: PathBuf,
    model: PathBuf,
    runtime: PathBuf,
    router: PathBuf,
    worker: PathBuf,
    worker_key: PathBuf,
    daemon_source: PathBuf,
    daemon: PathBuf,
    optimize: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Router {
    Semantic,
    Safe,
    VllmSemantic,
}

impl Router {
    const fn id(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Safe => "safe",
            Self::VllmSemantic => "vllm-semantic",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Runtime {
    LlamaCpp,
    External,
}

impl Runtime {
    const fn id(self) -> &'static str {
        match self {
            Self::LlamaCpp => "llama-cpp",
            Self::External => "external",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Choice {
    router: Router,
    runtime: Runtime,
}

impl Default for Choice {
    fn default() -> Self {
        Self {
            router: Router::Semantic,
            runtime: Runtime::LlamaCpp,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Adapter {
    addr: SocketAddrV4,
    key: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Selected {
    choice: Choice,
    vllm: Option<Adapter>,
    external: Option<Adapter>,
}

impl Selected {
    fn load(choice: Choice) -> io::Result<Self> {
        let vllm = if choice.router == Router::VllmSemantic {
            let addr = match env::var("LAO_VLLM_ROUTER_ADDR") {
                Ok(value) => loopback(&value, "LAO_VLLM_ROUTER_ADDR")?,
                Err(env::VarError::NotPresent) => SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080),
                Err(env::VarError::NotUnicode(_)) => {
                    return Err(invalid("LAO_VLLM_ROUTER_ADDR"));
                }
            };
            Some(Adapter {
                addr,
                key: optional_key("LAO_VLLM_ROUTER_KEY_FILE")?,
            })
        } else {
            None
        };
        let external = if choice.runtime == Runtime::External {
            let addr = required_addr("LAO_EXTERNAL_ADDR")?;
            let key = required_key("LAO_EXTERNAL_KEY_FILE")?;
            Some(Adapter {
                addr,
                key: Some(key),
            })
        } else {
            None
        };
        Ok(Self {
            choice,
            vllm,
            external,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Preview(Choice),
    Install(Choice),
    Status,
    Smoke,
    Off,
    Mcp,
}

struct Clients {
    codex: PathBuf,
    claude: PathBuf,
    cloud: &'static str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Entry {
    path: PathBuf,
    existed: bool,
    mode: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Installing,
    Installed,
    Restoring,
    Restored,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Record {
    phase: Phase,
    port: u16,
    codex: Entry,
    claude: Entry,
    claude_mcp: Entry,
    router: Router,
    router_addr: Option<SocketAddrV4>,
    router_key: Option<PathBuf>,
}

struct Lock(File);

impl Lock {
    #[cfg(target_os = "macos")]
    #[allow(unsafe_code)]
    fn acquire(root: &Path) -> io::Result<Self> {
        private_dir(root)?;
        if let Ok(metadata) = fs::symlink_metadata(root.join("install.lock"))
            && !metadata.file_type().is_file()
        {
            return Err(conflict("install lock is not a regular file"));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(root.join("install.lock"))?;
        // SAFETY: flock only reads this live descriptor and releases its lock when it closes.
        if unsafe {
            libc::flock(
                std::os::fd::AsRawFd::as_raw_fd(&file),
                libc::LOCK_EX | libc::LOCK_NB,
            )
        } != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "another lao install or off is running",
            ));
        }
        Ok(Self(file))
    }

    #[cfg(not(target_os = "macos"))]
    fn acquire(_: &Path) -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "macOS only"))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.metadata();
    }
}

struct Transaction {
    state: PathBuf,
    record: Record,
}

struct McpRestore {
    current: Option<Vec<u8>>,
    restored: Option<Vec<u8>>,
}

impl Transaction {
    fn prepare(
        paths: &Paths,
        port: u16,
        codex_after: &[u8],
        claude_after: &[u8],
        claude_mcp_after: &[u8],
        router: Router,
        adapter: Option<&Adapter>,
    ) -> io::Result<Self> {
        if paths.state.join(RECORD).exists() {
            return Err(conflict("lao is already installed or needs recovery"));
        }
        let (codex, codex_before) = inspect(&paths.codex)?;
        let (claude, claude_before) = inspect(&paths.claude)?;
        let (claude_mcp, claude_mcp_before) = inspect(&paths.claude_mcp)?;
        write_atomic(&paths.state.join(CODEX_BEFORE), &codex_before, 0o600)?;
        write_atomic(&paths.state.join(CODEX_AFTER), codex_after, 0o600)?;
        write_atomic(&paths.state.join(CLAUDE_BEFORE), &claude_before, 0o600)?;
        write_atomic(&paths.state.join(CLAUDE_AFTER), claude_after, 0o600)?;
        write_atomic(
            &paths.state.join(CLAUDE_MCP_BEFORE),
            &claude_mcp_before,
            0o600,
        )?;
        write_atomic(&paths.state.join(CLAUDE_MCP_AFTER), claude_mcp_after, 0o600)?;
        let record = Record {
            phase: Phase::Installing,
            port,
            codex,
            claude,
            claude_mcp,
            router,
            router_addr: adapter.map(|adapter| adapter.addr),
            router_key: adapter.and_then(|adapter| adapter.key.clone()),
        };
        write_record(&paths.state, &record)?;
        Ok(Self {
            state: paths.state.clone(),
            record,
        })
    }

    fn load(state: &Path) -> io::Result<Self> {
        let bytes = fs::read(state.join(RECORD))?;
        let record = serde_json::from_slice(&bytes).map_err(|_| invalid("install record"))?;
        Ok(Self {
            state: state.to_path_buf(),
            record,
        })
    }

    fn phase(&mut self, phase: Phase) -> io::Result<()> {
        self.record.phase = phase;
        write_record(&self.state, &self.record)
    }

    fn apply(&mut self) -> io::Result<()> {
        self.apply_with(|_, entry, bytes| write_entry(entry, bytes))
    }

    fn apply_with(
        &mut self,
        mut write: impl FnMut(usize, &Entry, &[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        self.validate_originals()?;
        let codex = fs::read(self.state.join(CODEX_AFTER))?;
        let claude = fs::read(self.state.join(CLAUDE_AFTER))?;
        let claude_mcp = fs::read(self.state.join(CLAUDE_MCP_AFTER))?;
        if let Err(error) = write(0, &self.record.codex, &codex)
            .and_then(|_| write(1, &self.record.claude, &claude))
            .and_then(|_| write(2, &self.record.claude_mcp, &claude_mcp))
        {
            let rollback = self.restore_changed();
            return match rollback {
                Ok(()) => Err(error),
                Err(rollback) => Err(io::Error::other(format!(
                    "client write failed and rollback failed: {rollback}"
                ))),
            };
        }
        self.phase(Phase::Installed)
    }

    fn validate_installed(&self) -> io::Result<()> {
        self.validate_codex()?;
        self.validate_claude()?;
        self.claude_mcp_restore().map(|_| ())
    }

    fn validate_originals(&self) -> io::Result<()> {
        validate_original(&self.record.codex, &self.state.join(CODEX_BEFORE))?;
        validate_original(&self.record.claude, &self.state.join(CLAUDE_BEFORE))?;
        validate_original(&self.record.claude_mcp, &self.state.join(CLAUDE_MCP_BEFORE))
    }

    fn restore(&mut self) -> io::Result<()> {
        let (codex_current, claude_current) = self.installed_clients()?;
        let claude_mcp = self.claude_mcp_restore()?;
        let codex_after = fs::read(self.state.join(CODEX_AFTER))?;
        let claude_after = fs::read(self.state.join(CLAUDE_AFTER))?;
        let codex_before = fs::read(self.state.join(CODEX_BEFORE))?;
        let claude_before = fs::read(self.state.join(CLAUDE_BEFORE))?;
        let codex_restored = lao_codex::restore(
            &codex_current,
            &codex_after,
            self.record.codex.existed.then_some(codex_before.as_slice()),
        )
        .map_err(|_| conflict("managed Codex settings changed"))?;
        let claude_restored = lao_claude::restore(
            &claude_current,
            &claude_after,
            self.record
                .claude
                .existed
                .then_some(claude_before.as_slice()),
        )
        .map_err(|_| conflict("managed Claude settings changed"))?;
        for (name, bytes) in [
            (CODEX_RESTORE_FROM, codex_current.as_slice()),
            (CODEX_RESTORE_TO, codex_restored.as_slice()),
            (CLAUDE_RESTORE_FROM, claude_current.as_slice()),
            (CLAUDE_RESTORE_TO, claude_restored.as_slice()),
            (
                CLAUDE_MCP_RESTORE_FROM,
                claude_mcp
                    .current
                    .as_deref()
                    .ok_or_else(|| conflict("managed Claude MCP file is missing"))?,
            ),
            (
                CLAUDE_MCP_RESTORE_TO,
                claude_mcp.restored.as_deref().unwrap_or_default(),
            ),
        ] {
            write_atomic(&self.state.join(name), bytes, 0o600)?;
        }
        self.phase(Phase::Restoring)?;
        self.finish_restore()?;
        self.phase(Phase::Restored)
    }

    fn finish_restore(&self) -> io::Result<()> {
        finish_restore(
            &self.record.codex,
            &self.state.join(CODEX_RESTORE_FROM),
            &self.state.join(CODEX_RESTORE_TO),
        )?;
        finish_restore(
            &self.record.claude,
            &self.state.join(CLAUDE_RESTORE_FROM),
            &self.state.join(CLAUDE_RESTORE_TO),
        )?;
        finish_restore(
            &self.record.claude_mcp,
            &self.state.join(CLAUDE_MCP_RESTORE_FROM),
            &self.state.join(CLAUDE_MCP_RESTORE_TO),
        )
    }

    fn installed_clients(&self) -> io::Result<(Vec<u8>, Vec<u8>)> {
        Ok((self.validate_codex()?, self.validate_claude()?))
    }

    fn validate_codex(&self) -> io::Result<Vec<u8>> {
        let codex = read_managed(&self.record.codex)?;
        let codex_after = fs::read(self.state.join(CODEX_AFTER))?;
        let codex_before = fs::read(self.state.join(CODEX_BEFORE))?;
        lao_codex::verify(
            &codex,
            &codex_after,
            self.record.codex.existed.then_some(codex_before.as_slice()),
        )
        .map_err(|_| conflict("managed Codex settings changed"))?;
        Ok(codex)
    }

    fn validate_claude(&self) -> io::Result<Vec<u8>> {
        let claude = read_managed(&self.record.claude)?;
        let claude_after = fs::read(self.state.join(CLAUDE_AFTER))?;
        lao_claude::verify(&claude, &claude_after)
            .map_err(|_| conflict("managed Claude settings changed"))?;
        Ok(claude)
    }

    fn restore_changed(&self) -> io::Result<()> {
        restore_if_managed(
            &self.record.codex,
            &self.state.join(CODEX_BEFORE),
            &self.state.join(CODEX_AFTER),
        )?;
        restore_if_managed(
            &self.record.claude,
            &self.state.join(CLAUDE_BEFORE),
            &self.state.join(CLAUDE_AFTER),
        )?;
        let restore = self.claude_mcp_restore()?;
        write_optional(&self.record.claude_mcp, restore.restored.as_deref())
    }

    fn claude_mcp_restore(&self) -> io::Result<McpRestore> {
        let current = read_managed_optional(&self.record.claude_mcp)?;
        let before = fs::read(self.state.join(CLAUDE_MCP_BEFORE))?;
        let original = self.record.claude_mcp.existed.then_some(before.as_slice());
        let after = fs::read(self.state.join(CLAUDE_MCP_AFTER))?;
        let restore = lao_claude::restore_worker(current.as_deref(), original, &after)
            .map_err(|_| conflict("managed Claude MCP entry changed"))?;
        Ok(McpRestore {
            current,
            restored: restore,
        })
    }

    fn discard(&self) -> io::Result<()> {
        for name in [
            CODEX_BEFORE,
            CODEX_AFTER,
            CLAUDE_BEFORE,
            CLAUDE_AFTER,
            CODEX_RESTORE_FROM,
            CODEX_RESTORE_TO,
            CLAUDE_RESTORE_FROM,
            CLAUDE_RESTORE_TO,
            CLAUDE_MCP_BEFORE,
            CLAUDE_MCP_AFTER,
            CLAUDE_MCP_RESTORE_FROM,
            CLAUDE_MCP_RESTORE_TO,
            PLIST_AFTER,
            RECORD,
        ] {
            remove_optional(&self.state.join(name))?;
        }
        Ok(())
    }
}

pub fn run() -> Result<()> {
    let _ = (lao_codex::status(), lao_claude::status());
    match parse(env::args_os().skip(1))? {
        Some(Action::Preview(choice)) => preview(&Selected::load(choice)?),
        Some(Action::Install(choice)) => install(&Selected::load(choice)?),
        Some(Action::Status) => status(),
        Some(Action::Smoke) => smoke(),
        Some(Action::Off) => off(),
        Some(Action::Mcp) => mcp(),
        None => {
            println!(
                "usage: lao <preview|install> [--router semantic|safe|vllm-semantic] \
                 [--runtime llama-cpp|external]\n       lao <status|smoke|off|mcp>"
            );
            Ok(())
        }
    }
}

fn parse(mut args: impl Iterator<Item = OsString>) -> io::Result<Option<Action>> {
    let Some(command) = args.next() else {
        return Ok(None);
    };
    let command = command.into_string().map_err(|_| invalid("command"))?;
    match command.as_str() {
        "preview" | "install" => {
            let choice = choice(&mut args)?;
            Ok(Some(if command == "preview" {
                Action::Preview(choice)
            } else {
                Action::Install(choice)
            }))
        }
        "status" | "smoke" | "off" | "mcp" => {
            if args.next().is_some() {
                return Err(invalid("unexpected option"));
            }
            Ok(Some(match command.as_str() {
                "status" => Action::Status,
                "smoke" => Action::Smoke,
                "off" => Action::Off,
                "mcp" => Action::Mcp,
                _ => unreachable!(),
            }))
        }
        _ => Err(invalid("command")),
    }
}

fn choice(args: &mut impl Iterator<Item = OsString>) -> io::Result<Choice> {
    let mut router = None;
    let mut runtime = None;
    while let Some(option) = args.next() {
        let option = option.into_string().map_err(|_| invalid("option"))?;
        let value = args
            .next()
            .ok_or_else(|| invalid("missing option value"))?
            .into_string()
            .map_err(|_| invalid("option value"))?;
        match option.as_str() {
            "--router" if router.is_none() => {
                router = Some(match value.as_str() {
                    "semantic" => Router::Semantic,
                    "safe" => Router::Safe,
                    "vllm-semantic" => Router::VllmSemantic,
                    _ => return Err(invalid("router")),
                });
            }
            "--runtime" if runtime.is_none() => {
                runtime = Some(match value.as_str() {
                    "llama-cpp" => Runtime::LlamaCpp,
                    "external" => Runtime::External,
                    _ => return Err(invalid("runtime")),
                });
            }
            "--router" | "--runtime" => return Err(invalid("duplicate option")),
            _ => return Err(invalid("option")),
        }
    }
    let defaults = Choice::default();
    Ok(Choice {
        router: router.unwrap_or(defaults.router),
        runtime: runtime.unwrap_or(defaults.runtime),
    })
}

#[cfg(target_os = "macos")]
fn status() -> Result<()> {
    let paths = paths()?;
    if !paths.state.join(RECORD).is_file() {
        println!("LAO: off");
        return Ok(());
    }
    let _lock = Lock::acquire(&paths.state)?;
    let transaction = Transaction::load(&paths.state)?;
    if transaction.record.phase != Phase::Installed {
        println!("LAO: needs recovery");
        return Err(invalid("run lao off, then lao install").into());
    }
    let codex = transaction.validate_codex().is_ok();
    let claude = transaction.validate_claude().is_ok();
    let plist = validate_path(&paths.plist, &paths.state.join(PLIST_AFTER), 0o600).is_ok();
    let service =
        plist && service_loaded().unwrap_or(false) && hello(transaction.record.port).is_ok();
    let optimize = lao_optimize::Store::new(paths.optimize.clone()).load()?;
    println!(
        "LAO: {}",
        if codex && claude && service {
            "ready"
        } else {
            "needs attention"
        }
    );
    println!(
        "service: {}",
        if service { "running" } else { "unavailable" }
    );
    println!(
        "Codex: {}",
        if codex {
            "routed through LAO"
        } else {
            "not routed through LAO"
        }
    );
    println!(
        "Claude: {}",
        if claude {
            "routed through LAO"
        } else {
            "not routed through LAO"
        }
    );
    println!(
        "local cache: {}",
        match optimize {
            Some(OptimizeState::Idle | OptimizeState::Warming) => "warming",
            Some(OptimizeState::Ready) => "ready",
            Some(OptimizeState::Failed) => "failed",
            None => "not started",
        }
    );
    if codex && claude && service {
        println!("safe fallback: cloud");
        println!("local route: selected policy or explicit canary");
        Ok(())
    } else {
        Err(invalid("installation needs attention").into())
    }
}

#[cfg(not(target_os = "macos"))]
fn status() -> Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "macOS only").into())
}

fn preview(selected: &Selected) -> Result<()> {
    let paths = paths()?;
    println!("router: {}", selected.choice.router.id());
    if selected.choice.router == Router::Semantic {
        println!("router model: sentence-transformers/all-MiniLM-L6-v2");
        println!("router download: {} bytes", lao_route::semantic_bytes());
    }
    if let Some(adapter) = &selected.vllm {
        println!("router address: {}", adapter.addr);
        println!(
            "router key file: {}",
            adapter
                .key
                .as_deref()
                .map_or_else(|| "none".into(), |path| path.display().to_string())
        );
    }
    println!("runtime: {}", selected.choice.runtime.id());
    if selected.choice.runtime == Runtime::LlamaCpp {
        let llama = lao_run::binary(&paths.runtime);
        let budget = llama
            .is_file()
            .then(|| lao_run::plan(&llama, lao_run::Mode::Light))
            .transpose()?;
        let model = &lao_model::QWEN;
        println!("model: {}", model.id);
        println!("source: {} @ {}", model.url, model.revision);
        println!("download: {} bytes", model.bytes);
        println!("license: {}", model.license);
        println!(
            "engine: {} ({} bytes)",
            model.runtime,
            lao_run::DOWNLOAD_BYTES
        );
        println!("context: {}", model.context);
        match budget {
            Some(budget) => println!(
                "Light: {:.2} GiB, {} threads",
                budget.bytes as f64 / (1_u64 << 30) as f64,
                budget.threads
            ),
            None => println!("Light: measured during install"),
        }
    } else if let Some(adapter) = &selected.external {
        println!("runtime address: {}", adapter.addr);
        println!(
            "runtime key file: {}",
            adapter
                .key
                .as_deref()
                .ok_or_else(|| invalid("LAO_EXTERNAL_KEY_FILE"))?
                .display()
        );
    }
    println!("Codex settings: {}", paths.codex.display());
    println!("Claude settings: {}", paths.claude.display());
    println!("worker: OpenCode v1.18.25 (local delegated turns)");
    println!("listener: launchd-owned IPv4 loopback port selected at install");
    println!("caller headers: X-LAO-Key: <redacted> (one per client)");
    Ok(())
}

#[cfg(target_os = "macos")]
fn install(selected: &Selected) -> Result<()> {
    let paths = paths()?;
    let _lock = Lock::acquire(&paths.state)?;
    if paths.state.join(RECORD).exists() {
        let mut transaction = Transaction::load(&paths.state)?;
        if transaction.record.phase == Phase::Installed {
            transaction.validate_installed()?;
            validate_path(&paths.plist, &paths.state.join(PLIST_AFTER), 0o600)?;
            if !service_loaded()? {
                return Err(invalid("launchd service").into());
            }
            verify_ready(&paths, transaction.record.port)?;
            if installed_daemon_matches(&paths)? {
                println!("installed: existing LAO setup is healthy and unchanged");
                return Ok(());
            }
            transaction.restore()?;
            deactivate(&paths)?;
            remove_optional(&paths.daemon)?;
            remove_optional(&paths.state.join(DAEMON_ERROR))?;
            transaction.discard()?;
            remove_optional(&paths.adopted)?;
        } else {
            recover(&paths, &transaction)?;
        }
    }
    if service_loaded()? || paths.plist.exists() {
        return Err(conflict("conflicting launchd service").into());
    }

    let clients = preflight_clients()?;
    if selected.choice.router == Router::Semantic {
        println!("preparing semantic router...");
        lao_route::prepare(&paths.router)?;
    }
    println!("preparing OpenCode worker...");
    lao_opencode::prepare(&paths.worker)?;
    if selected.choice.runtime == Runtime::LlamaCpp {
        println!("preparing local runtime...");
        let llama = lao_run::prepare(&paths.runtime)?;
        lao_run::plan(&llama, lao_run::Mode::Light)?;
        println!("preparing local model...");
        lao_model::prepare(&paths.model)?;
    }
    if !paths.daemon_source.is_file() {
        return Err(invalid("lao-daemon binary").into());
    }
    let daemon = fs::read(&paths.daemon_source)?;

    let codex_original = read_optional(&paths.codex)?;
    let claude_original = read_optional(&paths.claude)?;
    let claude_mcp_original = read_optional(&paths.claude_mcp)?;
    let codex_catalog = paths
        .codex
        .parent()
        .ok_or_else(|| invalid("Codex model catalog"))?
        .join("models_cache.json");
    if !codex_catalog.is_file() {
        probe(&clients.codex, &["debug", "models"])?;
    }
    if !codex_catalog.is_file() {
        return Err(invalid("Codex model catalog").into());
    }
    let codex_catalog = codex_catalog
        .to_str()
        .ok_or_else(|| invalid("Codex model catalog"))?;
    let port = free_port()?;
    let codex_caller = caller()?;
    let claude_caller = caller()?;
    let codex_after = lao_codex::configure(
        codex_original.as_deref(),
        port,
        &codex_caller,
        codex_catalog,
    )?;
    let command = env::current_exe()?;
    let codex_after = lao_codex::configure_worker(&codex_after, &command)?;
    let claude_after = lao_claude::configure(claude_original.as_deref(), port, &claude_caller)?;
    let claude_mcp_after = lao_claude::configure_worker(claude_mcp_original.as_deref(), &command)?;

    let mut transaction = Transaction::prepare(
        &paths,
        port,
        &codex_after,
        &claude_after,
        &claude_mcp_after,
        selected.choice.router,
        selected.vllm.as_ref(),
    )?;
    let worker_caller = caller()?;
    write_atomic(&paths.worker_key, worker_caller.as_bytes(), 0o600)?;
    let plist = plist(
        &paths,
        selected,
        port,
        &codex_caller,
        &claude_caller,
        &clients,
        &paths.worker_key,
    )?;
    write_atomic(&paths.state.join(PLIST_AFTER), plist.as_bytes(), 0o600)?;
    let result = (|| -> Result<()> {
        remove_optional(&paths.adopted)?;
        write_atomic(&paths.state.join(DAEMON_ERROR), b"", 0o600)?;
        write_atomic(&paths.daemon, &daemon, 0o700)?;
        write_atomic(&paths.plist, plist.as_bytes(), 0o600)?;
        bootstrap(&paths)?;
        verify_ready(&paths, port)?;
        transaction.apply()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = transaction.restore_changed();
        let _ = deactivate(&paths);
        let _ = remove_optional(&paths.daemon);
        let _ = remove_optional(&paths.adopted);
        let _ = remove_optional(&paths.state.join(DAEMON_ERROR));
        let _ = remove_optional(&paths.worker_key);
        let _ = transaction.discard();
        return Err(error);
    }
    println!("installed: Codex and Claude now use the launchd-owned LAO gate");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn install(_: &Selected) -> Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "macOS only").into())
}

#[cfg(target_os = "macos")]
fn off() -> Result<()> {
    let paths = paths()?;
    let _lock = Lock::acquire(&paths.state)?;
    let mut transaction = Transaction::load(&paths.state)?;
    match transaction.record.phase {
        Phase::Installed => {
            validate_path(&paths.plist, &paths.state.join(PLIST_AFTER), 0o600)?;
            transaction.restore()?;
        }
        Phase::Installing | Phase::Restoring => {
            if transaction.record.phase == Phase::Restoring {
                transaction.finish_restore()?;
            } else {
                transaction.restore_changed()?;
            }
            transaction.phase(Phase::Restored)?;
        }
        Phase::Restored => {}
    }
    deactivate(&paths)?;
    remove_optional(&paths.daemon)?;
    remove_optional(&paths.state.join(DAEMON_ERROR))?;
    remove_optional(&paths.worker_key)?;
    transaction.discard()?;
    remove_optional(&paths.adopted)?;
    println!("off: LAO settings removed; unrelated client settings preserved");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn off() -> Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "macOS only").into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolArgs {
    objective: String,
    allowed_paths: Vec<PathBuf>,
    #[serde(default)]
    session_id: Option<String>,
}

fn mcp() -> Result<()> {
    let paths = paths()?;
    let transaction = Transaction::load(&paths.state)?;
    if transaction.record.phase != Phase::Installed {
        return Err(conflict("lao is not installed").into());
    }
    let policy: Box<dyn lao_route_api::Policy> = match transaction.record.router {
        Router::Semantic => Box::new(lao_route::Semantic::open(&paths.router)?),
        Router::Safe => Box::new(lao_route::Router),
        Router::VllmSemantic => Box::new(lao_route::VllmSemantic::new(
            transaction
                .record
                .router_addr
                .ok_or_else(|| invalid("worker router"))?,
            transaction
                .record
                .router_key
                .as_deref()
                .map(read_bearer)
                .transpose()?,
        )),
    };
    let mut agent = None;
    let root = env::current_dir()?;
    if !root.join(".git").exists() {
        return Err(invalid("worker repository").into());
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in BufReader::new(stdin.lock()).lines() {
        let line = line?;
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(_) => continue,
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request
            .get("method")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let response = match method {
            "initialize" => serde_json::json!({
                "protocolVersion": request
                    .pointer("/params/protocolVersion")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("2025-06-18"),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "lao", "version": env!("CARGO_PKG_VERSION") }
            }),
            "ping" => serde_json::json!({}),
            "tools/list" => tools(),
            "tools/call" => call(
                &request,
                &*policy,
                &mut agent,
                &root,
                &paths,
                transaction.record.port,
            ),
            _ => {
                write_rpc(
                    &mut stdout,
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": "method not found" }
                    }),
                )?;
                continue;
            }
        };
        write_rpc(
            &mut stdout,
            serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": response }),
        )?;
    }
    Ok(())
}

fn tools() -> serde_json::Value {
    serde_json::json!({
        "tools": [{
            "name": "execute",
            "description": "Before editing directly, call this once when the request is one small mechanical implementation and every writable path is known. Pass one bounded objective and exact repository-relative paths. If LAO returns Cloud, do the work yourself. If Local completes, review its changed paths and verify; do not repeat the edit.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["objective", "allowed_paths"],
                "properties": {
                    "objective": { "type": "string", "maxLength": 4096 },
                    "allowed_paths": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 16,
                        "items": {
                            "type": "string",
                            "maxLength": 1024,
                            "description": "Exact repository-relative path; absolute paths and Git metadata are rejected."
                        }
                    },
                    "session_id": { "type": "string", "maxLength": 68 }
                }
            }
        }]
    })
}

fn call(
    request: &serde_json::Value,
    policy: &dyn lao_route_api::Policy,
    agent: &mut Option<lao_opencode::OpenCode>,
    root: &Path,
    paths: &Paths,
    port: u16,
) -> serde_json::Value {
    if request
        .pointer("/params/name")
        .and_then(serde_json::Value::as_str)
        != Some("execute")
    {
        return tool_error("unknown tool");
    }
    let args = match request
        .pointer("/params/arguments")
        .cloned()
        .and_then(|value| serde_json::from_value::<ToolArgs>(value).ok())
    {
        Some(args) if !args.objective.is_empty() && args.objective.len() <= 4096 => args,
        _ => return tool_error("invalid bounded turn"),
    };
    let context =
        lao_route_api::Context::new(lao_route_api::Client::Worker, lao_route_api::Op::Chat);
    if policy.decide_query(context, &args.objective) != lao_route_api::Decision::Local {
        return tool_result(
            "cloud",
            "LAO kept this turn in Cloud. Execute it in the current Codex or Claude harness.",
            None,
            &[],
        );
    }
    if agent.is_none() {
        let worker_key = match read_worker_key(&paths.worker_key) {
            Ok(key) => key,
            Err(_) => return tool_error("local worker is unavailable"),
        };
        let worker = match lao_opencode::OpenCode::new(
            lao_opencode::binary(&paths.worker),
            (Ipv4Addr::LOCALHOST, port).into(),
            worker_key,
        ) {
            Ok(worker) => worker,
            Err(_) => return tool_error("local worker is unavailable"),
        };
        *agent = Some(worker);
    }
    let task = lao_agent_api::Task {
        root: root.to_owned(),
        instruction: args.objective,
        allowed: args.allowed_paths,
        session: args.session_id,
        deadline: Duration::from_secs(10 * 60),
    };
    match lao_agent_api::Agent::turn(agent.as_ref().expect("agent initialized"), &task) {
        Ok(report) => {
            let status = match report.outcome {
                lao_agent_api::Outcome::Complete => "complete",
                lao_agent_api::Outcome::AgentFailed => "agent_failed",
                lao_agent_api::Outcome::TimedOut => "timed_out",
            };
            let message = if report.outcome == lao_agent_api::Outcome::Complete {
                "Local OpenCode turn completed. Review the changed files and run the appropriate verification in this harness."
            } else {
                "Local OpenCode turn stopped. Review before retrying in Cloud."
            };
            tool_result(status, message, report.session.as_deref(), &report.changed)
        }
        Err(_) => tool_error("local worker rejected or failed the bounded turn"),
    }
}

fn tool_result(
    status: &str,
    message: &str,
    session: Option<&str>,
    changed: &[PathBuf],
) -> serde_json::Value {
    let changed: Vec<_> = changed.iter().filter_map(|path| path.to_str()).collect();
    serde_json::json!({
        "content": [{ "type": "text", "text": message }],
        "structuredContent": {
            "status": status,
            "session_id": session,
            "changed_paths": changed
        },
        "isError": false
    })
}

fn tool_error(message: &str) -> serde_json::Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true
    })
}

fn write_rpc(output: &mut impl Write, value: serde_json::Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, &value).map_err(|_| invalid("MCP response"))?;
    output.write_all(b"\n")?;
    output.flush()
}

fn read_worker_key(path: &Path) -> io::Result<String> {
    key_file(path.to_owned(), "worker key")?;
    let key = fs::read_to_string(path)?;
    if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid("worker key"));
    }
    Ok(key)
}

fn read_bearer(path: &Path) -> io::Result<String> {
    key_file(path.to_owned(), "router key")?;
    let mut key = fs::read_to_string(path)?;
    while key.ends_with(['\r', '\n']) {
        key.pop();
    }
    if key.is_empty() || key.len() > 4096 || !key.bytes().all(|byte| byte > b' ' && byte < 0x7f) {
        return Err(invalid("router key"));
    }
    Ok(key)
}

#[cfg(target_os = "macos")]
fn smoke() -> Result<()> {
    let paths = paths()?;
    let _lock = Lock::acquire(&paths.state)?;
    let transaction = Transaction::load(&paths.state)?;
    if transaction.record.phase != Phase::Installed {
        return Err(conflict("lao is not installed").into());
    }
    transaction.validate_installed()?;
    validate_path(&paths.plist, &paths.state.join(PLIST_AFTER), 0o600)?;
    if !service_loaded()? {
        return Err(invalid("launchd service").into());
    }
    let codex = fs::read(&paths.codex)?;
    let claude = fs::read(&paths.claude)?;
    let (codex_caller, claude_caller) = managed_callers(&codex, &claude, transaction.record.port)?;

    let codex_catalog = paths
        .codex
        .parent()
        .ok_or_else(|| invalid("Codex model catalog"))?
        .join("models_cache.json");
    let codex_elapsed = lao_optimize::codex(
        "codex",
        codex_catalog,
        transaction.record.port,
        &codex_caller,
        lao_codex::DELEGATION_INSTRUCTIONS,
    )?;
    println!("Codex local: ok ({} ms)", codex_elapsed.as_millis());
    let claude_elapsed = lao_optimize::claude("claude", transaction.record.port, &claude_caller)?;
    println!("Claude local: ok ({} ms)", claude_elapsed.as_millis());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn smoke() -> Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "macOS only").into())
}

fn managed_callers(codex: &[u8], claude: &[u8], port: u16) -> io::Result<(String, String)> {
    let codex = std::str::from_utf8(codex)
        .map_err(|_| invalid("managed Codex settings"))?
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| invalid("managed Codex settings"))?;
    let provider = &codex["model_providers"]["lao"];
    let codex_caller = provider["http_headers"]["X-LAO-Key"]
        .as_str()
        .ok_or_else(|| invalid("managed Codex settings"))?;
    if codex["model_provider"].as_str() != Some("lao")
        || provider["base_url"].as_str() != Some(&format!("http://127.0.0.1:{port}/oai"))
        || provider["env_http_headers"]["X-LAO-Local"].as_str() != Some("LAO_LOCAL_SELECTOR")
    {
        return Err(invalid("managed Codex settings"));
    }

    let claude: serde_json::Value =
        serde_json::from_slice(claude).map_err(|_| invalid("managed Claude settings"))?;
    let env = claude["env"]
        .as_object()
        .ok_or_else(|| invalid("managed Claude settings"))?;
    let claude_caller = env
        .get("ANTHROPIC_CUSTOM_HEADERS")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| value.strip_prefix("X-LAO-Key: "))
        .ok_or_else(|| invalid("managed Claude settings"))?;
    if env
        .get("ANTHROPIC_BASE_URL")
        .and_then(serde_json::Value::as_str)
        != Some(&format!("http://127.0.0.1:{port}/ant"))
        || !managed_caller(codex_caller)
        || !managed_caller(claude_caller)
        || codex_caller == claude_caller
    {
        return Err(invalid("managed client settings"));
    }
    Ok((codex_caller.to_owned(), claude_caller.to_owned()))
}

fn managed_caller(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn installed_daemon_matches(paths: &Paths) -> io::Result<bool> {
    let source = fs::metadata(&paths.daemon_source)?;
    let installed = fs::symlink_metadata(&paths.daemon)?;
    if !source.file_type().is_file()
        || source.mode() & 0o777 != 0o700
        || !installed.file_type().is_file()
        || installed.mode() & 0o777 != 0o700
    {
        return Err(invalid("lao-daemon binary"));
    }
    Ok(fs::read(&paths.daemon_source)? == fs::read(&paths.daemon)?)
}

fn preflight_clients() -> Result<Clients> {
    let keys: Vec<_> = env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .collect();
    if !lao_codex::conflicts(keys.iter().map(String::as_str), false).is_empty() {
        return Err(conflict("conflicting Codex environment configuration").into());
    }
    if !lao_claude::conflicts(keys.iter().map(String::as_str), false, false).is_empty() {
        return Err(conflict("conflicting Claude environment configuration").into());
    }

    let codex_bin = executable("codex")?;
    let claude_bin = executable("claude")?;
    let codex = command(&codex_bin, &["--version"])?;
    if lao_codex::support(&codex) != lao_codex::Support::Observed {
        return Err(conflict("unsupported Codex version").into());
    }
    let auth = command(&codex_bin, &["login", "status"])?;
    let cloud = match lao_codex::auth(&auth) {
        lao_codex::Auth::ChatGpt => "chatgpt",
        lao_codex::Auth::ApiKey => "openai",
        _ => return Err(conflict("unsupported Codex authentication").into()),
    };
    let claude = command(&claude_bin, &["--version"])?;
    if lao_claude::support(&claude) != lao_claude::Support::Observed {
        return Err(conflict("unsupported Claude Code version").into());
    }
    Ok(Clients {
        codex: codex_bin,
        claude: claude_bin,
        cloud,
    })
}

fn executable(name: &str) -> io::Result<PathBuf> {
    let path = env::var_os("PATH").ok_or_else(|| invalid("client executable"))?;
    for directory in env::split_paths(&path).filter(|path| path.is_absolute()) {
        let candidate = directory.join(name);
        if fs::metadata(&candidate)
            .is_ok_and(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
        {
            return Ok(candidate);
        }
    }
    Err(invalid("client executable"))
}

fn command(bin: &Path, args: &[&str]) -> io::Result<String> {
    let output = Command::new(bin).args(args).stdin(Stdio::null()).output()?;
    if !output.status.success() {
        return Err(invalid("client preflight"));
    }
    let mut bytes = output.stdout;
    bytes.extend_from_slice(&output.stderr);
    String::from_utf8(bytes).map_err(|_| invalid("client preflight"))
}

fn probe(bin: &Path, args: &[&str]) -> io::Result<()> {
    Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success()
        .then_some(())
        .ok_or_else(|| invalid("client preflight"))
}

fn paths() -> io::Result<Paths> {
    let home = PathBuf::from(env::var_os("HOME").ok_or_else(|| invalid("HOME"))?);
    let state = home.join("Library/Application Support/lao");
    let codex_root = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let claude_root = env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".claude"));
    if !home.is_absolute() || !codex_root.is_absolute() || !claude_root.is_absolute() {
        return Err(invalid("client configuration path"));
    }
    let daemon_source = env::current_exe()?
        .parent()
        .ok_or_else(|| invalid("lao binary"))?
        .join("lao-daemon");
    Ok(Paths {
        state: state.clone(),
        codex: codex_root.join("config.toml"),
        claude: claude_root.join("settings.json"),
        claude_mcp: home.join(".claude.json"),
        plist: home
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")),
        adopted: state.join("adopted"),
        model: home.join("Library/Caches/lao/models"),
        runtime: home.join("Library/Caches/lao/runtimes"),
        router: home.join("Library/Caches/lao/routers/minilm"),
        worker: home.join("Library/Caches/lao/workers/opencode"),
        worker_key: state.join("worker.key"),
        daemon_source,
        daemon: state.join("lao-daemon"),
        optimize: state.join("optimize.state"),
    })
}

fn required_addr(name: &'static str) -> io::Result<SocketAddrV4> {
    match env::var(name) {
        Ok(value) => loopback(&value, name),
        Err(_) => Err(invalid(name)),
    }
}

fn loopback(value: &str, name: &'static str) -> io::Result<SocketAddrV4> {
    value
        .parse::<SocketAddrV4>()
        .ok()
        .filter(|addr| *addr.ip() == Ipv4Addr::LOCALHOST && addr.port() != 0)
        .ok_or_else(|| invalid(name))
}

fn optional_key(name: &'static str) -> io::Result<Option<PathBuf>> {
    env::var_os(name)
        .map(PathBuf::from)
        .map(|path| key_file(path, name))
        .transpose()
}

fn required_key(name: &'static str) -> io::Result<PathBuf> {
    let path = env::var_os(name).ok_or_else(|| invalid(name))?;
    key_file(PathBuf::from(path), name)
}

fn key_file(path: PathBuf, name: &'static str) -> io::Result<PathBuf> {
    if !path.is_absolute() || path.to_str().is_none() {
        return Err(invalid(name));
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| invalid(name))?;
    if !metadata.file_type().is_file() || metadata.mode() & 0o7777 != 0o600 {
        return Err(invalid(name));
    }
    #[cfg(target_os = "macos")]
    if metadata.uid().to_string() != uid()? {
        return Err(invalid(name));
    }
    Ok(path)
}

fn recover(paths: &Paths, transaction: &Transaction) -> io::Result<()> {
    if transaction.record.phase == Phase::Restoring {
        transaction.finish_restore()?;
    } else {
        transaction.restore_changed()?;
    }
    deactivate(paths)?;
    remove_optional(&paths.daemon)?;
    remove_optional(&paths.state.join(DAEMON_ERROR))?;
    remove_optional(&paths.worker_key)?;
    transaction.discard()?;
    remove_optional(&paths.adopted)
}

fn inspect(path: &Path) -> io::Result<(Entry, Vec<u8>)> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err(conflict("managed path is not a regular file"));
            }
            Ok((
                Entry {
                    path: path.to_path_buf(),
                    existed: true,
                    mode: metadata.mode() & 0o777,
                },
                fs::read(path)?,
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((
            Entry {
                path: path.to_path_buf(),
                existed: false,
                mode: 0o600,
            },
            Vec::new(),
        )),
        Err(error) => Err(error),
    }
}

fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    inspect(path).map(|(entry, bytes)| entry.existed.then_some(bytes))
}

fn write_entry(entry: &Entry, bytes: &[u8]) -> io::Result<()> {
    write_atomic(&entry.path, bytes, entry.mode)
}

fn restore_entry(entry: &Entry, before: &[u8]) -> io::Result<()> {
    if entry.existed {
        write_entry(entry, before)
    } else {
        remove_optional(&entry.path)
    }
}

fn finish_restore(entry: &Entry, from: &Path, to: &Path) -> io::Result<()> {
    let from = fs::read(from)?;
    let to = fs::read(to)?;
    let target_exists = entry.existed || !to.is_empty();
    let current = match fs::symlink_metadata(&entry.path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            Some((fs::read(&entry.path)?, metadata.mode() & 0o777))
        }
        Ok(_) => return Err(conflict("managed client file changed during restore")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let matches = |expected: &[u8], exists: bool| {
        if exists {
            current
                .as_ref()
                .is_some_and(|(bytes, mode)| bytes == expected && *mode == entry.mode)
        } else {
            current.is_none()
        }
    };
    if matches(&to, target_exists) {
        return Ok(());
    }
    if !matches(&from, true) {
        return Err(conflict("managed client file changed during restore"));
    }
    if target_exists {
        write_entry(entry, &to)
    } else {
        remove_optional(&entry.path)
    }
}

fn read_managed(entry: &Entry) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(&entry.path)
        .map_err(|_| conflict("managed file is missing or changed"))?;
    if !metadata.file_type().is_file() || metadata.mode() & 0o777 != entry.mode {
        return Err(conflict("managed file is missing or changed"));
    }
    fs::read(&entry.path)
}

fn read_managed_optional(entry: &Entry) -> io::Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(&entry.path) {
        Ok(metadata) if metadata.file_type().is_file() && metadata.mode() & 0o777 == entry.mode => {
            fs::read(&entry.path).map(Some)
        }
        Ok(_) => Err(conflict("managed file is missing or changed")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn write_optional(entry: &Entry, bytes: Option<&[u8]>) -> io::Result<()> {
    match bytes {
        Some(bytes) => write_entry(entry, bytes),
        None => remove_optional(&entry.path),
    }
}

fn restore_if_managed(entry: &Entry, before: &Path, after: &Path) -> io::Result<()> {
    let current = match fs::symlink_metadata(&entry.path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            Some((fs::read(&entry.path)?, metadata.mode() & 0o777))
        }
        Ok(_) => return Err(conflict("managed client file changed during recovery")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let old = fs::read(before)?;
    let managed = fs::read(after)?;
    if current
        .as_ref()
        .is_some_and(|(bytes, mode)| bytes == &managed && *mode == entry.mode)
    {
        restore_entry(entry, &old)
    } else if (entry.existed
        && current
            .as_ref()
            .is_some_and(|(bytes, mode)| bytes == &old && *mode == entry.mode))
        || (!entry.existed && current.is_none())
    {
        Ok(())
    } else {
        Err(conflict("managed client file changed during recovery"))
    }
}

fn validate(entry: &Entry, expected: &Path) -> io::Result<()> {
    validate_path(&entry.path, expected, entry.mode)
}

fn validate_original(entry: &Entry, expected: &Path) -> io::Result<()> {
    if entry.existed {
        validate(entry, expected)
    } else if entry.path.exists() {
        Err(conflict("client file changed during install"))
    } else {
        Ok(())
    }
}

fn validate_path(path: &Path, expected: &Path, mode: u32) -> io::Result<()> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| conflict("managed file is missing or changed"))?;
    if !metadata.file_type().is_file()
        || metadata.mode() & 0o777 != mode
        || fs::read(path)? != fs::read(expected)?
    {
        return Err(conflict("managed file is missing or changed"));
    }
    Ok(())
}

fn write_record(state: &Path, record: &Record) -> io::Result<()> {
    let bytes = serde_json::to_vec(record).map_err(|_| invalid("install record"))?;
    write_atomic(&state.join(RECORD), &bytes, 0o600)
}

fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| invalid("managed path"))?;
    if !parent.exists() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    if !fs::symlink_metadata(parent)?.file_type().is_dir() {
        return Err(conflict("managed parent is not a directory"));
    }
    if let Ok(metadata) = fs::symlink_metadata(path)
        && !metadata.file_type().is_file()
    {
        return Err(conflict("managed path is not a regular file"));
    }
    let mut random = [0_u8; 8];
    getrandom::getrandom(&mut random).map_err(|_| invalid("random"))?;
    let suffix = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("managed path"))?;
    let temporary = parent.join(format!(".{name}.{suffix}.tmp"));
    let mut pending = Pending(Some(temporary.clone()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(mode))?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    pending.0 = None;
    Ok(())
}

struct Pending(Option<PathBuf>);

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

fn private_dir(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => return Err(conflict("state path is not a directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(error) => return Err(error),
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

fn remove_optional(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn caller() -> io::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| invalid("random"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn free_port() -> io::Result<u16> {
    Ok(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port())
}

#[cfg(target_os = "macos")]
fn plist(
    paths: &Paths,
    selected: &Selected,
    port: u16,
    codex: &str,
    claude: &str,
    clients: &Clients,
    worker_key: &Path,
) -> io::Result<String> {
    let error_path = paths.state.join(DAEMON_ERROR);
    let codex_catalog = paths
        .codex
        .parent()
        .ok_or_else(|| invalid("non-UTF-8 install path"))?
        .join("models_cache.json");
    let values = [
        paths.daemon.to_str(),
        paths.adopted.to_str(),
        error_path.to_str(),
        clients.codex.to_str(),
        clients.claude.to_str(),
        paths.optimize.to_str(),
        worker_key.to_str(),
    ];
    if values.iter().any(Option::is_none) || codex_catalog.to_str().is_none() {
        return Err(invalid("non-UTF-8 install path"));
    }
    let mut adapters = adapter_env(paths, selected)?;
    env_entry(
        &mut adapters,
        "LAO_CODEX_BIN",
        values[3].ok_or_else(|| invalid("non-UTF-8 install path"))?,
    );
    env_entry(
        &mut adapters,
        "LAO_CLAUDE_BIN",
        values[4].ok_or_else(|| invalid("non-UTF-8 install path"))?,
    );
    env_entry(
        &mut adapters,
        "LAO_OPTIMIZE_STATE",
        values[5].ok_or_else(|| invalid("non-UTF-8 install path"))?,
    );
    env_entry(
        &mut adapters,
        "LAO_CODEX_CATALOG",
        codex_catalog
            .to_str()
            .ok_or_else(|| invalid("non-UTF-8 install path"))?,
    );
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>{LABEL}</string>\n<key>ProgramArguments</key><array><string>{daemon}</string></array>\n<key>EnvironmentVariables</key><dict>\n<key>LAO_ADOPTED_FILE</key><string>{adopted}</string>\n<key>LAO_LOCAL_CANARY</key><string>1</string>\n<key>LAO_CODEX_CALLER</key><string>{codex}</string>\n<key>LAO_CLAUDE_CALLER</key><string>{claude}</string>\n<key>LAO_WORKER_KEY_FILE</key><string>{worker}</string>\n<key>LAO_CODEX_CLOUD</key><string>{codex_cloud}</string>\n{adapters}</dict>\n<key>RunAtLoad</key><true/>\n<key>ThrottleInterval</key><integer>1</integer>\n<key>Sockets</key><dict><key>gate</key><dict><key>SockNodeName</key><string>127.0.0.1</string><key>SockServiceName</key><integer>{port}</integer><key>SockFamily</key><string>IPv4</string><key>SockType</key><string>stream</string><key>SockProtocol</key><string>TCP</string><key>SockPassive</key><true/></dict></dict>\n<key>StandardErrorPath</key><string>{error}</string>\n</dict></plist>\n",
        daemon = xml(values[0].unwrap()),
        adopted = xml(values[1].unwrap()),
        error = xml(values[2].unwrap()),
        codex_cloud = clients.cloud,
        worker = xml(values[6].unwrap()),
    ))
}

#[cfg(target_os = "macos")]
fn adapter_env(paths: &Paths, selected: &Selected) -> io::Result<String> {
    let mut output = String::new();
    env_entry(&mut output, "RAYON_NUM_THREADS", "2");
    env_entry(&mut output, "VECLIB_MAXIMUM_THREADS", "2");
    env_entry(&mut output, "LAO_ROUTER", selected.choice.router.id());
    env_entry(&mut output, "LAO_RUNTIME", selected.choice.runtime.id());
    if selected.choice.router == Router::Semantic {
        env_entry(
            &mut output,
            "LAO_ROUTER_DIR",
            paths
                .router
                .to_str()
                .ok_or_else(|| invalid("non-UTF-8 install path"))?,
        );
    }
    if let Some(adapter) = &selected.vllm {
        env_entry(
            &mut output,
            "LAO_VLLM_ROUTER_ADDR",
            &adapter.addr.to_string(),
        );
        if let Some(path) = &adapter.key {
            env_entry(
                &mut output,
                "LAO_VLLM_ROUTER_KEY_FILE",
                path.to_str()
                    .ok_or_else(|| invalid("LAO_VLLM_ROUTER_KEY_FILE"))?,
            );
        }
    }
    match selected.choice.runtime {
        Runtime::LlamaCpp => {
            env_entry(
                &mut output,
                "LAO_MODEL_DIR",
                paths
                    .model
                    .to_str()
                    .ok_or_else(|| invalid("non-UTF-8 install path"))?,
            );
            env_entry(
                &mut output,
                "LAO_LLAMA_SERVER",
                lao_run::binary(&paths.runtime)
                    .to_str()
                    .ok_or_else(|| invalid("non-UTF-8 install path"))?,
            );
        }
        Runtime::External => {
            let adapter = selected
                .external
                .as_ref()
                .ok_or_else(|| invalid("external runtime"))?;
            env_entry(&mut output, "LAO_EXTERNAL_ADDR", &adapter.addr.to_string());
            env_entry(
                &mut output,
                "LAO_EXTERNAL_KEY_FILE",
                adapter
                    .key
                    .as_deref()
                    .and_then(Path::to_str)
                    .ok_or_else(|| invalid("LAO_EXTERNAL_KEY_FILE"))?,
            );
        }
    }
    Ok(output)
}

#[cfg(target_os = "macos")]
fn env_entry(output: &mut String, name: &str, value: &str) {
    output.push_str("<key>");
    output.push_str(name);
    output.push_str("</key><string>");
    output.push_str(&xml(value));
    output.push_str("</string>\n");
}

#[cfg(target_os = "macos")]
fn bootstrap(paths: &Paths) -> io::Result<()> {
    if !Command::new("/usr/bin/plutil")
        .args(["-lint"])
        .arg(&paths.plist)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success()
    {
        return Err(invalid("launchd plist"));
    }
    let status = Command::new("/bin/launchctl")
        .args(["bootstrap", &domain()?])
        .arg(&paths.plist)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(invalid("launchd bootstrap"))
    }
}

#[cfg(target_os = "macos")]
fn deactivate(paths: &Paths) -> io::Result<()> {
    if service_loaded()? {
        let status = Command::new("/bin/launchctl")
            .args(["bootout", &service()?])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(invalid("launchd bootout"));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while service_loaded()? {
            if Instant::now() >= deadline {
                return Err(invalid("launchd shutdown"));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    remove_optional(&paths.plist)?;
    lao_optimize::Store::new(paths.optimize.clone()).remove()
}

#[cfg(not(target_os = "macos"))]
fn deactivate(_: &Paths) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "macOS only"))
}

#[cfg(target_os = "macos")]
fn service_loaded() -> io::Result<bool> {
    Ok(Command::new("/bin/launchctl")
        .args(["print", &service()?])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}

#[cfg(target_os = "macos")]
fn domain() -> io::Result<String> {
    Ok(format!("gui/{}", uid()?))
}

#[cfg(target_os = "macos")]
fn service() -> io::Result<String> {
    Ok(format!("{}/{LABEL}", domain()?))
}

#[cfg(target_os = "macos")]
fn uid() -> io::Result<String> {
    let output = Command::new("/usr/bin/id").arg("-u").output()?;
    if !output.status.success() {
        return Err(invalid("uid"));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| invalid("uid"))
}

#[cfg(target_os = "macos")]
fn verify_ready(paths: &Paths, port: u16) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if hello(port).is_ok() && adoption(paths, port).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(invalid("launchd listener readiness"));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(target_os = "macos")]
fn adoption(paths: &Paths, port: u16) -> io::Result<()> {
    let metadata = fs::symlink_metadata(&paths.adopted)?;
    if !metadata.file_type().is_file()
        || metadata.mode() & 0o777 != 0o600
        || fs::read_to_string(&paths.adopted)? != format!("127.0.0.1:{port}")
    {
        return Err(invalid("adoption proof"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn hello(port: u16) -> io::Result<()> {
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_secs(1),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    write!(
        stream,
        "HEAD /ant/api/hello HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = Vec::new();
    stream.take(4097).read_to_end(&mut response)?;
    let length = b"content-length: 0\r\n";
    if response.len() > 4096
        || !response.starts_with(b"HTTP/1.1 200 OK\r\n")
        || !response
            .windows(length.len())
            .any(|part| part.eq_ignore_ascii_case(length))
        || !response.ends_with(b"\r\n\r\n")
    {
        return Err(invalid("daemon hello"));
    }
    Ok(())
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn conflict(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::AlreadyExists, message)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = env::temp_dir().join(format!("lao-install-{}-{stamp}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn paths(&self) -> Paths {
            let state = self.0.join("state");
            Paths {
                state: state.clone(),
                codex: self.0.join("codex/config.toml"),
                claude: self.0.join("claude/settings.json"),
                claude_mcp: self.0.join(".claude.json"),
                plist: self.0.join("daemon.plist"),
                adopted: state.join("adopted"),
                model: self.0.join("models"),
                runtime: self.0.join("runtimes"),
                router: self.0.join("router"),
                worker: self.0.join("worker"),
                worker_key: state.join("worker.key"),
                daemon_source: self.0.join("lao-daemon-source"),
                daemon: self.0.join("lao-daemon"),
                optimize: state.join("optimize.state"),
            }
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn prepared(temp: &Temp) -> (Paths, Transaction, Vec<u8>, Vec<u8>, Vec<u8>) {
        let paths = temp.paths();
        fs::create_dir_all(paths.codex.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.claude.parent().unwrap()).unwrap();
        let codex = b"model = \"gpt-5.4\"\n".to_vec();
        let claude = br#"{"permissions":{"defaultMode":"default"}}"#.to_vec();
        let claude_mcp = br#"{"mcpServers":{}}"#.to_vec();
        fs::write(&paths.codex, &codex).unwrap();
        fs::write(&paths.claude, &claude).unwrap();
        fs::write(&paths.claude_mcp, &claude_mcp).unwrap();
        fs::set_permissions(&paths.codex, fs::Permissions::from_mode(0o640)).unwrap();
        fs::set_permissions(&paths.claude, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&paths.claude_mcp, fs::Permissions::from_mode(0o600)).unwrap();
        private_dir(&paths.state).unwrap();
        let codex_after = lao_codex::configure(
            Some(&codex),
            8765,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "/tmp/models.json",
        )
        .unwrap();
        let claude_after = lao_claude::configure(
            Some(&claude),
            8765,
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
        )
        .unwrap();
        let codex_after = lao_codex::configure_worker(&codex_after, Path::new("/tmp/lao")).unwrap();
        let claude_mcp_after =
            lao_claude::configure_worker(Some(&claude_mcp), Path::new("/tmp/lao")).unwrap();
        let transaction = Transaction::prepare(
            &paths,
            8765,
            &codex_after,
            &claude_after,
            &claude_mcp_after,
            Router::Semantic,
            None,
        )
        .unwrap();
        (paths, transaction, codex, claude, claude_mcp)
    }

    #[test]
    fn settings_install_and_off_are_exact_and_conflict_aware() {
        let temp = Temp::new();
        let (paths, mut transaction, codex, claude, _claude_mcp) = prepared(&temp);
        let _lock = Lock::acquire(&paths.state).unwrap();
        assert!(Lock::acquire(&paths.state).is_err());
        transaction.apply().unwrap();
        assert!(
            String::from_utf8(fs::read(&paths.codex).unwrap())
                .unwrap()
                .contains("X-LAO-Key")
        );
        assert_eq!(fs::metadata(&paths.codex).unwrap().mode() & 0o777, 0o640);
        fs::write(&paths.claude, b"user edit").unwrap();
        assert!(transaction.restore().is_err());
        write_entry(
            &transaction.record.claude,
            &fs::read(paths.state.join(CLAUDE_AFTER)).unwrap(),
        )
        .unwrap();
        let mut live: serde_json::Value =
            serde_json::from_slice(&fs::read(&paths.claude_mcp).unwrap()).unwrap();
        live["usageCount"] = serde_json::Value::from(3);
        fs::write(&paths.claude_mcp, serde_json::to_vec(&live).unwrap()).unwrap();
        transaction.restore().unwrap();
        assert_eq!(fs::read(&paths.codex).unwrap(), codex);
        assert_eq!(fs::read(&paths.claude).unwrap(), claude);
        let restored: serde_json::Value =
            serde_json::from_slice(&fs::read(&paths.claude_mcp).unwrap()).unwrap();
        assert_eq!(restored["usageCount"], 3);
        assert!(restored["mcpServers"].get("lao").is_none());
    }

    #[test]
    fn failure_at_either_client_write_restores_both_originals() {
        for boundary in 0..=2 {
            let temp = Temp::new();
            let (paths, mut transaction, codex, claude, claude_mcp) = prepared(&temp);
            let result = transaction.apply_with(|index, entry, bytes| {
                if index == boundary {
                    Err(io::Error::other("induced write failure"))
                } else {
                    write_entry(entry, bytes)
                }
            });
            assert!(result.is_err());
            assert_eq!(fs::read(&paths.codex).unwrap(), codex);
            assert_eq!(fs::read(&paths.claude).unwrap(), claude);
            assert_eq!(fs::read(&paths.claude_mcp).unwrap(), claude_mcp);
        }
    }

    #[test]
    fn off_preserves_unrelated_client_edits() {
        let temp = Temp::new();
        let (paths, mut transaction, _, _, _) = prepared(&temp);
        transaction.apply().unwrap();

        let mut codex = fs::read_to_string(&paths.codex).unwrap();
        codex.push_str("\n[projects.\"/tmp/new\"]\ntrust_level = \"trusted\"\n");
        fs::write(&paths.codex, codex).unwrap();
        fs::set_permissions(&paths.codex, fs::Permissions::from_mode(0o640)).unwrap();
        let mut claude: serde_json::Value =
            serde_json::from_slice(&fs::read(&paths.claude).unwrap()).unwrap();
        claude["theme"] = serde_json::Value::String("dark".into());
        fs::write(&paths.claude, serde_json::to_vec(&claude).unwrap()).unwrap();
        fs::set_permissions(&paths.claude, fs::Permissions::from_mode(0o600)).unwrap();

        transaction.validate_installed().unwrap();
        let codex_current = fs::read(&paths.codex).unwrap();
        let claude_current = fs::read(&paths.claude).unwrap();
        let codex_after = fs::read(paths.state.join(CODEX_AFTER)).unwrap();
        let claude_after = fs::read(paths.state.join(CLAUDE_AFTER)).unwrap();
        let codex_before = fs::read(paths.state.join(CODEX_BEFORE)).unwrap();
        let claude_before = fs::read(paths.state.join(CLAUDE_BEFORE)).unwrap();
        let codex_restored =
            lao_codex::restore(&codex_current, &codex_after, Some(&codex_before)).unwrap();
        let claude_restored =
            lao_claude::restore(&claude_current, &claude_after, Some(&claude_before)).unwrap();
        for (name, bytes) in [
            (CODEX_RESTORE_FROM, codex_current.as_slice()),
            (CODEX_RESTORE_TO, codex_restored.as_slice()),
            (CLAUDE_RESTORE_FROM, claude_current.as_slice()),
            (CLAUDE_RESTORE_TO, claude_restored.as_slice()),
        ] {
            write_atomic(&paths.state.join(name), bytes, 0o600).unwrap();
        }
        let claude_mcp = transaction.claude_mcp_restore().unwrap();
        write_atomic(
            &paths.state.join(CLAUDE_MCP_RESTORE_FROM),
            claude_mcp.current.as_deref().unwrap(),
            0o600,
        )
        .unwrap();
        write_atomic(
            &paths.state.join(CLAUDE_MCP_RESTORE_TO),
            claude_mcp.restored.as_deref().unwrap_or_default(),
            0o600,
        )
        .unwrap();
        transaction.phase(Phase::Restoring).unwrap();
        write_entry(&transaction.record.codex, &codex_restored).unwrap();
        transaction.finish_restore().unwrap();
        transaction.phase(Phase::Restored).unwrap();
        let codex = fs::read_to_string(&paths.codex).unwrap();
        assert!(codex.contains("[projects.\"/tmp/new\"]"));
        assert!(!codex.contains("model_provider"));
        let claude: serde_json::Value =
            serde_json::from_slice(&fs::read(&paths.claude).unwrap()).unwrap();
        assert_eq!(claude["theme"], "dark");
        assert_eq!(claude["permissions"]["defaultMode"], "default");
        assert!(claude.get("env").is_none());
    }

    #[test]
    fn repeated_install_reuses_only_the_current_daemon() {
        let temp = Temp::new();
        let paths = temp.paths();
        let source = temp.0.join("current-daemon");
        fs::write(&source, b"current").unwrap();
        std::os::unix::fs::symlink(&source, &paths.daemon_source).unwrap();
        fs::write(&paths.daemon, b"current").unwrap();
        for path in [&source, &paths.daemon] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert!(installed_daemon_matches(&paths).unwrap());

        fs::write(&source, b"new").unwrap();
        assert!(!installed_daemon_matches(&paths).unwrap());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(installed_daemon_matches(&paths).is_err());
    }

    #[test]
    fn smoke_accepts_only_the_installed_client_settings() {
        let codex_caller = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let claude_caller = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
        let codex = lao_codex::configure(None, 8765, codex_caller, "/tmp/models.json").unwrap();
        let claude = lao_claude::configure(None, 8765, claude_caller).unwrap();
        assert_eq!(
            managed_callers(&codex, &claude, 8765).unwrap(),
            (codex_caller.into(), claude_caller.into())
        );

        let changed = String::from_utf8(codex)
            .unwrap()
            .replace("LAO_LOCAL_SELECTOR", "UNMANAGED_SELECTOR");
        assert!(managed_callers(changed.as_bytes(), &claude, 8765).is_err());
    }

    #[test]
    fn selection_arguments_are_explicit_and_persisted() {
        assert_eq!(
            parse([OsString::from("preview")].into_iter()).unwrap(),
            Some(Action::Preview(Choice::default()))
        );
        let choice = Choice {
            router: Router::VllmSemantic,
            runtime: Runtime::External,
        };
        assert_eq!(
            parse(
                [
                    "install",
                    "--router",
                    "vllm-semantic",
                    "--runtime",
                    "external",
                ]
                .map(OsString::from)
                .into_iter()
            )
            .unwrap(),
            Some(Action::Install(choice))
        );

        #[cfg(target_os = "macos")]
        {
            let temp = Temp::new();
            let paths = temp.paths();
            let vllm_key = temp.0.join("vllm.key");
            let external_key = temp.0.join("external.key");
            let selected = Selected {
                choice,
                vllm: Some(Adapter {
                    addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080),
                    key: Some(vllm_key.clone()),
                }),
                external: Some(Adapter {
                    addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9090),
                    key: Some(external_key.clone()),
                }),
            };
            let clients = Clients {
                codex: temp.0.join("codex"),
                claude: temp.0.join("claude"),
                cloud: "chatgpt",
            };
            let bytes = plist(
                &paths,
                &selected,
                8765,
                "codex",
                "claude",
                &clients,
                &paths.worker_key,
            )
            .unwrap();
            for expected in [
                "<key>LAO_CODEX_CATALOG</key>",
                "<key>LAO_ROUTER</key><string>vllm-semantic</string>",
                "<key>LAO_RUNTIME</key><string>external</string>",
                "<key>LAO_VLLM_ROUTER_ADDR</key><string>127.0.0.1:8080</string>",
                "<key>LAO_EXTERNAL_ADDR</key><string>127.0.0.1:9090</string>",
                &format!(
                    "<key>LAO_VLLM_ROUTER_KEY_FILE</key><string>{}</string>",
                    vllm_key.display()
                ),
                &format!(
                    "<key>LAO_EXTERNAL_KEY_FILE</key><string>{}</string>",
                    external_key.display()
                ),
            ] {
                assert!(bytes.contains(expected));
            }
            assert!(!bytes.contains("LAO_MODEL_DIR"));
            assert!(!bytes.contains("LAO_LLAMA_SERVER"));
        }
    }

    #[test]
    fn invalid_selection_inputs_are_rejected() {
        for args in [
            vec!["preview", "--router"],
            vec!["install", "--router", "unknown"],
            vec!["install", "--runtime", "unknown"],
            vec!["preview", "--router", "safe", "--router", "safe"],
            vec!["off", "--runtime", "llama-cpp"],
        ] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
        assert!(
            parse(
                [
                    OsString::from("preview"),
                    OsString::from("--router"),
                    OsString::from_vec(vec![0xff]),
                ]
                .into_iter()
            )
            .is_err()
        );
        assert!(loopback("localhost:8080", "address").is_err());
        assert!(loopback("192.0.2.1:8080", "address").is_err());
        assert!(loopback("127.0.0.1:0", "address").is_err());

        let temp = Temp::new();
        let key = temp.0.join("key");
        fs::write(&key, b"secret").unwrap();
        fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(key_file(key.clone(), "key").is_err());
        fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(key_file(key.clone(), "key").unwrap(), key);
    }
}
