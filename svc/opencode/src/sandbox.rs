use std::{io, net::SocketAddr, path::Path, path::PathBuf, process::Command};

#[cfg(target_os = "macos")]
pub(super) fn wrap(
    source: &Command,
    config: &Path,
    state: &Path,
    allowed: &[PathBuf],
    addr: SocketAddr,
) -> io::Result<Command> {
    let root = source
        .get_current_dir()
        .ok_or_else(|| super::invalid("sandbox root"))?
        .canonicalize()?;
    let mut profile = String::from(
        r#"(version 1)
(deny default)
(import "system.sb")
(deny file-write* (subpath "/cores"))
(allow process-exec process-fork)
(allow signal (target self))
(allow file-read-metadata)
(allow file-read-data (literal (param "ROOT")))
(deny file-read* (regex #"(^|/)[.]git(/|$)"))
(allow file-read* file-map-executable
    (subpath "/bin") (subpath "/usr/bin")
    (literal (param "BIN")) (subpath (param "CONFIG")))
(allow file-read* file-write* (subpath (param "STATE")))
(allow network-outbound (remote tcp (param "GATE")))
"#,
    );
    let mut command = Command::new("/usr/bin/sandbox-exec");
    parameter(&mut command, "ROOT", &root)?;
    for (name, path) in [
        ("BIN", Path::new(source.get_program())),
        ("CONFIG", config),
        ("STATE", state),
    ] {
        parameter(&mut command, name, &path.canonicalize()?)?;
    }
    command
        .arg("-D")
        .arg(format!("GATE=localhost:{}", addr.port()));
    for (index, path) in allowed.iter().enumerate() {
        let name = format!("FILE{index}");
        parameter(&mut command, &name, &root.join(path))?;
        profile.push_str(&format!(
            "(allow file-read* file-write* (literal (param \"{name}\")))\n"
        ));
    }
    command
        .args(["-p", &profile])
        .arg(source.get_program())
        .args(source.get_args());
    command.current_dir(root).env_clear();
    for (key, value) in source.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    Ok(command)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::{
        env, fs,
        net::{TcpListener, TcpStream},
        process::Stdio,
        time::{Duration, Instant},
    };

    #[test]
    fn worker_os_boundary_allows_only_packet_files_state_and_gate() {
        if env::var_os("LAO_SANDBOX_PROBE").is_some() {
            assert_eq!(fs::read_to_string("allowed.txt").unwrap(), "before");
            fs::write("allowed.txt", "after").unwrap();
            fs::write(
                PathBuf::from(env::var_os("TMPDIR").unwrap()).join("scratch"),
                "ok",
            )
            .unwrap();
            for path in [
                "unlisted.txt",
                "../outside.txt",
                ".git/config",
                "../config/private.txt",
            ] {
                if path != "../config/private.txt" {
                    assert!(fs::read(path).is_err());
                }
                assert!(fs::write(path, "denied").is_err());
            }
            assert!(fs::read_dir(".").is_ok());
            fs::remove_file("allowed.txt").unwrap();
            std::os::unix::fs::symlink("../outside.txt", "allowed.txt").unwrap();
            assert!(fs::read("allowed.txt").is_err());
            assert!(fs::write("allowed.txt", "denied").is_err());
            fs::remove_file("allowed.txt").unwrap();
            assert!(fs::hard_link("../outside.txt", "allowed.txt").is_err());
            fs::write("allowed.txt", "after").unwrap();
            let gate = env::var("LAO_TEST_GATE").unwrap().parse().unwrap();
            let other = env::var("LAO_TEST_OTHER").unwrap().parse().unwrap();
            TcpStream::connect_timeout(&gate, Duration::from_secs(1)).unwrap();
            assert!(TcpStream::connect_timeout(&other, Duration::from_secs(1)).is_err());
            return;
        }
        let temp = super::super::Temp::new("sandbox-test").unwrap();
        let root = temp.path().join("repo \" (literal)");
        let config = temp.path().join("config");
        let state = temp.path().join("state");
        for path in [&root, &config, &state, &root.join(".git")] {
            fs::create_dir(path).unwrap();
        }
        fs::write(root.join("allowed.txt"), "before").unwrap();
        for path in [
            root.join("unlisted.txt"),
            temp.path().join("outside.txt"),
            root.join(".git/config"),
            config.join("private.txt"),
        ] {
            fs::write(path, "untouched").unwrap();
        }
        let gate = TcpListener::bind("127.0.0.1:0").unwrap();
        let other = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut source = Command::new(env::current_exe().unwrap());
        source
            .args([
                "--exact",
                "sandbox::tests::worker_os_boundary_allows_only_packet_files_state_and_gate",
            ])
            .current_dir(&root)
            .env_clear()
            .env("LAO_SANDBOX_PROBE", "1")
            .env("TMPDIR", &state)
            .env("LAO_TEST_GATE", gate.local_addr().unwrap().to_string())
            .env("LAO_TEST_OTHER", other.local_addr().unwrap().to_string());
        let mut command = wrap(
            &source,
            &config,
            &state,
            &["allowed.txt".into()],
            gate.local_addr().unwrap(),
        )
        .unwrap();
        command.stdin(Stdio::null());
        let result =
            super::super::run(&mut command, Instant::now() + Duration::from_secs(5)).unwrap();
        assert!(
            result.status.success(),
            "sandbox probe failed: {}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert!(!result.timed_out);
        assert_eq!(
            fs::read_to_string(root.join("allowed.txt")).unwrap(),
            "after"
        );
        assert_eq!(
            fs::read_to_string(root.join("unlisted.txt")).unwrap(),
            "untouched"
        );
    }
}

#[cfg(target_os = "macos")]
fn parameter(command: &mut Command, name: &str, path: &Path) -> io::Result<()> {
    let path = path
        .to_str()
        .ok_or_else(|| super::invalid("sandbox path"))?;
    command.arg("-D").arg(format!("{name}={path}"));
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn wrap(
    _source: &Command,
    _config: &Path,
    _state: &Path,
    _allowed: &[PathBuf],
    _addr: SocketAddr,
) -> io::Result<Command> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "worker sandbox requires macOS",
    ))
}
