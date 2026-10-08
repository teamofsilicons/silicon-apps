#!/usr/bin/env sh
# Bootstrap ONLY the checksum-verified Apps binary, then delegate validation/install to Rust.
set -eu
ARCHIVE=""; DIGEST=""; VERSION=""; NO_STARTUP=0; APPS_HOME="${SILICON_HOME:-$HOME}"; SERVER="${APPS_URL:-https://apps.teamofsilicons.com}"
RELEASES="${APPS_RELEASES_URL:-https://github.com/teamofsilicons/silicon-apps/releases}"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --archive) ARCHIVE="${2:?--archive requires a path}"; shift 2 ;;
    --sha256) DIGEST="${2:?--sha256 requires a trusted checksum}"; shift 2 ;;
    --version) VERSION="${2:?--version requires x.y.z}"; shift 2 ;;
    --home) APPS_HOME="${2:?--home requires an existing directory}"; shift 2 ;;
    --server) SERVER="${2:?--server requires a URL}"; shift 2 ;;
    --no-startup) NO_STARTUP=1; shift ;;
    --help|-h) echo "Usage: install.sh [--version x.y.z] [--home DIRECTORY] [--server URL] [--archive FILE --sha256 TRUSTED_DIGEST] [--no-startup]"; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done
[ -d "$APPS_HOME" ] || { echo "$APPS_HOME: not a directory" >&2; exit 2; }
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) TARGET=macos-aarch64 ;;
  Darwin-x86_64) TARGET=macos-x86_64 ;;
  Linux-x86_64) TARGET=linux-x86_64 ;;
  Linux-i686|Linux-i386) TARGET=linux-i686 ;;
  Linux-aarch64|Linux-arm64) TARGET=linux-aarch64 ;;
  Linux-armv7l) TARGET=linux-armv7hf ;;
  *) echo "Unsupported host. Windows: use scripts/install.ps1. Other hosts: choose a supported release explicitly." >&2; exit 2 ;;
esac
TEMP="$(mktemp -d "${TMPDIR:-/tmp}/apps-bootstrap.XXXXXXXX")"
trap 'rm -rf "$TEMP"' EXIT HUP INT TERM
if [ -z "$ARCHIVE" ]; then
  case "$RELEASES" in https://*) ;; *) echo "APPS_RELEASES_URL must use HTTPS" >&2; exit 2 ;; esac
  if [ -z "$VERSION" ]; then
    EFFECTIVE="$(curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --silent --show-error --location --output /dev/null --write-out '%{url_effective}' "$RELEASES/latest")"
    VERSION="${EFFECTIVE##*/}"; VERSION="${VERSION#v}"
  fi
  case "$VERSION" in ''|*[!0-9.]*|.*|*..*|*.) echo "Invalid release version: $VERSION" >&2; exit 2 ;; esac
  ARCHIVE="$TEMP/apps-$VERSION-$TARGET.tar.gz"
  URL="$RELEASES/download/v$VERSION/apps-$VERSION-$TARGET.tar.gz"
  curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --silent --show-error --location "$URL" --output "$ARCHIVE"
  curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --silent --show-error --location "$URL.sha256" --output "$TEMP/checksum"
  DIGEST="$(awk 'NR==1 {print $1}' "$TEMP/checksum")"
else
  [ -f "$ARCHIVE" ] || { echo "Archive not found: $ARCHIVE" >&2; exit 2; }
  [ -n "$DIGEST" ] || { echo "--archive requires --sha256 from a trusted source" >&2; exit 2; }
fi
[ "${#DIGEST}" -eq 64 ] || { echo "SHA-256 must contain 64 hexadecimal characters" >&2; exit 2; }
case "$DIGEST" in *[!a-fA-F0-9]*) echo "Invalid SHA-256" >&2; exit 2 ;; esac
if command -v sha256sum >/dev/null 2>&1; then ACTUAL="$(sha256sum "$ARCHIVE" | awk '{print $1}')"; else ACTUAL="$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')"; fi
EXPECTED="$(printf '%s' "$DIGEST" | tr A-F a-f)"
[ "$ACTUAL" = "$EXPECTED" ] || { echo "Checksum mismatch. Nothing was executed or installed." >&2; exit 1; }
# Stream a single known archive member into a fresh file; never extract archive paths or links.
tar -xOzf "$ARCHIVE" bin/apps > "$TEMP/apps"
[ -s "$TEMP/apps" ] || { echo "Release does not contain bin/apps" >&2; exit 1; }
chmod 700 "$TEMP/apps"
"$TEMP/apps" --home "$APPS_HOME" --server "$SERVER" install apps --archive "$ARCHIVE" --sha256 "$EXPECTED"
if [ "$NO_STARTUP" -eq 0 ]; then
  "$APPS_HOME/.apps/bin/apps" --home "$APPS_HOME" daemon install
fi
printf '\nAdd this directory to PATH: %s/.apps/bin\n' "$APPS_HOME"
printf 'Check automatic updates with: apps daemon status\n'
if [ "$NO_STARTUP" -eq 1 ]; then printf 'Startup service skipped. Enable it later with: apps daemon install\n'; fi
