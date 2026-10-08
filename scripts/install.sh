#!/usr/bin/env sh
# Bootstrap ONLY the checksum-verified Apps binary, then delegate validation/install to Rust.
set -eu
ARCHIVE=""; DIGEST=""; VERSION=""; NO_STARTUP=0; NO_PATH=0; APPS_HOME="${SILICON_HOME:-$HOME}"; SERVER="${APPS_URL:-https://apps.teamofsilicons.com}"
RELEASES="${APPS_RELEASES_URL:-https://github.com/teamofsilicons/silicon-apps/releases}"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --archive) ARCHIVE="${2:?--archive requires a path}"; shift 2 ;;
    --sha256) DIGEST="${2:?--sha256 requires a trusted checksum}"; shift 2 ;;
    --version) VERSION="${2:?--version requires x.y.z}"; shift 2 ;;
    --home) APPS_HOME="${2:?--home requires an existing directory}"; shift 2 ;;
    --server) SERVER="${2:?--server requires a URL}"; shift 2 ;;
    --no-startup) NO_STARTUP=1; shift ;;
    --no-path) NO_PATH=1; shift ;;
    --help|-h) echo "Usage: install.sh [--version x.y.z] [--home DIRECTORY] [--server URL] [--archive FILE --sha256 TRUSTED_DIGEST] [--no-startup] [--no-path]"; exit 0 ;;
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
COMMAND=silicon-apps
if ! tar -xOzf "$ARCHIVE" bin/silicon-apps > "$TEMP/apps" 2>/dev/null; then
  COMMAND=apps
  tar -xOzf "$ARCHIVE" bin/apps > "$TEMP/apps"
fi
[ -s "$TEMP/apps" ] || { echo "Release does not contain bin/apps" >&2; exit 1; }
chmod 700 "$TEMP/apps"
"$TEMP/apps" --home "$APPS_HOME" --server "$SERVER" install apps --archive "$ARCHIVE" --sha256 "$EXPECTED"
CLI="$APPS_HOME/.apps/bin/$COMMAND"
BIN_DIR="$(cd "$APPS_HOME/.apps/bin" && pwd)"
# Quote as shell data, including homes containing spaces or apostrophes.
quote_shell() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
PATH_LINE="export PATH=$(quote_shell "$BIN_DIR"):\"\$PATH\""
PROFILE_LINE="case \":\$PATH:\" in *:$(quote_shell "$BIN_DIR"):*) ;; *) $PATH_LINE ;; esac"
if [ "$NO_PATH" -eq 0 ]; then
  PROFILE=""
  USER_SHELL="${SHELL:-sh}"
  case "${USER_SHELL##*/}" in
    zsh) PROFILE="${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash) PROFILE="$HOME/.bashrc" ;;
    sh|dash|ksh) PROFILE="$HOME/.profile" ;;
  esac
  if [ -n "$PROFILE" ]; then
    mkdir -p "$(dirname "$PROFILE")"
    if ! grep -Fqx "$PROFILE_LINE" "$PROFILE" 2>/dev/null; then
      printf '\n# Silicon Apps command-line tools\n%s\n' "$PROFILE_LINE" >> "$PROFILE"
    fi
    printf '\nConfigured PATH in %s.\n' "$PROFILE"
  fi
fi
printf '\nTo use %s in this terminal now, run:\n  %s\n  %s --help\n' "$COMMAND" "$PATH_LINE" "$COMMAND"
if [ "$NO_STARTUP" -eq 0 ]; then
  # Older releases start a detached updater during install, then give it only
  # three seconds to stop. Wait for the in-flight operation before registering.
  "$CLI" --home "$APPS_HOME" daemon stop >/dev/null
  WAITED=0
  while "$CLI" --home "$APPS_HOME" --json daemon status | grep -Eq '"running"[[:space:]]*:[[:space:]]*true'; do
    if [ "$WAITED" -ge 60 ]; then break; fi
    sleep 1
    WAITED=$((WAITED + 1))
  done
  if ! "$CLI" --home "$APPS_HOME" daemon install; then
    printf '\n%s is installed. Startup registration is incomplete; retry: %s daemon install\n' "$COMMAND" "$COMMAND" >&2
  fi
fi
printf '\nCheck automatic updates with: %s daemon status\n' "$COMMAND"
if [ "$NO_STARTUP" -eq 1 ]; then printf 'Startup service skipped. Enable it later with: %s daemon install\n' "$COMMAND"; fi
