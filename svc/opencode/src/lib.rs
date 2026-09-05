use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    net::{IpAddr, SocketAddr},
    os::unix::{fs::DirBuilderExt, fs::OpenOptionsExt, fs::PermissionsExt, process::CommandExt},
    path::{Component, Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use lao_agent_api::{Agent, Outcome, Report, Task};
use serde_json::{Value, json};

pub const VERSION: &str = "1.18.25";
pub const LICENSE: &str = "MIT";
pub const DOWNLOAD_BYTES: u64 = 46_216_338;
pub const DOWNLOAD_URL: &str =
    "https://github.com/anomalyco/opencode/releases/download/v1.18.25/opencode-darwin-arm64.zip";
const DOWNLOAD_SHA256: &str = "606b09722d98069605e16037fb8c3c7c8ebbfed9ba713079a5efb2e5b065ae27";
const DIRECTORY: &str = "opencode-v1.18.25";
const OUTPUT_LIMIT: usize = 1024 * 1024;
const MAX_INSTRUCTION: usize = 4096;
const MAX_ALLOWED: usize = 16;
const MAX_DEADLINE: Duration = Duration::from_secs(10 * 60);
const CONFIG_BYTES: u64 = 80 * 1024 * 1024;
const CONFIG_ENTRIES: usize = 5000;
const CONFIG_EXPECTED_ENTRIES: usize = 3928;
const CONFIG_LOCK_SHA256: &str = "2c3f305186cf43d2ea8ac87aac5812ad735ba08e35db68126cd1ae166c5d5721";
const CONFIG_TREE_SHA256: &str = "c6191fcdbe6a82857ed7111d3d595f7538a1e4f99c861852f76e68131549a5c5";
const SUPERVISOR: &str = r#"parent=$1
shift
"$@" &
worker=$!
(
  while /bin/kill -0 "$parent" 2>/dev/null; do /bin/sleep 1; done
  /bin/kill -TERM -- "-$$" 2>/dev/null
) &
watcher=$!
wait "$worker"
status=$?
/bin/kill "$watcher" 2>/dev/null
wait "$watcher" 2>/dev/null
exit "$status""#;

pub struct OpenCode {
    bin: PathBuf,
    config: PathBuf,
    addr: SocketAddr,
    bearer: Box<str>,
    lock: Mutex<()>,
}

impl OpenCode {
    pub fn new(bin: PathBuf, addr: SocketAddr, bearer: impl Into<Box<str>>) -> io::Result<Self> {
        let bearer = bearer.into();
        if !matches!(addr, SocketAddr::V4(addr) if addr.ip().is_loopback())
            || bearer.is_empty()
            || bearer.len() > 4096
            || !bearer.bytes().all(|byte| byte > b' ' && byte < 0x7f)
        {
            return Err(invalid("endpoint"));
        }
        verify_bin(&bin)?;
        let config = bin
            .parent()
            .ok_or_else(|| invalid("OpenCode config"))?
            .join("config");
        verify_config(&config)?;
        Ok(Self {
            bin,
            config,
            addr,
            bearer,
            lock: Mutex::new(()),
        })
    }
}

impl Agent for OpenCode {
    fn turn(&self, task: &Task) -> io::Result<Report> {
        let _turn = self.lock.lock().map_err(|_| invalid("agent state"))?;
        let valid = ValidTask::new(task)?;
        let state = Temp::new("opencode-turn")?;
        let before = fingerprints(&valid.root, &valid.allowed)?;
        let config = config(&valid.allowed, self.addr, &self.bearer)?;
        let auth = r#"{"lao":{"type":"api","key":"local"}}"#;
        let prompt = prompt(&task.instruction, &valid.allowed);
        let deadline = Instant::now()
            .checked_add(task.deadline)
            .ok_or_else(|| invalid("deadline"))?;

        let mut command = Command::new(&self.bin);
        command
            .args([
                "run",
                "--format",
                "json",
                "--model",
                "lao/lao-local",
                "--agent",
                "lao",
            ])
            .arg("--dir")
            .arg(&valid.root)
            .current_dir(&valid.root);
        command.arg(prompt);
        isolated(&mut command, &state, &self.config, &config, auth);
        let result = run(&mut command, deadline)?;

        if result.timed_out {
            return report(Outcome::TimedOut, &valid, &before);
        }
        if !result.status.success() || result.truncated || output(&result.stdout).is_none() {
            return report(Outcome::AgentFailed, &valid, &before);
        }
        let mut report = report(Outcome::Complete, &valid, &before)?;
        if report.changed.is_empty() {
            report.outcome = Outcome::AgentFailed;
        }
        Ok(report)
    }
}

pub fn prepare(root: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let bin = binary(root);
    if bin.exists() {
        verify_bin(&bin)?;
        prepare_config(&bin)?;
        return Ok(bin);
    }

    let archive = root.join(format!(".{DIRECTORY}.zip.part"));
    let directory = root.join(format!(".{DIRECTORY}.part"));
    let mut pending = Pending {
        archive: Some(archive.clone()),
        directory: Some(directory.clone()),
    };
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&archive)?;
    let status = Command::new("/usr/bin/curl")
        .args(["--fail", "--location", "--silent", "--show-error"])
        .args(["--proto", "=https", "--proto-redir", "=https"])
        .args(["--max-filesize", &DOWNLOAD_BYTES.to_string()])
        .args(["--connect-timeout", "30", "--max-time", "300"])
        .arg("--output")
        .arg(&archive)
        .arg(DOWNLOAD_URL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(invalid("OpenCode download"));
    }
    verify_archive(&archive)?;
    fs::create_dir(&directory)?;
    let status = Command::new("/usr/bin/unzip")
        .args(["-q"])
        .arg(&archive)
        .arg("opencode")
        .arg("-d")
        .arg(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(invalid("OpenCode archive"));
    }
    let extracted = directory.join("opencode");
    fs::set_permissions(&extracted, fs::Permissions::from_mode(0o700))?;
    verify_bin(&extracted)?;
    fs::rename(&directory, root.join(DIRECTORY))?;
    pending.directory = None;
    fs::remove_file(&archive)?;
    pending.archive = None;
    prepare_config(&bin)?;
    Ok(bin)
}

