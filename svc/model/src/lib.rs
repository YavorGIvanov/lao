use std::{
    fs::{self, OpenOptions},
    io,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use lao_model_api::{Artifact, Status, Verified};

pub static QWEN: Artifact = Artifact {
    id: "qwen2.5-coder-1.5b-q4",
    url: "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF/resolve/edc3bdcfdc6406d6be331753248d4ac9b463cf1b/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
    revision: "edc3bdcfdc6406d6be331753248d4ac9b463cf1b",
    file: "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
    bytes: 1_117_320_768,
    sha256: "cc324af070c2ecbfd324a30884d2f951a7ff756aba85cb811a6ec436933bb046",
    license: "Apache-2.0",
    template: "chatml",
    context: 32_768,
    runtime: "llama.cpp 10280 (61881b1f7)",
    working_set: 3 * 1024 * 1024 * 1024,
};

pub fn prepare(root: &Path) -> io::Result<Verified> {
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    let path = root.join(QWEN.file);
    if path.exists() {
        return open(root);
    }

    let part = root.join(format!(".{}.part", QWEN.file));
    let mut pending = Pending(Some(part.clone()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&part)?;
    let status = Command::new("/usr/bin/curl")
        .args(["--fail", "--location", "--silent", "--show-error"])
        .args(["--proto", "=https", "--proto-redir", "=https"])
        .args(["--max-filesize", &QWEN.bytes.to_string()])
        .args(["--connect-timeout", "30", "--max-time", "1800"])
        .arg("--output")
        .arg(&part)
        .arg(QWEN.url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(invalid("download"));
    }
    verify(&part)?;
    fs::rename(&part, &path)?;
    pending.0 = None;
    Ok(Verified {
        artifact: &QWEN,
        path,
    })
}

pub fn open(root: &Path) -> io::Result<Verified> {
    let path = root.join(QWEN.file);
    verify(&path)?;
    Ok(Verified {
        artifact: &QWEN,
        path,
    })
}

fn verify(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() || meta.len() != QWEN.bytes {
        return Err(invalid("artifact"));
    }
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()?;
    let hash = String::from_utf8(output.stdout).map_err(|_| invalid("artifact"))?;
    if !output.status.success() || hash.split_whitespace().next() != Some(QWEN.sha256) {
        return Err(invalid("artifact"));
    }
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

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn status() -> Status {
    Status::Active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_artifact_is_rejected() {
        let root = std::env::temp_dir().join(format!("lao-model-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        fs::write(root.join(QWEN.file), b"wrong").unwrap();
        assert!(prepare(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
