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

# R11: the archive carries the audited notices and checks their integrity before writes.
cmp "$root/THIRD_PARTY_NOTICES.txt" "$bundle/THIRD_PARTY_NOTICES.txt"
(cd "$bundle" && shasum -a 256 -c SHA256SUMS) >"$stage/checksums.log"
for invalid in missing corrupt symlink; do
    mv "$bundle/THIRD_PARTY_NOTICES.txt" "$stage/notices"
    case "$invalid" in
        missing) ;;
        corrupt) printf 'corrupt notices\n' >"$bundle/THIRD_PARTY_NOTICES.txt" ;;
        symlink) ln -s "$stage/notices" "$bundle/THIRD_PARTY_NOTICES.txt" ;;
    esac
    if sh "$bundle/install.sh" >"$stage/notices.log" 2>&1; then exit 1; fi
    if [ "$invalid" = corrupt ]; then reason='checksum mismatch'; else reason='incomplete prebuilt archive'; fi
    grep -q "$reason" "$stage/notices.log"
    [ ! -e "$LAO_PREFIX" ]
    [ ! -e "$LAO_BIN_DIR" ]
    rm -f "$bundle/THIRD_PARTY_NOTICES.txt"
    mv "$stage/notices" "$bundle/THIRD_PARTY_NOTICES.txt"
done
printf 'PASS: audited notices included; missing, corrupt or symlinked notices rejected before writes\n'

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

# R11: upgrade synthetic prior binaries, preserving unrelated files and repeat reuse.
(
    export LAO_PREFIX="$stage/upgrade/libexec"
    export LAO_BIN_DIR="$stage/upgrade/bin"
    mkdir -p "$LAO_PREFIX"
    install -m 700 /usr/bin/true "$LAO_PREFIX/lao"
    install -m 700 /usr/bin/false "$LAO_PREFIX/lao-daemon"
    printf 'previous test revision\n' >"$LAO_PREFIX/source-revision"
    printf 'preserve me\n' >"$LAO_PREFIX/keep"
    sh "$bundle/install.sh" >"$stage/upgrade.log"
    cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
    cmp "$bundle/bin/lao-daemon" "$LAO_PREFIX/lao-daemon"
    [ "$(cat "$LAO_PREFIX/keep")" = 'preserve me' ]
    upgraded_inode=$(stat -f %i "$LAO_PREFIX/lao")
    sh "$bundle/install.sh" >"$stage/upgrade-repeat.log"
    [ "$(stat -f %i "$LAO_PREFIX/lao")" = "$upgraded_inode" ]
    [ ! -e "$LAO_PREFIX/.install-pending" ]
)
printf 'PASS: archive upgrade from synthetic prior binaries and repeat installation\n'

# R11: failure after publishing the CLI restores the previous pair and identity.
mkdir "$stage/fault-tools"
cat >"$stage/fault-tools/mv" <<'SH'
#!/bin/sh
case "$2" in
    */.install-pending/publish-lao-daemon)
        if [ "$INSTALL_TEST_FAULT" = signal ]; then kill -TERM "$PPID"; fi
        if [ "$INSTALL_TEST_FAULT" = kill ] || [ "$INSTALL_TEST_FAULT" = conflict ]; then kill -KILL "$PPID"; fi
        if [ "$INSTALL_TEST_FAULT" = missing ]; then rm "$LAO_PREFIX/.install-pending/old/lao-daemon"; fi
        exit 1 ;;
    */.install-pending/restore-lao-daemon)
        if [ "$INSTALL_TEST_FAULT" = restore-kill ]; then kill -KILL "$PPID"; exit 1; fi ;;
    */.install-pending/restore-lao)
        if [ "$INSTALL_TEST_FAULT" = recovery ]; then exit 1; fi ;;
