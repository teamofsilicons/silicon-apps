# Deployment examples

These files are installation templates. No infrastructure, domains, credentials, mail transport, or native workers are provisioned by adding them to the repository.

- `local.env.example`: configuration consumed by `bash scripts/dev.sh` after copying to `.env`.
- `api.env.example`: API configuration for both public domains; replace all credential and endpoint placeholders.
- `runner.env.example`: isolated Linux worker configuration. Replace image placeholders with pre-pulled immutable digests.
- `runner-gateway.env.example`: the same Python process routing selected targets to authenticated HTTPS workers on separate hosts.
- `Caddyfile`: same-origin `/v1` routing, installer downloads, immutable assets and security headers for the store; old developer paths redirect to the shared portal.
- `silicon-apps-api.service`: one API process with its persistent data at `/var/lib/silicon-apps`.
- `silicon-apps-runner.service`: a dedicated Linux Docker worker. Its scratch path must be visible to the local container engine; Docker access must be provisioned separately.

Create the named service users and install reviewed environment files with restricted permissions. The production Caddy configuration serves `/opt/silicon-apps/current/web`; install each complete frontend into `/opt/silicon-apps/releases/<revision>/web` and atomically select that release with the `current` symlink. Use the corresponding release's API executable in the supervised service. Do not run a development Vite server as the public web service. Linux units are examples for systemd; macOS validation hosts need their own supervised Python runner with `sandbox-exec` available. Windows hosts need compatible Hyper-V container infrastructure or a separate isolated protocol worker. These examples do not provision either. The user’s `apps daemon install` manages the client updater and is separate from these backend units.

`npm --prefix web run build` stages the reviewed POSIX and PowerShell bootstrap scripts at `web/dist/install.sh` and `web/dist/install.ps1`. Caddy serves both as plain text with revalidation; their network mode requires published GitHub release artifacts. Hashed `/assets/*` files are immutable and missing assets return 404. All SPA entry paths revalidate, API responses are not cached, and the content security policy permits bundled/inlined fonts, authored HTTPS media and same-origin API calls. Build with `VITE_APPS_API_URL` unset for this layout.

Register the store callback on the `apps` Accounts application:

```text
https://apps.teamofsilicons.com/v1/auth/callback
```

The first-party `developer` application uses `https://developers.teamofsilicons.com/auth/callback`. Its existing Next.js frontend, deployed with Accounts, hosts the common Apps and Accounts workspace. Its server-side Apps proxy uses the sealed developer session; the API verifies the developer audience only for authoring routes and still checks each immutable author UUID. The browser never receives its bearer token. Keep `VITE_APPS_API_URL` unset for the store and point `VITE_DEVELOPERS_URL` at the shared portal if overriding the production default.

Read [the operations guide](../docs/operations.md) before activating the examples. In particular, runner availability must correspond to native execution that actually exists; catalog support for a target does not provision a worker.

## First-party native releases

`publish-first-party.py` is an operator-only deployment helper for the `silicon-apps` and `silicon-accounts` CLI packages. It does not expose an API or change the upload validation workers. Before using it, finish the nine-target GitHub package build, verify the source commit and every archive checksum, and assemble `release.json` with `app_id`, `version`, `source_commit`, the build run URL as `workflow`, and nine report basenames as `reports`. Include the exact archives and native execution reports in the bundle.

Stop `silicon-apps-api`, take a catalog and package backup with `backup.py`, then run the helper as `silicon-apps` with `--data-dir /var/lib/silicon-apps --bundle DIRECTORY --sha256 RELEASE_JSON_SHA256`. Always restart the API on failure as well as success. The helper checks the manifest identity, archive bytes and required native command outputs before changing the catalog. Repeating the same publication is safe; changing an existing version is refused. It preserves existing app metadata and install counts. Save a new backup after publication and test public resolution and a fresh managed installation on each operating system.
