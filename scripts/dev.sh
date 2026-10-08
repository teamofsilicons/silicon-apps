#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -f .env ]]; then set -a; source .env; set +a; fi
mkdir -p .dev/logs
cargo build --workspace
if [[ ! -d web/node_modules ]]; then npm --prefix web ci; fi
pids=()
stop() { for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done; }
trap stop EXIT INT TERM
target/debug/apps-server >.dev/logs/server.log 2>&1 & pids+=("$!")
if [[ -n "${APPS_RUNNER_TOKEN:-}" ]]; then
  if [[ ! -x .dev/runner-venv/bin/python ]]; then
    python3 -m venv .dev/runner-venv
    .dev/runner-venv/bin/pip install -r runner/requirements.txt
  fi
  .dev/runner-venv/bin/python runner/server.py >.dev/logs/runner.log 2>&1 & pids+=("$!")
fi
npm --prefix web run dev >.dev/logs/web.log 2>&1 & pids+=("$!")
printf 'Silicon Apps: http://127.0.0.1:4311/store\nDeveloper: http://127.0.0.1:4311/developer\nLogs: .dev/logs/\n'
while true; do
  for pid in "${pids[@]}"; do
    if ! kill -0 "$pid" 2>/dev/null; then
      printf 'A service stopped; inspect .dev/logs/.\n' >&2
      exit 1
    fi
  done
  sleep 2
done
