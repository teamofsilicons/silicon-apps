# Silicon Apps

Create, publish, discover, and install command-line apps for Carbons and Silicons. This repository contains the Apps service, its primary Rust client, package tooling, the `apps` CLI and updater, and the store and developer frontend.

The requirements live in [understanding/UNDERSTANDING.md](understanding/UNDERSTANDING.md). The implementation does not add a manual publication review: authors publish when the required details and validated release packages are ready.

## Run locally

Requirements: Rust 1.98 or newer, a C compiler, Node.js 22.12 or newer, npm, and Python 3.11 or newer for the package runner. The official Silicon Accounts Rust client is vendored in `vendor/silicon-accounts-client`; building Apps does not require a sibling checkout. Real sign-in and authoring require a running Silicon Accounts deployment with the Apps integration described in [operations](docs/operations.md#connect-silicon-accounts).

From this directory:

```sh
cp deploy/local.env.example .env
# Configure the Accounts URL, Apps secret, and private service token in .env.
bash scripts/dev.sh
```

Open [the store](http://127.0.0.1:4311/store) or [the developer platform](http://127.0.0.1:4311/developer). The API listens at `127.0.0.1:4310`; Vite proxies `/v1` to it. With a runner token configured, the helper also starts the isolated runner at `127.0.0.1:4312`. Service logs are under `.dev/logs/`.

Without configured Accounts or runners, public browsing still works, but there are initially no published apps. Sign-in, global app creation, and package validation report their missing dependencies. The frontend does not insert demo listings. For a CLI-only isolated fixture environment, see [development authentication](docs/operations.md#isolated-development-authentication).

## Use the CLI

Build and install the CLI from this checkout:

```sh
cargo install --path crates/cli --locked
apps --help
apps accounts --json
apps login status --json
```

Point the CLI at a local stack explicitly:

```sh
apps config server http://127.0.0.1:4310
apps config accounts http://127.0.0.1:YOUR_ACCOUNTS_PORT
apps login
apps search
```

Public search and public production installs work without login. Installed commands live in the selected home’s `.apps/bin`; add that directory to your shell’s `PATH`.

```sh
apps install ring
apps install 'ring>dev'
apps install 'ring@1.2.3'
apps install 'ring>dev@0.4.0'
apps installed
apps daemon install
apps daemon status --json
```

`ring` is an example identifier; use an app that exists on your configured service. Exact versions choose the initial release, and subsequent updates follow the installed channel. `apps daemon install` configures launchd, systemd user services, or Windows Task Scheduler. See [state and updates](docs/operations.md#cli-state-and-updates) before using multiple homes or moving a CLI executable.

## Publish an app

```sh
apps login
apps create ring --name Ring
# Save the app_secret immediately; it is shown once.
apps setup ring details --description-file description.txt --tags tools,productivity
apps setup ring access --visibility public
apps validate ./package
apps pack ./package --output ./ring.tar.gz
apps upload ring --target macos-aarch64 ./ring.tar.gz
apps release ring --version 0.1.0 --package PACKAGE_ID
apps promote ring DEVELOPMENT_RELEASE_ID --version 1.0.0
apps readiness ring
apps publish ring
```

Use the actual package and release IDs returned by the preceding commands. The description must contain 200–600 characters. New app IDs contain 3–30 lowercase letters, digits, hyphens or underscores and cannot change. An app needs at least one validated package in a release before publishing; promote a release to production for the default installation command to work.

Every target’s binary must provide `--help`, `accounts --json` containing its `app_id`, and `login status --json`. Local `validate` checks manifest/files/archive safety; upload executes the three commands through a configured isolated target runner. An unavailable runner does not count as a passed validation.

The developer website exposes the same operations through seven saved setup steps. Optional links, media, Accounts webhook updates, co-authors and private access can be configured through either interface. Explore each command with `--help` or read `apps docs publish`.

## Packages and interfaces

| Path                                         | Responsibility                                                                                                                                                                                |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`crates/package`](crates/package/README.md) | Manifest validation, deterministic `.tar.gz` packing, checksums and bounded extraction; never executes archive contents.                                                                      |
| [`crates/client`](crates/client/README.md)   | Stateless HTTP interface; authentication and installation adapters take explicit local state.                                                                                                 |
| `crates/cli`                                 | Stateful `apps` CLI built on the client package; bundled command tree and instructive docs.                                                                                                   |
| `crates/server`                              | Accounts authorization, catalog/access/reviews/history, persistent SQLite state, artifacts, idempotency and delivery outbox.                                                                  |
| [`runner`](runner/README.md)                 | Authenticated target-routing gateway and isolated worker: native macOS, Linux Docker and a Windows Hyper-V container command path. Provisioning and target verification remain operator work. |
| [`web`](web/README.md)                       | React store and developer platform using actual [Arc UI](https://uiarc.dev/) free components. [Source provenance and MIT notice](web/vendor/uiarc/PROVENANCE.md) are retained.                |
| [`API_CONTRACT.md`](API_CONTRACT.md)         | User-facing HTTP contract, payloads, authentication and error semantics.                                                                                                                      |
| [`docs/operations.md`](docs/operations.md)   | Accounts integration, environment variables, nine-target runner provisioning, mail, deployment and recovery.                                                                                  |
| [`deploy`](deploy/README.md)                 | Example environment files, Caddy routing and systemd units.                                                                                                                                   |

## Verify

The [requirements audit](docs/requirements-coverage.md) maps each capability to its implementation and evidence. [Local verification](docs/verification.md) records actual service flows and separates them from public deployment and other target-host acceptance.

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
python3 -m venv .dev/runner-venv
.dev/runner-venv/bin/pip install -r runner/requirements.txt
.dev/runner-venv/bin/python -m unittest discover -s runner -v
npm --prefix web ci
npm --prefix web run build
cd web
npx playwright install --with-deps chrome
npm test
```

Browser tests use API fixtures to verify contract behavior, private views, one-time secrets, review writes, upload failures, save failure recovery, keyboard focus, telemetry opt-out, and responsive layouts. They do not prove a deployed Accounts flow or a provisioned native runner. macOS runner isolation tests execute only on supported macOS hosts; native Windows and all Linux target execution need their own provisioned validation evidence.

CI runs these checks. A manually triggered package-artifact workflow builds a limited native target matrix and retains archives/checksums as workflow artifacts. It does not publish an app, push a GitHub release, publish crates, or deploy either domain. See [release delivery](docs/operations.md#build-and-deliver-releases) for the remaining operations and external integration checks.
