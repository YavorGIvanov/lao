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

# A second installer must not replace files while another owns rollback snapshots.
mkdir -p "$prefix" "$bin_dir"
transaction="$prefix/.install-pending"
mkdir "$transaction" 2>/dev/null || fail "concurrent or unfinished install; inspect $transaction before retrying"
updating=false
original_files=
cli_link_new=true
daemon_link_new=true
[ ! -L "$cli_link" ] || cli_link_new=false
[ ! -L "$daemon_link" ] || daemon_link_new=false
cleanup() {
    result=$?
    trap - EXIT
    trap '' HUP INT TERM
    if [ "$result" -ne 0 ]; then
        if [ "$updating" = true ]; then
            for file in $original_files; do
                [ -f "$transaction/old/$file" ] && [ ! -L "$transaction/old/$file" ] || {
                    printf 'LAO setup: rollback snapshot missing; retained state in %s\n' "$transaction" >&2
                    exit 1
                }
            done
            for file in lao lao-daemon source-revision; do
                if [ -f "$transaction/old/$file" ]; then
                    ln "$transaction/old/$file" "$transaction/restore-$file" &&
                        mv -f "$transaction/restore-$file" "$prefix/$file" || {
                        printf 'LAO setup: rollback failed; snapshots retained in %s\n' "$transaction" >&2
                        exit 1
                    }
                else
                    rm -f "$prefix/$file" || exit 1
                fi
            done
        fi
        if [ "$cli_link_new" = true ] && [ -L "$cli_link" ] &&
            [ "$(readlink "$cli_link")" = "$cli" ]; then
            rm "$cli_link" || exit 1
        fi
        if [ "$daemon_link_new" = true ] && [ -L "$daemon_link" ] &&
            [ "$(readlink "$daemon_link")" = "$daemon" ]; then
            rm "$daemon_link" || exit 1
        fi
    fi
    rm -rf -- "$transaction"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
check_link "$cli_link" "$cli"
check_link "$daemon_link" "$daemon"
for file in lao lao-daemon source-revision; do
    [ ! -L "$prefix/$file" ] && { [ ! -e "$prefix/$file" ] || [ -f "$prefix/$file" ]; } ||
        fail "installed $file is not a regular file"
done

signature() {
    signature_dir=${1:-"$prefix"}
    cli_hash=$(/usr/bin/shasum -a 256 "$signature_dir/lao") || return 1
    daemon_hash=$(/usr/bin/shasum -a 256 "$signature_dir/lao-daemon") || return 1
    printf '%s\n%s\n%s\n' "$revision" "${cli_hash%% *}" "${daemon_hash%% *}"
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

if [ "$reuse" = false ]; then
    mkdir "$transaction/old" "$transaction/new"
    for file in lao lao-daemon source-revision; do
        if [ -f "$prefix/$file" ]; then
            original_files="$original_files $file"
            ln "$prefix/$file" "$transaction/old/$file"
        fi
    done
    /usr/bin/install -m 700 "$binaries/lao" "$transaction/new/lao"
    /usr/bin/install -m 700 "$binaries/lao-daemon" "$transaction/new/lao-daemon"
    if [ -n "$revision" ]; then
        signature "$transaction/new" >"$transaction/new/source-revision"
    fi

    updating=true
    mv -f "$transaction/new/lao" "$cli"
    mv -f "$transaction/new/lao-daemon" "$daemon"
    if [ -n "$revision" ]; then
        mv -f "$transaction/new/source-revision" "$revision_file"
    else
        rm -f -- "$revision_file"
    fi
fi

ensure_link "$cli_link" "$cli"
ensure_link "$daemon_link" "$daemon"
updating=false

printf '\nLAO binaries are installed. Finish setup with: "%s" install\n' "$cli_link"
