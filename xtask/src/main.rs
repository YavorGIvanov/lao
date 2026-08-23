use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Restricted = BTreeMap<String, Vec<String>>;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
    metadata: WorkspaceMetadata,
}

#[derive(Deserialize)]
struct WorkspaceMetadata {
    lao: WorkspaceLao,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceLao {
    policy: u8,
    #[serde(default)]
    restricted: Restricted,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    id: String,
    manifest_path: PathBuf,
    dependencies: Vec<Dependency>,
    metadata: PackageMetadata,
}

#[derive(Deserialize)]
struct PackageMetadata {
    lao: Option<Lao>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    kind: Option<String>,
    path: Option<PathBuf>,
    source: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lao {
    kind: String,
    owner: String,
    #[serde(default)]
    api: Option<String>,
    state: String,
    isolate: String,
    status: String,
    #[serde(default)]
    migrations: Option<PathBuf>,
    #[serde(default)]
    state_access: Vec<String>,
}

struct Edge {
    to: String,
    kind: String,
}

struct Node {
    name: String,
    root: PathBuf,
    lao: Lao,
    deps: Vec<Edge>,
}

fn main() -> Result<()> {
    let command = env::args().nth(1).unwrap_or_else(|| "check".into());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no workspace parent")?
        .to_path_buf();
    match command.as_str() {
        "check" => check(&root),
        "graph" => graph(&root),
        "extract" => extract(&root),
        _ => Err(format!("unknown xtask command: {command}").into()),
    }
}

fn load(root: &Path) -> Result<(Vec<Node>, Restricted)> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout)?;
    let members: BTreeSet<_> = metadata.workspace_members.into_iter().collect();
    let roots: BTreeMap<_, _> = metadata
        .packages
        .iter()
        .filter(|package| members.contains(&package.id))
        .filter_map(|package| {
            package
                .manifest_path
                .parent()
                .map(|root| (root.to_path_buf(), package.name.clone()))
        })
        .collect();
    let names: BTreeSet<_> = roots.values().cloned().collect();
    if metadata.metadata.lao.policy != 1 {
        return Err("unsupported architecture policy version".into());
    }
    let mut nodes = Vec::new();
    for package in metadata
        .packages
        .into_iter()
        .filter(|package| members.contains(&package.id))
    {
        let lao = package
            .metadata
            .lao
            .ok_or_else(|| format!("{} has no package.metadata.lao", package.name))?;
        let package_root = package
            .manifest_path
            .parent()
            .ok_or_else(|| format!("{} has no package root", package.name))?
            .to_path_buf();
        let mut deps = Vec::new();
        for dependency in package.dependencies {
            if let Some(edge) = resolve_dependency(&package.name, dependency, &roots, &names)? {
                deps.push(edge);
            }
        }
        nodes.push(Node {
            name: package.name,
            root: package_root,
            lao,
            deps,
        });
    }
    let restricted = metadata.metadata.lao.restricted;
    if metadata.workspace_root != root {
        return Err("cargo metadata returned an unexpected workspace root".into());
    }
    Ok((nodes, restricted))
}

fn resolve_dependency(
    package: &str,
    dependency: Dependency,
    roots: &BTreeMap<PathBuf, String>,
    names: &BTreeSet<String>,
) -> Result<Option<Edge>> {
    if let Some(path) = &dependency.path {
        if let Some(target) = roots.get(path) {
            return Ok(Some(Edge {
                to: target.clone(),
                kind: dependency.kind.unwrap_or_else(|| "normal".into()),
            }));
        }
        return Err(format!(
            "{package} has an unclassified path dependency {}",
            path.display()
        )
        .into());
    }
    if names.contains(&dependency.name) {
        return Err(format!(
            "{package} shadows workspace package {} from {}",
            dependency.name,
            dependency.source.as_deref().unwrap_or("a path")
        )
        .into());
    }
    Ok(None)
}

fn check(root: &Path) -> Result<()> {
    let (nodes, restricted) = load(root)?;
    let errors = validate(&nodes, &restricted);
    if errors.is_empty() {
        println!("architecture: {} packages valid", nodes.len());
        return Ok(());
    }
    Err(errors.join("\n").into())
}

