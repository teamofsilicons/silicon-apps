# Deployment status — 8 October 2026

This continues the [initial local verification phase](verification.md). The complete public Apps deployment is **not yet active**. Operational steps are in [the production deployment guide](../deploy/PRODUCTION.md).

## Verified production work

| Component | Observed state |
| --- | --- |
| Silicon Accounts | Revision `875a30af17a49a8e694e830e7e3ab7107566f0f9` is active on the existing production host. Additive migration 7 is applied; public readiness and authenticated registry listing pass. |
| Accounts recovery | The predeployment PostgreSQL dump was restored into a separate temporary database, verified at migration 6 with zero accounts and two built-in apps, then removed. Production retained those records. No local fixture identities were migrated. |
| Apps infrastructure | AWS stack `silicon-apps-production` in `us-east-2` is created: separate ARM64 API/web and x86_64 validation hosts, private versioned artifacts, SSM administration and restricted worker ingress. |
| Validation worker | Revision `b2a3db8e575701575dfc283ba8bab0ed51a1cb60` is active. The actual Apps 0.1.3 Linux x86_64 archive passed `--help`, `accounts --json` and signed-out `login status --json` inside its pinned container. Unauthenticated requests return 401. |
| Worker isolation | A separate execution probe confirmed an unprivileged UID, no capabilities, no new privileges, read-only root, no host secrets or Docker socket, and no internet or instance-metadata access. The API host reaches the worker privately. |
| API and website | Revision `b2a3db8e575701575dfc283ba8bab0ed51a1cb60` is staged. Its exact API executable passed an ephemeral AWS health/anonymous-session check with an isolated database. Production API/web services are **not activated**, and that check did not create a production catalog. |
| Mail and telemetry | Accounts' native Postmark provider is configured. Dedicated Space Station table `tos.siliconapps` exists with `@c:saket` access; its recording key and the private worker URL are stored in the Apps runtime secret. No real-recipient email was sent. |

The deployed worker bundle SHA-256 is `7a4c010798a73571f0225556b7eed703464e947caf38518bb14c2f8cf7cd1c31`. Its API executable SHA-256 is `0e4797c48e2242706ab94c3269bc0fa2e442d33141688c0e73771dd78c4c4ac9`. The exercised Linux CLI archive SHA-256 is `03d41d8658c550b55fc3475ec41824ea7e1b4cb5a72af6fa20ec32ef7f87d7b7`.

Accounts backup `backups/predeploy-20261008T124704Z.dump` has SHA-256 `2c6cbaba7b55b61afdf68dccbd68958e7452d88c587b8b81f677c4a96e3a483b`. Accounts deployment receipts are tracked in the sibling Accounts repository under `deploy/production.json` and `deploy/verification-2026-10-08.md`. Ignored local Apps evidence includes `.dev/production/worker-proof-013.log`, `worker-isolation.log`, `api-stage.log`, `api-smoke.log`, and the bundle receipt named by its revision. An earlier verification harness encountered a closed connection while submitting a large unauthenticated body; the final proof used a small unauthorized request and verified the actual 401 response.

## Remaining cutover gates

- A real production Accounts owner must sign up/sign in and identify their account for Apps registration; resolve its genuine immutable UUID from Accounts. The existing IAM identity used for Space Station is a separate identity system; it does not establish Accounts ownership. Register Apps and merge its credentials without creating or verifying an account on the user's behalf.
- Move the existing Accounts developer portal to `developer.accounts.teamofsilicons.com`, verify HTTPS and its callback, then move the Apps store/developer domains to the Apps host. DNS changes and this hostname migration remain pending.
- Activate the staged API/web release after credentials and domain routing are ready. Complete real public Accounts login, catalog publication, private-access checks, a fresh install, live telemetry delivery and an Apps backup restore before claiming end-to-end production verification.
- Only Linux x86_64 upload execution is currently provisioned in production. Other supported package targets require their own verified isolated workers. Native release archives alone do not prove those workers exist.

## Published packages and source checks

- [silicon-apps-cli 0.1.3](https://crates.io/crates/silicon-apps-cli/0.1.3) is published, with [silicon-apps-client 0.1.2](https://crates.io/crates/silicon-apps-client/0.1.2), [silicon-apps-package 0.1.2](https://crates.io/crates/silicon-apps-package/0.1.2) and the official [silicon-accounts-client 0.1.0](https://crates.io/crates/silicon-accounts-client/0.1.0).
- [Source CI for b2a3db8](https://github.com/teamofsilicons/silicon-apps/actions/runs/37781034398) passed Rust tests, formatting, strict Clippy, browser behavior, native Windows/macOS client contracts, worker safety on three operating systems and deployment rollback/backup tests.
- [Apps CLI v0.1.3](https://github.com/teamofsilicons/silicon-apps/releases/tag/v0.1.3) is public with all nine target archives, installers, checksums and full command-output evidence (33 assets). The tag and recorded artifact source are `073a950`. [All nine native/compatible-host checks](https://github.com/teamofsilicons/silicon-apps/actions/runs/37780427814) passed; ARMv7 execution used QEMU.
- [Follow-up full CI](https://github.com/teamofsilicons/silicon-apps/actions/runs/37781706492) passed at `87155a8`. A fresh crates.io CLI install passed. The public GitHub installer downloaded the published macOS ARM64 archive, verified its checksum, installed version 0.1.3 and passed signed-out discovery and updater start/stop checks in an isolated home. No real OS startup registration was created.
- Local receipts are `.dev/publication-summary.json`, `.dev/public-bootstrap-013-verification.json`, `.dev/registry-install-013-verification.json` and `.dev/release-v0.1.3/`. GitHub/crates distribution is published; production Apps catalog publication still requires registration, authentication and activation above.
