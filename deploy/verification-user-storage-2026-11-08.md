# Guarded user storage production release — 8 November 2026

Optional app user storage is deployed in the Apps API and the Developer portal. CLI and Rust client version 0.3.3 are published to crates.io. The [GitHub release](https://github.com/teamofsilicons/silicon-apps/releases/tag/v0.3.3) contains all nine platform packages, execution reports, checksums and release provenance.

## Sources and validation

| Component | Source | Verification |
| --- | --- | --- |
| Apps API, CLI and client | `6fe845f7c59009d8fde9bd970af95f98a42aaacf` | 142 Rust tests passed, 1 existing fixture generator ignored; 87 deployment tests passed, 8 skipped |
| Developer portal | `72369a0056184864030264217b5120c2bd240c68` in silicon-accounts | 97 unit tests, 2 storage browser tests, typecheck, lint, production build and standalone routing check passed |
| CLI platform packages | [Build 42501372383](https://github.com/teamofsilicons/silicon-apps/actions/runs/42501372383) | Nine targets passed archive, discovery and documentation checks; native or host-compatible execution |
| Native ARM64 API build | [Build 42501375199](https://github.com/teamofsilicons/silicon-apps/actions/runs/42501375199) | Build and health checks passed |
| Public installation | [Run 42503311859](https://github.com/teamofsilicons/silicon-apps/actions/runs/42503311859) | Fresh public bootstrap and signed catalog installs passed on Linux x86_64, macOS ARM64 and Windows x86_64; repeated successfully on the local Mac |

The deployed API binary was cross-built from the exact Apps source, checked for ARM64/GLIBC compatibility, and executed on the production ARM64 host against a separate temporary database before cutover. Its SHA-256 is `2c17cfe01d9f4bcae123d4b069431a4cb68a297d62fa1a4c4b4167c85100c518`.

## Deployment

The API archive SHA-256 is `1ea04a3eae4bef6e1f7417e3ee69de35fe4e6adb79e511f2ab1737cab2524b02`. The release preserves the previous live store, installers, runtime and deployment components from `8003a53fb1c8e24472129fb7e0e3179d25566afb`. SSM command `4a096202-f407-46ab-9020-af6461e5abcd` completed successfully and retained `backups/20261108T165112Z.tar.gz` in the production Apps artifact bucket.

The Developer archive SHA-256 is `3b85208d5b14f5ced6a1118d6b80846395869b1e66679ec3fa5056ab1423483c`. SSM command `f3e68be1-b8e7-43af-b7a9-2a27ad917de6` activated `user-storage-dev-72369a005618-76411478a999`. Its tree hash is `76411478a99990f341ff04155fcf95a1598b317e8efc727a385319d43119dafb`. The installer verified every file outside the Developer overlay, preserved the Accounts API process, and checked the existing Cells feature gate and successful migrations 1–37. Postflight command `b749f8f2-8948-47f3-9a18-d89b4e846915` confirmed all Accounts services healthy and the API/web hashes unchanged.

## CLI publication

SSM command `98a7dd93-eed2-48ce-9d03-3a335e294f98` published the nine verified packages through the operator first-party publisher. The API was stopped during the pre-publication snapshot and catalog transaction, then restarted before the final backup. Both backups are retained in the production Apps artifact bucket:

- Before publication: `backups/20261108T170259Z.tar.gz`.
- After publication: `backups/20261108T170517Z.tar.gz`.

The publication bundle SHA-256 is `f46d7069a9c251cbbb947f6fe1863c2f2bdf930ab4d37eef86a54f05762fd579`; its `release.json` hash is `08431f8a3859287f18832c6448462778d6d4838df2ae3c49020468b12dc1c817`. Production release ID is `baf1fc37-cfd8-5f80-89de-ddff96582487`. All nine public resolutions return version 0.3.3, signatures and the exact native archive hashes. All 31 GitHub assets matched their local hashes, and tag `v0.3.3` points to the exact source commit above. Both crates.io versions were independently checked as published and not yanked.

Fresh installation checks used temporary signed-out homes, disabled daemon registration and left existing user sessions and installations untouched. They exercised the public bootstrap installers and a separate signed catalog installation, followed by version, discovery, login status, storage help and bundled storage documentation checks.

## Production behavior

Sixteen checks used real Accounts-issued proofs under `si:saket-agents` and two private, unpublished QA apps. They verified:

- Cross-app reads of the authorized user's complete settings.
- Rejection of user-proof-only writes, another app's writes and a different user's reads.
- Owner writes and subsequent cross-app reads.
- Rejection of stale revisions, wrong types and undeclared keys.
- Independent anonymous defaults, authorized anonymous writes and resets to defaults.
- Production CLI reads and absence of runtime values from author metadata.
- Immediate rejection of revoked proof tokens.

All storage responses used private/no-store caching. Signed-in and anonymous records were reset, and all four issued QA proofs were revoked after testing. No tokens or app secrets are included in this report.

Public health, Accounts readiness, [storage documentation](https://developers.teamofsilicons.com/docs/apps/start/user-storage), configured proof verification capability, and the five storage operations in OpenAPI were checked. The shared `anon` record and immutable existing schema definitions are documented in the public guide.

After CLI catalog publication and the API restart, all sixteen production checks passed again using the exact published macOS ARM64 CLI archive. QA values were reset and the new proofs revoked. Final Apps host postflight `985d2884-3875-4d2b-b69a-aaa07bc9b66c` confirmed the exact API binary, preserved store hash, both storage tables, healthy SQLite integrity/foreign-key checks, CLI production version 0.3.3, all services and the backup timer active, and no pending recovery receipt.
