# Silicon Apps release 2026-10-09: signed releases, events and the server-rendered store

Source `d5a264df9855544bf20acb936234a5631ffff727` (main). The Apps API now signs every package it serves
with Ed25519, applies the built-in migrations `002_events` and `003_signing` on start, and the store is the
server-rendered Next.js store, run by the new `silicon-apps-store` systemd service on `127.0.0.1:4320`
behind Caddy 2.11.7. The API version string stays `0.1.2`. The validation worker is unchanged: `runner/`
has not changed since its deployed source `b2a3db8e575701575dfc283ba8bab0ed51a1cb60`, PRODUCTION.md does
not require a lockstep worker release, and the new API reports the worker configured and reachable.

## Checks before the release

- `CARGO_TARGET_DIR=target/integration cargo test --workspace --locked`: 103 tests passed, none failed.
- Store: `pnpm install --frozen-lockfile`, `pnpm typecheck` and `pnpm lint` passed; the tree stayed clean.
- Deploy tests with Homebrew Python 3.14: 45 ran, 8 Caddy routing tests skipped without `APPS_TEST_CADDY`.
  All 9 tests in `deploy/test_caddy.py` then passed against a local Caddy 2.11.4 (the only macOS Caddy at hand).
- API built with `cargo zigbuild --release --locked --target aarch64-unknown-linux-gnu.2.34`, zig 0.15.2 from
  the `ziglang` Python package (`CARGO_ZIGBUILD_PYTHON_PATH`). Highest glibc symbol: `GLIBC_2.34`.

## Archive

`python3 deploy/package.py --caddy caddy_2.11.7_linux_arm64.tar.gz --api target/integration/aarch64-unknown-linux-gnu/release/apps-server`
built the store and wrote the archive:

| | |
|---|---|
| archive SHA-256 | `a4165f5e71fac0e6bfde9557d8e2fccd8b21fcee37bf89593614eea58e44fffe` |
| size | 80,440,262 bytes, 1,426 files, 28 symlinks |
| API binary SHA-256 | `65129f9690374342236327129082a083fc2621ba936cfe8365d0098487dd28e3` |
| Node.js | 24.21.0 (pinned checksum, reused from `silicon-accounts/.dev/production`) |
| store build | `tF3CwAi7OYA7_dw2cE1TS`, built by the packager |

The archive was extracted and verified locally with the installer's own `prepare_release` before upload,
then uploaded to `releases/d5a264df9855544bf20acb936234a5631ffff727.tar.gz` (S3 version
`B10ee_vLntmImUYSPKCtjKKmNQgKZgTc`).

## Signing key and backups

`APPS_SIGNING_KEYS` was added to the runtime secret from the release key file without displaying it. All
14 existing fields were read back unchanged (secret version `b54319c7-d928-43f0-85d4-7be83dd4a5f0`).
`APPS_ALLOWED_ORIGINS` already included `https://apps.teamofsilicons.com`. The installer's own secret checks
passed locally before the install, including the seed matching the pinned public key.

On-demand backup before the install: `backups/20261009T070318Z.tar.gz`, 103,561,569 bytes, S3 version
`Zc8XKT07dwurMJzbV6f4GAsO89Y.Pusv` (SSM `36c9c1a9-d5cb-429b-a502-61325e4e6ab7`). The installer took another
after stopping the API: `backups/20261009T073312Z.tar.gz`, 103,561,564 bytes. Either restores the catalog
from before signatures and events, together with archive `8299691f859fb91c5599ea58a144c71d20712660`.

## Install

SSM `922cd178-f493-45a1-8135-73758deed13c` downloaded the archive, checked its SHA-256 before extracting
anything, extracted only `deploy/install.py` and ran it with `--role api`. The candidate store rendered on
`127.0.0.1:4321` before the cutover. The API started in two seconds and logged
`Release signing: read 25 package install scripts and signed 50 release packages with apps-2026-10.`
The previous release, `8299691f859fb91c5599ea58a144c71d20712660`, is retained in `previous-release`.

