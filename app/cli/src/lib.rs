use serde::{Deserialize, Serialize};
use std::{
    env,
    error::Error,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
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
const PLIST_AFTER: &str = "launchd.after";
const RECORD: &str = "install.json";

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone)]
struct Paths {
    state: PathBuf,
    codex: PathBuf,
    claude: PathBuf,
    plist: PathBuf,
    adopted: PathBuf,
    model: PathBuf,
    daemon: PathBuf,
    llama: PathBuf,
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

impl Transaction {
    fn prepare(
        paths: &Paths,
        port: u16,
        codex_after: &[u8],
        claude_after: &[u8],
    ) -> io::Result<Self> {
        if paths.state.join(RECORD).exists() {
            return Err(conflict("lao is already installed or needs recovery"));
        }
        let (codex, codex_before) = inspect(&paths.codex)?;
        let (claude, claude_before) = inspect(&paths.claude)?;
        write_atomic(&paths.state.join(CODEX_BEFORE), &codex_before, 0o600)?;
        write_atomic(&paths.state.join(CODEX_AFTER), codex_after, 0o600)?;
        write_atomic(&paths.state.join(CLAUDE_BEFORE), &claude_before, 0o600)?;
        write_atomic(&paths.state.join(CLAUDE_AFTER), claude_after, 0o600)?;
        let record = Record {
            phase: Phase::Installing,
            port,
            codex,
            claude,
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
        if let Err(error) = write(0, &self.record.codex, &codex)
            .and_then(|_| write(1, &self.record.claude, &claude))
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
        validate(&self.record.codex, &self.state.join(CODEX_AFTER))?;
        validate(&self.record.claude, &self.state.join(CLAUDE_AFTER))
    }

    fn validate_originals(&self) -> io::Result<()> {
        validate_original(&self.record.codex, &self.state.join(CODEX_BEFORE))?;
        validate_original(&self.record.claude, &self.state.join(CLAUDE_BEFORE))
    }

    fn restore(&mut self) -> io::Result<()> {
        self.validate_installed()?;
        self.phase(Phase::Restoring)?;
        let codex_after = fs::read(self.state.join(CODEX_AFTER))?;
        let claude_after = fs::read(self.state.join(CLAUDE_AFTER))?;
        let codex_before = fs::read(self.state.join(CODEX_BEFORE))?;
        let claude_before = fs::read(self.state.join(CLAUDE_BEFORE))?;
        restore_entry(&self.record.codex, &codex_before)?;
        if let Err(error) = restore_entry(&self.record.claude, &claude_before) {
            let rollback = write_entry(&self.record.codex, &codex_after)
                .and_then(|_| write_entry(&self.record.claude, &claude_after));
            if rollback.is_ok() {
                self.phase(Phase::Installed)?;
                return Err(error);
            }
            return Err(io::Error::other(format!(
                "client restore failed and rollback failed: {}",
                rollback.unwrap_err()
            )));
        }
        self.phase(Phase::Restored)
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
        )
    }

