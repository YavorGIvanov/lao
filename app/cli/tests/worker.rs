#![cfg(target_os = "macos")]

use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("lao-worker-e2e-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Hybrid worker: one packet stays Cloud; the next routes through real
// OpenCode/Qwen, makes one bounded patch, and passes an independent verifier.
#[test]
#[ignore = "requires an active default LAO install and the pinned local runtime"]
fn installed_worker_makes_one_verified_patch() {
    let temp = Temp::new();
    assert!(
        Command::new("/usr/bin/git")
            .args(["init", "-q"])
            .current_dir(&temp.0)
            .status()
            .unwrap()
            .success()
    );
    fs::write(temp.0.join("word.txt"), "teh\n").unwrap();
    fs::write(temp.0.join("cloud.txt"), "unchanged\n").unwrap();
    fs::write(
        temp.0.join("verify.sh"),
        "#!/bin/sh\nset -eu\n[ \"$(cat word.txt)\" = the ]\n",
    )
    .unwrap();
    fs::set_permissions(temp.0.join("verify.sh"), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        Command::new("/usr/bin/git")
            .args(["add", "word.txt", "cloud.txt", "verify.sh"])
            .current_dir(&temp.0)
            .status()
            .unwrap()
            .success()
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_lao"))
        .arg("mcp")
        .current_dir(&temp.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let input = [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "execute",
                "arguments": {
                    "objective": "Plan and implement a broad production authentication migration across the whole repository.",
                    "allowed_paths": ["cloud.txt"]
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "execute",
                "arguments": {
                    "objective": "Make one small mechanical code change in one named file. Change only word.txt from teh to the. Do not change any other file.",
                    "allowed_paths": ["word.txt"]
                }
            }
        }),
    ];
    {
        let stdin = child.stdin.as_mut().unwrap();
        for request in input {
            serde_json::to_writer(&mut *stdin, &request).unwrap();
            stdin.write_all(b"\n").unwrap();
        }
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 3);
    assert_eq!(
        responses[1].pointer("/result/structuredContent/status"),
        Some(&serde_json::Value::String("cloud".into()))
    );
    assert_eq!(
        responses[2].pointer("/result/structuredContent/status"),
        Some(&serde_json::Value::String("complete".into()))
    );
    assert_eq!(
        fs::read_to_string(temp.0.join("word.txt")).unwrap(),
        "the\n"
    );
    assert_eq!(
        fs::read_to_string(temp.0.join("cloud.txt")).unwrap(),
        "unchanged\n"
    );
    assert!(
        Command::new("./verify.sh")
            .current_dir(&temp.0)
            .status()
            .unwrap()
            .success()
    );
    let changed = Command::new("/usr/bin/git")
        .args(["diff", "--name-only"])
        .current_dir(&temp.0)
        .output()
        .unwrap();
    assert!(changed.status.success());
    assert_eq!(String::from_utf8(changed.stdout).unwrap(), "word.txt\n");
}
