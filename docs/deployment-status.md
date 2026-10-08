# Deployment status — 8 October 2026

The [Apps store and API](https://apps.teamofsilicons.com) and the [shared developer portal](https://developers.teamofsilicons.com) are deployed. A legitimate Silicon Accounts login, publication of Apps CLI 0.1.4, and a fresh native Linux installation from the production catalog have passed. A fresh authenticated browser session on the plural developer hostname remains pending because this Mac's local Unbound resolver retains an NXDOMAIN response. Authoritative DNS, Google and Cloudflare resolvers, and certificate-validated HTTPS checks already pass.

The machine-readable deployment receipt is [deploy/production.json](../deploy/production.json); operating and recovery steps are in [the production deployment guide](../deploy/PRODUCTION.md). The [initial local verification report](verification.md) describes earlier checks and is not the current production status.

## Active services and routing

| Component | Verified state |
| --- | --- |
| Apps API and store | Release `f1932610bde7d3de43f89e688e59d74d27b431dd` is active on the ARM64 production host. API version remains **0.1.2**. The API, Caddy and backup timer are active; health, database integrity and foreign-key checks pass, with no pending outbox entries at verification. |
| Shared developer portal | The existing **Silicon Accounts Next frontend**, extended with Apps authoring, is deployed at `developers.teamofsilicons.com`. Existing Accounts management remains alongside Apps setup, packages, releases, authors and history. It has no Explore/store experience. |
| Store boundary | `apps.teamofsilicons.com` serves discovery, app pages, reviews and installation documentation. Creation and management links and legacy authoring routes lead to the shared developer portal. Store routes, external management links, CSP, assets/cache behavior, installer bytes and API proxy checks pass. |
| Legacy developer URLs | `developer.teamofsilicons.com` and `developer.accounts.teamofsilicons.com` redirect to the plural shared portal, preserving the relevant management section. |
| Silicon Accounts | Source `6e828d99d449369754bcc63ff32b5d2bbd19b79a` is active, with migration 7. The real owner `c:saket` registered Apps and completed production Google and Apps CLI login. No fixture identity or fabricated grant was used. |
| Upload validation | The separate x86_64 worker remains at `b2a3db8e575701575dfc283ba8bab0ed51a1cb60`, with pinned image `python@sha256:f85c5697265c178cc6887276c55fe16cf3d14ca35c3df6a5eab3b360534a55d2`. **Linux x86_64 is the only provisioned production upload target.** |
| Worker isolation | The execution probe verified an unprivileged UID, no capabilities, no new privileges, a read-only root, no host secrets or Docker socket, and no internet or instance-metadata access. Unauthenticated worker requests return 401; the API reaches the worker privately. |

The Apps API/store bundle SHA-256 is `c36a3e26bbe9e1044e4436d21b72395a8e2941159ab6e454d7b2d0a6104a50be`; the active API executable SHA-256 is `1a76a4f0bfbaa763da287e6390ba80393d539a9974df52b25b83d827788ee62a`. SSM verification `edc483fb-26de-4d58-9c79-06c50e01c41a` confirmed the final active revision and services. The binary is unchanged from the preceding common-portal bundle; the final store build updates the published CLI installation instructions.

The Accounts/shared-portal bundle SHA-256 is `4e2ffa0367b4693063ace2a2739f1aa86499e20055d02056741560da0d9b057b`, with API executable SHA-256 `77c4bee655eff6c9e37ae90664b2771bd7ae0dfd880a884bb500160f82520f35`. Its deployment receipt and detailed recovery/verification history are in the sibling Accounts repository's `deploy/production.json` and `deploy/verification-2026-10-08.md`. Public session and PKCE checks verified the exact callback `https://developers.teamofsilicons.com/auth/callback`; the common portal's published Docs JavaScript matches its local build byte-for-byte.

## Published CLI and production catalog

[Apps CLI v0.1.4](https://github.com/teamofsilicons/silicon-apps/releases/tag/v0.1.4) contains **nine target archives and 33 assets**: archives, individual checksums, captured command results, both installers, aggregate checksums and provenance. Every public asset size and digest was compared with the assembled release. The exact tag and artifact source are `fa1caf26a24c628c3f9f0127deb9fe1687156b4e`.

The GitHub release covers Linux x86_64, i686, aarch64 and armv7hf; macOS x86_64 and aarch64; and Windows x86_64, i686 and aarch64. ARMv7 execution used QEMU; the other checks used native or compatible hosts. These artifact checks do not imply nine permanent catalog validation workers.

| Production catalog field | Verified value |
| --- | --- |
| App / version / target | `apps` / `0.1.4` / `linux-x86_64` |
| Package ID | `54e5011d-ac7f-4bf7-9a34-e108fcfef5af` |
| Production release ID | `1114010e-bf50-47a9-9584-77967a8765a8` |
| Archive SHA-256 | `dfe6aa8fd560e2bd5f2d582b99b88583314d6ca029f6aa7e86a2f3253b3610e7` |

The legitimate Apps author uploaded those exact release bytes through production validation and promoted the release. Anonymous catalog resolution and download match the published GitHub digest. macOS catalog resolution remains unavailable because no macOS upload worker is provisioned; GitHub's macOS archive verification is separate evidence.

A fresh **native Linux x86_64** home installed `apps@0.1.4` from the production registry. Verification checked the recorded server, package and release IDs, checksum, installed executable's `--help`, `accounts --json` and signed-out `login status --json`, updater status and update check, then stopped the isolated updater. SSM `f802ed7c-9420-4d7d-a9d9-702d6dabb511` ran this check; `302cbd5d-9453-4701-96dd-5b27523ce9b9` collected its persistent JSON receipt. Receipt SHA-256: `b7a80a955aed887e63ff69d39bc4f245dd6575a4c458e992d34dceb88d717688`.

The published GitHub installer also passed a fresh macOS ARM64 installation: checksum verification, Apps 0.1.4 registration, discovery/status commands and updater start/stop. A clean crates.io CLI installation passed independently. These isolated checks deliberately omitted OS startup registration; neither a Windows installer execution nor reboot persistence is claimed.

Published Rust packages are [silicon-apps-cli 0.1.4](https://crates.io/crates/silicon-apps-cli/0.1.4), [silicon-apps-client 0.1.3](https://crates.io/crates/silicon-apps-client/0.1.3), [silicon-apps-package 0.1.2](https://crates.io/crates/silicon-apps-package/0.1.2), and the official [silicon-accounts-client 0.1.0](https://crates.io/crates/silicon-accounts-client/0.1.0). The CLI/client update fixes TLS provider selection for explicitly configured direct telemetry and Windows first-use telemetry socket startup. Fresh-process loopback TLS tests pass on Linux, macOS and Windows, including preservation of an embedding application's existing provider.

## Recovery and telemetry evidence

Apps backup `backups/20261008T143156Z.tar.gz`, SHA-256 `df7f06b9cca9ea8eb3aee617985bba0fb494b7d340007b4cb8d1e94173ca9e08`, was restored into scratch storage. SQLite integrity passed with zero foreign-key errors, the Apps catalog remained published, and **both package objects** matched their stored digests. The scratch restore was removed afterward.

Accounts' pre-cutover backup `backups/predeploy-20261008T142732Z.dump`, SHA-256 `e891291f36b1804cadc6f963b2907fb7a1dd2f637c1b79518490f7ff2efeaa5c`, was restored into a separate PostgreSQL database. Migration 7, the genuine account, three registered apps, membership and Apps ownership were checked before removing that database. Its newer redundant backup `backups/predeploy-20261008T143304Z.dump` was uploaded but was not the restored copy.

Space Station delivery to `tos.siliconapps` is verified. A read-only post-final-restart check at 14:38:13 UTC verified last spool sequence **170**, acknowledgement cursor **170** and **zero pending records**, without modifying the spool (SSM `c7dc2c37-ef21-4b81-9cdd-ebfee9bcf9f5`). Separately, a **read-only** destination query observed nine new records with nine distinct record IDs after the earlier `9972028` deployment; this final check did not copy a spool, replay records, change a cursor or emit a probe event. An earlier isolated diagnostic replay of 136 existing records produced 136 duplicate destination rows: its snapshot had 274 rows but only 138 distinct record IDs. Those duplicate rows are diagnostic history and must not be counted as new production events.

## Test and receipt references

- [CLI release CI](https://github.com/teamofsilicons/silicon-apps/actions/runs/37791505935) and [all nine native artifact jobs](https://github.com/teamofsilicons/silicon-apps/actions/runs/37791540306) passed at the exact CLI release source.
- [Common-portal routing source CI](https://github.com/teamofsilicons/silicon-apps/actions/runs/37792428162) passed at `99720289095e34b32b2883fb415e84bfb43a8bde`.
- The shared Accounts/Apps portal passed **25 browser tests and 13 frontend unit tests**; the separate Apps store passed **15 browser tests**. Production builds and frontend checks passed. Browser tests use controlled test identities and do not substitute for a completed production browser login.
- Ignored local receipts include `.dev/production/api-final-014-verified.log`, `store-final-014-live.log`, `catalog-install-live-014.log`, `catalog-install-receipt-014.log`, `backup-restore-evidence.json`, `telemetry-post-final.log` and `telemetry-final-readonly-evidence.json`.
- Release evidence is in `.dev/publication-summary-014.json`, `.dev/public-bootstrap-014-verification.json`, `.dev/registry-install-014-verification.json` and `.dev/release-v0.1.4/`. Public archive command results and provenance are included in the GitHub release.

## Remaining verification

- Complete a fresh authenticated browser session on `developers.teamofsilicons.com` after clearing this Mac's local Unbound negative cache. Both authoritative nameservers and public resolvers already return the intended Accounts host; the remaining cache is local, not an authoritative DNS cutover failure. The targeted cache reload requires local administrator access and remains pending.
- Exercise production private-app access with legitimate permitted and denied accounts. Local/API authorization tests do not establish that live multi-account flow.
- Verify real-recipient mail delivery and any remaining identity-provider flows. Postmark is configured and real Google login passed; Apple login, SMS delivery and real-recipient mail remain unverified. No test email was sent during this deployment.
- Verify startup-service behavior across an actual OS reboot, and execute the Windows bootstrap installer on a Windows host. Native command tests and isolated updater start/stop checks do not establish those results.
- Provision and verify additional isolated target workers before enabling their production catalog uploads. The currently verified catalog target is Linux x86_64 only.
