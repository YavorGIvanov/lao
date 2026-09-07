#![cfg(target_os = "macos")]

use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::{fs::DirBuilderExt, fs::MetadataExt, fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const LIMIT: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(10 * 60 + 5);
const MANIFEST: &str = include_str!("../../../docs/benchmarks/tasks.json");
const PROJECTS: &[(&str, &str)] = &[
    (
        "web",
        include_str!("../../../docs/benchmarks/fixtures/web/package.json"),
    ),
    (
        "service",
        include_str!("../../../docs/benchmarks/fixtures/service/settings.json"),
    ),
    (
        "catalog",
        include_str!("../../../docs/benchmarks/fixtures/catalog/products.json"),
    ),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Task {
    id: String,
    project: String,
    file: String,
    objective: String,
    pointer: String,
    value: Value,
}

impl Task {
    fn source(&self) -> &'static str {
        PROJECTS
            .iter()
            .find(|(name, _)| *name == self.project)
            .unwrap()
            .1
    }

    fn expected(&self) -> Value {
        let mut expected: Value = serde_json::from_str(self.source()).unwrap();
        let value = expected.pointer_mut(&self.pointer).unwrap();
        assert_ne!(*value, self.value, "fixture must require an edit");
        *value = self.value.clone();
        expected
    }
}

struct Repo(PathBuf);

impl Repo {
    fn new(task: &Task) -> Self {
        let mut nonce = [0u8; 8];
        getrandom::getrandom(&mut nonce).unwrap();
        let root =
            std::env::temp_dir().join(format!("lao-task-{:016x}", u64::from_ne_bytes(nonce)));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let repo = Self(root);
        assert!(
            Command::new("/usr/bin/git")
                .env_clear()
                .args(["-c", "init.templateDir=", "init", "-q"])
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .current_dir(&repo.0)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success(),
            "fixture Git setup failed"
        );
        fs::write(repo.0.join(&task.file), task.source()).unwrap();
        fs::write(
            repo.0.join("README.md"),
            "Public LAO fixture. This file must remain unchanged.\n",
        )
        .unwrap();
        repo
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(PartialEq, Eq)]
struct Entry {
    mode: u32,
    bytes: Vec<u8>,
}

type Tree = BTreeMap<PathBuf, Entry>;

fn tree(root: &Path) -> Option<Tree> {
    let mut result = Tree::new();
    let mut pending = vec![PathBuf::new()];
    let mut bytes = 0;
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(root.join(dir)).ok()? {
            let entry = entry.ok()?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).ok()?;
            if !meta.is_file() && !meta.is_dir()
                || meta.is_file() && meta.nlink() != 1
                || result.len() >= 256
            {
                return None;
            }
            let relative = path.strip_prefix(root).ok()?.to_path_buf();
            let mut data = Vec::new();
            if meta.is_dir() {
                pending.push(relative.clone());
            } else {
                fs::File::open(path)
                    .ok()?
                    .take((LIMIT + 1) as u64)
                    .read_to_end(&mut data)
                    .ok()?;
                bytes += data.len();
                if bytes > LIMIT {
                    return None;
                }
            }
            result.insert(
                relative,
                Entry {
                    mode: meta.permissions().mode(),
                    bytes: data,
                },
            );
        }
    }
    Some(result)
}

fn verdict(root: &Path, task: &Task, before: &Tree, expected: &Value) -> (bool, bool) {
    let Some(after) = tree(root) else {
        return (false, false);
    };
    let verified = after
        .get(Path::new(&task.file))
        .and_then(|entry| serde_json::from_slice::<Value>(&entry.bytes).ok())
        .is_some_and(|actual| actual == *expected);
    let scope_ok = before.len() == after.len()
        && before.iter().all(|(path, old)| {
            after.get(path).is_some_and(|new| {
                old.mode == new.mode && (path == Path::new(&task.file) || old.bytes == new.bytes)
            })
        });
    (verified, scope_ok)
}

struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        // Kill the isolated test process group; never touch the installed daemon/runtime.
        if self.0.try_wait().ok().flatten().is_some() {
            return;
        }
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", self.0.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.0.wait();
    }
}

