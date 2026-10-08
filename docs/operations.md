# Operate Silicon Apps

The repository provides application code and local development tooling. A deployment still needs Accounts registration and the corresponding Accounts server integration, persistent storage, HTTPS for both websites, authenticated isolated target workers, mail delivery, and a Space Station destination when telemetry is enabled. The example configuration does not provision those services.

## Start the full stack

1. Build the workspace with Rust 1.98 or newer and a C compiler: `cargo build --workspace --locked`. The official Accounts client source is vendored; see `vendor/silicon-accounts-client` for its provenance.
2. Run Silicon Accounts using that service’s own deployment instructions, database migrations, and mail configuration. Its server must include the registry export/sync, accepted-author authorization, private Apps mail bridge, and webhook configuration methods used by this Apps version.
3. Register the `apps` application and callbacks in Accounts, then copy `deploy/local.env.example` to `.env` and set the Accounts URL, app secret, and internal service token. Use a separate local Accounts environment when developing locally.
4. Set `APPS_PUBLIC_URL=http://127.0.0.1:4311` and allow that same origin. The browser enters through Vite on 4311; the Rust API listens on 4310. For browser sign-in, register `http://127.0.0.1:4311/v1/auth/callback` in the local Accounts app.
5. Configure an isolated runner and a random runner token if package uploads are needed. Set `APPS_RUNNER_TARGETS` only to targets this worker or gateway can execute. On an Apple Silicon Mac, the bundled native target is `macos-aarch64`; an Intel Mac has `macos-x86_64`.
6. Run `bash scripts/dev.sh`. It builds the workspace, installs frontend dependencies if needed, starts the API and frontend, and starts the Python runner when a runner token exists. Logs go to `.dev/logs/`. It stops its child processes when interrupted.
7. Open `/store` and `/developer` on port 4311. Sign in through Accounts, create a draft, upload a real package, create a development release, promote it, and publish. Empty catalog data is expected before authors publish.

To run the processes separately, source your reviewed environment file in the appropriate shell and run:

```sh
cargo run -p silicon-apps-server --bin apps-server
```

```sh
python3 -m venv .dev/runner-venv
.dev/runner-venv/bin/pip install -r runner/requirements.txt
.dev/runner-venv/bin/python runner/server.py
```

```sh
npm --prefix web ci
npm --prefix web run dev
```

The API reads environment variables; it does not load `.env` itself. The development helper loads `.env` as shell syntax. Do not source an untrusted environment file.

## Connect Silicon Accounts

The Apps backend authenticates app-scoped Accounts access tokens through the official client, verifies signature/issuer/audience, and reads the current account. Carbons and Silicons are stored by immutable UUID; public account IDs are display names for those identities.

The backend needs two distinct credentials:

| Credential                    | Purpose                                                                                                                                                                 |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `APPS_ACCOUNTS_APP_SECRET`    | Secret for the Accounts app whose ID is exactly `apps`. Used for browser code exchange, one-use token exchange, refresh, user information and account resolution.       |
| `APPS_ACCOUNTS_SERVICE_TOKEN` | Private Accounts integration credential, matching that deployment’s `ACCOUNTS_INTERNAL_TOKEN`. Used for registry availability/import/sync and the private email bridge. |

Neither credential belongs in frontend configuration, JavaScript, a CLI installation, a package runner, or an application package. The browser receives an opaque server-side session cookie. The CLI stores its own user session under its explicit `.apps` home and exchanges one-use Accounts tokens through the Apps backend.

For the production website layout, register **both** callbacks on the `apps` Accounts application:

```text
https://apps.teamofsilicons.com/v1/auth/callback
https://developer.teamofsilicons.com/v1/auth/callback
```

Configure `APPS_ALLOWED_ORIGINS` to contain both origins, and have the reverse proxy overwrite `X-Forwarded-Host` with the actual incoming host. The backend selects only an explicitly allowed origin, records it in the browser-bound OAuth attempt, and exchanges the callback against that origin. Unlisted origins cannot perform cookie-authenticated mutations.

