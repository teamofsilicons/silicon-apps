# silicon-apps-client

The primary Rust interface for Silicon Apps. The HTTP client holds only an explicit URL and optional bearer token; it does not load environment variables, persist sessions or initialize local state. The CLI calls this package for every service operation, authentication, installation and update.

```rust,no_run
use silicon_apps_client::Client;

# async fn example() -> anyhow::Result<()> {
let apps = Client::new("https://apps.teamofsilicons.com", None)?;
let matches = apps.search("terminal", false, false).await?;
let app = apps.app("briefcase").await?;
# Ok(()) }
```

`create`, `edit`, `action`, `upload`, `resolve`, `report`, `register_platform` and the typed package/release models cover author and store operations. `request` exposes the documented user-facing HTTP contract for additional filters and optional fields. Mutations accept caller-supplied idempotency keys; otherwise a new UUID identifies the operation. Reuse your key when retrying after an unknown network outcome.

Authentication uses the official `silicon-accounts-client`. Device or Silicon sign-in obtains a first-party token, requests a single-use Apps token, and exchanges it through the Apps backend. Only the backend holds the app secret. `auth::authenticated_client` takes a `LocalState` explicitly and serializes rotating refresh-token use across processes. Saved sessions bind access and refresh tokens to the complete Apps and Accounts service URLs, including any tenant path; changing either URL requires a fresh login before tokens can be sent. Session files have owner-only Unix permissions and are written atomically. `APPS_TOKEN` is interpreted by the CLI, not by the HTTP client.

```rust,no_run
use silicon_apps_client::{Client, Config, LocalState, install};

# async fn example() -> anyhow::Result<()> {
let state = LocalState::new("/home/me")?; // must already be a directory
let config = Config::default();
let client = Client::new(&config.server, None)?;
let spec = "briefcase>dev@1.2.3".parse()?;
let result = install::install(&client, &state, &config, &spec, false, false).await?;
# Ok(()) }
```

Install checks the selected app/channel/target and SHA-256 digest, extracts a bounded archive, checks command ownership, and uses staging plus backup directories to restore a prior installation on errors. Local scripts require explicit consent and have a timeout. Filesystem rollback cannot undo a script's unrelated external side effects. Successful installation counts use a durable idempotent outbox when the service is temporarily unavailable.

Every installed record also binds the app to its registry URL. Updating against another registry is refused; explicitly reinstalling with confirmation is required to change the source. Sessions saved by 0.1.0 require a new login, and installed records without a saved source require an explicit reinstall with `--yes` before automatic updates resume.

`updater::run` takes explicit state and checks installed channels each minute, including `apps`. `updater::service_definition` produces launchd, systemd or Windows Task Scheduler configuration; `install_service` activates it when requested. Windows startup resolves the stable installed command and runs a separate executable copy so installed app executables can be replaced. A successful Apps update hands off to the latest executable, and the next startup also picks up that version. Unix updaters reload with the same process ID so their launchd/systemd supervisor continues managing the current code. Interactive Windows self-installs return a scheduled receipt and continue through a helper, with the final result in `self-update.log`.

Telemetry is configurable and default-on. The optional `APPS_TELEMETRY_TABLE_KEY` (legacy alias `APPS_TELEMETRY_KEY`) routes source/step/progress/context events to Space Station; no events are emitted when disabled or when the deployment key is absent. Signed-in platform registration supplies aggregate target population independently of diagnostic telemetry.

The package intentionally has no endpoints for isolated-runner verdict submission, account migration or other service-internal administration.

The instructive guides and informative rationale are available without filesystem or network access through `docs::guide(topic)`, and the CLI renders the same text with `apps docs TOPIC`. The CLI also offers `apps docs tree` for every subcommand and flag.
