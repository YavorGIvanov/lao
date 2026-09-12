#![cfg(target_os = "macos")]

#[path = "support/tasks.rs"]
mod fixtures;
use fixtures::*;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(10 * 60 + 5);

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

#[test]
fn public_tasks_fail_before_and_pass_independent_reference_edits() {
    assert_eq!(tasks().len(), 6);
    let tasks = workflow_tasks();
    assert_eq!(tasks.len(), 12);
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
