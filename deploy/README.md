# Deployment examples

These files are installation templates. No infrastructure, domains, credentials, mail transport, or native workers are provisioned by adding them to the repository.

- `local.env.example`: configuration consumed by `bash scripts/dev.sh` after copying to `.env`.
- `api.env.example`: API configuration for both public domains; replace all credential and endpoint placeholders.
- `runner.env.example`: isolated Linux worker configuration. Replace image placeholders with pre-pulled immutable digests.
- `runner-gateway.env.example`: the same Python process routing selected targets to authenticated HTTPS workers on separate hosts.
- `Caddyfile`: same-origin `/v1` routing, installer downloads, immutable assets, SPA history fallback and security headers for the store and developer domains.
- `silicon-apps-api.service`: one API process with its persistent data at `/var/lib/silicon-apps`.
- `silicon-apps-runner.service`: a dedicated Linux Docker worker. Its scratch path must be visible to the local container engine; Docker access must be provisioned separately.

Create the named service users and install reviewed environment files with restricted permissions. The production Caddy configuration serves `/opt/silicon-apps/current/web`; install each complete frontend into `/opt/silicon-apps/releases/<revision>/web` and atomically select that release with the `current` symlink. Use the corresponding release's API executable in the supervised service. Do not run a development Vite server as the public web service. Linux units are examples for systemd; macOS validation hosts need their own supervised Python runner with `sandbox-exec` available. Windows hosts need compatible Hyper-V container infrastructure or a separate isolated protocol worker. These examples do not provision either. The user’s `apps daemon install` manages the client updater and is separate from these backend units.

`npm --prefix web run build` stages the reviewed POSIX and PowerShell bootstrap scripts at `web/dist/install.sh` and `web/dist/install.ps1`. Caddy serves both as plain text with revalidation; their network mode requires published GitHub release artifacts. Hashed `/assets/*` files are immutable and missing assets return 404. All SPA entry paths revalidate, API responses are not cached, and the content security policy permits bundled/inlined fonts, authored HTTPS media and same-origin API calls. Build with `VITE_APPS_API_URL` unset for this layout.

Both Accounts callbacks must be registered:

```text
https://apps.teamofsilicons.com/v1/auth/callback
https://developer.teamofsilicons.com/v1/auth/callback
```

Keep `VITE_APPS_API_URL` unset for this layout. Each host proxies `/v1` to the same API and receives its own HttpOnly session cookie. Uploaded media uses origin-relative `/v1` URLs so draft/private media follows the current host’s session. The API’s allowed-origin list permits both origins, but does not create or share a browser login automatically.

Read [the operations guide](../docs/operations.md) before activating the examples. In particular, runner availability must correspond to native execution that actually exists; catalog support for a target does not provision a worker.
