# Deployment status — 8 October 2026

The [Apps store and API](https://apps.teamofsilicons.com) and the [shared developer portal](https://developers.teamofsilicons.com) are deployed. A legitimate Silicon Accounts login, publication of Apps CLI 0.1.4, and a fresh native Linux installation from the production catalog have passed. After the user refreshed this Mac's local Unbound negative cache, a genuine Chrome session on the plural developer hostname showed the signed-in owner and Silicon Apps in Your apps. Authoritative/public DNS and certificate-validated HTTPS checks pass.

The machine-readable deployment receipt is [deploy/production.json](../deploy/production.json); operating and recovery steps are in [the production deployment guide](../deploy/PRODUCTION.md). The [initial local verification report](verification.md) describes earlier checks and is not the current production status.

## Active services and routing

| Component | Verified state |
| --- | --- |
| Apps API and store | Release `0f41edef91d6de61a3770d2f791a4e352a35f2ae` is active on the ARM64 production host. API version remains **0.1.2**. The API, Caddy and backup timer are active; health, database integrity and foreign-key checks pass, with no pending outbox entries at verification. |
| Shared developer portal | The existing **Silicon Accounts Next frontend**, extended with Apps authoring, is deployed at `developers.teamofsilicons.com`. Existing Accounts management remains alongside Apps setup, packages, releases, authors and history. It has no Explore/store experience. |
| Store boundary | `apps.teamofsilicons.com` serves discovery, app pages, reviews and installation. Documentation lives in the shared portal at `/docs/apps`. Creation and management links and legacy authoring routes lead to the shared developer portal. Store routes, external management links, CSP, assets/cache behavior, installer bytes and API proxy checks pass. |
| Legacy developer URLs | `developer.teamofsilicons.com` and `developer.accounts.teamofsilicons.com` redirect to the plural shared portal, preserving the relevant management section. |
| Silicon Accounts | Source `ace71ff0a39ac7b861d8025000c25d3de4cbfdf0` is active, with migration 9. The real owner `c:saket` registered Apps and completed production Google and Apps CLI login. No fixture identity or fabricated grant was used. |
| Upload validation | Since 9 October 2026 the separate x86_64 worker (release `1d92c9a`) validates four Linux targets: linux-x86_64 and linux-i686 natively, linux-aarch64 and linux-armv7hf through pinned QEMU user-mode emulation, each with a pinned per-platform image from the `python@sha256:f85c5697...` index. **macOS and Windows have no worker yet.** See deploy/verification-2026-10-09.md. |
| Worker isolation | The execution probe verified an unprivileged UID, no capabilities, no new privileges, a read-only root, no host secrets or Docker socket, and no internet or instance-metadata access. Unauthenticated worker requests return 401; the API reaches the worker privately. |

The Apps API/store bundle SHA-256 is `857a45c0284559835bc63475659b1e0c2b111a831e7dd6cd62e6e6348f0fe454`; the active API executable SHA-256 is `1a76a4f0bfbaa763da287e6390ba80393d539a9974df52b25b83d827788ee62a`. SSM verification `5bcebb0e-9a5e-48b0-a599-0f504540874a` confirmed the final active revision and services. The API binary is unchanged. The store now matches the common developer portal with warm ivory/charcoal surfaces, self-hosted Instrument Serif, Geist and JetBrains Mono, a compact shared-style header, and blue controls. Catalog, details, documentation, settings and install dialogs use the same light/dark visual language.

The Accounts/shared-portal bundle SHA-256 is `79d8c3d4526d8ae99feef009cdbb14ca87134c3a936e0fdc016a4952b52cea2c`, with API executable SHA-256 `d4019457215edcb6fd1565a75851f677cc6b2be82a6f5c8dc401a63569f7f28d`. Its deployment receipt and detailed recovery/verification history are in the sibling Accounts repository's `deploy/production.json` and `deploy/verification-2026-10-08.md`. Public session and PKCE checks verified the exact callback `https://developers.teamofsilicons.com/auth/callback`; the common portal's published Docs JavaScript matches its local build byte-for-byte.

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

- Store styling release `23d18e5` passed all **15 browser tests**, production build, and the [source web CI job](https://github.com/teamofsilicons/silicon-apps/actions/runs/37798115584). Fresh production Chrome checks covered 16 route/theme/viewport combinations (catalog, details, docs and settings; light/dark; 390/1440 px), all four install-dialog combinations, mobile navigation and visible input focus, with no horizontal overflow or browser errors. Live HTML and all four referenced JS/CSS files match the release build exactly; CSP, API proxy, installers and developer redirects pass. Screenshots and receipts are under ignored `.dev/production/store-style-live/`, `store-style-live-assets.json`, `api-store-style-verified.log` and `store-style-live.log`.

- [CLI release CI](https://github.com/teamofsilicons/silicon-apps/actions/runs/37791505935) and [all nine native artifact jobs](https://github.com/teamofsilicons/silicon-apps/actions/runs/37791540306) passed at the exact CLI release source.
- [Common-portal routing source CI](https://github.com/teamofsilicons/silicon-apps/actions/runs/37792428162) passed at `99720289095e34b32b2883fb415e84bfb43a8bde`.
- The shared Accounts/Apps portal passed **52 browser tests and 18 frontend unit tests**; the separate Apps store passed **18 browser tests**. Production builds and frontend checks passed. Browser tests use controlled test identities and do not substitute for a completed production browser login.
- Ignored local receipts include `.dev/production/api-final-014-verified.log`, `store-final-014-live.log`, `catalog-install-live-014.log`, `catalog-install-receipt-014.log`, `backup-restore-evidence.json`, `telemetry-post-final.log` and `telemetry-final-readonly-evidence.json`.
- Release evidence is in `.dev/publication-summary-014.json`, `.dev/public-bootstrap-014-verification.json`, `.dev/registry-install-014-verification.json` and `.dev/release-v0.1.4/`. Public archive command results and provenance are included in the GitHub release.

## Remaining verification

- The genuine signed-in developer dashboard is verified. App-specific publishing and Accounts screens have automated browser coverage; every authenticated screen and mutation was not separately exercised against production during this rollout.
- Exercise production private-app access with legitimate permitted and denied accounts. Local/API authorization tests do not establish that live multi-account flow.
- Verify remaining identity-provider and SMS flows. Real Google login and both manual verification demo email deliveries are verified; Apple login and SMS delivery remain unverified.
- Verify startup-service behavior across an actual OS reboot, and execute the Windows bootstrap installer on a Windows host. Native command tests and isolated updater start/stop checks do not establish those results.
- Provision and verify macOS and Windows workers before enabling their production catalog uploads. The four Linux targets are verified and enabled.

The common portal now includes central App verification history at `/app-verification`, scoped to currently managed apps, with retained issuance/refresh/revocation events. Accounts user-facing labels use User verification. Public compiled assets match the release; populated history uses local integration/browser fixtures, as production has no proof records.

App Sign-in setup also includes a reason-only manual account verification request for own-domain authentication, with an up-to-48-hour response estimate. Requests are durable, scoped to the signed-in manager and deduplicated while pending; each new request queues notifications to both designated team addresses. The genuine signed-in owner submitted one clearly marked demo, and both recipient mail servers accepted their respective message on the first attempt. This records requests and notifications only; it does not provision domains or grant verified status.

## Shared documentation rollout

The common documentation is live at https://developers.teamofsilicons.com/docs.
All 42 Accounts pages are preserved under `/docs/accounts`; nine Apps pages live
under `/docs/apps`, with one shared landing, search and navigation. Markdown,
`/llms.txt` and `/llms-full.txt` include both products. The account site links to
the shared docs; legacy Accounts pages retain their paths under the Accounts
namespace through 308 redirects. The store links and legacy docs route point to
the Apps namespace. No new CLI or crate release was needed for this move.

Final Accounts source `ace71ff0a39ac7b861d8025000c25d3de4cbfdf0`, bundle SHA-256 `79d8c3d4526d8ae99feef009cdbb14ca87134c3a936e0fdc016a4952b52cea2c`,
API SHA-256 `d4019457215edcb6fd1565a75851f677cc6b2be82a6f5c8dc401a63569f7f28d`, migration 9. Install SSM `325d61b3-4e54-4c00-8794-e2dde32311f8` and verification
`bb16fe14-4621-4ca0-8265-2ad3ca1415ad` confirm the exact release, six active services/timers, readiness and
continued anonymous denials on protected verification APIs. The existing demo
request and two notification records remain; no request or email was sent by this
rollout. Backup `backups/predeploy-20261008T162902Z.dump`, SHA-256 `540c6f4ae333ecbb29004300b669ca4d60e388f2a3b7c67aebe78ecd6dd54f8a`, was uploaded;
previous restore evidence remains separate.

Apps source `0f41edef91d6de61a3770d2f791a4e352a35f2ae` supplies the store links and
Caddy redirects. Bundle SHA-256 is
`857a45c0284559835bc63475659b1e0c2b111a831e7dd6cd62e6e6348f0fe454`.
Its API binary and worker are unchanged. Install `679b906f-4b26-4de3-9079-e66c87e2d7d1` and verification
`5bcebb0e-9a5e-48b0-a599-0f504540874a` confirm the release, services, catalog and database integrity.

Validation passed: 884 Rust tests (three existing ignored), frontend typechecks,
lint and builds, 18 developer unit tests, four account-site redirect tests,
52 developer browser tests and 18 store browser tests. The initial live crawl
found that Next normalized the standalone listener's 127.0.0.1 origin to localhost,
causing an internal missing-page rewrite to attempt TLS to the HTTP listener.
The final release enables the supported `skipProxyUrlNormalize` flag and preserves
raw origins for rewrites/relative legacy redirects. A new standalone regression
passes 22 route/header combinations with production Host/forwarded-HTTPS headers,
including valid pages, unknown-page 404s, Markdown, aliases, CSP and HSTS.

The final live crawl checked all 52 pages, 663 search records,
5166 rendered internal links, raw Markdown/LLM exports,
canonical metadata, legacy redirects and metadata/discovery documentation URLs.
Production IAB verification showed the shared landing and search results from both
products. Local visual checks covered light/dark at 320/1440 px and mobile theme
switching; the store's public browser/CSP/assets/installer/API checks also passed.
