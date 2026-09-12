use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const LIMIT: usize = 1024 * 1024;
const MANIFEST: &str = include_str!("../../../../docs/benchmarks/tasks.json");
const PROJECTS: &[(&str, &str)] = &[
    (
        "web",
        include_str!("../../../../docs/benchmarks/fixtures/web/package.json"),
    ),
    (
        "service",
        include_str!("../../../../docs/benchmarks/fixtures/service/settings.json"),
    ),
    (
        "catalog",
        include_str!("../../../../docs/benchmarks/fixtures/catalog/products.json"),
    ),
    (
        "vite",
        include_str!("../../../../docs/benchmarks/fixtures/vite/package.json"),
    ),
    (
        "fastify",
        include_str!("../../../../docs/benchmarks/fixtures/fastify/package.json"),
    ),
    (
        "express",
        include_str!("../../../../docs/benchmarks/fixtures/express/package.json"),
    ),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Task {
    pub(crate) id: String,
    pub(crate) project: String,
    pub(crate) file: String,
    pub(crate) objective: String,
    pub(crate) pointer: String,
    pub(crate) value: Value,
}

impl Task {
    pub(crate) fn source(&self) -> &'static str {
        PROJECTS
            .iter()
            .find(|(name, _)| *name == self.project)
            .unwrap()
            .1
    }

    pub(crate) fn expected(&self) -> Value {
        let mut expected: Value = serde_json::from_str(self.source()).unwrap();
        let value = expected.pointer_mut(&self.pointer).unwrap();
        assert_ne!(*value, self.value, "fixture must require an edit");
        *value = self.value.clone();
        expected
    }
}

pub(crate) struct Repo(pub(crate) PathBuf);

impl Repo {
    pub(crate) fn new(task: &Task) -> Self {
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
pub(crate) struct Entry {
    mode: u32,
    bytes: Vec<u8>,
}

pub(crate) type Tree = BTreeMap<PathBuf, Entry>;

pub(crate) fn tree(root: &Path) -> Option<Tree> {
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

pub(crate) fn verdict(root: &Path, task: &Task, before: &Tree, expected: &Value) -> (bool, bool) {
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

pub(crate) fn tasks() -> Vec<Task> {
    serde_json::from_str(MANIFEST).unwrap()
}

pub(crate) fn workflow_tasks() -> Vec<Task> {
    let mut all = tasks();
    all.extend(
        serde_json::from_str::<Vec<Task>>(include_str!(
            "../../../../docs/benchmarks/workflow-tasks.json"
        ))
        .unwrap(),
    );
    all
}