pub fn binary(root: &Path) -> PathBuf {
    root.join(DIRECTORY).join("opencode")
}

fn verify_archive(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() || meta.len() != DOWNLOAD_BYTES {
        return Err(invalid("OpenCode archive"));
    }
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()?;
    let hash = String::from_utf8(output.stdout).map_err(|_| invalid("OpenCode archive"))?;
    if !output.status.success() || hash.split_whitespace().next() != Some(DOWNLOAD_SHA256) {
        return Err(invalid("OpenCode archive"));
    }
    Ok(())
}

fn verify_bin(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() {
        return Err(invalid("OpenCode binary"));
    }
    let output = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .output()?;
    let observed = [output.stdout, output.stderr].concat();
    if !output.status.success() || String::from_utf8_lossy(&observed).trim() != VERSION {
        return Err(invalid("OpenCode version"));
    }
    Ok(())
}

fn prepare_config(bin: &Path) -> io::Result<()> {
    let config = bin
        .parent()
        .ok_or_else(|| invalid("OpenCode config"))?
        .join("config");
    if verify_config(&config).is_ok() {
        return Ok(());
    }
    match fs::symlink_metadata(&config) {
        Ok(meta) if meta.file_type().is_dir() => fs::remove_dir_all(&config)?,
        Ok(_) => return Err(invalid("OpenCode config")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir(&config)?;
    fs::set_permissions(&config, fs::Permissions::from_mode(0o700))?;
    let state = Temp::new("opencode-prepare")?;
    let mut source = Command::new(bin);
    source.args([
        "run",
        "--format",
        "json",
        "--model",
        "lao/lao-local",
        "prepare local worker",
    ]);
    isolated(
        &mut source,
        &state,
        &config,
        r#"{"plugin":[],"enabled_providers":["lao"],"model":"lao/lao-local","provider":{"lao":{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"http://127.0.0.1:1/v1"},"models":{"lao-local":{"name":"LAO local","tool_call":true,"limit":{"context":16384,"output":4096}}}}}}"#,
        r#"{"lao":{"type":"api","key":"local"}}"#,
    );
    let mut command = supervised(&source);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = Guard::new(command.spawn()?);
    let deadline = Instant::now() + Duration::from_secs(5 * 60);
    loop {
        let entries = config_bounds(&config)?;
        if entries == CONFIG_EXPECTED_ENTRIES
            && config.join("opencode/package-lock.json").is_file()
            && verify_config(&config).is_ok()
        {
            child.stop();
            let _ = child.child.as_mut().expect("child present").wait();
            child.child = None;
            return Ok(());
        }
        if child
            .child
            .as_mut()
            .expect("child present")
            .try_wait()?
            .is_some()
            || Instant::now() >= deadline
        {
            return Err(invalid("OpenCode config"));
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn verify_config(root: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(root)?;
    if !meta.file_type().is_dir() || meta.permissions().mode() & 0o077 != 0 {
        return Err(invalid("OpenCode config"));
    }
    let lock = sha256_file(&root.join("opencode/package-lock.json"))?;
    let tree = tree_sha256(root)?;
    if lock != CONFIG_LOCK_SHA256 || tree != CONFIG_TREE_SHA256 {
        return Err(invalid("OpenCode config"));
    }
    Ok(())
}

fn config_bounds(root: &Path) -> io::Result<usize> {
    Tree::read(root).map(|tree| tree.entries)
}

fn tree_sha256(root: &Path) -> io::Result<String> {
    let mut tree = Tree::read(root)?;
    tree.files.sort();
    tree.links.sort();
    tree.dirs.sort();
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .args(&tree.files)
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(invalid("OpenCode config"));
    }
    let mut manifest = output.stdout;
    for line in tree.links.into_iter().chain(tree.dirs) {
        manifest.extend_from_slice(line.as_bytes());
    }
    sha256_bytes(&manifest)
}

struct Tree {
    files: Vec<String>,
    links: Vec<String>,
    dirs: Vec<String>,
    entries: usize,
    bytes: u64,
}

impl Tree {
    fn read(root: &Path) -> io::Result<Self> {
        let mut tree = Self {
            files: Vec::new(),
            links: Vec::new(),
            dirs: vec!["D .\n".to_owned()],
            entries: 1,
            bytes: 0,
        };
        tree.collect(root, root)?;
        Ok(tree)
    }

    fn collect(&mut self, root: &Path, directory: &Path) -> io::Result<()> {
        let mut children = fs::read_dir(directory)?.collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(fs::DirEntry::file_name);
        for child in children {
            self.entries += 1;
            if self.entries > CONFIG_ENTRIES {
                return Err(invalid("OpenCode config"));
            }
            let path = child.path();
            let relative = path
                .strip_prefix(root)
                .ok()
                .and_then(Path::to_str)
                .filter(|path| !path.contains(['\n', '\r', '\0']))
                .ok_or_else(|| invalid("OpenCode config"))?;
            let name = format!("./{relative}");
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_dir() {
                self.dirs.push(format!("D {name}\n"));
                self.collect(root, &path)?;
            } else if meta.file_type().is_file() {
                self.bytes = self
                    .bytes
                    .checked_add(meta.len())
                    .filter(|bytes| *bytes <= CONFIG_BYTES)
                    .ok_or_else(|| invalid("OpenCode config"))?;
                self.files.push(name);
            } else if meta.file_type().is_symlink() {
                let target = fs::read_link(path)?;
                let target = target
                    .to_str()
                    .filter(|target| !target.contains(['\n', '\r', '\0']))
                    .ok_or_else(|| invalid("OpenCode config"))?;
                self.links.push(format!("L {name} {target}\n"));
            } else {
                return Err(invalid("OpenCode config"));
            }
        }
        Ok(())
    }
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(invalid("OpenCode config"));
    }
    parse_sha256(&output.stdout)
}

fn sha256_bytes(bytes: &[u8]) -> io::Result<String> {
    let mut child = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| invalid("OpenCode config"))?
        .write_all(bytes)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(invalid("OpenCode config"));
    }
    parse_sha256(&output.stdout)
}

fn parse_sha256(output: &[u8]) -> io::Result<String> {
    let hash = std::str::from_utf8(output)
        .ok()
        .and_then(|output| output.split_whitespace().next())
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| invalid("OpenCode config"))?;
    Ok(hash.to_owned())
}