Cookies are HttpOnly, SameSite=Lax and Secure when `APPS_PUBLIC_URL` is HTTPS. They are host-specific: the two websites may each need their own Accounts sign-in redirect. Uploaded media uses relative `/v1/apps/.../media/...` URLs, so private and draft images use the session of the current website. Do not place a shared public CDN cache in front of authenticated API or private artifact responses.

OAuth codes, SLTs and rotating refresh tokens are one-use credentials. Catalog idempotency does not make an already consumed auth credential reusable. If an auth exchange has an unknown outcome, use the appropriate new sign-in/refresh flow rather than repeatedly submitting the consumed credential.

### Import existing Accounts apps

`APPS_IMPORT_ACCOUNTS=1` fetches the private Accounts app registry before the API begins listening and imports missing apps. It defaults to enabled when real authentication and a service token are configured. `APPS_IMPORT_ACCOUNTS=0` explicitly disables startup import.

The import preserves app IDs, Accounts users and sign-in configuration. It brings names, descriptions, logos, website links and accepted author UUIDs into Apps. Existing short legacy IDs, such as `dm`, remain addressable even though **new** Apps IDs require 3–30 characters. Imported apps start as unpublished setup drafts with no fabricated packages or releases. Already imported Apps records are skipped on subsequent starts; import does not overwrite their app-store edits.

Before the first production import:

1. Back up the Accounts database and Apps data directory.
2. Confirm the Accounts registry export returns accepted authors and valid permanent account UUIDs for every existing app.
3. Start Apps with the private service credential and startup import enabled. Invalid/unsafe IDs or missing authors fail startup with an actionable error.
4. Verify original IDs and authors through `apps list --mine`, `apps show APP`, the developer UI and the import history entries.
5. Test existing Accounts sign-in for a migrated app. Import keeps its existing users and secret in Accounts; it does not reveal an old secret to Apps. If an author chooses to rotate the app secret later, save the replacement and update the consuming app immediately.
6. Add real release packages and publish each app when ready. Migration itself is not store publication.

Creates, explicit secret rotations, and accepted authorship changes synchronize through the official Accounts integration. The durable `accounts.sync` outbox retries metadata/author reconciliation. Operator registry migration is a service startup operation, not an end-user CLI command.

## Configure the API

Use [deploy/api.env.example](../deploy/api.env.example) as a starting point. Empty variables are treated as unset.

| Variable                      | Default                               | Meaning                                                                                                 |
| ----------------------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `APPS_BIND`                   | `127.0.0.1:4310`                      | Rust HTTP listener. Use a reverse proxy for public TLS.                                                 |
| `APPS_DATA_DIR`               | `.data`                               | Persistent SQLite, package, media and telemetry files. Use an absolute path in supervised deployments.  |
| `APPS_PUBLIC_URL`             | `http://<bind>`                       | Canonical Apps URL and secure-cookie policy. Set to the actual HTTPS store origin for deployment.       |
| `APPS_ALLOWED_ORIGINS`        | Canonical public origin               | Additional comma-separated HTTP(S) origins, with no path/query/fragment. Include both website origins.  |
| `APPS_DEV_AUTH`               | Off                                   | `1` enables explicit loopback fixture bearer identities. Never expose this mode through a public proxy. |
| `APPS_ACCOUNTS_URL`           | `https://accounts.teamofsilicons.com` | Accounts API/issuer URL.                                                                                |
| `APPS_ACCOUNTS_APP_SECRET`    | Unset                                 | Apps app credential held only by the backend.                                                           |
| `APPS_ACCOUNTS_SERVICE_TOKEN` | Unset                                 | Private registry and mail bridge credential.                                                            |
| `APPS_IMPORT_ACCOUNTS`        | On with real auth + service token     | `1` imports existing registry entries on startup; `0` skips import.                                     |
| `APPS_RUNNER_URL`             | Unset                                 | Worker/gateway base URL; the API appends `/validate`.                                                   |
| `APPS_RUNNER_TOKEN`           | Unset                                 | Bearer credential shared with that worker/gateway.                                                      |
| `APPS_RUNNER_TARGETS`         | Empty                                 | Comma-separated targets for which an execution worker is actually configured.                           |
| `APPS_MAIL_URL`               | Unset                                 | Optional alternative mail adapter. Unset uses the Accounts private mail bridge.                         |
| `APPS_MAIL_TOKEN`             | Unset                                 | Optional bearer credential for the alternative mail adapter.                                            |
| `APPS_TELEMETRY_ENABLED`      | On                                    | `0`, `false`, or `off` disables backend telemetry.                                                      |
| `APPS_TELEMETRY_TABLE_KEY`    | Unset                                 | Space Station table key for the native client integration and local telemetry spool.                    |
| `APPS_TELEMETRY_URL`          | Unset                                 | Optional HTTP telemetry adapter destination.                                                            |
| `SPACE_STATION_URL`           | Space Station client default          | Overrides the native Space Station service URL.                                                         |