fn graph(root: &Path) -> Result<()> {
    let (mut nodes, _) = load(root)?;
    nodes.sort_by(|left, right| left.name.cmp(&right.name));
    for node in nodes {
        let mut deps: Vec<_> = node.deps.into_iter().map(|edge| edge.to).collect();
        deps.sort();
        println!("{} [{}] -> {}", node.name, node.lao.kind, deps.join(", "));
    }
    Ok(())
}

fn validate(nodes: &[Node], restricted: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut errors = Vec::new();
    let by_name: BTreeMap<_, _> = nodes
        .iter()
        .map(|node| (node.name.as_str(), node))
        .collect();
    let mut states = BTreeMap::new();

    for (target, consumers) in restricted {
        match by_name.get(target.as_str()) {
            Some(node) if node.lao.kind == "api" => {}
            Some(_) => errors.push(format!("restricted target {target} is not an API")),
            None => errors.push(format!("restricted target {target} does not exist")),
        }
        for consumer in consumers {
            if !by_name.contains_key(consumer.as_str()) {
                errors.push(format!("restricted consumer {consumer} does not exist"));
            }
        }
    }

    for node in nodes {
        for value in [
            node.lao.owner.as_str(),
            node.lao.isolate.as_str(),
            node.lao.status.as_str(),
        ] {
            if value.is_empty() {
                errors.push(format!("{} has incomplete component metadata", node.name));
            }
        }
        if !matches!(
            node.lao.kind.as_str(),
            "api" | "wire" | "svc" | "app" | "test" | "tool"
        ) {
            errors.push(format!("{} has unknown kind {}", node.name, node.lao.kind));
        }
        if !matches!(node.lao.status.as_str(), "active" | "draft" | "stub") {
            errors.push(format!(
                "{} has unknown status {}",
                node.name, node.lao.status
            ));
        }
        let isolation_ok = matches!(
            (node.lao.kind.as_str(), node.lao.isolate.as_str()),
            ("api" | "wire", "contract")
                | ("svc", "linked" | "worker")
                | ("app" | "test" | "tool", "process")
        );
        if !isolation_ok {
            errors.push(format!(
                "{} has invalid isolation {}",
                node.name, node.lao.isolate
            ));
        }
        if !valid_name(&node.lao.state) {
            errors.push(format!("{} has invalid state name", node.name));
        }
        if node.lao.kind != "svc" && node.lao.state != "none" {
            errors.push(format!("{} may not own service state", node.name));
        }
        let path_kind = node
            .root
            .components()
            .rev()
            .nth(1)
            .and_then(|part| part.as_os_str().to_str());
        let path_ok = match node.lao.kind.as_str() {
            "api" => path_kind == Some("api"),
            "wire" => path_kind == Some("api") && node.name == "lao-wire",
            "svc" => path_kind == Some("svc"),
            "app" => path_kind == Some("app"),
            "test" => path_kind == Some("test"),
            "tool" => node.name == "xtask",
            _ => false,
        };
        if !path_ok {
            errors.push(format!(
                "{} is stored under the wrong package path",
                node.name
            ));
        }
        if node.lao.kind == "svc" {
            match &node.lao.api {
                Some(api)
                    if by_name
                        .get(api.as_str())
                        .is_some_and(|node| node.lao.kind == "api")
                        && node.deps.iter().any(|edge| edge.to == *api) => {}
                Some(api) => errors.push(format!("{} has invalid declared API {api}", node.name)),
                None => errors.push(format!("{} has no declared API", node.name)),
            }
        } else if node.lao.api.is_some() {
            errors.push(format!("{} declares a service API", node.name));
        }
        if node.lao.state != "none"
            && let Some(owner) = states.insert(node.lao.state.as_str(), node.name.as_str())
        {
            errors.push(format!(
                "state {} is owned by both {owner} and {}",
                node.lao.state, node.name
            ));
        }
        for state in &node.lao.state_access {
            if state != &node.lao.state {
                errors.push(format!(
                    "{} declares foreign state access {state}",
                    node.name
                ));
            }
        }
        if let Some(path) = &node.lao.migrations {
            let full = node.root.join(path);
            let contained = fs::canonicalize(&full)
                .and_then(|full| fs::canonicalize(&node.root).map(|root| full.starts_with(root)))
                .unwrap_or(false);
            if !is_local(path)
                || node.lao.state == "none"
                || !full.is_dir()
                || full.is_symlink()
                || !contained
            {
                errors.push(format!("{} has an invalid migration root", node.name));
            }
        }
        for edge in &node.deps {
            let Some(target) = by_name.get(edge.to.as_str()) else {
                continue;
            };
            if !allowed(&node.lao.kind, &target.lao.kind, &edge.kind) {
                errors.push(format!(
                    "forbidden {} dependency: {} ({}) -> {} ({})",
                    edge.kind, node.name, node.lao.kind, target.name, target.lao.kind
                ));
            }
            if let Some(consumers) = restricted.get(&target.name)
                && !consumers.contains(&node.name)
            {
                errors.push(format!(
                    "{} may not consume restricted API {}",
                    node.name, target.name
                ));
            }
        }
        if matches!(node.lao.kind.as_str(), "api" | "wire" | "svc" | "app") {
            scan_sources(node, &mut errors);
        }
    }
    detect_cycles(nodes, &mut errors);
    errors
}