struct Pending {
    archive: Option<PathBuf>,
    directory: Option<PathBuf>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(path) = &self.archive {
            let _ = fs::remove_file(path);
        }
        if let Some(path) = &self.directory {
            let _ = fs::remove_dir_all(path);
        }
    }
}

struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> io::Result<Self> {
        for _ in 0..8 {
            let path = std::env::temp_dir().join(format!("lao-{name}-{}", hex::<8>()?));
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ValidTask {
    root: PathBuf,
    allowed: Vec<PathBuf>,
}

impl ValidTask {
    fn new(task: &Task) -> io::Result<Self> {
        if task.instruction.is_empty()
            || task.instruction.len() > MAX_INSTRUCTION
            || task.instruction.contains('\0')
            || task.allowed.is_empty()
            || task.allowed.len() > MAX_ALLOWED
            || task.deadline.is_zero()
            || task.deadline > MAX_DEADLINE
        {
            return Err(invalid("task"));
        }
        let root = fs::canonicalize(&task.root)?;
        if !fs::metadata(&root)?.is_dir() {
            return Err(invalid("root"));
        }
        let mut seen = HashSet::new();
        let mut allowed = Vec::with_capacity(task.allowed.len());
        for path in &task.allowed {
            let path = clean_relative(path)?;
            if path.components().any(|component| {
                matches!(component, Component::Normal(name) if name.to_str().is_some_and(|name| name.eq_ignore_ascii_case(".git")))
            }) || !seen.insert(path.clone())
            {
                return Err(invalid("allowed path"));
            }
            contained(&root, &path)?;
            allowed.push(path);
        }
        Ok(Self { root, allowed })
    }
}

fn clean_relative(path: &Path) -> io::Result<PathBuf> {
    let text = path.to_str().ok_or_else(|| invalid("allowed path"))?;
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.as_os_str().len() > 4096
        || text.contains(['*', '?', '\\'])
    {
        return Err(invalid("allowed path"));
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) if value != OsStr::new("") => clean.push(value),
            _ => return Err(invalid("allowed path")),
        }
    }
    Ok(clean)
}

