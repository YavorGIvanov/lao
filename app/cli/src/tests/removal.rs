use super::*;
use crate::uninstall::{Files, hash, remove};

fn files(temp: &Temp) -> Files {
    let files = Files {
        prefix: temp.0.join("prefix"),
        bin: temp.0.join("bin"),
        cache: temp.0.join("cache"),
    };
    for path in [&files.prefix, &files.bin, &files.cache] {
        fs::create_dir(path).unwrap();
    }
    for name in ["lao", "lao-daemon"] {
        fs::write(files.prefix.join(name), format!("owned {name}")).unwrap();
        std::os::unix::fs::symlink(files.prefix.join(name), files.bin.join(name)).unwrap();
    }
    fs::write(
        files.prefix.join("source-revision"),
        format!(
            "test revision\n{}\n{}\n",
            hash(&files.prefix.join("lao")).unwrap(),
            hash(&files.prefix.join("lao-daemon")).unwrap()
        ),
    )
    .unwrap();
    fs::write(files.cache.join("model"), b"cached artifact").unwrap();
    files
}

#[test]
fn uninstall_restores_settings_removes_owned_artifacts_and_can_repeat() {
    let mut temp = Temp::new();
    temp.0 = fs::canonicalize(&temp.0).unwrap();
    let (paths, mut transaction, codex, claude, mcp) = prepared(&temp, ClientChoice::Both, 8765);
    transaction.apply().unwrap();
    for path in [&paths.plist, &paths.state.join(PLIST_AFTER)] {
        write_atomic(path, b"owned plist", 0o600).unwrap();
    }
    let files = files(&temp);
    fs::write(files.prefix.join("keep"), b"unrelated").unwrap();
    fs::write(files.bin.join("other-command"), b"unrelated command").unwrap();
    let outside = temp.0.join("external-runtime");
    fs::write(&outside, b"user managed").unwrap();
    std::os::unix::fs::symlink(&outside, files.cache.join("external-link")).unwrap();
    let mut stopped = false;
    remove(&paths, &files, |paths| {
        assert_eq!(fs::read(&paths.codex).unwrap(), codex);
        stopped = true;
        remove_optional(&paths.plist)
    })
    .unwrap();
    assert!(stopped);
    assert_eq!(fs::read(&paths.codex).unwrap(), codex);
    assert_eq!(fs::read(&paths.claude).unwrap(), claude);
    assert_eq!(fs::read(&paths.claude_mcp).unwrap(), mcp);
    assert!(!paths.state.exists());
    assert!(!files.cache.exists());
    for name in ["lao", "lao-daemon", "source-revision", ".install-lock"] {
        assert!(!files.prefix.join(name).exists());
    }
    for name in ["lao", "lao-daemon"] {
        assert!(fs::symlink_metadata(files.bin.join(name)).is_err());
    }
    assert_eq!(fs::read(files.prefix.join("keep")).unwrap(), b"unrelated");
    assert_eq!(
        fs::read(files.bin.join("other-command")).unwrap(),
        b"unrelated command"
    );
    assert_eq!(fs::read(&outside).unwrap(), b"user managed");
    remove(&paths, &files, |_| {
        panic!("an absent install must not stop a service")
    })
    .unwrap();
    assert!(!paths.state.exists());
    assert_eq!(
        parse([OsString::from("uninstall")].into_iter()).unwrap(),
        Some(Action::Uninstall)
    );
    assert!(parse([OsString::from("uninstall"), OsString::from("--force")].into_iter()).is_err());
}

#[test]
fn uninstall_conflicts_preserve_settings_binaries_cache_and_recovery() {
    for fault in ["settings", "binary", "pending", "cache-link", "stop"] {
        let mut temp = Temp::new();
        temp.0 = fs::canonicalize(&temp.0).unwrap();
        let (paths, mut transaction, _, _, _) = prepared(&temp, ClientChoice::Both, 8765);
        transaction.apply().unwrap();
        for path in [&paths.plist, &paths.state.join(PLIST_AFTER)] {
            write_atomic(path, b"owned plist", 0o600).unwrap();
        }
        let files = files(&temp);
        match fault {
            "stop" => {}
            "settings" => fs::write(&paths.codex, b"invalid [").unwrap(),
            "binary" => fs::write(files.prefix.join("lao"), b"user replacement").unwrap(),
            "pending" => fs::create_dir(files.prefix.join(".install-pending")).unwrap(),
            "cache-link" => {
                fs::rename(&files.cache, temp.0.join("external-cache")).unwrap();
                std::os::unix::fs::symlink(temp.0.join("external-cache"), &files.cache).unwrap();
            }
            _ => unreachable!(),
        }
        let settings = fs::read(&paths.codex).unwrap();
        let binary = fs::read(files.prefix.join("lao")).unwrap();
        assert!(
            remove(&paths, &files, |_| {
                assert_eq!(fault, "stop", "conflict must precede shutdown");
                Err(io::Error::other("test shutdown failure"))
            })
            .is_err()
        );
        if fault == "stop" {
            assert_eq!(
                Transaction::load(&paths.state).unwrap().record.phase,
                Phase::Restored
            );
        } else {
            assert_eq!(fs::read(&paths.codex).unwrap(), settings);
        }
        assert_eq!(fs::read(files.prefix.join("lao")).unwrap(), binary);
        assert!(paths.state.join(RECORD).is_file());
        assert_eq!(
            fs::read(files.cache.join("model")).unwrap(),
            b"cached artifact"
        );
        assert!(files.prefix.join("source-revision").is_file());
        if fault == "stop" {
            remove(&paths, &files, |paths| remove_optional(&paths.plist)).unwrap();
            assert!(!paths.state.exists());
            assert!(!files.prefix.exists());
            assert!(!files.cache.exists());
        }
    }
}
