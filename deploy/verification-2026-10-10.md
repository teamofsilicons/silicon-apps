# Production verification, 2026-10-10

## API release cf57201: historical app IDs (`dm`)

Why: eight first-party apps are moving onto Silicon Apps and Silicon Accounts. DM's app ID is the historical
two-character `dm`, which `silicon-apps create` refused (new IDs are 3–30 characters). `cf57201` adds
`APPS_HISTORICAL_APP_IDS` (`app_id:owner_uuid` entries): a listed 1–2 character ID can be checked and created only by
its configured owner, signed in; to anyone else it answers exactly like an invalid ID.

Build: `cargo zigbuild --release --locked --target aarch64-unknown-linux-gnu.2.34 -p silicon-apps-server --bin
apps-server` (zig 0.14.1 from the `ziglang` Python package), then `deploy/package.py` with the pinned Caddy 2.11.7 and
Node.js 24.21.0 archives. Archive `releases/cf572019789e9c26258bf79454aad43306ade500.tar.gz`, SHA-256
`420703c17619ba31bbbf5422af27e4126bca198ab1148e4c634da28360c3740c`, 80,452,678 bytes, S3 version
`lZGouw4G3vHEF4Ty9lcSD161sLaaVM_B`; API binary SHA-256 `c1a39737…a486a00f`; store build `gcdayXjthz4pTco-Csfte`.

Runtime secret: added the one key `APPS_HISTORICAL_APP_IDS=dm:zQo` (zQo is c:saket's Silicon Accounts uuid); the other
15 keys were checked unchanged after the write. No secret value was printed or put in SSM command text.

Backups and install: on-demand backup first (SSM `892fa230-ec5c-49e7-8953-f02dc664e0d6`):
`backups/20261009T224755Z.tar.gz`. SSM `ccc54b87-0564-41df-a952-ab31f3624b18` downloaded the archive, checked its
SHA-256 (`OK`) before extracting anything, extracted only `deploy/install.py` and ran it with `--role api`; the
installer took `backups/20261009T224825Z.tar.gz`, switched `current` and recorded `deployment.json` with revision
`cf572019789e9c26258bf79454aad43306ade500` (previous release `ce17f57` retained). The API logged
`Historical app IDs, each creatable only by its configured owner: dm.`

Public checks after the release (`https://apps.teamofsilicons.com`): `/health` 200, `/` 200 (136,343 bytes),
`/v1/apps/silicon-apps` 200, `/.well-known/silicon-apps-keys.json` 200; `GET /v1/apps/availability/dm` signed out →
`{"available":false}`; signed in as c:saket → `{"available":true}`.

Then `silicon-apps create dm --name "Silicon DM"` as c:saket created the draft app `dm` (Silicon Accounts registered it
too; `GET https://accounts.teamofsilicons.com/v1/apps/dm/public` answers `dm`). The secret is kept outside the
repository.