fn contained(root: &Path, relative: &Path) -> io::Result<()> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(invalid("allowed path"));
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta)
                if meta.file_type().is_symlink()
                    || meta.is_dir() && current == root.join(relative) =>
            {
                return Err(invalid("allowed path"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(error),
        }
    }
    let mut ancestor = root.join(relative);
    while !ancestor.exists() {
        if !ancestor.pop() {
            return Err(invalid("allowed path"));
        }
    }
    if !fs::canonicalize(ancestor)?.starts_with(root) {
        return Err(invalid("allowed path"));
    }
    Ok(())
}

fn config(allowed: &[PathBuf], addr: SocketAddr, bearer: &str) -> io::Result<String> {
    let IpAddr::V4(ip) = addr.ip() else {
        return Err(invalid("endpoint"));
    };
    if !ip.is_loopback() {
        return Err(invalid("endpoint"));
    }
    let mut files = serde_json::Map::new();
    files.insert("*".into(), Value::String("deny".into()));
    for path in allowed {
        files.insert(
            path.to_string_lossy().into_owned(),
            Value::String("allow".into()),
        );
    }
    serde_json::to_string(&json!({
        "autoupdate": false,
        "enabled_providers": ["lao"],
        "model": "lao/lao-local",
        "small_model": "lao/lao-local",
        "share": "disabled",
        "snapshot": false,
        "plugin": [],
        "mcp": {},
        "formatter": false,
        "lsp": false,
        "instructions": [],
        "agent": {
            "lao": {
                "description": "One bounded local implementation turn",
                "mode": "primary",
                "model": "lao/lao-local",
                "temperature": 0,
                "prompt": "You implement one small requested change. You MUST call read on the named file, then call edit to make the exact change. Emit each call only as <tool_call> followed by its JSON object and </tool_call>. Never put a tool call in a Markdown code fence and never only describe the edit. Stop immediately after edit succeeds."
            }
        },
        "permission": {
            "*": "deny",
            "read": Value::Object(files.clone()),
            "edit": Value::Object(files),
            "external_directory": "deny"
        },
        "provider": {
            "lao": {
                "npm": "@ai-sdk/openai-compatible",
                "name": "LAO local",
                "options": {
                    "baseURL": format!("http://{addr}/wrk/v1"),
                    "headers": { "X-LAO-Key": bearer },
                    "includeUsage": false
                },
                "models": {
                    "lao-local": {
                        "name": "LAO local",
                        "tool_call": true,
                        "limit": { "context": 16384, "output": 4096 }
                    }
                }
            }
        }
    }))
    .map_err(|_| invalid("config"))
}

