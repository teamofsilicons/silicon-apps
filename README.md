# Silicon Apps

Create, publish, discover, and install command-line apps for Carbons and Silicons. This repository contains the Apps service, its primary Rust client, package tooling, the `silicon-apps` CLI and updater, and the store frontend. The shared Apps and Accounts developer frontend lives in the sibling `silicon-accounts/developer` project and is served at `developers.teamofsilicons.com`.

The requirements live in [understanding/UNDERSTANDING.md](understanding/UNDERSTANDING.md). The implementation does not add a manual publication review: authors publish when the required details and validated release packages are ready.

## Run locally

Requirements: Rust 1.98 or newer, a C compiler, Node.js 22.12 or newer, npm, and Python 3.11 or newer for the package runner. The official Silicon Accounts Rust client is vendored in `vendor/silicon-accounts-client`; building Apps does not require a sibling checkout. Real sign-in and authoring require a running Silicon Accounts deployment with the Apps integration described in [operations](docs/operations.md#connect-silicon-accounts).

From this directory:

```sh
cp deploy/local.env.example .env
# Configure the Accounts URL, Apps secret, and private service token in .env.
bash scripts/dev.sh
```

Open [the store](http://127.0.0.1:4311/store). The API listens at `127.0.0.1:4310`; Vite proxies `/v1` to it. Creation and management links open [the shared developer portal](https://developers.teamofsilicons.com); use `VITE_DEVELOPERS_URL` to select a local instance of the Accounts developer project. With a runner token configured, the helper also starts the isolated runner at `127.0.0.1:4312`. Service logs are under `.dev/logs/`.

Without configured Accounts or runners, public browsing still works, but there are initially no published apps. Sign-in, global app creation, and package validation report their missing dependencies. The frontend does not insert demo listings. For a CLI-only isolated fixture environment, see [development authentication](docs/operations.md#isolated-development-authentication).

## Use the CLI

For the latest prebuilt CLI and Accounts, use the [combined installation commands](https://developers.teamofsilicons.com/docs/apps/start/install#install-apps-and-accounts-together). Accounts installs with `silicon-apps install silicon-accounts` and receives updates through Apps.

To build and install the CLI from this checkout:

```sh
cargo install --path crates/cli
silicon-apps --help
silicon-apps accounts --json
silicon-apps login status --json
```

Point the CLI at a local stack explicitly:

```sh
silicon-apps config server http://127.0.0.1:4310
silicon-apps config accounts http://127.0.0.1:YOUR_ACCOUNTS_PORT
silicon-apps login --slt TOKEN
silicon-apps search
```

Public search and public production installs work without login. Installed commands live in the selected home’s `.apps/bin`; add that directory to your shell’s `PATH`.

```sh
silicon-apps install ring
silicon-apps install 'ring>dev'
silicon-apps install 'ring@1.2.3'
silicon-apps install 'ring>dev@0.4.0'
silicon-apps installed
silicon-apps daemon install
silicon-apps daemon status --json
```

`ring` is an example identifier; use an app that exists on your configured service. Exact versions choose the initial release, and subsequent updates follow the installed channel. `silicon-apps daemon install` configures launchd, systemd user services, or Windows Task Scheduler. See [state and updates](docs/operations.md#cli-state-and-updates) before using multiple homes or moving a CLI executable.

## Publish an app

```sh
silicon-apps login --slt TOKEN
silicon-apps create ring --name Ring
# Save the app_secret immediately; it is shown once.
silicon-apps setup ring details --description-file description.txt --tags tools,productivity
silicon-apps setup ring access --visibility public
silicon-apps validate ./package
silicon-apps pack ./package --output ./ring.tar.gz
silicon-apps upload ring --target macos-aarch64 ./ring.tar.gz
silicon-apps release ring --version 0.1.0 --package PACKAGE_ID
silicon-apps promote ring DEVELOPMENT_RELEASE_ID --version 1.0.0
apps readiness ring
silicon-apps publish ring
```

Use the actual package and release IDs returned by the preceding commands. The description must contain 200–600 characters. New app IDs contain 3–30 lowercase letters, digits, hyphens or underscores and cannot change. An app needs at least one validated package in a release before publishing; promote a release to production for the default installation command to work.

Every target’s binary must provide `--help`, `accounts --json` containing its `app_id`, and `login status --json`. Local `validate` checks manifest/files/archive safety; upload executes the three commands through a configured isolated target runner. An unavailable runner does not count as a passed validation.

The developer website exposes the same operations through seven saved setup steps. Optional links, media, Accounts webhook updates, co-authors and private access can be configured through either interface. Explore each command with `--help` or read `silicon-apps docs publish`.

## Packages and interfaces

| Path                                         | Responsibility                                                                                                                                                                                |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`crates/package`](crates/package/README.md) | Manifest validation, deterministic `.tar.gz` packing, checksums and bounded extraction; never executes archive contents.                                                                      |
| [`crates/client`](crates/client/README.md)   | Stateless HTTP interface; authentication and installation adapters take explicit local state.                                                                                                 |
| `crates/cli`                                 | Stateful `silicon-apps` CLI built on the client package; bundled command tree and instructive docs.                                                                                                   |
| `crates/server`                              | Accounts authorization, catalog/access/reviews/history, persistent SQLite state, artifacts, idempotency and delivery outbox.                                                                  |
| [`runner`](runner/README.md)                 | Authenticated target-routing gateway and isolated worker: native macOS, Linux Docker and a Windows Hyper-V container command path. Provisioning and target verification remain operator work. |
| [`web`](web/README.md)                       | React store using actual [Arc UI](https://uiarc.dev/) free components. Creation and management open the common Apps and Accounts portal. [Source provenance and MIT notice](web/vendor/uiarc/PROVENANCE.md) are retained. |
| [`API_CONTRACT.md`](API_CONTRACT.md)         | User-facing HTTP contract, payloads, authentication and error semantics.                                                                                                                      |
| [`docs/operations.md`](docs/operations.md)   | Accounts integration, environment variables, nine-target runner provisioning, mail, deployment and recovery.                                                                                  |
| [`deploy`](deploy/README.md)                 | Example environment files, Caddy routing and systemd units.                                                                                                                                   |

## Verify

The [requirements audit](docs/requirements-coverage.md) maps each capability to its implementation and evidence. [Local verification](docs/verification.md) records the initial service flows. [Deployment status](docs/deployment-status.md) records published artifacts, live infrastructure and the remaining public activation gates.

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
