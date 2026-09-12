use super::*;

#[cfg(target_os = "macos")]
pub(super) fn run() -> Result<()> {
    let paths = paths()?;
    let home = PathBuf::from(env::var_os("HOME").ok_or_else(|| invalid("HOME"))?);
    check(&home, true)?;
    let files = Files {
        prefix: env::var_os("LAO_PREFIX")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/libexec/lao")),
        bin: env::var_os("LAO_BIN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/bin")),
        cache: home.join("Library/Caches/lao"),
    };
    if files.prefix == home || files.bin == home {
        return Err(invalid("broad uninstall path").into());
    }
    remove(&paths, &files, deactivate)?;
    println!(
        "uninstalled: LAO settings, cached artifacts and installed binaries removed; unrelated files preserved"
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn run() -> Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "macOS only").into())
}

#[cfg(target_os = "macos")]
pub(super) struct Files {
    pub(super) prefix: PathBuf,
    pub(super) bin: PathBuf,
    pub(super) cache: PathBuf,
}

#[cfg(target_os = "macos")]
pub(super) fn remove(
    paths: &Paths,
    files: &Files,
    stop: impl FnOnce(&Paths) -> io::Result<()>,
) -> Result<()> {
    let roots = [&files.prefix, &files.bin, &files.cache, &paths.state];
    for (index, root) in roots.iter().enumerate() {
        check(root, true)?;
        if roots
            .iter()
            .skip(index + 1)
            .any(|other| root.starts_with(other) || other.starts_with(root))
        {
            return Err(invalid("overlapping uninstall paths").into());
        }
    }
    fs::create_dir_all(&files.prefix)?;
    let _binary_lock = Lock::file(&files.prefix.join(".install-lock"))?;
    let _state_lock = Lock::acquire(&paths.state)?;
    for name in [".install-pending", ".install-staging"] {
        if present(&files.prefix.join(name))? {
            return Err(
                conflict("binary recovery is pending; rerun the archive installer first").into(),
            );
        }
    }
    for name in ["lao", "lao-daemon"] {
        check(&files.prefix.join(name), false)?;
        let link = files.bin.join(name);
        match fs::symlink_metadata(&link) {
            Ok(meta)
                if meta.file_type().is_symlink()
                    && fs::read_link(&link)? == files.prefix.join(name) => {}
            Ok(_) => return Err(conflict("command belongs to another installation").into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    receipt(&files.prefix)?;
    let installed = check(&paths.state.join(RECORD), false)?;
    check(&paths.plist, false)?;
    if installed {
        let transaction = Transaction::load(&paths.state)?;
        for entry in [
            &transaction.record.codex,
            &transaction.record.claude,
            &transaction.record.claude_mcp,
        ]
        .into_iter()
        .flatten()
        {
            if entry.path.starts_with(&paths.state) || entry.path.starts_with(&files.cache) {
                return Err(conflict("client settings overlap LAO removal paths").into());
            }
        }
        disable(paths, stop)?;
    } else {
        for path in [
            &paths.plist,
            &paths.daemon,
            &paths.adopted,
            &paths.worker_key,
        ] {
            if present(path)? {
                return Err(conflict(
                    "service state has no recovery record; retained for inspection",
                )
                .into());
            }
        }
    }

    // Remove only the private LAO namespaces, never client homes or external runtimes.
    if present(&files.cache)? {
        fs::remove_dir_all(&files.cache)?;
    }
    for entry in fs::read_dir(&paths.state)? {
        let entry = entry?;
        if entry.file_name() == "install.lock" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    for name in ["lao", "lao-daemon"] {
        remove_optional(&files.bin.join(name))?;
        remove_optional(&files.prefix.join(name))?;
    }
    remove_optional(&files.prefix.join("source-revision"))?;
    // No further writes in each namespace after releasing its lock pathname.
    remove_optional(&files.prefix.join(".install-lock"))?;
    empty_dir(&files.prefix)?;
    remove_optional(&paths.state.join("install.lock"))?;
    empty_dir(&paths.state)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "macos")]
fn check(path: &Path, directory: bool) -> io::Result<bool> {
    if !path.is_absolute()
        || path.parent().is_none()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(invalid("uninstall path"));
    }
    for parent in path.ancestors().skip(1) {
        match fs::symlink_metadata(parent) {
            Ok(meta) if !meta.file_type().is_dir() => {
                return Err(conflict("uninstall path has a non-directory ancestor"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    match fs::symlink_metadata(path) {
        Ok(meta)
            if (directory && meta.file_type().is_dir())
                || (!directory && meta.file_type().is_file()) =>
        {
            Ok(true)
        }
        Ok(_) => Err(conflict("uninstall path has an unexpected file type")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "macos")]
fn receipt(prefix: &Path) -> io::Result<()> {
    let path = prefix.join("source-revision");
    if !check(&path, false)? {
        if prefix.join("lao").exists() || prefix.join("lao-daemon").exists() {
            return Err(conflict(
                "binary ownership receipt missing; rerun install.sh first",
            ));
        }
        return Ok(());
    }
    if fs::metadata(&path)?.len() > 4096 {
        return Err(invalid("binary ownership receipt"));
    }
    let bytes = fs::read_to_string(path)?;
    let lines: Vec<_> = bytes.lines().collect();
    if lines.len() != 3 || lines[0].is_empty() {
        return Err(invalid("binary ownership receipt"));
    }
    for (name, expected) in ["lao", "lao-daemon"].into_iter().zip(&lines[1..]) {
        if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("binary ownership checksum"));
        }
        if prefix.join(name).exists() && hash(&prefix.join(name))? != *expected {
            return Err(conflict("installed binary changed; removal refused"));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn hash(path: &Path) -> io::Result<String> {
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(invalid("binary checksum"));
    }
    let text = String::from_utf8(output.stdout).map_err(|_| invalid("binary checksum"))?;
    Ok(text
        .split_whitespace()
        .next()
        .ok_or_else(|| invalid("binary checksum"))?
        .to_owned())
}

#[cfg(target_os = "macos")]
fn empty_dir(path: &Path) -> io::Result<()> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
