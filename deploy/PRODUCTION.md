# Native production deployment

The production topology is one ARM64 API/store host and one separate x86_64
validation host in AWS `us-east-2`. `stack.json` provisions the hosts, a stable
public address, private versioned artifact storage, scoped instance roles and
SSM administration. There are no inbound SSH rules. Only the API host can reach
the worker port. The worker cannot read the API's Accounts credentials.

The API host runs three services behind Caddy at `apps.teamofsilicons.com`:

| service | listens on | runs |
|---|---|---|
| `silicon-apps-api` | `127.0.0.1:4310` | `bin/apps-server` as `silicon-apps`, data in `/var/lib/silicon-apps` |
| `silicon-apps-store` | `127.0.0.1:4320` | the server-rendered store, `node/bin/node store/server.js` as `silicon-apps-store` |
| `caddy` | `:80`, `:443` | `bin/caddy` with `deploy/Caddyfile` as `caddy` |

Caddy sends the event streams (`/v1/events/stream`, `/v1/apps/{app_id}/events/stream`)
to the API unbuffered, then `/v1/*`, `/health`, `/openapi.json` and the
`/.well-known/agent.json`, `agent-card.json` and `silicon-apps-keys.json`
documents to the API, serves `/install.sh` and `/install.ps1` from `web/` and
the store's hashed `/_next/static/*` files from disk, and sends every other
path to the store. The store sets its own Content-Security-Policy with a nonce
per response; API and installer responses get a strict policy from Caddy.

The common Apps and Accounts developer frontend remains in
`silicon-accounts/developer` and runs on the Accounts host at
`developers.teamofsilicons.com`. It does not serve store discovery. The
Accounts service owns sign-in, email delivery and account updates. Register the
`apps` application against an existing, verified Carbon owner, including both
callback URLs in the operations guide. Do not create an account or verify an
email as a shortcut to registration. An authenticated owner is also required to
publish the CLI in the Apps catalog; publishing GitHub archives alone does not
publish a catalog app.

## Build and stage

1. Run the repository checks (Rust workspace, `pnpm --dir store typecheck`,
   `pnpm --dir store lint`, and `python3 -m unittest discover -s deploy -p 'test_*.py'`).
   Build the API for `aarch64-unknown-linux-gnu.2.34` with `cargo zigbuild --release --locked
   --target aarch64-unknown-linux-gnu.2.34 -p silicon-apps-server --bin apps-server`.
2. Commit reviewed changes, including everything under `store/`. Run
   `python3 deploy/package.py --caddy /path/to/caddy_2.11.7_linux_arm64.tar.gz`.
   The packager refuses a dirty tree or untracked files under `store/`,
   `deploy/`, `runner/` and `scripts/`, then:
   - runs `pnpm install --frozen-lockfile` and `pnpm run build` in `store/`
     (`--prebuilt-store` packages an existing `store/.next` build instead and
     records that in the manifest);
   - copies the Next.js standalone server with its pnpm links preserved, plus
     `.next/static` and `public/`, and refuses broken, absolute or escaping
     links and any `.env` file in the build;
   - takes Node.js 24.21.0 for Linux ARM64 from the official archive, reusing
     `.dev/production/node-v24.21.0-linux-arm64.tar.xz` or the copy under
     `silicon-accounts/.dev/production`, or downloading it, and always checks its
     pinned SHA-256 (only `bin/node` and its licence are bundled);
   - checks Caddy's pinned checksum and puts the installers from `scripts/` at
     `web/install.sh` and `web/install.ps1`.

   It writes an immutable archive and a receipt in `.dev/production`. The
   archive's `build.json` lists every file's hash and every symlink's target. It
   contains no runtime credentials or development databases. Pass `--api` when
   the executable was built into another target directory, and `--node` to use a
   specific Node.js archive.
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

On the API host the installer, in order:

1. validates the runtime secret, including `APPS_SIGNING_KEYS` (below), and
   stops with a clear message before anything is installed if it is unusable;