The frontend’s preferred production configuration leaves `VITE_APPS_API_URL` unset and proxies `/v1` on each website to the same backend. `APPS_API_PROXY` changes the Vite development proxy target. A cross-origin `VITE_APPS_API_URL` additionally needs correctly configured credentialed proxy/CORS/cookie behavior; the deployment examples use same-origin requests.

### Isolated development authentication

Fixture authentication is useful for testing catalog and CLI behavior without real accounts. It is not an Accounts login or an email-verification substitute. The backend requires a loopback listener, loopback public URL and loopback allowed origins when this mode is enabled.

```sh
APPS_DEV_AUTH=1 \
APPS_BIND=127.0.0.1:4310 \
APPS_PUBLIC_URL=http://127.0.0.1:4311 \
APPS_ALLOWED_ORIGINS=http://127.0.0.1:4311 \
APPS_IMPORT_ACCOUNTS=0 \
APPS_DATA_DIR=.dev/fixture-data \
cargo run -p silicon-apps-server --bin apps-server
```

In another terminal, use an isolated existing home and explicit fixture token:

```sh
mkdir -p .dev/fixture-home
APPS_TOKEN='dev:alice:c:alice' target/debug/apps \
  --home .dev/fixture-home --server http://127.0.0.1:4310 \
  create fixture-app --name 'Fixture app'
```

The UUID in this fixture is `alice`, and its display ID is `c:alice`. No verified email domains are inferred. Browser sign-in still uses Accounts; the frontend has no token-pasting login bypass. Use a separate fixture data directory and do not point a public reverse proxy at this service.

## Provision target validation

The service recognizes nine package targets. Runtime support is determined by real workers, not by the target list in a manifest or `APPS_RUNNER_TARGETS` alone.

| Package target    | Worker needed                                    | Bundled support                                                              |
| ----------------- | ------------------------------------------------ | ---------------------------------------------------------------------------- |
| `linux-x86_64`    | Linux amd64 image/host                           | Docker image mapping to `linux/amd64`.                                       |
| `linux-i686`      | Linux 32-bit x86 image/host                      | Docker image mapping to `linux/386`; provision runtime compatibility.        |
| `linux-aarch64`   | Linux ARM64 image/host                           | Docker image mapping to `linux/arm64`; native host or operator emulator.     |
| `linux-armv7hf`   | Linux ARMv7 hard-float image/host                | Docker image mapping to `linux/arm/v7`; native host or operator emulator.    |
| `windows-x86_64`  | Windows x64 host with Hyper-V container support  | Bundled Docker Hyper-V command path; host/image must be provisioned.         |
| `windows-i686`    | Windows host/image supporting 32-bit executables | Hyper-V `windows/amd64` image with working WOW64; verify compatibility.      |
| `windows-aarch64` | Windows ARM64 host/image with Hyper-V support    | Hyper-V `windows/arm64` command path; runtime availability must be verified. |
| `macos-x86_64`    | Intel macOS worker with `sandbox-exec`           | Bundled native worker on a matching Intel Mac.                               |
| `macos-aarch64`   | Apple Silicon macOS worker with `sandbox-exec`   | Bundled native worker on a matching Apple Silicon Mac.                       |

