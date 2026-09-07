#!/bin/sh
set -eu
umask 077

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
archive="$root/target/release/lao-macos-arm64.tar.gz"
[ -f "$archive" ] || { printf 'Run sh package.sh first\n' >&2; exit 1; }
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
trap 'exit 1' HUP INT TERM
tar -xzf "$archive" -C "$stage"
bundle="$stage/lao-macos-arm64"
export LAO_PREFIX="$stage/user home/libexec/lao"
export LAO_BIN_DIR="$stage/user home/bin"
# Only stock OS tools are available; never activate clients or models in this test.
export PATH=/usr/bin:/bin:/usr/sbin:/sbin

sh "$bundle/install.sh" >"$stage/install.log"
cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
cmp "$bundle/bin/lao-daemon" "$LAO_PREFIX/lao-daemon"
[ "$(readlink "$LAO_BIN_DIR/lao")" = "$LAO_PREFIX/lao" ]
[ "$(readlink "$LAO_BIN_DIR/lao-daemon")" = "$LAO_PREFIX/lao-daemon" ]
inode=$(stat -f %i "$LAO_PREFIX/lao")
sh "$bundle/install.sh" >"$stage/repeat.log"
[ "$(stat -f %i "$LAO_PREFIX/lao")" = "$inode" ]
"$LAO_BIN_DIR/lao" >"$stage/cli.log" 2>&1
grep -q 'usage: lao' "$stage/cli.log"
printf 'PASS: prebuilt install and repeat install without source tooling; CLI starts\n'

printf '\ncorrupt\n' >>"$bundle/bin/lao-daemon"
if sh "$bundle/install.sh" >"$stage/corrupt.log" 2>&1; then
    exit 1
fi
grep -q 'checksum mismatch' "$stage/corrupt.log"
cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
[ "$(stat -f %i "$LAO_PREFIX/lao")" = "$inode" ]
cmp "$root/target/release/lao-daemon" "$LAO_PREFIX/lao-daemon"
printf 'PASS: corrupt archive rejected without changing installed binaries\n'