fn allowed(from: &str, to: &str, dependency_kind: &str) -> bool {
    match from {
        "api" => to == "api",
        "wire" => false,
        "svc" => to == "api",
        "app" => matches!(to, "api" | "wire" | "svc"),
        "test" => matches!(to, "api" | "wire" | "svc" | "app" | "test"),
        "tool" => to == "api" && dependency_kind == "dev",
        _ => false,
    }
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_local(path: &Path) -> bool {
    !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
}

fn scan_sources(node: &Node, errors: &mut Vec<String>) {
    let mut paths = Vec::new();
    collect_rust(&node.root.join("src"), &mut paths);
    paths.push(node.root.join("build.rs"));
    for path in paths {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for forbidden in ["CARGO_MANIFEST_DIR", "../migrations", "../state"] {
            if text.contains(forbidden) {
                errors.push(format!(
                    "{} uses ambient workspace path pattern {forbidden}",
                    node.name
                ));
            }
        }
    }
}

fn collect_rust(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            files.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_rust(&entry.path(), files);
    }
}

fn detect_cycles(nodes: &[Node], errors: &mut Vec<String>) {
    let graph: BTreeMap<_, _> = nodes
        .iter()
        .map(|node| {
            (
                node.name.as_str(),
                node.deps
                    .iter()
                    .map(|edge| edge.to.as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let mut done = BTreeSet::new();
    let mut active = BTreeSet::new();
    for name in graph.keys() {
        visit(name, &graph, &mut done, &mut active, errors);
    }
}

fn visit<'a>(
    name: &'a str,
    graph: &BTreeMap<&'a str, Vec<&'a str>>,
    done: &mut BTreeSet<&'a str>,
    active: &mut BTreeSet<&'a str>,
    errors: &mut Vec<String>,
) {
    if done.contains(name) {
        return;
    }
    if !active.insert(name) {
        errors.push(format!("dependency cycle includes {name}"));
        return;
    }
    if let Some(deps) = graph.get(name) {
        for dependency in deps {
            visit(dependency, graph, done, active, errors);
        }
    }
    active.remove(name);
    done.insert(name);
}

fn extract(root: &Path) -> Result<()> {
    for package in ["lao-core-api", "lao-wire"] {
        run(Command::new("cargo")
            .args(["package", "-p", package, "--allow-dirty"])
            .current_dir(root))?;
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temp = env::temp_dir().join(format!("lao-extract-{}-{stamp}", std::process::id()));
    let packages = temp.join("packages");
    let kit = temp.join("kit");
    let external = temp.join("external");
    let run_dir = temp.join("run");
    fs::create_dir_all(&packages)?;
    fs::create_dir_all(kit.join("src"))?;
    fs::create_dir_all(external.join("src"))?;
    fs::create_dir_all(&run_dir)?;

    let version = env!("CARGO_PKG_VERSION");
    for package in ["lao-core-api", "lao-wire"] {
        let archive = root
            .join("target/package")
            .join(format!("{package}-{version}.crate"));
        run(Command::new("tar")
            .args(["-xzf"])
            .arg(archive)
            .arg("-C")
            .arg(&packages))?;
    }

    let core = packages.join(format!("lao-core-api-{version}"));
    let wire = packages.join(format!("lao-wire-{version}"));
    fs::copy(root.join("test/kit/src/lib.rs"), kit.join("src/lib.rs"))?;
    fs::write(
        kit.join("Cargo.toml"),
        format!(
            "[package]\nname='lao-test-kit'\nversion='{version}'\nedition='2024'\n\n[dependencies]\nlao-core-api={{path='{}'}}\nlao-wire={{path='{}'}}\nserde={{version='1',features=['derive']}}\n",
            core.display(),
            wire.display(),
        ),
    )?;

    fs::write(
        external.join("Cargo.toml"),
        format!(
            "[package]\nname='external-fake'\nversion='0.1.0'\nedition='2024'\n\n[dependencies]\nlao-core-api={{path='{}'}}\nlao-test-kit={{path='{}'}}\n",
            core.display(),
            kit.display(),
        ),
    )?;
    fs::write(
        external.join("src/main.rs"),
        r#"use lao_core_api::Fault;
use lao_test_kit::{Check, Checked, Probe};
use std::sync::Arc;

struct External;

impl Probe for External {
    fn check(&self, request: Check) -> Result<Checked, Fault> {
        if request.value == 0 {
            Err(Fault::unsupported())
        } else {
            Ok(Checked { value: request.value * 2 })
        }
    }
}

fn main() {
    lao_test_kit::assert_conformance(Arc::new(External));
}
"#,
    )?;
    run(Command::new("cargo")
        .args(["run", "--offline", "--manifest-path"])
        .arg(external.join("Cargo.toml"))
        .current_dir(&run_dir))?;
    fs::remove_dir_all(&temp)?;
    println!("extraction: external component passed the public conformance suite");
    Ok(())
}

fn run(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed with {status}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, kind: &str, state: &str, deps: &[(&str, &str)]) -> Node {
        let isolate = match kind {
            "api" | "wire" => "contract",
            "svc" => "linked",
            _ => "process",
        };
        Node {
            name: name.into(),
            root: PathBuf::new(),
            lao: Lao {
                kind: kind.into(),
                owner: "test".into(),
                api: (kind == "svc").then(|| "contract".into()),
                state: state.into(),
                isolate: isolate.into(),
                status: "stub".into(),
                migrations: None,
                state_access: Vec::new(),
            },
            deps: deps
                .iter()
                .map(|(to, kind)| Edge {
                    to: (*to).into(),
                    kind: (*kind).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn service_implementation_edge_fails() {
        let nodes = [
            node("contract", "api", "none", &[]),
            node(
                "left",
                "svc",
                "left",
                &[("contract", "normal"), ("right", "normal")],
            ),
            node("right", "svc", "right", &[("contract", "normal")]),
        ];
        assert!(
            validate(&nodes, &BTreeMap::new())
                .iter()
                .any(|error| error.contains("svc"))
        );
    }

    #[test]
    fn restricted_api_edge_fails() {
        let nodes = [
            node("secret-api", "api", "none", &[]),
            node("route", "api", "none", &[("secret-api", "normal")]),
            node("gate", "api", "none", &[]),
        ];
        let restricted = BTreeMap::from([("secret-api".into(), vec!["gate".into()])]);
        assert!(
            validate(&nodes, &restricted)
                .iter()
                .any(|error| error.contains("restricted"))
        );
    }

    #[test]
    fn duplicate_state_and_foreign_access_fail() {
        let mut left = node("left", "app", "vault", &[]);
        left.lao.state_access.push("route".into());
        let nodes = [left, node("right", "app", "vault", &[])];
        let errors = validate(&nodes, &BTreeMap::new());
        assert!(errors.iter().any(|error| error.contains("both")));
        assert!(errors.iter().any(|error| error.contains("foreign")));
    }

    #[test]
    fn dev_cycle_fails() {
        let nodes = [
            node("left", "test", "none", &[("right", "dev")]),
            node("right", "test", "none", &[("left", "dev")]),
        ];
        assert!(
            validate(&nodes, &BTreeMap::new())
                .iter()
                .any(|error| error.contains("cycle"))
        );
    }

    #[test]
    fn parent_migration_root_fails() {
        let mut owner = node("owner", "app", "owner", &[]);
        owner.lao.migrations = Some(PathBuf::from("../other/migrations"));
        assert!(
            validate(&[owner], &BTreeMap::new())
                .iter()
                .any(|error| error.contains("migration"))
        );
    }

    #[test]
    fn metadata_typos_fail_to_parse() {
        let value = serde_json::json!({
            "kind": "api", "owner": "test", "state": "none",
            "isolate": "contract", "status": "stub", "typo": true
        });
        assert!(serde_json::from_value::<Lao>(value).is_err());
    }

    #[test]
    fn stale_restrictions_fail() {
        let nodes = [node("contract", "api", "none", &[])];
        let restricted = BTreeMap::from([
            ("missing".into(), vec!["consumer".into()]),
            ("contract".into(), vec!["missing-consumer".into()]),
        ]);
        let errors = validate(&nodes, &restricted);
        assert!(errors.iter().any(|error| error.contains("target")));
        assert!(errors.iter().any(|error| error.contains("consumer")));
    }

    #[test]
    fn hidden_local_and_shadow_dependencies_fail() {
        let roots = BTreeMap::from([(PathBuf::from("/work/api"), "contract".into())]);
        let names = BTreeSet::from(["contract".into()]);
        let hidden = Dependency {
            name: "hidden".into(),
            kind: Some("dev".into()),
            path: Some(PathBuf::from("/work/excluded")),
            source: None,
        };
        assert!(resolve_dependency("app", hidden, &roots, &names).is_err());
        let outside = Dependency {
            name: "private".into(),
            kind: Some("normal".into()),
            path: Some(PathBuf::from("/private/crate")),
            source: None,
        };
        assert!(resolve_dependency("app", outside, &roots, &names).is_err());
        let shadow = Dependency {
            name: "contract".into(),
            kind: Some("build".into()),
            path: None,
            source: Some("registry+example".into()),
        };
        assert!(resolve_dependency("app", shadow, &roots, &names).is_err());
    }

    #[test]
    fn nested_ambient_path_fails() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = env::temp_dir().join(format!("lao-scan-{stamp}"));
        let nested = root.join("src/nested");
        fs::create_dir_all(&nested).expect("create fixture");
        fs::write(
            nested.join("mod.rs"),
            "const ROOT: &str = env!(\"CARGO_MANIFEST_DIR\");",
        )
        .expect("write fixture");
        let mut contract = node("contract", "api", "none", &[]);
        contract.root = root.clone();
        let errors = validate(&[contract], &BTreeMap::new());
        fs::remove_dir_all(root).expect("remove fixture");
        assert!(errors.iter().any(|error| error.contains("ambient")));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_migration_root_fails() {
        use std::os::unix::fs::symlink;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = env::temp_dir().join(format!("lao-migration-{stamp}"));
        let owner_root = root.join("svc/owner");
        let outside = root.join("other");
        fs::create_dir_all(&owner_root).expect("create owner");
        fs::create_dir_all(&outside).expect("create target");
        symlink(&outside, owner_root.join("migrations")).expect("create symlink");
        let mut owner = node("owner", "svc", "owner", &[("contract", "normal")]);
        owner.root = owner_root;
        owner.lao.migrations = Some("migrations".into());
        let contract = node("contract", "api", "none", &[]);
        let errors = validate(&[owner, contract], &BTreeMap::new());
        fs::remove_dir_all(root).expect("remove fixture");
        assert!(errors.iter().any(|error| error.contains("migration")));
    }
}