For a native macOS worker, install `runner/requirements.txt`, generate a shared random token of at least 32 characters, and run `runner/server.py` under a supervisor. The runner executes only its matching native target. Rosetta or an ARM binary’s presence is not proof that another target is natively validated.

For Linux, provision a dedicated host with a container engine and operator-selected validation images. Pin images by digest and pull them before enabling uploads; the worker uses `--pull=never`. `APPS_RUNNER_IMAGES` is a JSON map such as:

```json
{
  "linux-x86_64": "registry.example/validator-amd64@sha256:YOUR_VERIFIED_DIGEST",
  "linux-aarch64": "registry.example/validator-arm64@sha256:YOUR_VERIFIED_DIGEST"
}
```

The image must provide the libraries and interpreter required by the uploaded binary. Cross-architecture execution also needs a separately provisioned emulator; selecting Docker’s `--platform` alone does not install one. A successful build or manifest check cannot replace executing the three required commands on the selected target.

Containers run without a network, as a non-root user, with a read-only package mount/filesystem, a bounded scratch filesystem, dropped capabilities, no-new-privileges and resource limits. Native macOS uses a deny-by-default sandbox. Workers must not receive Accounts secrets, service databases, cloud credentials or the API host’s filesystem. Container-engine access stays on the dedicated worker host. The Linux systemd example uses a shared host scratch directory because a Docker daemon must see the same package path the Python worker mounts.

The same Python process can route each target to a separate worker through `APPS_RUNNER_WORKERS`. Point the API's `APPS_RUNNER_URL` and `APPS_RUNNER_TOKEN` at this gateway, then configure each downstream worker's URL and its own bearer token in the gateway environment:

```json
{
  "macos-aarch64": {
    "url": "https://mac-arm.worker.example/validate",
    "token": "REPLACE_WITH_AT_LEAST_32_RANDOM_CHARACTERS"
  },
  "linux-x86_64": {
    "url": "https://linux-x64.worker.example/validate",
    "token": "REPLACE_WITH_ANOTHER_32_RANDOM_CHARACTERS"
  }
}
```

See [the gateway environment example](../deploy/runner-gateway.env.example). Worker tokens must be at least 32 ASCII characters without whitespace. Remote downstream URLs require HTTPS; only loopback addresses and `localhost` accept HTTP. Forwarding uses a 100-second deadline and a 1 MiB response limit. It does not follow redirects, use environment proxies, or execute locally after a downstream failure. This is a static target router, without a queue or worker autoscaling. Keep gateway and worker listeners on private networks; the bundled HTTP listener needs a TLS reverse proxy for remote access.

On Windows, install Python and the runner requirements on a dedicated Windows host with a compatible container engine and Hyper-V support. Use `APPS_RUNNER_IMAGES` for the Windows targets just as for Linux, with an immutable image digest. The worker requests Hyper-V isolation, no network, an unprivileged `ContainerUser`, a read-only package mount, 1 GiB memory and one CPU; it removes the container after each command. The host and image must support the actual executable architecture and needed runtime libraries. In particular, an ARM64 target mapping does not establish that an appropriate Windows ARM64 container runtime/image exists on the selected machine. A separately supplied worker can implement the protocol when the bundled container path is not available.

Native macOS sandbox execution has been exercised locally. Linux and Windows container command construction and gateway behavior have tests; actual Linux/Windows container execution and every target's runtime compatibility still require verification on the provisioned hosts. Do not advertise unverified targets through `APPS_RUNNER_TARGETS`.

A worker receives `app_id`, `target`, `package_sha256`, `package_base64` and `manifest` at authenticated `POST /validate`. It verifies the archive and returns:

```json
{
  "isolated": true,
  "target": "macos-aarch64",
  "validation": [
    {
      "command": "--help",
      "exit_code": 0,
      "stdout": "Example help",
      "stderr": "",
      "passed": true,
      "expected": "Exit 0 and nonempty help text."
    }
  ]
}
```

The example omits the other two entries for brevity; every real response must include `accounts --json` and `login status --json`. The Apps API independently checks the result, requires the matching `app_id`, and expects `authenticated:false` in the clean signed-out validation environment. It persists exact command output and failed validation history. Missing or unavailable workers fail uploads instead of accepting untested packages. Installation scripts are not executed during upload validation.

