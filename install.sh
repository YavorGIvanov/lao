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
    for file in SHA256SUMS install.sh bin/lao bin/lao-daemon LICENSE source-revision; do
        [ -f "$root/$file" ] && [ ! -L "$root/$file" ] || fail "incomplete prebuilt archive"
    done
    [ ! -L "$binaries" ] || fail "prebuilt binaries must be in a real directory"
    expected=$(cd "$root" && /usr/bin/shasum -a 256 install.sh bin/lao bin/lao-daemon LICENSE source-revision)
    [ "$(cat "$root/SHA256SUMS")" = "$expected" ] || fail "prebuilt archive checksum mismatch"
    revision="prebuilt:$(/usr/bin/shasum -a 256 "$root/SHA256SUMS" | cut -d ' ' -f 1)"
elif command -v git >/dev/null 2>&1 &&
    [ -z "$(git -C "$root" status --porcelain --untracked-files=normal 2>/dev/null)" ]; then
    revision=$(git -C "$root" rev-parse --verify HEAD 2>/dev/null || :)
fi

signature() {
    printf '%s\n%s\n%s\n' \
        "$revision" \
        "$(/usr/bin/shasum -a 256 "$cli" | cut -d ' ' -f 1)" \
        "$(/usr/bin/shasum -a 256 "$daemon" | cut -d ' ' -f 1)"
}

reuse=false
if [ -n "$revision" ] && [ -f "$revision_file" ] && [ ! -L "$revision_file" ] &&
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

mkdir -p "$prefix" "$bin_dir"
if [ "$reuse" = false ]; then
    cli_pending="$prefix/.lao.$$"
    daemon_pending="$prefix/.lao-daemon.$$"
    revision_pending="$prefix/.source-revision.$$"
    cleanup() {
        rm -f -- "$cli_pending" "$daemon_pending" "$revision_pending"
    }
    trap cleanup EXIT
    trap 'exit 1' HUP INT TERM

    /usr/bin/install -m 700 "$binaries/lao" "$cli_pending"
    /usr/bin/install -m 700 "$binaries/lao-daemon" "$daemon_pending"
    mv -f "$cli_pending" "$cli"
    mv -f "$daemon_pending" "$daemon"
    if [ -n "$revision" ]; then
        signature >"$revision_pending"
        chmod 600 "$revision_pending"
        mv -f "$revision_pending" "$revision_file"
    else
        rm -f -- "$revision_file"
    fi
fi

ensure_link "$cli_link" "$cli"
ensure_link "$daemon_link" "$daemon"

printf '\nLAO binaries are installed. Finish setup with: "%s" install\n' "$cli_link"