fn isolated(command: &mut Command, temp: &Temp, config_root: &Path, config: &str, auth: &str) {
    let home = temp.path().join("home");
    let data = temp.path().join("data");
    let cache = temp.path().join("cache");
    let state = temp.path().join("state");
    let config_dir = config_root.join("opencode");
    command
        .env_clear()
        .env("HOME", &home)
        .env("TMPDIR", temp.path())
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", &cache)
        .env("XDG_STATE_HOME", &state)
        .env("XDG_CONFIG_HOME", config_root)
        .env("OPENCODE_CONFIG_DIR", &config_dir)
        .env("OPENCODE_CONFIG_CONTENT", config)
        .env("OPENCODE_AUTH_CONTENT", auth)
        .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
        .env("OPENCODE_DISABLE_MODELS_FETCH", "1")
        .env("OPENCODE_DISABLE_PROJECT_CONFIG", "1")
        .env("OPENCODE_DISABLE_CLAUDE_CODE", "1")
        .env("OPENCODE_DISABLE_DEFAULT_PLUGINS", "1")
        .env("OPENCODE_DISABLE_EMBEDDED_WEB_UI", "1")
        .env("OPENCODE_DISABLE_EXTERNAL_SKILLS", "1")
        .env("OPENCODE_DISABLE_LSP_DOWNLOAD", "1")
        .env("OPENCODE_PURE", "1")
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LANG", "C.UTF-8");
}

fn prompt(instruction: &str, allowed: &[PathBuf]) -> String {
    let files = allowed
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Complete exactly one bounded implementation turn. Do not run commands or use network access. Read and edit only these exact files:\n{files}\nThe parent coding harness will review and verify the result.\n\nTask:\n{instruction}"
    )
}

struct Run {
    status: ExitStatus,
    stdout: Vec<u8>,
    truncated: bool,
    timed_out: bool,
}

fn run(command: &mut Command, deadline: Instant) -> io::Result<Run> {
    let mut command = supervised(command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = Guard::new(command.spawn()?);
    let stdout = child
        .child
        .as_mut()
        .and_then(|child| child.stdout.take())
        .map(|stream| thread::spawn(move || bounded(stream, OUTPUT_LIMIT)));
    let stderr = child
        .child
        .as_mut()
        .and_then(|child| child.stderr.take())
        .map(|stream| thread::spawn(move || drain(stream)));

    let (status, timed_out) = loop {
        if let Some(status) = child.child.as_mut().expect("child present").try_wait()? {
            break (status, false);
        }
        if Instant::now() >= deadline {
            child.stop();
            let status = child.child.as_mut().expect("child present").wait()?;
            break (status, true);
        }
        thread::sleep(Duration::from_millis(25));
    };
    child.stop_group();
    child.child = None;
    let (stdout, truncated) = match stdout {
        Some(reader) => reader.join().map_err(|_| invalid("OpenCode output"))??,
        None => (Vec::new(), false),
    };
    if let Some(reader) = stderr {
        reader
            .join()
            .map_err(|_| invalid("OpenCode error output"))??;
    }
    Ok(Run {
        status,
        stdout,
        truncated,
        timed_out,
    })
}

struct Guard {
    child: Option<Child>,
    pid: u32,
}

impl Guard {
    fn new(child: Child) -> Self {
        let pid = child.id();
        Self {
            child: Some(child),
            pid,
        }
    }

    fn stop_group(&self) {
        let _ = Command::new("/bin/kill")
            .args(["-TERM", "--"])
            .arg(format!("-{}", self.pid))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    fn stop(&mut self) {
        if self.child.is_none() {
            return;
        }
        self.stop_group();
        if let Some(child) = &mut self.child {
            let _ = child.kill();
        }
    }
}

fn supervised(source: &Command) -> Command {
    supervised_for(source, std::process::id())
}

fn supervised_for(source: &Command, parent: u32) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(SUPERVISOR)
        .arg("lao-opencode-supervisor")
        .arg(parent.to_string())
        .arg(source.get_program())
        .args(source.get_args())
        .env_clear();
    if let Some(path) = source.get_current_dir() {
        command.current_dir(path);
    }
    for (key, value) in source.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    command
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.stop();
        if let Some(child) = &mut self.child {
            let _ = child.wait();
        }
    }
}