The package library permits at most 512 MiB compressed and 1 GiB expanded archives. The bundled runner is stricter: 128 MiB compressed, 512 MiB expanded and 20,000 entries. The effective upload limit is the smallest configured proxy/API/worker limit. Media uploads accept PNG, JPEG, WebP, GIF, MP4 or WebM up to 100 MiB; new SVG uploads are rejected. Existing Accounts inline logos are handled by the migration path.

## Mail, invitations and telemetry

Email and `c:id` author invitations use a durable outbox. The preferred transport is Accounts’ private `/v1/internal/apps/mail` bridge, authenticated with the service token. Accounts resolves a Carbon UUID to its verified email and sends the invitation; private contact data is not returned to Apps. A Carbon without a verified email cannot receive the corresponding email delivery until that is corrected. Silicon invitations remain account invitations and do not invent an email address.

Accounts must have its real mail provider configured and its delivery worker running. An Apps outbox acknowledgement or Accounts `queued` response is not proof of inbox delivery. Check the destination delivery state for an invitation and a test bug report before declaring mail operational. Bug reports go to `saketdev12@gmail.com`, `shubhastro2@gmail.com`, and `bugs@teamofsilicons.com`.

Set `ACCOUNTS_SILICON_APPS_URL` in the **Accounts service's** environment to the website users can reach: `http://127.0.0.1:4311` for a local stack, or `https://apps.teamofsilicons.com` for the deployed store. Accounts appends `/developer/invitations` for Apps invitation links. This setting is separate from `ACCOUNTS_DEVELOPER_URL`, which belongs to the Accounts developer interface and its own callbacks.

An optional `APPS_MAIL_URL` adapter receives `{id,kind,data}` with `Idempotency-Key: <outbox-id>` and the optional mail bearer token. It must implement idempotent processing for `mail.invite` and `mail.report`, including private Carbon contact resolution where needed. The durable worker retries undelivered records every ten seconds. Inspect `outbox.attempts`, `last_error` and `delivered_at`; do not delete pending records to hide a delivery failure.

For the native Space Station integration, set `APPS_TELEMETRY_TABLE_KEY` and optionally `SPACE_STATION_URL`. The service writes source, route template, step, progress and operation context; frontend input is allowlisted and raw identities/secrets are excluded. The CLI also uses `APPS_TELEMETRY_TABLE_KEY` and its selected `.apps/telemetry` spool; `APPS_TELEMETRY_KEY` remains a legacy alias. Telemetry is opted in by default, but no configured destination means no remote delivery. The frontend returns to normal work even when telemetry is unavailable.

Browser settings persist telemetry opt-out locally and send `X-Apps-Telemetry: off`. CLI users run `apps config telemetry off`; operators can disable service telemetry with `APPS_TELEMETRY_ENABLED=false`. Registered platform counts are maintained separately from diagnostic telemetry. The Packages step’s market counts describe distinct observed registered accounts, including deduplicated reach across selected targets; they do not estimate every ecosystem user.

## Host both websites

Build the frontend with `npm --prefix web ci` and `npm --prefix web run build`. Install `web/dist` as static files, with history fallback to `index.html`. The root route chooses the developer workspace on a `developer.*` hostname and the store on other hosts. Explicit `/developer`, `/store`, `/docs` and `/settings` routes work on either host and locally.

[deploy/Caddyfile](../deploy/Caddyfile) serves the same build at `apps.teamofsilicons.com` and `developer.teamofsilicons.com`, proxies `/v1/*` and `/health` to the API, and overwrites the forwarded host. Set both DNS records, configure TLS, register both Accounts callbacks, and use the exact allowed origins from [the API environment example](../deploy/api.env.example). Keep backend and runner listeners private. The API systemd unit is distinct from the per-user CLI updater service.

Use one API instance per persistent SQLite data directory. The catalog and mutation gate are process-local around one SQLite store; these examples do not establish a multi-replica write architecture. A reverse proxy and restart supervisor are included as examples, not a claim of high availability.

