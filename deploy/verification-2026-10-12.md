# Production verification, 2026-10-12

## Release 9332344: no telemetry (API 0.2.0, CLI 0.3.0)

PR #16, at the Carbon's request ("remove telemetry from the services and CLIs too, no telemetry"). The server, client,
CLI and runner record and send nothing: the Space Station dependency is gone, `POST /v1/telemetry` answers 404,
`X-Apps-Telemetry` is accepted with any value and ignored, old `telemetry` outbox rows are marked delivered without
being sent, and `silicon-apps config telemetry …` is a hidden no-op. No migration.

API and store: CI-built binary `apps-server-linux-aarch64-2d92cb0…` (server-artifact run 38175369836; the merge commit
has the same tree), binary SHA-256 `fe61aba873a024c968ae17d197775981d40fa2fe3177c455f3ac5ed155c13404`, packaged with the
pinned Caddy 2.11.7 and Node.js 24.21.0. Archive `releases/9332344aed5b2f4fd45e0d93d308964c45e28969.tar.gz`, SHA-256
`f85721370d7ff871b574d25f12ac1ba683b7af238e142bc43adbd5ebbe5713b8`, 96,673,117 bytes, S3 version
`81f7kDSQ9NPTlEx7tO5pzQ_aWqgRAiyF`; store build `1Qh2-Nntaqd33V3zV8qE8`. SSM `a78bc1fe-b16c-403b-a384-a3c1a5f52355`,
installer backup `backups/20261011T213201Z.tar.gz`, previous release `94c2a40` retained.

Live: `/health` reports 0.2.0; `POST /v1/telemetry` 404; a read with `X-Apps-Telemetry: on` 200; `/` and `/apps/commit`
200. The API logs `Ignoring retired settings: APPS_TELEMETRY_ENABLED, APPS_TELEMETRY_TABLE_KEY` at boot: both keys are
still in the runtime secret and are ignored.

## Native CLI publications

All with `silicon-apps-api` stopped, `silicon-apps-backup.service` run before and after, and each bundle verified as
`silicon-apps` before the catalog changed. Every app is published on its development and production channels.

| App | Version | Targets | Source | Native evidence | SSM |
|---|---|---|---|---|---|
| commit | 0.6.0 | 6 | `47f30a3` | silicon-commit run 38175162023 | `4098b801-fa30-4a5a-969e-b40be22894d4` |
| remind | 0.7.0 | 6 | `73e4bc8` | silicon-remind run 38175165131 | `4098b801-…` |
| hook | 1.1.0 | 6 | `b42bcbf` | silicon-hook run 38175169757 | `4098b801-…` |
| briefcase | 4.1.0 | 6 | `738afd9` | silicon-briefcase native-proof (proof/4.1.0) | `bc539562-7759-417b-889f-cf390dd77c96` |
| silicon-apps | 0.3.0 | 9 | `9332344` | silicon-apps run 38177247302 | `bc539562-…` |
| silicon-accounts | 0.6.0 | 9 | `81c601c` | silicon-accounts run 38177249577 | `bc539562-…` |
| dm | 0.16.0 | 6 | `770fff1` | silicon-dm run 38177803563 | `237b593c-edd2-4ba6-a73e-054255f8b1c8` |
| extend | 4.1.0 | 6 | `884b514` | silicon-extend run 38177805975 | `237b593c-…` |
| mcport | 0.4.0 | 6 | `5b54eae` | silicon-mcport run 38177806142 | `237b593c-…` |
| waveform | 0.7.0 | 6 | `dde5c0c` | silicon-waveform run 38178612242 | `78199070-781c-4c23-9b3a-514a33d0a8ec` |

Third-party bundles went through `publish-migrated-apps.py`; `silicon-apps` and `silicon-accounts` through
`publish-first-party.py`. GitHub release `v0.3.0` carries the nine archives, native reports and `release.json`.
crates.io: `silicon-apps-client` and `silicon-apps-cli` 0.3.0 (package crate unchanged at 0.2.1). A managed macOS
install updated itself to `silicon-apps` 0.3.0 and `silicon-accounts` 0.6.0; `silicon-apps --help` no longer mentions
telemetry and `silicon-apps config telemetry on` prints the removal notice.