fn bounded(mut reader: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut kept = Vec::new();
    let mut buf = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }
        let available = limit.saturating_sub(kept.len());
        kept.extend_from_slice(&buf[..read.min(available)]);
        truncated |= read > available;
    }
    Ok((kept, truncated))
}

fn drain(mut reader: impl Read) -> io::Result<()> {
    io::copy(&mut reader, &mut io::sink())?;
    Ok(())
}

fn output(bytes: &[u8]) -> Option<()> {
    let mut values = 0;
    for line in std::str::from_utf8(bytes).ok()?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        serde_json::from_str::<Value>(line).ok()?;
        values += 1;
    }
    (values > 0).then_some(())
}

#[derive(Eq, PartialEq)]
enum Fingerprint {
    Missing,
    File(u64, u64),
}

fn fingerprints(root: &Path, allowed: &[PathBuf]) -> io::Result<HashMap<PathBuf, Fingerprint>> {
    allowed
        .iter()
        .map(|path| {
            contained(root, path)?;
            Ok((path.clone(), fingerprint(&root.join(path))?))
        })
        .collect()
}

fn fingerprint(path: &Path) -> io::Result<Fingerprint> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Fingerprint::Missing),
        Err(error) => return Err(error),
    };
    if !meta.file_type().is_file() {
        return Err(invalid("allowed file"));
    }
    let mut file = File::open(path)?;
    let mut hash = DefaultHasher::new();
    let mut buf = [0_u8; 8192];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        buf[..read].hash(&mut hash);
    }
    Ok(Fingerprint::File(meta.len(), hash.finish()))
}

fn report(
    outcome: Outcome,
    task: &ValidTask,
    before: &HashMap<PathBuf, Fingerprint>,
) -> io::Result<Report> {
    let after = fingerprints(&task.root, &task.allowed)?;
    let changed = task
        .allowed
        .iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect();
    Ok(Report { outcome, changed })
}