## CLI state and updates

Home selection has this precedence:

1. `apps --home EXISTING_DIRECTORY ...`
2. `SILICON_HOME`
3. the location saved by `apps config home EXISTING_DIRECTORY`
4. the operating system’s normal home directory

The selected directory must already exist and be a directory. State is then under `<selected-home>/.apps`. The saved-home pointer is in the normal home’s `.apps/home`; setting it does not migrate another home’s sessions or installed files. Use the same explicit home for installation, login, update and daemon commands.

The primary Rust HTTP client does not discover a home or write state on construction. Its installation/authentication/updater adapters accept an explicit `LocalState`. CLI configuration is persisted in `config.json`; installed records in `installed.json`; commands in `bin/`; package trees in `installed/`; sessions, locks and delivery receipts remain under the same `.apps` root. Unix session/state files use owner-only permissions.

Since 0.1.1, a saved login session is scoped to the exact Apps and Accounts service URLs, including a tenant path. Changing either endpoint requires a matching session or a new login; credentials are not sent to a newly selected service. Installed app records are also bound to their registry source. The updater refuses to resolve an existing install against a different registry merely because `apps config server` changed.

For a 0.1.0 home, run `apps login` again after upgrading because the old session has no trusted service binding. For an old installed record without a registry source, select the intended server and explicitly reinstall that app with `apps install APP --yes` to bind it. Review the registry and channel before confirming. Keep a backup of the original state until fresh login, installation and an update check succeed.

A successful CLI install attempts to start the updater. For persistence across login/restart, review and install its native service:

```sh
apps --home /existing/home config server https://apps.teamofsilicons.com
apps --home /existing/home daemon definition
apps --home /existing/home daemon install
apps --home /existing/home daemon status --json
```

The generated service embeds the exact executable and `--home` paths. Keep that executable installed, and reinstall the service definition if the executable or home is moved. Native service names are per operating-system user, so do not register competing startup services for multiple Apps homes. `daemon start` starts or reactivates the configured updater, `daemon stop` stops after the current installation, and `daemon remove` removes startup registration. `daemon run --once` is useful for a supervised verification pass.

The default interval is 60 seconds. `apps config set update_interval_seconds N` changes it, with a minimum of ten seconds. Each app follows its installed production or development channel, including `apps` itself when installed from the store. An exact-version install is not a permanent pin. `apps update [APP]` performs an immediate check; inspect per-app failures and `.apps/updater.json`/`updater.log` rather than treating a running daemon as proof every app updated.

Channel changes prompt before switching; `--yes` explicitly allows a noninteractive switch. Install scripts require explicit consent with `--allow-install-script`; that consent is retained for that installed app’s updates. An install stages and verifies the archive, preserves a backup, checks command ownership and restores the previous installation on managed-file errors. A script’s external side effects cannot be undone by archive rollback.

On Windows, updater execution uses a separate copy so the installed executable can be replaced. Interactive self-install may return `scheduled`; inspect `apps installed` and `.apps/self-update.log` for completion. Native service activation and reboot persistence still need verification on the actual Windows host.

## Build and deliver releases

Development and production releases each use independent strict `x.y.z` versions. Upload validated packages, create a development release from their IDs, and promote that release with a production version. A new release reuses the app’s existing metadata, authorship and access. Publishing an app with only development releases is allowed; default production installs will explain that no production release exists until promotion.

The CLI artifact helper builds a target-specific `apps.yaml` archive and a SHA-256 sidecar:

```sh
bash scripts/build-release.sh --target macos-aarch64 --output dist --version 0.1.2
```

Use the version declared by the workspace. A version argument does not rewrite Cargo package metadata. Cross compilation needs the matching Rust target, system linker/runtime and any native dependencies; a target name is not a provisioned toolchain.

