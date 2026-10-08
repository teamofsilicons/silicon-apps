# Native production deployment

The production topology is one ARM64 API/web host and one separate x86_64
validation host in AWS `us-east-2`. `stack.json` provisions the hosts, a stable
public address, private versioned artifact storage, scoped instance roles and
SSM administration. There are no inbound SSH rules. Only the API host can reach
the worker port. The worker cannot read the API's Accounts credentials.

The API serves the store through Caddy at `apps.teamofsilicons.com`. The common
Apps and Accounts developer frontend remains in `silicon-accounts/developer`
and runs on the Accounts host at `developers.teamofsilicons.com`. It preserves
Accounts configuration and adds Apps authoring within the same app workspace;
it does not serve store discovery. The Accounts service owns sign-in, email
delivery and account updates. Register the `apps`
application against an existing, verified Carbon owner, including both callback
URLs in the operations guide. Do not create an account or verify an email as a shortcut to
registration. An authenticated owner is also required to publish the CLI in the
Apps catalog; publishing GitHub archives alone does not publish a catalog app.

## Build and stage

1. Run the repository checks, build the API for
   `aarch64-unknown-linux-gnu.2.34` with `cargo zigbuild --release --locked
   --target aarch64-unknown-linux-gnu.2.34 -p silicon-apps-server --bin apps-server`,
   and run `npm --prefix web run build` with `VITE_APPS_API_URL` unset.
2. Commit reviewed changes. Run `python3 deploy/package.py --caddy
   /path/to/caddy_2.11.7_linux_arm64.tar.gz`. The packager checks Caddy's pinned
   checksum and writes an immutable archive plus a receipt in `.dev/production`.
   The archive contains a file-by-file hash manifest, API executable, static
   website, worker and deployment tools. It contains no runtime credentials or
   development databases.
3. Upload that archive under `releases/<revision>.tar.gz` in the stack's private
   artifact bucket. Retain its receipt, full source revision and checksum.
4. Through SSM, download the archive on the intended host, verify its checksum
   **before** extracting or running its installer, then invoke `deploy/install.py`
   with `--archive`, `--sha256`, `--revision`, `--role`, `--secret` and `--bucket`.
   The secret argument is an ARN; never put secret values in SSM command text.
   `deploy/ssm.py INSTANCE SCRIPT` executes a reviewed, secret-free command file.

The installer reads runtime values with the instance's own role and writes
root-readable environment files. Complete releases live at
`/opt/silicon-apps/releases/<revision>`; an atomic `current` link chooses the live
version. The installer records the exact archive hash and revision in
`/opt/silicon-apps/deployment.json`. A healthy version endpoint alone is not proof
that the intended revision was deployed.

## Required runtime configuration

The API secret requires `APPS_ACCOUNTS_APP_SECRET`,
`APPS_ACCOUNTS_SERVICE_TOKEN`, `APPS_RUNNER_URL`, `APPS_RUNNER_TOKEN`,
`APPS_DEV_AUTH=0`, the production Accounts URL, the public Apps URL and both
allowed origins. Its data directory is `/var/lib/silicon-apps`. The worker secret
contains only its runner token. Configure a dedicated Space Station table key
for telemetry. Mail defaults to the official Accounts integration.

Initially this installer provisions `linux-x86_64` execution only. It pulls
the official `python:3.14-slim-trixie` runtime (including CA certificates) and
pins its resolved immutable image digest. Set
`APPS_RUNNER_TARGETS` only to execution environments that have actually been
deployed and verified. Other native release archives do not imply other upload
workers exist. Additional Linux, macOS and Windows workers follow the isolation
and forwarding contracts in `runner/README.md`.

## DNS and verification

Point `apps.teamofsilicons.com` to the Apps API host and
`developers.teamofsilicons.com` to the Accounts host serving the common developer
frontend. Preserve all unrelated DNS records, including mail records. Update
the developer application's registered callback to the plural origin before
cutover, retaining old callbacks while existing flows finish. The singular
`developer.teamofsilicons.com` and old store `/developer` paths redirect to the
shared portal. Caddy obtains and renews the certificates.

Before accepting traffic:

- Verify the deployed executable hash, revision receipt, systemd services, local
  API health and public HTTPS responses for both domains.
- Run `deploy/verify-worker.py` on the worker with a checksum-verified release
  archive. It runs all three discovery commands inside the sandbox, retains
  their exact output and verifies unauthenticated requests return 401.
- Run `node web/scripts/verify-production.mjs https://apps.teamofsilicons.com`,
  the common developer frontend checks, and a real Accounts login.
- Publish via the authenticated author API/CLI, check anonymous public discovery
  and private access denial, and install from a fresh isolated client home.
- Verify hourly backups and a restore into a separate scratch directory before
  calling recovery tested.

## Backups and rollback

The installer configures an hourly systemd backup timer. `backup.py` uses
SQLite's backup API and an integrity check, then includes immutable package and
media objects in a private S3 archive. It selects only committed hash-named
objects and MIME sidecars, skipping temporary uploads before inspecting files.
Duplicate uploads never replace committed objects. It refuses to create an empty database
when the source is absent. The stack retains the bucket and host data volumes;
backup objects expire after 14 days.

For a restore, stop the API, preserve the current state, download the chosen
backup through an authorized operator, verify it, and restore the database,
packages and media together. Keep files owned by `silicon-apps`, directory mode
0700 and database mode 0600. Never restore production data into the development
fixture directory. Restart and verify database integrity, actual catalog
contents and authenticated behavior before restoring traffic.

Retain the prior release until live verification is complete. Code rollback
cannot undo a database migration by itself; restore a compatible backup when a
schema change is not backward compatible.