    fn discard(&self) -> io::Result<()> {
        for name in [
            CODEX_BEFORE,
            CODEX_AFTER,
            CLAUDE_BEFORE,
            CLAUDE_AFTER,
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
    match env::args_os().nth(1).as_deref() {
        Some(command) if command == "preview" => preview(),
        Some(command) if command == "install" => install(),
        Some(command) if command == "off" => off(),
        _ => {
            println!("usage: lao <preview|install|off>");
            Ok(())
        }
    }
}

fn preview() -> Result<()> {
    let paths = paths()?;
    let budget = lao_run::plan(&paths.llama, lao_run::Mode::Light)?;
    let model = &lao_model::QWEN;
    println!("model: {}", model.id);
    println!("source: {} @ {}", model.url, model.revision);
    println!("download: {} bytes", model.bytes);
    println!("license: {}", model.license);
    println!("runtime: {}", model.runtime);
    println!("context: {}", model.context);
    println!(
        "Light: {:.2} GiB, {} threads",
        budget.bytes as f64 / (1_u64 << 30) as f64,
        budget.threads
    );
    println!("Codex settings: {}", paths.codex.display());
    println!("Claude settings: {}", paths.claude.display());
    println!("listener: launchd-owned IPv4 loopback port selected at install");
    println!("caller headers: X-LAO-Key: <redacted> (one per client)");
    Ok(())
}

#[cfg(target_os = "macos")]
fn install() -> Result<()> {
    let paths = paths()?;
    let _lock = Lock::acquire(&paths.state)?;
    if paths.state.join(RECORD).exists() {
        let transaction = Transaction::load(&paths.state)?;
        if transaction.record.phase == Phase::Installed {
            return Err(conflict("lao is already installed").into());
        }
        recover(&paths, &transaction)?;
    }
    if service_loaded()? || paths.plist.exists() {
        return Err(conflict("conflicting launchd service").into());
    }

    let codex_cloud = preflight_clients()?;
    lao_model::prepare(&paths.model)?;
    lao_run::plan(&paths.llama, lao_run::Mode::Light)?;
    if !paths.daemon.is_file() {
        return Err(invalid("lao-daemon binary").into());
    }

    let codex_original = read_optional(&paths.codex)?;
    let claude_original = read_optional(&paths.claude)?;
    let port = free_port()?;
    let codex_caller = caller()?;
    let claude_caller = caller()?;
    let codex_after = lao_codex::configure(codex_original.as_deref(), port, &codex_caller)?;
    let claude_after = lao_claude::configure(claude_original.as_deref(), port, &claude_caller)?;

    let mut transaction = Transaction::prepare(&paths, port, &codex_after, &claude_after)?;
    let plist = plist(&paths, port, &codex_caller, &claude_caller, codex_cloud)?;
    write_atomic(&paths.state.join(PLIST_AFTER), plist.as_bytes(), 0o600)?;
    let result = (|| -> Result<()> {
        remove_optional(&paths.adopted)?;
        write_atomic(&paths.plist, plist.as_bytes(), 0o600)?;
        bootstrap(&paths)?;
        verify_ready(&paths, port)?;
        transaction.apply()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = transaction.restore_changed();
        let _ = deactivate(&paths);
        let _ = transaction.discard();
        return Err(error);
    }
    println!("installed: Codex and Claude now use the launchd-owned LAO gate");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn install() -> Result<()> {
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
            transaction.restore_changed()?;
            transaction.phase(Phase::Restored)?;
        }
        Phase::Restored => {}
    }
    deactivate(&paths)?;
    transaction.discard()?;
    remove_optional(&paths.adopted)?;
    println!("off: original Codex and Claude settings restored exactly");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn off() -> Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "macOS only").into())
}

fn preflight_clients() -> Result<&'static str> {
    let keys: Vec<_> = env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .collect();
    if !lao_codex::conflicts(keys.iter().map(String::as_str), false).is_empty() {
        return Err(conflict("conflicting Codex environment configuration").into());
    }
    if !lao_claude::conflicts(keys.iter().map(String::as_str), false, false).is_empty() {
        return Err(conflict("conflicting Claude environment configuration").into());
    }

    let codex = command("codex", &["--version"])?;
    if lao_codex::support(&codex) != lao_codex::Support::Observed {
        return Err(conflict("unsupported Codex version").into());
    }
    let auth = command("codex", &["login", "status"])?;
    let cloud = match lao_codex::auth(&auth) {
        lao_codex::Auth::ChatGpt => "chatgpt",
        lao_codex::Auth::ApiKey => "openai",
        _ => return Err(conflict("unsupported Codex authentication").into()),
    };
    let claude = command("claude", &["--version"])?;
    if lao_claude::support(&claude) != lao_claude::Support::Observed {
        return Err(conflict("unsupported Claude Code version").into());
    }
    Ok(cloud)
}

fn command(bin: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(bin).args(args).stdin(Stdio::null()).output()?;
    if !output.status.success() {
        return Err(invalid("client preflight"));
    }
    let mut bytes = output.stdout;
    bytes.extend_from_slice(&output.stderr);
    String::from_utf8(bytes).map_err(|_| invalid("client preflight"))
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
    let daemon = env::current_exe()?
        .parent()
        .ok_or_else(|| invalid("lao binary"))?
        .join("lao-daemon");
    Ok(Paths {
        state: state.clone(),
        codex: codex_root.join("config.toml"),
        claude: claude_root.join("settings.json"),
        plist: home
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")),
        adopted: state.join("adopted"),
        model: home.join("Library/Caches/lao/models"),
        daemon,
        llama: env::var_os("LAO_LLAMA_SERVER")
            .map(PathBuf::from)
            .unwrap_or_else(|| "/opt/homebrew/bin/llama-server".into()),
    })
}