`.github/workflows/package-artifacts.yml` is manually triggered and builds native Linux x64, macOS ARM64 and Windows x64 artifacts using [GitHub's hosted runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners). It does not claim all nine targets, upload to the store, promote releases, publish crates or create remote releases. The CI artifact retention period is not a permanent distribution channel.

To release to users, first verify the package on its actual isolated target runner, then upload to the `apps` app, create a development release, test fresh install/update/uninstall and service behavior, promote it, and verify public resolution/download checksums. Only after artifacts are published at their real distribution URLs should bootstrap installers be advertised as a working network install. Their local-archive modes can be used before public artifact hosting exists.

The bootstrap installers verify the archive checksum before starting the bundled executable, then delegate installation and store registration to the Rust CLI. To inspect a local artifact without installing a startup service for the real operating-system user:

```sh
bash scripts/install.sh \
  --archive ./dist/apps-0.1.2-macos-aarch64.tar.gz \
  --sha256 TRUSTED_64_HEX_SHA256_DIGEST \
  --home /existing/test-home \
  --server http://127.0.0.1:4310 \
  --no-startup
```

Use the artifact for the current host and its trusted checksum. Windows uses `scripts/install.ps1 -Archive PATH -Sha256 DIGEST -HomeDirectory PATH -Server URL -NoStartup`. The home must already exist. `--no-startup` / `-NoStartup` suppresses native startup registration; a successful installation may still start its updater process, which can be stopped with `apps --home PATH daemon stop`. Without this option, the installers also register the native updater service. A temporary `--home` does not isolate that service registration from the real operating-system user.

Network bootstrap mode selects a release by `--version` (PowerShell `-Version`) or the latest GitHub release and uses the published archive and `.sha256` sidecar. The POSIX installer accepts `APPS_RELEASES_URL` to override the HTTPS releases base; the PowerShell installer uses the project's GitHub releases. Do not use a checksum from an untrusted source to authorize executable bootstrap.

For crates.io distribution, publish dependency crates in order: a compatible official `silicon-accounts-client`, `silicon-apps-package`, `silicon-apps-client`, then `silicon-apps-cli`. Run `cargo package`/publish validation against the registry-resolvable versions first. A workspace path dependency or successful local build does not make its unpublished matching version available on crates.io. No registry publication is performed by the checked-in workflows.

## Back up, restore and diagnose

`APPS_DATA_DIR/apps.sqlite` contains the catalog, author/invite/release/history records, browser sessions, OAuth attempts, idempotency responses and delivery outbox. Packages are addressed by checksum under `packages/`; media and MIME metadata live under `media/`; native telemetry may have a local spool. Treat the whole directory as private service data: session tokens and short-lived secret replay responses are present. Preserve the database and matching artifacts together.

Take a consistent backup using SQLite’s backup API or stop the API and copy the entire directory. Copying only a live database file while ignoring WAL state is not a consistent backup. Restore to a separate reviewed location and test catalog, private access and artifact downloads before switching traffic. Keep Accounts backups and deployment versions available because app registration, users and webhook delivery remain owned by Accounts.

Replays of ordinary catalog mutations are durable. Create/rotate/webhook responses containing a secret are replayable for ten minutes; then plaintext is removed and retry returns `secret_replay_expired` without executing the action again. Save one-time secrets promptly. Never log them in release notes, reports or CI output.

`GET /health` proves the process responds, not that Accounts, workers, mail or Space Station are functioning. A complete deployment verification exercises:

- Accounts browser sign-in on each hostname, CLI login/refresh/logout, invalid-audience rejection, and sign-in return paths.
- Real app creation and Accounts registration, imported IDs/users, accepted authors and admin-only access changes.
- Public anonymous browsing/install and private account/domain filtering, including direct download/media access.
- Actual validation for every advertised target and exact three-command failure output.
- Development/production promotion, fresh installation, receipt counting, channel switching, update and uninstall.
- Incoming/cancelled/declined invitations, private email delivery, one review per account and persisted audit history.
- Accounts webhook generation before the endpoint, saved subscriptions without unexpected rotation, signed delivery, and explicit secret rotation.
- Telemetry opt-out and a configured destination’s actual receipt.
- Persistent data recovery and the installed updater’s behavior after a host reboot.

These checks depend on operator-provisioned services and hardware. Local unit tests, fixture browser tests and a successful web build remain useful evidence, but do not substitute for these external outcomes.
