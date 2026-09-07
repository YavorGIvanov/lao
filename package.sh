#!/bin/sh
set -eu
umask 077

case "$(uname -s):$(uname -m)" in
    Darwin:arm64) ;;
    *) printf 'LAO packaging requires Apple Silicon macOS\n' >&2; exit 1 ;;
esac

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
cd "$root"
cargo build --release --locked --jobs 2 -p lao-cli -p lao-daemon

# A prebuilt archive must not rely on the builder's Homebrew or Rust libraries.
for binary in target/release/lao target/release/lao-daemon; do
    /usr/bin/lipo "$binary" -verify_arch arm64
    libraries=$(/usr/bin/otool -L "$binary")
    printf '%s\n' "$libraries" | awk '
        NR > 1 && $1 !~ /^\/usr\/lib\// && $1 !~ /^\/System\/Library\// { exit 1 }
    '
done

stage=$(mktemp -d "$root/target/release/.package.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
trap 'exit 1' HUP INT TERM
bundle="$stage/lao-macos-arm64"
mkdir -p "$bundle/bin"
/usr/bin/install -m 700 target/release/lao target/release/lao-daemon "$bundle/bin/"
/usr/bin/install -m 700 install.sh "$bundle/install.sh"
/usr/bin/install -m 600 LICENSE "$bundle/LICENSE"
git rev-parse --verify HEAD >"$bundle/source-revision"
if [ -n "$(git status --porcelain --untracked-files=normal)" ]; then
    printf 'modified working tree\n' >>"$bundle/source-revision"
fi
(cd "$bundle" && /usr/bin/shasum -a 256 install.sh bin/lao bin/lao-daemon LICENSE source-revision >SHA256SUMS)
COPYFILE_DISABLE=1 tar -czf "$stage/lao-macos-arm64.tar.gz" -C "$stage" lao-macos-arm64
mv -f "$stage/lao-macos-arm64.tar.gz" target/release/lao-macos-arm64.tar.gz
(cd target/release && /usr/bin/shasum -a 256 lao-macos-arm64.tar.gz >lao-macos-arm64.tar.gz.sha256)
printf 'Created target/release/lao-macos-arm64.tar.gz (unsigned research preview)\n'