fn recover(paths: &Paths, transaction: &Transaction) -> io::Result<()> {
    transaction.restore_changed()?;
    deactivate(paths)?;
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
    port: u16,
    codex: &str,
    claude: &str,
    codex_cloud: &str,
) -> io::Result<String> {
    let error_path = paths.state.join("daemon.err");
    let values = [
        paths.daemon.to_str(),
        paths.adopted.to_str(),
        paths.model.to_str(),
        paths.llama.to_str(),
        error_path.to_str(),
    ];
    if values.iter().any(Option::is_none) {
        return Err(invalid("non-UTF-8 install path"));
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>{LABEL}</string>\n<key>ProgramArguments</key><array><string>{daemon}</string></array>\n<key>EnvironmentVariables</key><dict>\n<key>LAO_ADOPTED_FILE</key><string>{adopted}</string>\n<key>LAO_LOCAL_CANARY</key><string>1</string>\n<key>LAO_CODEX_CALLER</key><string>{codex}</string>\n<key>LAO_CLAUDE_CALLER</key><string>{claude}</string>\n<key>LAO_CODEX_CLOUD</key><string>{codex_cloud}</string>\n<key>LAO_MODEL_DIR</key><string>{model}</string>\n<key>LAO_LLAMA_SERVER</key><string>{llama}</string>\n</dict>\n<key>RunAtLoad</key><true/>\n<key>ThrottleInterval</key><integer>1</integer>\n<key>Sockets</key><dict><key>gate</key><dict><key>SockNodeName</key><string>127.0.0.1</string><key>SockServiceName</key><integer>{port}</integer><key>SockFamily</key><string>IPv4</string><key>SockType</key><string>stream</string><key>SockProtocol</key><string>TCP</string><key>SockPassive</key><true/></dict></dict>\n<key>StandardErrorPath</key><string>{error}</string>\n</dict></plist>\n",
        daemon = xml(values[0].unwrap()),
        adopted = xml(values[1].unwrap()),
        model = xml(values[2].unwrap()),
        llama = xml(values[3].unwrap()),
        error = xml(values[4].unwrap()),
    ))
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
    }
    remove_optional(&paths.plist)
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
        if adoption(paths, port).is_ok() && hello(port).is_ok() {
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
                plist: self.0.join("daemon.plist"),
                adopted: state.join("adopted"),
                model: self.0.join("models"),
                daemon: self.0.join("lao-daemon"),
                llama: self.0.join("llama-server"),
            }
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn prepared(temp: &Temp) -> (Paths, Transaction, Vec<u8>, Vec<u8>) {
        let paths = temp.paths();
        fs::create_dir_all(paths.codex.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.claude.parent().unwrap()).unwrap();
        let codex = b"model = \"gpt-5.4\"\n".to_vec();
        let claude = br#"{"permissions":{"defaultMode":"default"}}"#.to_vec();
        fs::write(&paths.codex, &codex).unwrap();
        fs::write(&paths.claude, &claude).unwrap();
        fs::set_permissions(&paths.codex, fs::Permissions::from_mode(0o640)).unwrap();
        fs::set_permissions(&paths.claude, fs::Permissions::from_mode(0o600)).unwrap();
        private_dir(&paths.state).unwrap();
        let codex_after = lao_codex::configure(
            Some(&codex),
            8765,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        let claude_after = lao_claude::configure(
            Some(&claude),
            8765,
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
        )
        .unwrap();
        let transaction = Transaction::prepare(&paths, 8765, &codex_after, &claude_after).unwrap();
        (paths, transaction, codex, claude)
    }

    #[test]
    fn settings_install_and_off_are_exact_and_conflict_aware() {
        let temp = Temp::new();
        let (paths, mut transaction, codex, claude) = prepared(&temp);
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
        transaction.restore().unwrap();
        assert_eq!(fs::read(&paths.codex).unwrap(), codex);
        assert_eq!(fs::read(&paths.claude).unwrap(), claude);
    }

    #[test]
    fn failure_at_either_client_write_restores_both_originals() {
        for boundary in 0..=1 {
            let temp = Temp::new();
            let (paths, mut transaction, codex, claude) = prepared(&temp);
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
        }
    }
}
