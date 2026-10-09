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

## Release of 6870857: the store without WebMCP

Source `687085739e48c595a4a450349c1aef1c90b44820` (main), which includes `606a30c` "Remove WebMCP from the store".
The store no longer renders the inline WebMCP script, so no page registers `navigator.modelContext` tools; the
MCP server at `/mcp` is unchanged. Since `d5a264d` only the store, these records and the version numbers of the
CLI, client and package crates changed. The server code, `deploy/Caddyfile`, `deploy/install.py`,
`deploy/package.py`, `scripts/` and `runner/` are unchanged, the API still reports `0.1.2`, and the validation
worker was not touched.

### Checks before the release

- `CARGO_TARGET_DIR=target/integration cargo test -p silicon-apps-server --locked`: 53 tests passed, none failed.
- Store: `pnpm install --frozen-lockfile`, `pnpm typecheck`, `pnpm lint` and `pnpm build` passed. The build
  output under `store/.next/standalone` and `store/.next/static` held no `modelContext` or `webmcp`.
- Deploy tests with `/opt/homebrew/bin/python3` (3.14.6): 45 ran, 8 Caddy routing tests skipped; the 9 tests
  in `deploy/test_caddy.py` then passed against the local Caddy 2.11.4.
- API built with `cargo zigbuild --release --locked --target aarch64-unknown-linux-gnu.2.34 -p silicon-apps-server
  --bin apps-server` (`CARGO_TARGET_DIR=target/integration`, zig 0.15.2 from the uv-installed `ziglang` package
  through `CARGO_ZIGBUILD_PYTHON_PATH`). Highest glibc symbol: `GLIBC_2.34`.

### Archive

`deploy/package.py --caddy …/silicon-accounts/.dev/production/caddy_2.11.7_linux_arm64.tar.gz --api
target/integration/aarch64-unknown-linux-gnu/release/apps-server` built the store and wrote the archive:

