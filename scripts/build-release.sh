#!/usr/bin/env bash
# Build one target's immutable, checksum-addressed Apps release package.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
TARGET=""; OUTPUT="$ROOT/dist"; VERSION=""; BINARY=""
while (($#)); do
  case "$1" in
    --target) TARGET="${2:?--target requires a value}"; shift 2 ;;
    --output) OUTPUT="${2:?--output requires a directory}"; shift 2 ;;
    --version) VERSION="${2:?--version requires x.y.z}"; shift 2 ;;
    --binary) BINARY="${2:?--binary requires a prebuilt executable}"; shift 2 ;;
    --help|-h) echo "Usage: scripts/build-release.sh --target TARGET [--output dist] [--version x.y.z] [--binary PREBUILT]"; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done
if [[ -z "$TARGET" ]]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) TARGET=macos-aarch64 ;;
    Darwin-x86_64) TARGET=macos-x86_64 ;;
    Linux-x86_64) TARGET=linux-x86_64 ;;
    Linux-aarch64|Linux-arm64) TARGET=linux-aarch64 ;;
    Linux-i686|Linux-i386) TARGET=linux-i686 ;;
    Linux-armv7l) TARGET=linux-armv7hf ;;
    MINGW*-x86_64|MSYS*-x86_64) TARGET=windows-x86_64 ;;
    *) echo "Cannot infer this target; pass --target from apps targets." >&2; exit 2 ;;
  esac
fi
case "$TARGET" in
  linux-x86_64) TRIPLE=x86_64-unknown-linux-gnu ;;
  linux-i686) TRIPLE=i686-unknown-linux-gnu ;;
  linux-aarch64) TRIPLE=aarch64-unknown-linux-gnu ;;
  linux-armv7hf) TRIPLE=armv7-unknown-linux-gnueabihf ;;
  windows-x86_64) TRIPLE=x86_64-pc-windows-msvc ;;
  windows-i686) TRIPLE=i686-pc-windows-msvc ;;
  windows-aarch64) TRIPLE=aarch64-pc-windows-msvc ;;
  macos-x86_64) TRIPLE=x86_64-apple-darwin ;;
  macos-aarch64) TRIPLE=aarch64-apple-darwin ;;
  *) echo "Unsupported target: $TARGET" >&2; exit 2 ;;
esac
if [[ -z "$VERSION" ]]; then
  VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "silicon-apps-cli"))')"
fi
if [[ ! "$VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then echo "Version must be x.y.z: $VERSION" >&2; exit 2; fi
NAME=silicon-apps
[[ "$TARGET" == windows-* ]] && NAME=silicon-apps.exe
if [[ -z "$BINARY" ]]; then
  cargo build --locked --release --target "$TRIPLE" -p silicon-apps-cli
  BINARY="$ROOT/target/$TRIPLE/release/$NAME"
fi
[[ -f "$BINARY" ]] || { echo "Built executable not found: $BINARY" >&2; exit 1; }
mkdir -p "$OUTPUT"
OUTPUT="$(cd "$OUTPUT" && pwd)"
STAGING="$(mktemp -d "${TMPDIR:-/tmp}/apps-release.XXXXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
mkdir -p "$STAGING/bin"
cp "$BINARY" "$STAGING/bin/$NAME"
chmod 755 "$STAGING/bin/$NAME"
cat > "$STAGING/apps.yaml" <<EOF
schema_version: 1
app_id: silicon-apps
version: $VERSION
command: silicon-apps
targets:
  $TARGET:
    binary: bin/$NAME
EOF
ARCHIVE="$OUTPUT/apps-$VERSION-$TARGET.tar.gz"
cargo run --locked --quiet -p silicon-apps-cli -- pack "$STAGING" --output "$ARCHIVE"
python3 - "$ARCHIVE" <<'PY'
import hashlib,pathlib,sys
p=pathlib.Path(sys.argv[1]); checksum=hashlib.sha256(p.read_bytes()).hexdigest()
sidecar=p.with_name(p.name+'.sha256'); sidecar.write_text(checksum+'  '+p.name+'\n')
print(sidecar)
PY