fn hex<const N: usize>() -> io::Result<String> {
    let mut bytes = [0_u8; N];
    getrandom::getrandom(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn invalid(part: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, part)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(Temp);

    impl Fixture {
        fn new(script: &str) -> Self {
            let temp = Temp::new("opencode-test").unwrap();
            fs::create_dir(temp.path().join("repo")).unwrap();
            fs::write(temp.path().join("repo/file.txt"), "unchanged\n").unwrap();
            fs::write(temp.path().join("opencode"), script).unwrap();
            fs::set_permissions(
                temp.path().join("opencode"),
                fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            Self(temp)
        }

        fn agent(&self) -> OpenCode {
            OpenCode {
                bin: self.0.path().join("opencode"),
                config: self.0.path().join("config"),
                addr: "127.0.0.1:9".parse().unwrap(),
                bearer: "secret".into(),
                lock: Mutex::new(()),
            }
        }

        fn task(&self) -> Task {
            Task {
                root: self.0.path().join("repo"),
                instruction: "Inspect the file and make no unnecessary change.".into(),
                allowed: vec!["file.txt".into()],
                deadline: Duration::from_secs(2),
            }
        }
    }

    #[test]
    fn each_packet_has_fresh_state_and_returns_only_report_metadata() {
        let fixture = Fixture::new(
            r#"#!/bin/sh
[ "$OPENCODE_DISABLE_PROJECT_CONFIG" = 1 ] || exit 2
[ "$OPENCODE_AUTH_CONTENT" = '{"lao":{"type":"api","key":"local"}}' ] || exit 3
for arg in "$@"; do [ "$arg" != --session ] || exit 4; done
case "$OPENCODE_CONFIG_CONTENT" in *'/wrk/v1'*'X-LAO-Key'*secret*) ;; *) exit 5;; esac
[ ! -e "$XDG_DATA_HOME/seen" ] || exit 6
mkdir -p "$XDG_DATA_HOME" && : > "$XDG_DATA_HOME/seen"
printf '%s\n' "$TMPDIR" > file.txt
printf '{"type":"step_finish"}\n'
"#,
        );
        let agent = fixture.agent();
        let task = fixture.task();
        let mut previous = String::new();
        for _ in 0..2 {
            let report = agent.turn(&task).unwrap();
            assert_eq!(report.outcome, Outcome::Complete);
            assert_eq!(report.changed, [PathBuf::from("file.txt")]);
            let state = fs::read_to_string(task.root.join("file.txt")).unwrap();
            assert_ne!(state, previous);
            assert!(!Path::new(state.trim()).exists());
            previous = state;
        }
        let config: Value = serde_json::from_str(
            &config(
                &[PathBuf::from("file.txt")],
                "127.0.0.1:9".parse().unwrap(),
                "secret",
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(config["permission"]["read"]["*"], "deny");
        assert_eq!(config["permission"]["read"]["file.txt"], "allow");
        assert_eq!(config["permission"]["bash"], Value::Null);
        assert_eq!(
            config["provider"]["lao"]["models"]["lao-local"]["tool_call"],
            true
        );
    }

    #[test]
    fn failed_agent_is_bounded_evidence() {
        let fixture = Fixture::new("#!/bin/sh\nprintf '%s\\n' \"$TMPDIR\" > file.txt\nexit 1\n");
        let task = fixture.task();
        let report = fixture.agent().turn(&task).unwrap();
        assert_eq!(report.outcome, Outcome::AgentFailed);
        assert_eq!(report.changed, [PathBuf::from("file.txt")]);
        let state = fs::read_to_string(task.root.join("file.txt")).unwrap();
        assert!(!Path::new(state.trim()).exists());
    }

    #[test]
    fn packets_require_exact_paths_inside_the_repository() {
        let fixture = Fixture::new("#!/bin/sh\nexit 0\n");
        let mut task = fixture.task();
        task.allowed = vec!["../outside".into()];
        assert_eq!(
            fixture.agent().turn(&task).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        task.allowed = vec![".git/config".into()];
        assert_eq!(
            fixture.agent().turn(&task).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        task.allowed = vec!["nested/.GIT/hooks/pre-commit".into()];
        assert_eq!(
            fixture.agent().turn(&task).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        for path in ["*", "file?.txt", "nested\\file.txt"] {
            task.allowed = vec![path.into()];
            assert_eq!(
                fixture.agent().turn(&task).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn supervisor_reaps_the_worker_when_its_parent_is_gone() {
        let temp = Temp::new("opencode-supervisor-test").unwrap();
        let pid_file = temp.path().join("child.pid");
        let mut source = Command::new("/bin/sh");
        source
            .args(["-c", "sleep 30 & echo $! > child.pid; wait"])
            .current_dir(temp.path());
        let mut parent = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let mut command = supervised_for(&source, parent.id());
        command.process_group(0);
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !pid_file.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let pid = fs::read_to_string(pid_file).unwrap();
        parent.kill().unwrap();
        parent.wait().unwrap();
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(child.try_wait().unwrap().is_some());
        assert!(
            !Command::new("/bin/kill")
                .args(["-0", pid.trim()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    }
}