fn packet(root: &Path, task: &Task, objective: &str) -> Option<&'static str> {
    let mut child = Process(
        Command::new(env!("CARGO_BIN_EXE_lao"))
            .arg("mcp")
            .current_dir(root)
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?,
    );
    let stdout = child.0.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).ok()?;
        (bytes.len() <= 65536).then_some(bytes)
    });
    let request = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "execute", "arguments": {
            "objective": objective, "allowed_paths": [&task.file]
        }}
    });
    let mut stdin = child.0.stdin.take()?;
    let initialize = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "lao-public-tasks", "version": "1"}}});
    writeln!(stdin, "{initialize}\n{request}").ok()?;
    drop(stdin);
    let deadline = Instant::now() + DEADLINE;
    let status = loop {
        if let Some(status) = child.0.try_wait().ok()? {
            break status;
        }
        if Instant::now() >= deadline {
            drop(child);
            let _ = reader.join();
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    drop(child);
    let bytes = reader.join().ok()??;
    if !status.success() {
        return None;
    }
    let responses = serde_json::Deserializer::from_slice(&bytes)
        .into_iter::<Value>()
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if responses.len() != 2 || responses[0]["id"] != 1 || responses[1]["id"] != 2 {
        return None;
    }
    match responses[1]
        .pointer("/result/structuredContent/status")?
        .as_str()?
    {
        "cloud" => Some("cloud"),
        "complete" => Some("complete"),
        "agent_failed" => Some("agent_failed"),
        "timed_out" => Some("timed_out"),
        _ => None,
    }
}

fn tasks() -> Vec<Task> {
    serde_json::from_str(MANIFEST).unwrap()
}

#[test]
fn public_tasks_fail_before_and_pass_independent_reference_edits() {
    let tasks = tasks();
    assert_eq!(tasks.len(), 6);
    let mut ids = std::collections::BTreeSet::new();
    for task in tasks {
        assert!(ids.insert(task.id.clone()));
        assert!(!task.file.contains(['/', '\\']));
        let repo = Repo::new(&task);
        let before = tree(&repo.0).unwrap();
        let expected = task.expected();
        assert_eq!(verdict(&repo.0, &task, &before, &expected), (false, true));
        fs::write(
            repo.0.join(&task.file),
            serde_json::to_vec_pretty(&expected).unwrap(),
        )
        .unwrap();
        assert_eq!(verdict(&repo.0, &task, &before, &expected), (true, true));
    }
}

#[test]
fn correct_content_cannot_hide_unlisted_changes_or_link_replacement() {
    let task = tasks().remove(0);
    let repo = Repo::new(&task);
    let before = tree(&repo.0).unwrap();
    let expected = task.expected();
    fs::write(
        repo.0.join(&task.file),
        serde_json::to_vec(&expected).unwrap(),
    )
    .unwrap();
    fs::write(repo.0.join("extra.txt"), "unrequested").unwrap();
    assert_eq!(verdict(&repo.0, &task, &before, &expected), (true, false));
    fs::remove_file(repo.0.join("extra.txt")).unwrap();
    fs::remove_file(repo.0.join(&task.file)).unwrap();
    std::os::unix::fs::symlink("README.md", repo.0.join(&task.file)).unwrap();
    assert_eq!(verdict(&repo.0, &task, &before, &expected), (false, false));
}

#[test]
#[ignore = "local diagnostic: active default LAO install; six serial packets, no cloud generation"]
fn installed_public_tasks_report_every_outcome_without_claiming_savings() {
    let tasks = tasks();
    let mut rows = Vec::new();
    for task in &tasks {
        let repo = Repo::new(task);
        let before = tree(&repo.0).unwrap();
        let expected = task.expected();
        let start = Instant::now();
        let status = packet(&repo.0, task, &task.objective).unwrap_or("infrastructure_invalid");
        let (verified, scope_ok) = verdict(&repo.0, task, &before, &expected);
        let route = match status {
            "cloud" => "cloud",
            "infrastructure_invalid" => "unknown",
            _ => "local",
        };
        let row = json!({"task": task.id, "route": route, "status": status, "verified": verified,
            "scope_ok": scope_ok, "elapsed_ms": start.elapsed().as_millis()});
        println!("{row}");
        rows.push(row);
    }
    let task = &tasks[0];
    let repo = Repo::new(task);
    let before = tree(&repo.0).unwrap();
    let start = Instant::now();
    let control = packet(
        &repo.0,
        task,
        "Plan and implement a broad production authentication migration across the whole repository.",
    );
    let unchanged = tree(&repo.0).is_some_and(|after| after == before);
    println!(
        "{}",
        json!({"task": "broad-cloud-control", "status": control.unwrap_or("infrastructure_invalid"),
        "unchanged": unchanged, "elapsed_ms": start.elapsed().as_millis()})
    );
    let verified = rows
        .iter()
        .filter(|r| r["verified"] == true && r["scope_ok"] == true && r["route"] == "local")
        .count();
    println!(
        "{}",
        json!({"kind": "local_diagnostic", "tasks": rows.len(), "verified_local": verified,
        "cloud_deferred": rows.iter().filter(|r| r["status"] == "cloud").count(),
        "boundary": "mcp_dispatch_through_verification", "cache": "uncontrolled",
        "claim": "no paired comparison or savings evidence"})
    );
    assert!(
        rows.iter()
            .all(|r| r["status"] != "infrastructure_invalid" && r["scope_ok"] == true),
        "diagnostic infrastructure or scope failure"
    );
    assert!(verified > 0, "diagnostic verified no Local work");
    assert_eq!(control, Some("cloud"));
    assert!(unchanged, "Cloud control changed its fixture");
}
