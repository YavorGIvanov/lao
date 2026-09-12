#!/bin/sh
set -eu

umask 077

fail() {
    printf 'LAO setup: %s\n' "$1" >&2
    exit 1
}

case "$(uname -s):$(uname -m)" in
    Darwin:arm64) ;;
    *) fail "Stage 1 supports Apple Silicon macOS only" ;;
esac

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
user_home=${HOME:?HOME is required}
prefix=${LAO_PREFIX:-"$user_home/.local/libexec/lao"}
bin_dir=${LAO_BIN_DIR:-"$user_home/.local/bin"}

for path in "$prefix" "$bin_dir"; do
    case "$path" in
        /*) ;;
        *) fail "install paths must be absolute" ;;
    esac
    if [ "$path" = "/" ] || [ "$path" = "$user_home" ]; then
        fail "refusing broad install path"
    fi
    if [ -L "$path" ] || { [ -e "$path" ] && [ ! -d "$path" ]; }; then
        fail "install path is not a real directory: $path"
    fi
done

check_link() {
    target=$1
    source=$2
    if [ -L "$target" ]; then
        [ "$(readlink "$target")" = "$source" ] || fail "command already belongs to another install: $target"
    elif [ -e "$target" ]; then
        fail "command already exists and will not be overwritten: $target"
    fi
}

ensure_link() {
    target=$1
    source=$2
    if [ ! -L "$target" ]; then
        ln -s "$source" "$target"
    fi
}

cli="$prefix/lao"
daemon="$prefix/lao-daemon"
cli_link="$bin_dir/lao"
daemon_link="$bin_dir/lao-daemon"
revision_file="$prefix/source-revision"
check_link "$cli_link" "$cli"
check_link "$daemon_link" "$daemon"

revision=
binaries="$root/target/release"
prebuilt=false
if [ -e "$root/SHA256SUMS" ] || [ -e "$root/bin" ]; then
    prebuilt=true
    binaries="$root/bin"
    for file in SHA256SUMS install.sh bin/lao bin/lao-daemon LICENSE THIRD_PARTY_NOTICES.txt source-revision; do
        [ -f "$root/$file" ] && [ ! -L "$root/$file" ] || fail "incomplete prebuilt archive"
    done
    [ ! -L "$binaries" ] || fail "prebuilt binaries must be in a real directory"
    expected=$(cd "$root" && /usr/bin/shasum -a 256 install.sh bin/lao bin/lao-daemon LICENSE THIRD_PARTY_NOTICES.txt source-revision)
    [ "$(cat "$root/SHA256SUMS")" = "$expected" ] || fail "prebuilt archive checksum mismatch"
    revision="prebuilt:$(/usr/bin/shasum -a 256 "$root/SHA256SUMS" | cut -d ' ' -f 1)"
elif command -v git >/dev/null 2>&1 &&
    [ -z "$(git -C "$root" status --porcelain --untracked-files=normal 2>/dev/null)" ]; then
    revision=$(git -C "$root" rev-parse --verify HEAD 2>/dev/null || :)
fi

# The open descriptor serializes installers and releases the lock after process death.
mkdir -p "$prefix" "$bin_dir"
lock="$prefix/.install-lock"
[ ! -L "$lock" ] && { [ ! -e "$lock" ] || [ -f "$lock" ]; } || fail "install lock is not a regular file"
exec 9>>"$lock"
/usr/bin/lockf -s -t 0 9 || fail "another binary installer is running"
transaction="$prefix/.install-pending"
staging="$prefix/.install-staging"

manifest() (
    cd "$1" || exit 1
    for directory in old new; do
        [ -d "$directory" ] && [ ! -L "$directory" ] || exit 1
        for file in lao lao-daemon source-revision; do
            path="$directory/$file"
            if [ -e "$path" ] || [ -L "$path" ]; then
                [ ! -e "$path.absent" ] && [ ! -L "$path.absent" ] || exit 1
            else
                path="$path.absent"
                [ ! -s "$path" ] || exit 1
            fi
            [ -f "$path" ] && [ ! -L "$path" ] || exit 1
            /usr/bin/shasum -a 256 "$path" || exit 1
        done
    done
    for file in format bin-dir links; do
        [ -f "$file" ] && [ ! -L "$file" ] || exit 1
        /usr/bin/shasum -a 256 "$file" || exit 1
    done
)

recover() {
    [ -d "$transaction" ] && [ ! -L "$transaction" ] || fail "invalid binary recovery directory"
    [ -f "$transaction/SHA256SUMS" ] && [ ! -L "$transaction/SHA256SUMS" ] ||
        fail "incomplete binary recovery record; snapshots retained"
    recorded=$(manifest "$transaction") || fail "invalid binary recovery snapshots; retained for inspection"
    [ "$recorded" = "$(cat "$transaction/SHA256SUMS")" ] || fail "binary recovery checksum mismatch; snapshots retained"
    [ "$(cat "$transaction/format")" = 1 ] || fail "unknown binary recovery format"
    [ "$(cat "$transaction/bin-dir")" = "$(printf '%s' "$bin_dir" | /usr/bin/shasum -a 256)" ] ||
        fail "binary recovery requires the original LAO_BIN_DIR"
    links=$(cat "$transaction/links")
    case "$links" in
        'true true'|'true false'|'false true'|'false false') ;;
        *) fail "invalid binary recovery links" ;;
    esac
    cli_link_new=${links% *}
    daemon_link_new=${links#* }

    # Validate every destination before restoring any of them; preserve external edits.
    for file in lao lao-daemon source-revision; do
        destination="$prefix/$file"
        [ ! -L "$destination" ] || fail "binary recovery conflicts with installed $file"
        if [ -e "$destination" ]; then
            [ -f "$destination" ] || fail "binary recovery conflicts with installed $file"
            cmp -s "$destination" "$transaction/old/$file" ||
                cmp -s "$destination" "$transaction/new/$file" || fail "binary recovery conflicts with installed $file"
        else
            [ -f "$transaction/old/$file.absent" ] || [ -f "$transaction/new/$file.absent" ] ||
                fail "binary recovery conflicts with missing $file"
        fi
    done
    check_link "$cli_link" "$cli"
    check_link "$daemon_link" "$daemon"
    [ "$cli_link_new" = true ] || [ -L "$cli_link" ] || fail "binary recovery conflicts with missing CLI link"
    [ "$daemon_link_new" = true ] || [ -L "$daemon_link" ] || fail "binary recovery conflicts with missing daemon link"
    for file in lao lao-daemon source-revision; do
        if [ -f "$transaction/old/$file" ]; then
            rm -f "$transaction/restore-$file"
            ln "$transaction/old/$file" "$transaction/restore-$file" &&
                mv -f "$transaction/restore-$file" "$prefix/$file" || fail "rollback failed; snapshots retained in $transaction"
        else
            rm -f "$prefix/$file"
        fi
    done
    if [ "$cli_link_new" = true ]; then rm -f "$cli_link"; fi
    if [ "$daemon_link_new" = true ]; then rm -f "$daemon_link"; fi
    mv "$transaction" "$staging"
    rm -rf -- "$staging"
    printf 'Recovered interrupted binary installation.\n'
}

# Staging is never authoritative: it is either unpublished preparation or completed work.
[ ! -L "$staging" ] && { [ ! -e "$staging" ] || [ -d "$staging" ]; } || fail "invalid binary staging directory"
rm -rf -- "$staging"
if [ -e "$transaction" ] || [ -L "$transaction" ]; then recover; fi

cleanup() {
    result=$?
    trap - EXIT
    trap '' HUP INT TERM
    if [ "$result" -ne 0 ] && { [ -e "$transaction" ] || [ -L "$transaction" ]; }; then recover; fi
    rm -rf -- "$staging"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
check_link "$cli_link" "$cli"
check_link "$daemon_link" "$daemon"
cli_link_new=true
daemon_link_new=true
[ ! -L "$cli_link" ] || cli_link_new=false
[ ! -L "$daemon_link" ] || daemon_link_new=false
for file in lao lao-daemon source-revision; do
    [ ! -L "$prefix/$file" ] && { [ ! -e "$prefix/$file" ] || [ -f "$prefix/$file" ]; } ||
        fail "installed $file is not a regular file"
done

signature() {
    signature_dir=${1:-"$prefix"}
    cli_hash=$(/usr/bin/shasum -a 256 "$signature_dir/lao") || return 1
    daemon_hash=$(/usr/bin/shasum -a 256 "$signature_dir/lao-daemon") || return 1
    printf '%s\n%s\n%s\n' "${revision:-source:unversioned}" "${cli_hash%% *}" "${daemon_hash%% *}"
}

reuse=false
if [ -L "$cli_link" ] && [ -L "$daemon_link" ] &&
    [ -n "$revision" ] && [ -f "$revision_file" ] && [ ! -L "$revision_file" ] &&
    [ -f "$cli" ] && [ ! -L "$cli" ] && [ -x "$cli" ] &&
    [ -f "$daemon" ] && [ ! -L "$daemon" ] && [ -x "$daemon" ] &&
    [ "$(cat "$revision_file")" = "$(signature)" ]; then
    reuse=true
fi

if [ "$reuse" = false ] && [ "$prebuilt" = false ]; then
    if cargo=$(command -v cargo 2>/dev/null); then
        :
    elif [ -x "$user_home/.cargo/bin/cargo" ]; then
        cargo="$user_home/.cargo/bin/cargo"
    else
        fail "Rust with Cargo is required to build this source checkout"
    fi

    printf 'Building LAO release binaries...\n'
    (cd "$root" && "$cargo" build --release --locked --jobs 2 -p lao-cli -p lao-daemon)
elif [ "$reuse" = false ]; then
    printf 'Installing prebuilt LAO binaries...\n'
else
    printf 'Reusing verified LAO binaries.\n'
fi

if [ "$reuse" = false ]; then
    mkdir "$staging" "$staging/old" "$staging/new"
    for file in lao lao-daemon source-revision; do
        if [ -f "$prefix/$file" ]; then
            ln "$prefix/$file" "$staging/old/$file"
        else
            : >"$staging/old/$file.absent"
        fi
    done
    /usr/bin/install -m 700 "$binaries/lao" "$staging/new/lao"
    /usr/bin/install -m 700 "$binaries/lao-daemon" "$staging/new/lao-daemon"
    signature "$staging/new" >"$staging/new/source-revision"
    printf '1\n' >"$staging/format"
    printf '%s' "$bin_dir" | /usr/bin/shasum -a 256 >"$staging/bin-dir"
    printf '%s %s\n' "$cli_link_new" "$daemon_link_new" >"$staging/links"
    manifest "$staging" >"$staging/SHA256SUMS"
    mv "$staging" "$transaction"

    for file in lao lao-daemon source-revision; do
        if [ -f "$transaction/new/$file" ]; then
            ln "$transaction/new/$file" "$transaction/publish-$file"
            mv -f "$transaction/publish-$file" "$prefix/$file"
        else
            rm -f "$prefix/$file"
        fi
    done
fi

ensure_link "$cli_link" "$cli"
ensure_link "$daemon_link" "$daemon"
if [ "$reuse" = false ]; then mv "$transaction" "$staging"; fi

printf '\nLAO binaries are installed. Finish setup with: "%s" install\n' "$cli_link"