| | |
|---|---|
| archive SHA-256 | `bbcaf9efb04f1e0594ea6f3ebc026c85410bfdeab03982f7304e8247a46f07e3` |
| size | 80,441,284 bytes, 1,428 files, 28 symlinks |
| API binary SHA-256 | `6d26a415926a6090e221478b9e3ceb6146d2b95b8f921e6e2a860fc8f7aaf55f` (only the package crate's version moved) |
| Node.js | 24.21.0 (pinned checksum) |
| store build | `Xs47GxeutOf_q3iKTdmKf`, built by the packager |

The installer's `prepare_release` extracted and verified it locally; the extracted store held no `modelContext`
or `webmcp` outside `node_modules`, and `web/install.sh` matched `scripts/install.sh`. Uploaded to
`releases/687085739e48c595a4a450349c1aef1c90b44820.tar.gz` (S3 version `X4PfKMfO5MFOKTVAgc7y1uUX_B6HmYXg`) in
about two minutes this time.

### Backups and install

On-demand backup first (SSM `7cb9e60f-c98f-4fd0-81df-168b6af69518`): `backups/20261009T101337Z.tar.gz`,
180,225,803 bytes, S3 version `kDiyRLbqLVQEsBqy3kVITLLKDkKSRydd`, confirmed with `head-object`. The catalog grew
since the morning with the CLI 0.2.0 and Accounts 0.4.0 packages, hence the larger backups. No secret was read
or changed.

SSM `2d5dbb09-0199-4189-b34b-1e0f7cadf1ab` downloaded the archive, checked its SHA-256 (`OK`) before extracting
anything, extracted only `deploy/install.py` and ran it with `--role api`. The candidate store rendered on
`127.0.0.1:4321`; the installer then uploaded `backups/20261009T101539Z.tar.gz` (180,225,816 bytes, S3 version
`bvFo7XGatYTC02Mvfs0fEM6njPzdeAXm`) after stopping the API and switched `current`. The API restarted at
10:15:47 UTC with no signing pass (every release was already signed) and the store at 10:15:48. `d5a264d` is
retained in `previous-release`. When the installer stopped the old store, systemd recorded its SIGTERM exit
(status 143) as `Failed with result 'exit-code'`; the new store process started normally and has not restarted.

Host checks, SSM `f236d59c-6308-4167-b6ba-45c3dc0a40db`: `current` and `deployment.json` name `6870857` and
the archive hash; `build.json` records store build `Xs47GxeutOf_q3iKTdmKf`; `silicon-apps-api`,
`silicon-apps-store` (as `silicon-apps-store`), `caddy` and `silicon-apps-backup.timer` are active and
enabled with no restarts; the preflight unit is inactive; the binary hash matches; `/health` is ok; the keys
document lists `apps-2026-10` active with public key `5WC06dtS61w+mPv3E0xNVz5ZoNQNMpX5/8YutuLZ3DU=` and
nothing revoked; `GET /` on `127.0.0.1:4320` answered 200 with 141,652 bytes, one `<main>`, no `modelContext`
and no `webmcp`. Log watch over five minutes (SSM `dfd818c7-9cab-4977-88ce-91b93de8576f` and
`bc96db7c-1935-41a5-8c5a-b95387ee39ae`): no API warnings or errors, store journal only its start lines, no
Caddy warnings or errors after the restart (Caddy keeps no access log), all restart counters 0.

### Public checks

All against `https://apps.teamofsilicons.com` with `curl --max-time`, before and after the release:

| path | before (d5a264d) | after (6870857) |
|---|---|---|
| `/` | 200, 148,615 bytes | 200, 141,652 bytes, one `<main>`, the same JSON-LD (`Organization`, `WebSite`, `WebPage`, `ItemList`, `FAQPage`), nonce CSP |
| `/search?q=accounts` | 200 | 200 HTML listing `silicon-accounts` |
| `/apps/silicon-accounts` | 200, 0.4.0 | 200, shows 0.4.0, `SoftwareApplication` JSON-LD, install command |
| `/apps/silicon-apps` | 200, 0.2.0 | 200, shows 0.2.0, `SoftwareApplication` JSON-LD, install command |
| `modelContext` in raw HTML of `/`, `/search`, `/apps/silicon-apps` | 14, 12, 12 | 0, 0, 0 |
| `webmcp` (case-insensitive) in the same pages | 2, 0, 0 | 0, 0, 0 |

After the release, unchanged from the morning: `/apps/no-such-app-xyz` 404 HTML; `/llms.txt` 200 `text/plain`
starting `# Silicon Apps`; `/robots.txt` 200; `/sitemap.xml` 200 `application/xml` with 10 URLs; `POST /mcp`
initialize 200 JSON-RPC result from `silicon-apps-store`, protocol `2025-06-18`; `/health` 200
`{"status":"ok","version":"0.1.2"}` with the strict CSP; `/openapi.json` 200, OpenAPI 3.1.0, API version
2026-10-09, 61 paths; `/v1/capabilities` 200; `/.well-known/agent.json` 200 with the strict CSP;
`/.well-known/silicon-apps-keys.json` 200 with `apps-2026-10` active; `/install.sh` and `/install.ps1` 200
`text/plain`, `no-cache`, byte-identical to `scripts/`; `/developer` and `/docs` 308 to the shared portal;
`/store/silicon-accounts` 308; a `/_next/static/` chunk named by the new home page 200 `immutable`, a missing one 404;
`/v1/events/stream` 401 JSON; `developers.teamofsilicons.com` 200. The API catalog still lists
`silicon-accounts` 0.4.0 and `silicon-apps` 0.2.0 in production, both signed. Five minutes later `/`, `/search`
and `/apps/silicon-apps` still answered 200 with no `modelContext` or `webmcp`.

Nothing was rolled back. `production.json` records the new identities; `PreSigningBackupKey` keeps the
pre-signing backup from the morning.
