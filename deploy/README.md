# Deployment examples

These files are installation templates. No infrastructure, domains, credentials, mail transport, or native workers are provisioned by adding them to the repository.

- `local.env.example`: configuration consumed by `bash scripts/dev.sh` after copying to `.env`.
- `api.env.example`: API configuration for both public domains; replace all credential and endpoint placeholders. The optional, commented `APPS_HISTORICAL_APP_IDS` lists comma-separated `app_id:owner_uuid` entries, such as `dm:zQo`: each reserves a historical Silicon Accounts app ID of 1 or 2 characters, which new apps cannot use, for the one account (by its case-sensitive Accounts UUID) allowed to create it. Only that account, signed in, sees the ID as available and can create it; to everyone else it is an invalid ID. The API refuses to start on a malformed value. See [Reserve a historical app ID for its owner](../docs/operations.md#reserve-a-historical-app-id-for-its-owner).
- `runner.env.example`: isolated Linux worker configuration. Replace image placeholders with pre-pulled immutable digests.
- `runner-gateway.env.example`: the same Python process routing selected targets to authenticated HTTPS workers on separate hosts.
- `Caddyfile`: unbuffered event streams and same-origin `/v1` routing to the API, installer downloads, the store's immutable `/_next/static` files and every other path to the server-rendered store, with security headers; old developer paths redirect to the shared portal.
- `silicon-apps-api.service`: one API process with its persistent data at `/var/lib/silicon-apps`.
- `silicon-apps-store.service`: the server-rendered store (`store/`, a Next.js standalone server) on `127.0.0.1:4320`, as its own user with no secrets and no writable paths.
- `package.py` and `install.py`: build the auditable release archive and install it on a host; see [PRODUCTION.md](PRODUCTION.md).
- `silicon-apps-runner.service`: a dedicated Linux Docker worker. Its scratch path must be visible to the local container engine; Docker access must be provisioned separately.

Create the named service users and install reviewed environment files with restricted permissions. Each release at `/opt/silicon-apps/releases/<revision>` holds `bin/`, `node/`, `store/` and `web/` (the installers), and the `current` symlink selects one atomically; `install.py` does all of this. Use the corresponding release's API executable and store in the supervised services. Do not run a development server as the public web service. Linux units are examples for systemd; macOS validation hosts need their own supervised Python runner with `sandbox-exec` available. Windows hosts need compatible Hyper-V container infrastructure or a separate isolated protocol worker. These examples do not provision either. The user’s `apps daemon install` manages the client updater and is separate from these backend units.

`package.py` places the reviewed POSIX and PowerShell bootstrap scripts from `scripts/` at `web/install.sh` and `web/install.ps1` in the release. Caddy serves both as plain text with revalidation; their network mode requires published GitHub release artifacts. The store's hashed `/_next/static/*` files are immutable and missing files return 404. Store pages carry the store's own nonce-based content security policy; API and installer responses get a strict one from Caddy. The store reaches the API at `APPS_API_URL` (`http://127.0.0.1:4310`) and sends `STORE_PUBLIC_URL` as the Origin of visitor writes.

Register the store callback on the `apps` Accounts application:

```text
https://apps.teamofsilicons.com/v1/auth/callback
```

The first-party `developer` application uses `https://developers.teamofsilicons.com/auth/callback`. Its existing Next.js frontend, deployed with Accounts, hosts the common Apps and Accounts workspace. Its server-side Apps proxy uses the sealed developer session; the API verifies the developer audience only for authoring routes and still checks each immutable author UUID. The browser never receives its bearer token. The store redirects old `/developer` and `/docs` addresses to `https://developers.teamofsilicons.com` unless `DEVELOPERS_URL` overrides it (Caddy answers them first in production).

Read [the operations guide](../docs/operations.md) before activating the examples. In particular, runner availability must correspond to native execution that actually exists; catalog support for a target does not provision a worker.

## First-party native releases

`publish-first-party.py` is an operator-only deployment helper for the `silicon-apps` and `silicon-accounts` CLI packages. It does not expose an API or change the upload validation workers. Before using it, finish the nine-target GitHub package build, verify the source commit and every archive checksum, and assemble `release.json` with `app_id`, `version`, `source_commit`, the build run URL as `workflow`, and nine report basenames as `reports`. Include the exact archives and native execution reports in the bundle.

Stop `silicon-apps-api`, take a catalog and package backup with `backup.py`, then run the helper as `silicon-apps` with `--data-dir /var/lib/silicon-apps --bundle DIRECTORY --sha256 RELEASE_JSON_SHA256`. Always restart the API on failure as well as success. The helper checks the manifest identity, archive bytes and required native command outputs before changing the catalog. Repeating the same publication is safe; changing an existing version is refused. It preserves existing app metadata and install counts. Save a new backup after publication and test public resolution and a fresh managed installation on each operating system.