Host checks, SSM `9ce54adb-3346-47f1-b0ef-2428531be994`: `current` and `deployment.json` name the new
revision and archive hash; `silicon-apps-api`, `silicon-apps-store` (as `silicon-apps-store`), `caddy` and
`silicon-apps-backup.timer` are active and enabled with no restarts; the preflight unit is gone; the binary
hash matches; `/health` is ok; the keys document lists `apps-2026-10` active with public key
`5WC06dtS61w+mPv3E0xNVz5ZoNQNMpX5/8YutuLZ3DU=`; `GET /` on `127.0.0.1:4320` answered 200 with 148,613
bytes of rendered HTML. Over the next eight minutes (SSM `8088a508-a426-4c8d-a15a-1d1190fb3c73` and
`81399f56-8c10-4821-835f-7bb166f46d04`) the API and store journals held no warnings or errors, neither
service restarted, and Caddy logged only the old process exiting on its restart.

## Public checks

All against `https://apps.teamofsilicons.com` with `curl --max-time`:

| path | result |
|---|---|
| `/` | 200 HTML, 148,613 bytes: one `<main>`, JSON-LD (`Organization`, `WebSite`, `WebPage`, `ItemList`, `FAQPage`), h1 "Apps for Carbons and Silicons", the catalog apps and `silicon-apps install` in the raw HTML; CSP `script-src 'self' 'nonce-…' 'strict-dynamic'` with the nonce on all 13 scripts |
| `/search?q=accounts` | 200 HTML listing `silicon-accounts` |
| `/apps/silicon-apps`, `/apps/silicon-accounts` | 200 HTML from the live catalog, `<main>`, `SoftwareApplication` JSON-LD, the install command |
| `/apps/no-such-app-xyz` | 404 HTML page |
| `/llms.txt`, `/llms-full.txt` | 200 `text/plain`, first line `# Silicon Apps` |
| `/robots.txt`, `/sitemap.xml` | 200 `text/plain`, 200 `application/xml` with 10 URLs |
| `POST /mcp` initialize | 200 JSON-RPC result, server `silicon-apps-store`, protocol `2025-06-18` |
| `/health` | 200 `{"status":"ok","version":"0.1.2"}`, strict CSP |
| `/openapi.json` | 200 JSON, OpenAPI 3.1.0, API version 2026-10-09, 61 paths |
| `/v1/capabilities` | 200 JSON; validation runner configured and reachable |
| `/.well-known/agent.json`, `agent-card.json` | 200 JSON, strict CSP |
| `/.well-known/silicon-apps-keys.json` | 200 JSON, `apps-2026-10` active with the pinned public key, nothing revoked |
| `/install.sh`, `/install.ps1` | 200 `text/plain`, `no-cache`, byte-identical to `scripts/` |
| `/developer`, `/docs` | 308 to `https://developers.teamofsilicons.com/` and `/docs/apps` |
| `/store/silicon-accounts` | 308 to `/apps/silicon-accounts` |
| `/_next/static/…` | real file 200 `immutable`; missing file 404 |
| `/v1/events/stream`, `/v1/apps/silicon-apps/events/stream` | 401 JSON `authentication_required` without a session, never HTML |
| `http://apps.teamofsilicons.com/` | 308 to HTTPS |
| `developers.teamofsilicons.com` | 200; `developer.teamofsilicons.com` 308 to it |

The first public `GET /` right after the cutover stopped after 43,433 bytes at the 30-second limit. Every
later request rendered in about two seconds, and no log showed an error; it was most likely this operator's
slow network, which also made the 80 MB archive upload take about 17 minutes.

## Clients

In a temporary `SILICON_HOME`, the live `/install.sh` (byte-identical to source) installed the released CLI
0.1.10 from GitHub with `--no-startup --no-path`. With it, `silicon-apps install silicon-accounts` installed
0.3.1, `silicon-accounts --version` printed `silicon-accounts 0.3.1`, `silicon-apps search accounts` returned
the catalog entry, and its updater check reported `silicon-apps` current. The CLI built from this revision
installed `silicon-accounts` 0.3.1 (macos-aarch64) in another fresh home and recorded
`signature_key_id: apps-2026-10`, so a fresh install verifies the release signature. Both temporary updaters
were stopped afterwards.

Nothing was rolled back. See `production.json` for the deployment identities.