2. extracts the archive and verifies every file hash and symlink target;
3. creates the `silicon-apps-store` system user, validates the candidate
   Caddyfile, and runs the candidate store from its release directory on
   `127.0.0.1:4321` with the store unit's user and sandbox against the running
   API. `GET /` must answer 200 with the server-rendered page, or nothing live
   changes;
4. stops the API and uploads a backup when a catalog exists, writes the unit
   files, switches `current`, restarts the API and waits up to five minutes
   for `/health` (the first signed start reads every package first), checks
   that `/.well-known/silicon-apps-keys.json` publishes the configured key,
   restarts the store and waits for `GET /` to render, then restarts Caddy and
   the backup timer, and records `deployment.json`.

Any failure after step 3 restores the previous unit files, `current` and the
service states. The store unit is `deploy/silicon-apps-store.service`: no
secrets, no writable paths, `ProtectSystem=strict`, `NoNewPrivileges`, an empty
capability set, and only `APPS_API_URL=http://127.0.0.1:4310` and
`STORE_PUBLIC_URL=https://apps.teamofsilicons.com` as settings.

## Required runtime configuration

The API secret requires `APPS_ACCOUNTS_APP_SECRET`,
`APPS_ACCOUNTS_SERVICE_TOKEN`, `APPS_RUNNER_URL`, `APPS_RUNNER_TOKEN`,
`APPS_DEV_AUTH=0`, the production Accounts URL, the public Apps URL, both
allowed origins (`APPS_ALLOWED_ORIGINS` must include
`https://apps.teamofsilicons.com`, the Origin the store sends with every visitor
write) and `APPS_SIGNING_KEYS`. Its data directory is `/var/lib/silicon-apps`.
The worker secret contains only its runner token. Configure a dedicated Space
Station table key for telemetry. Mail defaults to the official Accounts
integration. The store needs no secret.

`APPS_HISTORICAL_APP_IDS` is optional. It reserves historical Silicon Accounts
app IDs of 1 or 2 characters, which new apps cannot use, for the one account
allowed to create each: comma-separated `app_id:owner_uuid` entries, with the
owner's case-sensitive Accounts UUID. Only that account, signed in, sees the ID
as available and can create it (`silicon-apps create dm --name DM`); to everyone
else it is an invalid ID, so the list is never revealed. The ID must not be in
the Accounts registry, or it is taken. For DM the value is `dm:zQo`, reserving
`dm` for the account with UUID `zQo`. The installer writes every key of the
secret to `/etc/silicon-apps/api.env` during an install, so add the key before
installing the release that reads it; the API in service ignores keys it does
not know. Add it without printing the other values:

```sh
python3 - <<'PY'
import json, subprocess
arn = 'arn:aws:secretsmanager:us-east-2:234951665042:secret:silicon-apps/production/runtime-XDvPON'
aws = ['aws', '--profile', 'silicon-production', '--region', 'us-east-2', 'secretsmanager']
current = json.loads(subprocess.run(aws + ['get-secret-value', '--secret-id', arn, '--output', 'json'],
                                    check=True, capture_output=True, text=True).stdout)['SecretString']
secret = json.loads(current)
secret['APPS_HISTORICAL_APP_IDS'] = 'dm:zQo'
subprocess.run(aws + ['put-secret-value', '--secret-id', arn, '--secret-string', 'file:///dev/stdin'],
               input=json.dumps(secret), check=True, capture_output=True, text=True)
print('APPS_HISTORICAL_APP_IDS set; keys now:', ', '.join(sorted(secret)))
PY
```

