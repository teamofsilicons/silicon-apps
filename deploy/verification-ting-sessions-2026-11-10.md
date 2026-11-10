# Ting session production release — 10 November 2026

Ting-backed Apps event streams and store updates are live at https://apps.teamofsilicons.com. CLI and Rust client 0.3.4 are published, including all nine signed catalog targets and the [GitHub release](https://github.com/teamofsilicons/silicon-apps/releases/tag/v0.3.4).

## Deployed sources

- API/store source: `8b05168be5117ce67a5dc82835a6e4a865173c2a`. CLI source: `99fba8f28942a12af3b2a683a3da25355163eeea`.
- API archive SHA-256: `a4d05932892d657c986679c79b12781fb2257530be141689aed4ad312bcb3ad5`.
- Running API SHA-256: `5ec10b3c619e6abce35b3a789040352d33d49cfaa877e60b1c8c88b16573ce27`.
- Store build ID: `H-9Dud-CUlJ4qlB5sfWuk`.
- Final install SSM command: `40a69d41-379f-42ac-9640-02998f8202a9`. Postflight: `c2851bc0-3dc9-42e4-9a7f-64db9cf8160a`.
- Final pre-cutover backup: `backups/20261110T135324Z.tar.gz`.
- CLI production release: `b306f04f-0ae2-5aff-882d-5b57ab243883`.

The release branch starts from the deployed user-storage source, preserving durable login recovery, storage, interface metadata, platform confirmation and unsupported-target reasons. The working checkout's unrelated edits were left in place. The validation worker was not changed.

## Checks

- 148 workspace tests passed before deployment, with one existing real-Accounts fixture generator ignored. After the TLS correction, all 27 focused server tests passed.
- A separate opt-in check opened a real secure Ting connection and registered an anonymous Apps-only receiver successfully.
- Store typecheck, lint and production build passed. Seven browser tests passed together, including live page refresh and preservation of unfinished review text. These browser checks used isolated local fixtures.
- Deployment suite: 87 checks completed, with eight platform-dependent skips.
- [Nine-platform native package build](https://github.com/teamofsilicons/silicon-apps/actions/runs/42820403623) passed. All 31 GitHub assets matched their local hashes, and all nine live catalog resolutions matched the verified archive hashes and carried release signatures.
- [Fresh public installations](https://github.com/teamofsilicons/silicon-apps/actions/runs/42826749798) passed on Linux x86_64, macOS ARM64 and Windows x86_64.
- [Corrected native API build](https://github.com/teamofsilicons/silicon-apps/actions/runs/42825532045) and [production bundle build](https://github.com/teamofsilicons/silicon-apps/actions/runs/42827113193) passed. The host verified the artifact digest and archive checksum, executed the candidate against an isolated database, and preflighted the actual standalone store before cutover.

## Live behavior

The shared `si:saket-agents` identity verified the following through authenticated public Apps endpoints using a private, unpublished QA app:

- Two authorized streams own distinct Ting receivers.
- Both Ting-backed streams delivered the same committed event.
- Strict Ting session remains healthy after the publish timeout window.
- Reconnect creates a new Ting session and replays the missed event.
- Explicit direct mode remains available without a Ting session.
- Unauthenticated stream refused.
- The exact published macOS CLI consumed a real authenticated Ting stream and replayed a saved event with `--follow --transport ting --after … --limit 1`.

The QA app's description was restored. Tests created no public app, account-wide notification subscription or receiver hook. Ting carries empty hints; Apps retains payloads, authorization and durable cursors. Each reconnect receives a new ephemeral Ting session.

Postflight verified the exact running binary, SQLite integrity and foreign keys, both storage tables, all services and the backup timer active, and no pending recovery receipt.

## Release correction and operations

The first live check found a Rustls provider-selection panic caused by the combined HTTPS and WebSocket dependencies enabling two crypto providers. Apps temporarily used direct event delivery while the corrected server was built. The final implementation selects a provider before connecting, includes a fresh-process regression test, and passed the secure Ting and final Apps stream checks above.

Large uploads from the local Mac failed, so the native API/store bundle and verified CLI assets were packaged in GitHub Actions. A short-lived artifact download descriptor was delivered privately to the host and deleted after download; no credentials or signed download URL are retained in this report. The final runtime configuration enables `APPS_TING_URL=https://backend.ting.teamofsilicons.com`, and the empty-object `silicon-apps.eventsAvailable` type is registered. Removing that setting restores direct delivery.