esac
exec /bin/mv "$@"
SH
chmod 700 "$stage/fault-tools/mv"
for fault in command signal recovery missing kill conflict; do
    (
        export LAO_PREFIX="$stage/failed-$fault/libexec"
        export LAO_BIN_DIR="$stage/failed-$fault/bin"
        mkdir -p "$LAO_PREFIX" "$LAO_BIN_DIR"
        install -m 700 /usr/bin/true "$LAO_PREFIX/lao"
        install -m 700 /usr/bin/false "$LAO_PREFIX/lao-daemon"
        printf 'previous test revision\n' >"$LAO_PREFIX/source-revision"
        printf 'preserve me\n' >"$LAO_PREFIX/keep"
        ln -s "$LAO_PREFIX/lao" "$LAO_BIN_DIR/lao"
        ln -s "$LAO_PREFIX/lao-daemon" "$LAO_BIN_DIR/lao-daemon"
        previous_inode=$(stat -f %i "$LAO_PREFIX/lao")
        if { PATH="$stage/fault-tools:$PATH" INSTALL_TEST_FAULT="$fault" \
            sh "$bundle/install.sh"; } >"$stage/failed-$fault.log" 2>&1; then exit 1; fi
        if [ "$fault" = kill ] || [ "$fault" = conflict ]; then
            cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
            cmp /usr/bin/false "$LAO_PREFIX/lao-daemon"
            if [ "$fault" = conflict ]; then
                printf 'unrelated replacement\n' >"$LAO_PREFIX/replacement"
                mv "$LAO_PREFIX/replacement" "$LAO_PREFIX/lao"
                if sh "$bundle/install.sh" >"$stage/conflict.log" 2>&1; then exit 1; fi
                grep -q 'binary recovery conflicts with installed lao' "$stage/conflict.log"
                [ "$(cat "$LAO_PREFIX/lao")" = 'unrelated replacement' ]
                cmp /usr/bin/false "$LAO_PREFIX/lao-daemon"
                [ "$(cat "$LAO_PREFIX/source-revision")" = 'previous test revision' ]
                [ -f "$LAO_PREFIX/.install-pending/SHA256SUMS" ]
                exit 0
            fi
            exec 8>>"$LAO_PREFIX/.install-lock"
            /usr/bin/lockf -s -t 0 8
            if sh "$bundle/install.sh" >"$stage/concurrent.log" 2>&1; then exit 1; fi
            grep -q 'another binary installer is running' "$stage/concurrent.log"
            exec 8>&-
            # Kill recovery itself after the first restore; another retry must be safe.
            if { PATH="$stage/fault-tools:$PATH" INSTALL_TEST_FAULT=restore-kill \
                sh "$bundle/install.sh"; } >"$stage/restore-kill.log" 2>&1; then exit 1; fi
            cmp /usr/bin/true "$LAO_PREFIX/lao"
            cmp /usr/bin/false "$LAO_PREFIX/lao-daemon"
            sh "$bundle/install.sh" >"$stage/kill-retry.log"
            grep -q 'Recovered interrupted binary installation' "$stage/kill-retry.log"
            cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
            cmp "$bundle/bin/lao-daemon" "$LAO_PREFIX/lao-daemon"
            [ ! -e "$LAO_PREFIX/.install-pending" ]
            [ ! -e "$LAO_PREFIX/.install-staging" ]
            exit 0
        fi
        if [ "$fault" = missing ]; then
            grep -q 'invalid binary recovery snapshots' "$stage/failed-$fault.log"
            cmp /usr/bin/true "$LAO_PREFIX/.install-pending/old/lao"
            if sh "$bundle/install.sh" >"$stage/blocked-retry.log" 2>&1; then exit 1; fi
            grep -q 'invalid binary recovery snapshots' "$stage/blocked-retry.log"
            cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
            cmp /usr/bin/false "$LAO_PREFIX/lao-daemon"
            exit 0
        fi
        if [ "$fault" = recovery ]; then
            grep -q 'rollback failed; snapshots retained' "$stage/failed-$fault.log"
            cmp /usr/bin/true "$LAO_PREFIX/.install-pending/old/lao"
            cmp /usr/bin/false "$LAO_PREFIX/.install-pending/old/lao-daemon"
            sh "$bundle/install.sh" >"$stage/recovered-retry.log"
            grep -q 'Recovered interrupted binary installation' "$stage/recovered-retry.log"
            cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
            cmp "$bundle/bin/lao-daemon" "$LAO_PREFIX/lao-daemon"
            exit 0
        fi
        cmp /usr/bin/true "$LAO_PREFIX/lao"
        cmp /usr/bin/false "$LAO_PREFIX/lao-daemon"
        [ "$(stat -f %i "$LAO_PREFIX/lao")" = "$previous_inode" ]
        [ "$(cat "$LAO_PREFIX/source-revision")" = 'previous test revision' ]
        [ "$(cat "$LAO_PREFIX/keep")" = 'preserve me' ]
        [ "$(readlink "$LAO_BIN_DIR/lao")" = "$LAO_PREFIX/lao" ]
        [ "$(readlink "$LAO_BIN_DIR/lao-daemon")" = "$LAO_PREFIX/lao-daemon" ]
        [ ! -e "$LAO_PREFIX/.install-pending" ]
        sh "$bundle/install.sh" >"$stage/retry-$fault.log"
        cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
        cmp "$bundle/bin/lao-daemon" "$LAO_PREFIX/lao-daemon"
    )
done
printf 'PASS: failed/interrupted binary upgrade restores prior binaries, identity and links; retry succeeds; SIGKILL recovery retries safely; conflicts retain snapshots\n'

printf 'local-test-key' >"$stage/runtime.key"
for client in codex claude; do
    mkdir "$stage/$client"
    ln -s /usr/bin/false "$stage/$client/$client"
    PATH="$stage/$client:$PATH" LAO_EXTERNAL_ADDR=127.0.0.1:9999 LAO_EXTERNAL_KEY_FILE="$stage/runtime.key" \
        "$LAO_BIN_DIR/lao" preview --router safe --runtime external >"$stage/preview.log"
    if [ "$client" = codex ]; then selected=Codex; unselected=Claude; else selected=Claude; unselected=Codex; fi
    grep -q "$selected settings:" "$stage/preview.log"
    if grep -q "$unselected settings:" "$stage/preview.log"; then exit 1; fi
done
printf 'PASS: CLI auto-detects either harness with the other absent from PATH\n'

printf '\ncorrupt\n' >>"$bundle/bin/lao-daemon"
if sh "$bundle/install.sh" >"$stage/corrupt.log" 2>&1; then
    exit 1
fi
grep -q 'checksum mismatch' "$stage/corrupt.log"
cmp "$bundle/bin/lao" "$LAO_PREFIX/lao"
[ "$(stat -f %i "$LAO_PREFIX/lao")" = "$inode" ]
cmp "$root/target/release/lao-daemon" "$LAO_PREFIX/lao-daemon"
printf 'PASS: corrupt archive rejected without changing installed binaries\n'