The installer does not check this value. A malformed one stops the API at
startup with `invalid_historical_app_ids` and a message naming the entry, the
health wait of up to five minutes fails, and the installer rolls back to the
previous release and environment file. A valid one is logged at startup as
`Historical app IDs, each creatable only by its configured owner: dm.` Once the
app exists the entry has no further effect. See
[the operations guide](../docs/operations.md#reserve-a-historical-app-id-for-its-owner).

Set `APPS_RUNNER_TARGETS` only to execution environments that have actually
been deployed and verified (below). Other native release archives do not imply
other upload workers exist. macOS and Windows workers follow the isolation and
forwarding contracts in `runner/README.md`.

## Validation worker (four Linux targets)

The x86_64 worker validates `linux-x86_64`, `linux-i686`, `linux-aarch64` and
`linux-armv7hf` uploads. `deploy/install.py` pins one immutable per-platform
image for each (`WORKER_IMAGES`), all from the official
`python:3.14-slim-trixie` index the worker already used for `linux-x86_64`
(`python@sha256:f85c5697...55d2`, Python 3.14.8). The `linux-x86_64` image is
that index's `linux/amd64` manifest, so its bytes did not change. The runtime
includes CA certificates and a glibc compatible with Ubuntu 24.04 builds.

| target | Docker platform | how it runs on the x86_64 host |
|---|---|---|
| `linux-x86_64` | `linux/amd64` | natively |
| `linux-i686` | `linux/386` | natively, as a 32-bit process (the kernel needs 32-bit x86 support) |
| `linux-aarch64` | `linux/arm64` | QEMU user-mode emulation (`qemu-aarch64`) |
| `linux-armv7hf` | `linux/arm/v7` | QEMU user-mode emulation (`qemu-arm`) |

Emulation is registered for those two architectures only, by the official
`tonistiigi/binfmt` image pinned to the `linux/amd64` manifest of
`qemu-v10.2.3-68` (`BINFMT_IMAGE`). It registers each entry in
`/proc/sys/fs/binfmt_misc` with the F (fix binary) flag: the kernel opens the
emulator once, so every validation container can use it without the emulator
in its image, network or filesystem. Registrations are kernel state that a
reboot clears, so the installer adds `silicon-apps-binfmt.service`, a oneshot
unit that registers again at boot from the local pinned image
(`--pull=never`), checks that both entries are enabled with the F flag, and
runs before the worker. The worker unit requires it: if emulation cannot be
registered the worker stays down, and uploads get the retryable "worker
unavailable" answer instead of ARM packages failing validation.

On the worker host the installer, in order:

1. validates the runner token, extracts and verifies the archive, and prepares
   the revision's Python environment, as before;
2. pulls the four pinned images and the binfmt image, each with `--platform`;
3. registers `qemu-aarch64` and `qemu-arm` by running the binfmt image once,
   touching only those two entries, then requires both to be enabled with the
   F flag. Entries that already are (a live worker may be using them) are left
   until step 5;
4. self-checks every target: `python3` in each pinned image, under its platform
   and with the runner's container isolation, must report the expected
   `uname -m` (`x86_64`; `i686` or `x86_64`; `aarch64`; `armv7l` or `armv8l`)
   and word size (64, 32, 64, 32 bits). Any failure names the target, its exit
   code and error, and stops the install before any file or service changes;
5. writes `worker.env` with all four images in `APPS_RUNNER_IMAGES`, the worker
   and binfmt units and `/etc/silicon-apps/worker-image.json` (the four images,
   the index and the emulator image), switches `current`, enables and restarts
   `silicon-apps-binfmt` (proving the boot unit and registering from the pinned
   image; systemd stops the worker meanwhile), restarts the worker, waits
   for its 401 readiness answer, and runs the self-check again on the
   registrations the unit made. A failure here restores the previous files,
   `current` and service states; the two emulator registrations stay, since
   they only let ARM programs run under QEMU. The installer prints each
   target's result under `platforms`.

Isolation flags, limits and tokens are unchanged. Keep the pinned images on the
host: the runner and the boot unit never pull, so `docker image prune -a` would
make validation fail. To move to a newer runtime, resolve the new index's four
per-platform digests (`docker buildx imagetools inspect` or the registry API),
update `WORKER_IMAGES` and its comment, and release the worker.

After a successful worker install, run `deploy/verify-worker.py --target T` on
the worker with a checksum-verified package for each Linux target, then add the
verified targets to the API secret's `APPS_RUNNER_TARGETS` (comma separated)
and restart the API. A target the API lists but the worker has not verified
must not be enabled; `production.json` records the targets actually live.

## Release signing key

Every package Apps serves is signed with Ed25519, and the API refuses to start
without `APPS_SIGNING_KEYS` (`key_id:base64-seed` entries, comma separated,
newest first; `APPS_REVOKED_SIGNING_KEYS` lists key IDs CLIs must stop
trusting). The production key ID is `apps-2026-10`; its public key is pinned in
the CLI (`crates/client/src/signing.rs`) and mirrored in `deploy/install.py`,
which derives the public key from the seed in the secret and refuses a seed that
does not match. A wrong key would otherwise be recorded under that ID for good
and every install would fail verification.

The seed lives only in `~/.silicon-release/silicon-apps/release-signing-key.json`
on the operator's machine (the output of `apps-server signing-key generate
--key-id apps-2026-10`). Never print it, paste it into a terminal, commit it,
or put it in SSM command text. Add it to the runtime secret from that file
without displaying it:

```sh
python3 - <<'PY'
import json, pathlib, subprocess
arn = 'arn:aws:secretsmanager:us-east-2:234951665042:secret:silicon-apps/production/runtime-XDvPON'
aws = ['aws', '--profile', 'silicon-production', '--region', 'us-east-2', 'secretsmanager']
key = json.loads(pathlib.Path('~/.silicon-release/silicon-apps/release-signing-key.json').expanduser().read_text())
entry = key['env'].split('=', 1)[1]
assert entry.startswith('apps-2026-10:'), 'unexpected key file shape'
current = json.loads(subprocess.run(aws + ['get-secret-value', '--secret-id', arn, '--output', 'json'],
                                    check=True, capture_output=True, text=True).stdout)['SecretString']
secret = json.loads(current)
secret['APPS_SIGNING_KEYS'] = entry
subprocess.run(aws + ['put-secret-value', '--secret-id', arn, '--secret-string', 'file:///dev/stdin'],
               input=json.dumps(secret), check=True, capture_output=True, text=True)
print('APPS_SIGNING_KEYS set; keys now:', ', '.join(sorted(secret)))
PY
```

The first start of an API with a signing key reads every stored package's
install script and signs every existing release with the active key, then saves
the catalog (the journal shows `Release signing: read N package install scripts
and signed M release packages with apps-2026-10`). Take an on-demand backup
before installing, and note its key; the installer also uploads one after
stopping the API. After the release, `/.well-known/silicon-apps-keys.json`
must list `apps-2026-10` as active with the pinned public key, and a fresh
`silicon-apps install` must verify.

## DNS and verification

Point `apps.teamofsilicons.com` to the Apps API host and
`developers.teamofsilicons.com` to the Accounts host serving the common developer
frontend. Preserve all unrelated DNS records, including mail records. The
singular `developer.teamofsilicons.com` and old store `/developer` and `/docs`
paths redirect to the shared portal. Old store addresses (`/store`,
`/store/{app_id}`) are redirected by the store itself. Caddy obtains and renews
the certificates.

Before accepting traffic:

- Verify the deployed executable hash, revision receipt, the three systemd
  services, local API health, the store's `GET /` on `127.0.0.1:4320` and public
  HTTPS responses for both domains.
- Run `deploy/verify-worker.py` on the worker with a checksum-verified release
  archive, once per enabled Linux target (`--target`). It runs all three
  discovery commands inside the sandbox, retains their exact output and
  verifies unauthenticated requests return 401.
- Check the public routes: pages and agent files are served by the store with
  a nonce CSP, `/_next/static/*` is immutable and a missing file is a 404, the
  installers are byte-identical to `scripts/` as `text/plain` with `no-cache`,
  `/health`, `/openapi.json`, `/.well-known/agent.json` and
  `/.well-known/silicon-apps-keys.json` come from the API with a strict CSP, and
  `/developer` and `/docs` answer 308. `deploy/test_caddy.py` runs the same
  routing against a local Caddy (`APPS_TEST_CADDY=/path/to/caddy`).
- Check the common developer frontend and a real Accounts login.
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
schema change is not backward compatible. The first signed release adds
signatures, event and subscription data to the catalog: to go back to a release
before it, restore the pre-deploy backup together with that release. A store
problem alone never needs a data restore.
