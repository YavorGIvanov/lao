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
home=${HOME:?HOME is required}
prefix=${LAO_PREFIX:-"$home/.local/libexec/lao"}
bin_dir=${LAO_BIN_DIR:-"$home/.cargo/bin"}

for path in "$prefix" "$bin_dir"; do
    case "$path" in
        /*) ;;
        *) fail "install paths must be absolute" ;;
    esac
    if [ "$path" = "/" ] || [ "$path" = "$home" ]; then
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
check_link "$cli_link" "$cli"
check_link "$daemon_link" "$daemon"

if cargo=$(command -v cargo 2>/dev/null); then
    :
elif [ -x "$home/.cargo/bin/cargo" ]; then
    cargo="$home/.cargo/bin/cargo"
else
    fail "Rust with Cargo is required to build this source checkout"
fi

printf 'Building LAO release binaries...\n'
(cd "$root" && "$cargo" build --release --locked -p lao-cli -p lao-daemon)

mkdir -p "$prefix" "$bin_dir"
cli_pending="$prefix/.lao.$$"
daemon_pending="$prefix/.lao-daemon.$$"
cleanup() {
    rm -f -- "$cli_pending" "$daemon_pending"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

/usr/bin/install -m 700 "$root/target/release/lao" "$cli_pending"
/usr/bin/install -m 700 "$root/target/release/lao-daemon" "$daemon_pending"
mv -f "$cli_pending" "$cli"
mv -f "$daemon_pending" "$daemon"

ensure_link "$cli_link" "$cli"
ensure_link "$daemon_link" "$daemon"

printf '\nLAO is ready. Finish setup with: lao install\n'
