use anyhow::{Context, Result, ensure};
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{Value, json};
use silicon_apps_client::{self as apps, Client, LocalState, auth, install, package, updater};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "Silicon Apps is the developer platform, app store and sole app updater.\n\nQUICK START\n  silicon-apps search terminal\n  silicon-apps install briefcase\n  silicon-apps login --slt TOKEN\n  silicon-apps create ring --name Ring\n  silicon-apps setup ring details --description-file description.txt\n  silicon-apps validate ./package\n  silicon-apps pack ./package --output ring.tar.gz\n  silicon-apps upload ring --target macos-aarch64 ring.tar.gz\n  silicon-apps release ring --version 0.1.0 --package PACKAGE_ID\n  silicon-apps promote ring RELEASE_ID --version 1.0.0\n  silicon-apps publish ring\n\nTraverse every branch with --help. `silicon-apps docs` includes the complete guide;\n`silicon-apps docs tree` prints every command and flag. Public browsing and installs need no login.\nUse --json for machine output and --idempotency-key KEY to safely retry a mutation.\n\nState: $SILICON_HOME/.apps or ~/.apps. Every published package must implement\n--help, accounts --json, and login status --json. Only isolated server runners\nexecute upload validation. Bundled install scripts run automatically during installation and updates.\n\nSource: https://github.com/teamofsilicons/silicon-apps\nDocs: https://developers.teamofsilicons.com/docs\nRust: https://docs.rs/silicon-apps-client";

#[derive(Parser)]
#[command(name="silicon-apps",version,about="Create, publish, discover and install Silicon Apps",long_about=HELP,subcommand_required=true,arg_required_else_help=true)]
struct Cli {
    /// Return structured machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,
    /// Existing directory to use instead of SILICON_HOME or saved home.
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    /// Silicon Apps server (HTTPS, or loopback HTTP for development).
    #[arg(long, global = true, env = "APPS_URL")]
    server: Option<String>,
    /// Silicon Accounts server for the official authentication client.
    #[arg(long, global = true, env = "ACCOUNTS_URL")]
    accounts_url: Option<String>,
    /// Retry a mutation with the same key to avoid duplicate effects.
    #[arg(long, global = true)]
    idempotency_key: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Discover apps by ID, name, description or tags; spelling mistakes are accepted.
    Search {
        #[arg(default_value = "")]
        query: String,
        #[arg(long)]
        private: bool,
        #[arg(long)]
        mine: bool,
    },
    /// List public, shared private, or your authored apps (including Continue setup drafts).
    List {
        #[arg(long)]
        private: bool,
        #[arg(long)]
        mine: bool,
    },
    /// View an app's page: authors, packages, links, media, rating and installs.
    /// --install-script prints the script a release runs on install and update, with its sha256.
    Show {
        /// An app ID, or 'app>dev', 'app@1.2.3' or 'app>dev@1.2.3' with --install-script.
        app: String,
        /// Print the install script's sha256 and contents, after checking the release's signatures.
        #[arg(long)]
        install_script: bool,
        /// The target to read it for. Defaults to this computer's target.
        #[arg(long, requires = "install_script")]
        target: Option<String>,
    },
    /// Check whether an immutable app ID (3–30 characters, a-z 0-9 - _) is available.
    ///
    /// A 1–2 character historical Silicon Accounts ID is sent to the server as well:
    /// it is available only to the signed-in account the operator reserved it for.
    Availability { app: String },
    /// Create an app and show its secret once. Next: setup, upload, release, publish.
    Create {
        /// Permanent app ID: 3–30 of a-z, 0-9, - and _. A 1–2 character historical
        /// Silicon Accounts ID is sent to the server unchanged; the server accepts it
        /// only from the account the operator reserved it for.
        app: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long)]
        logo: Option<String>,
    },
    /// Save publishing setup one step at a time. Resume any step at any time.
    Setup {
        app: String,
        #[command(subcommand)]
        step: Setup,
    },
    /// Manage equal authors, invitations, leaving, and administrative ownership.
    Authors {
        app: String,
        #[command(subcommand)]
        action: Authors,
    },
    /// List and respond to invitations addressed to your account.
    Invites {
        #[command(subcommand)]
        action: Invites,
    },
    /// Show all manifest, missing file and package safety errors at once.
    Validate {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Build a deterministic .tar.gz from a valid apps.yaml directory. Next: upload.
    Pack {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Upload for isolated target validation of --help, accounts and login status.
    /// Add --sign-key to sign the package with your author key as well.
    Upload {
        app: String,
        #[arg(long)]
        target: String,
        file: PathBuf,
        /// Sign with an author key: its ID from `silicon-apps keys list`, or a key file's path.
        #[arg(long)]
        sign_key: Option<String>,
    },
    /// List uploaded packages and exact validation results.
    Packages { app: String },
    /// Create an immutable DEVELOPMENT release. Next: promote and publish.
    Release {
        app: String,
        #[arg(long)]
        version: String,
        #[arg(long = "package", required = true)]
        packages: Vec<String>,
        #[arg(long, default_value = "")]
        notes: String,
    },
    /// List immutable release history, optionally restricted to a channel.
    Releases {
        app: String,
        #[arg(long,value_parser=["production","development"])]
        channel: Option<String>,
    },
    /// Promote a development release with an independent production version.
    Promote {
        app: String,
        release: String,
        #[arg(long)]
        version: String,
    },
    /// Withdraw a bad release: it stops being served, installs get the previous good
    /// release on its channel, and updaters move installed copies off it. Final.
    /// Example: silicon-apps withdraw ring RELEASE_ID --reason "1.4.0 deletes the config file"
    Withdraw {
        app: String,
        release: String,
        /// Why, in a sentence. Shown on the app page and to everyone who had it installed.
        #[arg(long)]
        reason: String,
    },
    /// Manage your author signing keys. Packages you upload with --sign-key carry your
    /// signature too, and installs check it.
    Keys {
        #[command(subcommand)]
        action: Keys,
    },
    /// Review the saved app and all missing requirements before publishing.
    Readiness { app: String },
    /// Make a ready app immediately live; no manual verification or review gate.
    Publish { app: String },
    /// Set your one review, remove it, or list reviews. Example: silicon-apps review ring --rating 5.
    Review {
        app: String,
        #[arg(long,value_parser=clap::value_parser!(u8).range(1..=5))]
        rating: Option<u8>,
        #[arg(long, default_value = "")]
        text: String,
        #[arg(long, conflicts_with = "rating")]
        remove: bool,
    },
    /// View the complete author-accessible change audit trail.
    History {
        app: String,
        #[arg(long, default_value_t = 100)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Configure Silicon Accounts-owned signed account update webhooks.
    Webhook {
        app: String,
        #[command(subcommand)]
        action: Webhook,
    },
    /// Install a channel or exact version for this platform. Quote 'app>dev@1.2.3'.
    Install {
        app: String,
        #[arg(short = 'y', long)]
        yes: bool,
        /// Install a local archive and register it for channel updates (bootstrap/offline).
        #[arg(long, requires = "sha256")]
        archive: Option<PathBuf>,
        /// Trusted SHA-256 for --archive; required before local bytes can be installed.
        #[arg(long, requires = "archive")]
        sha256: Option<String>,
    },
    /// Remove an app and its command entirely. You can then leave a review.
    Uninstall { app: String },
    /// List local installed app versions, channels and checksums.
    Installed,
    /// Check for newer releases on each installed app's channel, including apps itself.
    Update { app: Option<String> },
    /// Control the only updater. It checks every minute; install enables startup at login.
    Daemon {
        #[command(subcommand)]
        action: Daemon,
    },
    /// Sign in through Silicon Accounts. Use --slt for a one-use token from Accounts.
    Login {
        #[arg(long, conflicts_with = "silicon")]
        slt: Option<String>,
        #[arg(long)]
        silicon: Option<String>,
        #[arg(long, default_value = "SILICON_STK")]
        stk_env: String,
        #[command(subcommand)]
        action: Option<Login>,
    },
    /// Revoke your Apps session through Silicon Accounts and remove local credentials.
    Logout,
    /// Identify this app and its Accounts integration (required discovery command).
    Accounts,
    /// Inspect or change configuration; telemetry is on by default.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Show target support and total addressable market, where measured.
    Targets,
    /// Report a bug to the maintainers, optionally with a PR that fixes it.
    Report {
        message: String,
        #[arg(long)]
        pr: Option<String>,
    },
    /// Read or follow events: your account feed, an app you author, or a subscription.
    /// Example: silicon-apps events --app ring --type 'package.*' --follow
    Events {
        /// An app you author. Omit for your own account feed.
        #[arg(long, conflicts_with = "subscription")]
        app: Option<String>,
        /// Read through one of your subscriptions' filters.
        #[arg(long)]
        subscription: Option<String>,
        /// Event types: exact (release.promoted), a group (package.*) or *. Repeatable.
        #[arg(long = "type", value_delimiter = ',')]
        types: Vec<String>,
        /// Keep the stream open and print each event as one JSON line as it happens.
        #[arg(long)]
        follow: bool,
        /// Start after this event seq (sent as Last-Event-ID with --follow).
        #[arg(long)]
        after: Option<i64>,
        /// Page size, or with --follow the number of events to print before exiting.
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Get told about releases and other events by signed webhook or stream.
    /// Example: silicon-apps subscriptions create --app briefcase --type release.promoted --webhook https://example.com/hooks/apps
    Subscriptions {
        #[command(subcommand)]
        action: Subscriptions,
    },
    /// Show what this server supports. --require checks needs such as streaming or target:linux-x86_64.
    Capabilities {
        /// Requirements to check: streaming, subscriptions, webhooks, version:V, auth:METHOD, event:TYPE, target:TARGET.
        #[arg(long, value_delimiter = ',')]
        require: Vec<String>,
    },
    /// Read bundled instructive/informative docs or the complete command tree.
    Docs {
        #[arg(default_value="start",value_parser=["start","publish","manifest","install","auth","why","tree","links"])]
        topic: String,
    },
}
#[derive(Subcommand)]
enum Setup {
    /// Save required details; description must be 200–600 characters to publish.
    Details {
        #[arg(long)]
        name: Option<String>,
        #[arg(long, conflicts_with = "description_file")]
        description: Option<String>,
        #[arg(long)]
        description_file: Option<PathBuf>,
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
    },
    /// Set public/private access, shared account IDs and verified email domains (admin).
    Access {
        #[arg(long,value_parser=["public","private"],default_value="public")]
        visibility: String,
        #[arg(long = "domain")]
        domains: Vec<String>,
        #[arg(long = "account")]
        accounts: Vec<String>,
    },
    /// Save optional links as JSON: website, developer_docs, android, ios, custom[].
    Links { file: PathBuf },
    /// Save optional media JSON: logo, banner, carousel[{url,kind,alt}].
    Media { file: PathBuf },
    /// Save your resume position (1 details through 7 review and publish).
    Step {
        #[arg(value_parser=clap::value_parser!(u8).range(1..=7))]
        step: u8,
    },
    /// Show the entire saved draft and its Continue setup position.
    Show,
}
#[derive(Subcommand)]
enum Authors {
    List,
    /// Invite by c:id, si:id or email. Recipient must accept before appearing publicly.
    Invite {
        to: String,
    },
    Invites,
    Cancel {
        invite: String,
    },
    /// Leave the app; the last remaining author cannot leave.
    Leave,
    /// Transfer adminship to an existing author's immutable UUID.
    Transfer {
        uuid: String,
    },
    /// Remove another author (admin only).
    Remove {
        uuid: String,
    },
    /// Rotate the app secret and show the replacement once; old secret stops working.
    RotateSecret,
}
#[derive(Subcommand)]
enum Keys {
    /// Create a key pair, keep the private key in .apps/keys (owner-only) and register
    /// the public key with Apps. --public-key registers a key you already have instead.
    Add {
        /// A label for the key, such as the machine it lives on.
        #[arg(long)]
        name: Option<String>,
        /// Register this base64 Ed25519 public key instead of creating a key pair.
        #[arg(long)]
        public_key: Option<String>,
    },
    /// List your author keys and whether this home holds each private key.
    List,
    /// Revoke a key, for example when it leaked. Nothing new can be signed with it.
    Revoke {
        key_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
}
#[derive(Subcommand)]
enum Invites {
    List,
    Accept { invite: String },
    Decline { invite: String },
}
#[derive(Subcommand)]
enum Webhook {
    Show,
    /// Save endpoint and subscribed events; the five recommended events default on.
    Set {
        url: String,
        #[arg(long="event",default_values=["id_change","display_name_change","pfp_change","access_removed","account_deleted"])]
        events: Vec<String>,
    },
    /// Generate a replacement whsec_ secret; save it now because it is shown once.
    Rotate,
}
#[derive(Subcommand)]
enum Subscriptions {
    /// List your subscriptions (active and paused unless --status says otherwise).
    List {
        #[arg(long,value_parser=["active","paused","cancelled","all"])]
        status: Option<String>,
    },
    /// Subscribe to an app you can see, or omit --app for your account feed.
    /// A webhook subscription prints its whsec_ signing secret once; save it now.
    Create {
        /// The app to follow. Omit to follow your account feed.
        #[arg(long)]
        app: Option<String>,
        /// Event types: exact, group.* or *. Default: what you can see.
        #[arg(long = "type", value_delimiter = ',')]
        types: Vec<String>,
        /// Only releases on this channel. Repeatable; default both.
        #[arg(long = "channel", value_parser = ["production", "development"])]
        channels: Vec<String>,
        /// Deliver each event to this HTTPS URL, signed with the subscription secret.
        #[arg(long, conflicts_with = "stream", required_unless_present = "stream")]
        webhook: Option<String>,
        /// Deliver to a stream you read with `silicon-apps events --subscription ID --follow`.
        #[arg(long)]
        stream: bool,
        #[arg(long)]
        description: Option<String>,
    },
    /// Show a subscription and its delivery counts.
    Show { id: String },
    /// Change event types, channels, delivery or description.
    Update {
        id: String,
        #[arg(long = "type", value_delimiter = ',')]
        types: Vec<String>,
        #[arg(long = "channel", value_parser = ["production", "development"])]
        channels: Vec<String>,
        /// Receive both channels again.
        #[arg(long, conflicts_with = "channels")]
        all_channels: bool,
        #[arg(long, conflicts_with = "stream")]
        webhook: Option<String>,
        #[arg(long)]
        stream: bool,
        #[arg(long)]
        description: Option<String>,
    },
    /// Stop deliveries for now. Events while paused are held for 72 hours.
    Pause { id: String },
    /// Resume a paused subscription; held deliveries go out.
    Resume { id: String },
    /// Cancel a subscription for good.
    Cancel { id: String },
    /// List recent webhook deliveries with attempts and the exact last error.
    Deliveries {
        id: String,
        #[arg(long,value_parser=["pending","delivered","failed"])]
        status: Option<String>,
    },
    /// Replace the signing secret and print the new one once.
    RotateSecret { id: String },
    /// Send a signed test delivery to the webhook.
    Ping { id: String },
}
#[derive(Subcommand)]
enum Daemon {
    Run {
        #[arg(long)]
        once: bool,
        /// Start a fresh detached updater from the current installed Apps executable.
        #[arg(long, conflicts_with = "once")]
        detached: bool,
    },
    Start,
    Stop,
    Status,
    Install,
    /// Stop the updater and remove automatic startup configuration.
    Remove,
    Definition,
}
#[derive(Subcommand)]
enum Login {
    Status,
}
#[derive(Subcommand)]
enum ConfigAction {
    Show,
    Home {
        location: PathBuf,
    },
    Server {
        url: String,
    },
    Accounts {
        url: String,
    },
    Telemetry {
        #[arg(value_parser=["on","off"])]
        enabled: String,
    },
    Set {
        #[arg(value_parser=["install_script_timeout_seconds","update_interval_seconds"])]
        key: String,
        value: u64,
    },
}

/// Parsing and running the command tree needs more stack than the 1 MiB
/// Windows gives the main thread in debug builds, so everything runs on one
/// thread with a known, larger stack.
fn main() {
    let worker = std::thread::Builder::new()
        .name("silicon-apps".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(run()),
                Err(error) => {
                    eprintln!("error: could not start the async runtime: {error}");
                    std::process::exit(1);
                }
            }
        });
    match worker.map(|handle| handle.join()) {
        Ok(Ok(())) => {}
        Ok(Err(_)) => std::process::exit(101),
        Err(error) => {
            eprintln!("error: could not start the command thread: {error}");
            std::process::exit(1);
        }
    }
}
async fn run() {
    let arguments: Vec<_> = std::env::args_os().collect();
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error) => {
            if error.use_stderr() && arguments.iter().any(|arg| arg == "--json") {
                eprintln!(
                    "{}",
                    json!({"error":{"code":"invalid_arguments","message":error.to_string()}})
                );
                std::process::exit(2);
            }
            error.exit();
        }
    };
    let json_output = cli.json;
    // The command future contains every operation's state; keep it off Windows' 1 MiB
    // main stack so rendering a nested help tree has enough stack in debug builds too.
    let listing = matches!(
        &cli.command,
        Command::Subscriptions {
            action: Subscriptions::Deliveries { .. }
        } | Command::Events { .. }
    );
    match Box::pin(execute(&cli)).await {
        Ok(value) => {
            let failed = !listing && failed_result(&value);
            if !value.is_null() {
                if !json_output && value.get("message").and_then(Value::as_str).is_some() {
                    println!("{}", value["message"].as_str().unwrap());
                    if let Some(warning) = value["warning"].as_str() {
                        eprintln!("{warning}");
                    }
                } else {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value).unwrap_or_default()
                    );
                }
            }
            if failed {
                std::process::exit(1);
            }
        }
        Err(error) => {
            if json_output {
                eprintln!("{}", error_json(&error));
            } else {
                eprintln!("error: {error:#}");
            }
            std::process::exit(1);
        }
    }
}
async fn execute(cli: &Cli) -> Result<Value> {
    if let Command::Docs { topic } = &cli.command {
        let content = bundled_docs(topic);
        if cli.json {
            return Ok(json!({"topic":topic,"content":content}));
        }
        println!("{content}");
        return Ok(Value::Null);
    }
    if let Command::Accounts = &cli.command {
        return Ok(
            json!({"app_id":apps::APP_ID,"version":CLI_VERSION,"accounts_url":cli.accounts_url.as_deref().unwrap_or("https://accounts.teamofsilicons.com"),"client":"silicon-accounts-client"}),
        );
    }
    let state = LocalState::discover(cli.home.as_deref())?;
    let mut config = state.config()?;
    if let Some(server) = &cli.server {
        config.server = server.clone();
    }
    if let Some(accounts) = &cli.accounts_url {
        config.accounts_url = accounts.clone();
    }
    let local = matches!(
        &cli.command,
        Command::Validate { .. }
            | Command::Pack { .. }
            | Command::Config { .. }
            | Command::Daemon { .. }
            | Command::Installed
            | Command::Uninstall { .. }
            | Command::Install {
                archive: Some(_),
                ..
            }
            | Command::Logout
            | Command::Login { action: None, .. }
    );
    let token = std::env::var("APPS_TOKEN").ok();
    let client = if local {
        Client::new(&config.server, None)?.with_telemetry(config.telemetry)
    } else {
        auth::authenticated_client(&state, &config, token.clone()).await?
    };
    if !local
        && (token.is_some() || auth::read(&state)?.is_some())
        && let Ok(target) = package::current_target()
    {
        let _ = client.register_platform(target).await;
    }
    let key = cli.idempotency_key.as_deref();
    let result = match &cli.command {
        Command::Search {
            query,
            private,
            mine,
        } => client.search(query, *private, *mine).await?,
        Command::List { private, mine } => client.search("", *private, *mine).await?,
        Command::Show {
            app,
            install_script,
            target,
        } => {
            if *install_script {
                let spec: install::InstallSpec = app.parse()?;
                let target = match target {
                    Some(t) => {
                        ensure!(
                            package::TARGETS.contains(&t.as_str()),
                            "unknown target `{t}`; choose from {}",
                            package::TARGETS.join(", ")
                        );
                        t.as_str()
                    }
                    None => package::current_target()?,
                };
                install::inspect_install_script(&client, &state, &spec, target).await?
            } else {
                client.app(app).await?
            }
        }
        Command::Availability { app } => client.available(app).await?,
        Command::Create {
            app,
            name,
            description,
            logo,
        } => {
            client
                .create(app, name, description, logo.as_deref(), key)
                .await?
        }
        Command::Setup { app, step } => match step {
            Setup::Details {
                name,
                description,
                description_file,
                tags,
            } => {
                let mut body = json!({"setup_step":1});
                if let Some(name) = name {
                    body["name"] = json!(name);
                }
                if let Some(description) = description {
                    body["description"] = json!(description);
                }
                if let Some(path) = description_file {
                    body["description"] = json!(std::fs::read_to_string(path)?.trim());
                }
                if let Some(tags) = tags {
                    body["tags"] = json!(tags);
                }
                client.edit(app, body, key).await?
            }
            Setup::Access {
                visibility,
                domains,
                accounts,
            } => client
                .action(
                    "PUT",
                    app,
                    &["access"],
                    Some(json!({"visibility":visibility,"domains":domains,"account_ids":accounts})),
                    key,
                )
                .await?,
            Setup::Links { file } => {
                client
                    .edit(app, json!({"links":read_json(file)?,"setup_step":4}), key)
                    .await?
            }
            Setup::Media { file } => {
                let mut data = read_json(file)?;
                ensure!(data.is_object(), "media file must contain a JSON object");
                data["setup_step"] = json!(5);
                client.edit(app, data, key).await?
            }
            Setup::Step { step } => client.edit(app, json!({"setup_step":step}), key).await?,
            Setup::Show => client.app(app).await?,
        },
        Command::Authors { app, action } => match action {
            Authors::List => client.action("GET", app, &["authors"], None, None).await?,
            Authors::Invite { to } => {
                client
                    .action("POST", app, &["invites"], Some(json!({"to":to})), key)
                    .await?
            }
            Authors::Invites => client.action("GET", app, &["invites"], None, None).await?,
            Authors::Cancel { invite } => {
                client
                    .action("DELETE", app, &["invites", invite], Some(json!({})), key)
                    .await?
            }
            Authors::Leave => {
                client
                    .action("POST", app, &["authors", "leave"], Some(json!({})), key)
                    .await?
            }
            Authors::Transfer { uuid } => {
                client
                    .action("POST", app, &["admin"], Some(json!({"uuid":uuid})), key)
                    .await?
            }
            Authors::Remove { uuid } => {
                client
                    .action("DELETE", app, &["authors", uuid], Some(json!({})), key)
                    .await?
            }
            Authors::RotateSecret => {
                client
                    .action("POST", app, &["secret", "rotate"], Some(json!({})), key)
                    .await?
            }
        },
        Command::Invites { action } => match action {
            Invites::List => {
                client
                    .request("GET", &["v1", "invites"], &[], None, None)
                    .await?
            }
            Invites::Accept { invite } => {
                client
                    .request(
                        "POST",
                        &["v1", "invites", invite, "accept"],
                        &[],
                        Some(json!({})),
                        key,
                    )
                    .await?
            }
            Invites::Decline { invite } => {
                client
                    .request(
                        "POST",
                        &["v1", "invites", invite, "decline"],
                        &[],
                        Some(json!({})),
                        key,
                    )
                    .await?
            }
        },
        Command::Validate { path } => {
            let report = if path.is_dir() {
                package::validate_directory(path)
            } else {
                match package::inspect_archive(&std::fs::read(path)?) {
                    Ok(manifest) => package::ValidationReport {
                        valid: true,
                        errors: vec![],
                        manifest: Some(manifest),
                    },
                    Err(error) => package::ValidationReport {
                        valid: false,
                        errors: vec![format!("{error:#}")],
                        manifest: None,
                    },
                }
            };
            serde_json::to_value(report)?
        }
        Command::Pack { path, output } => {
            let absolute_output = if output.is_absolute() {
                output.clone()
            } else {
                std::env::current_dir()?.join(output)
            };
            ensure!(
                !absolute_output.starts_with(path.canonicalize()?),
                "output must be outside the package directory to avoid including an earlier archive"
            );
            let bytes = package::pack_directory(path)?;
            std::fs::write(output, &bytes)?;
            json!({"path":output,"sha256":package::sha256(&bytes),"size":bytes.len()})
        }
        Command::Upload {
            app,
            target,
            file,
            sign_key,
        } => {
            let author = sign_key
                .as_deref()
                .map(|id| apps::signing::AuthorKey::load(&state, id))
                .transpose()?;
            client
                .upload_signed(
                    app,
                    target,
                    std::fs::read(file)
                        .with_context(|| format!("cannot read {}", file.display()))?,
                    author.as_ref(),
                    key,
                )
                .await?
        }
        Command::Packages { app } => client.action("GET", app, &["packages"], None, None).await?,
        Command::Release {
            app,
            version,
            packages,
            notes,
        } => {
            client
                .action(
                    "POST",
                    app,
                    &["releases"],
                    Some(json!({"version":version,"package_ids":packages,"notes":notes})),
                    key,
                )
                .await?
        }
        Command::Releases { app, channel } => {
            let query = channel
                .as_ref()
                .map(|c| vec![("channel", c.clone())])
                .unwrap_or_default();
            client
                .request("GET", &["v1", "apps", app, "releases"], &query, None, None)
                .await?
        }
        Command::Promote {
            app,
            release,
            version,
        } => {
            client
                .action(
                    "POST",
                    app,
                    &["releases", release, "promote"],
                    Some(json!({"version":version})),
                    key,
                )
                .await?
        }
        Command::Withdraw {
            app,
            release,
            reason,
        } => client.withdraw_release(app, release, reason, key).await?,
        Command::Keys { action } => match action {
            Keys::Add { name, public_key } => {
                let (public, saved) = match public_key {
                    Some(public) => (public.trim().to_owned(), None),
                    None => {
                        let key = apps::signing::AuthorKey::generate();
                        let path = key.save(&state)?;
                        (key.public_key.clone(), Some(path))
                    }
                };
                let mut result = client
                    .add_author_key(&public, name.as_deref(), key)
                    .await
                    .with_context(|| match &saved {
                        Some(path) => format!(
                            "the private key is saved at {}, but Apps did not register it; retry with --public-key {public}",
                            path.display()
                        ),
                        None => "Apps did not register the key".into(),
                    })?;
                result["private_key_path"] = json!(saved);
                result
            }
            Keys::List => {
                let mut result = client.author_keys().await?;
                for item in result["items"].as_array_mut().into_iter().flatten() {
                    let local = item["key_id"]
                        .as_str()
                        .and_then(|id| apps::signing::key_path(&state, id).ok())
                        .filter(|path| path.is_file());
                    item["private_key_path"] = json!(local);
                }
                result
            }
            Keys::Revoke { key_id, reason } => {
                client
                    .revoke_author_key(key_id, reason.as_deref(), key)
                    .await?
            }
        },
        Command::Readiness { app } => {
            client
                .action("GET", app, &["readiness"], None, None)
                .await?
        }
        Command::Publish { app } => {
            client
                .action("POST", app, &["publish"], Some(json!({})), key)
                .await?
        }
        Command::Review {
            app,
            rating,
            text,
            remove,
        } => {
            if *remove {
                client
                    .action("DELETE", app, &["review"], Some(json!({})), key)
                    .await?
            } else if let Some(rating) = rating {
                client
                    .action(
                        "PUT",
                        app,
                        &["review"],
                        Some(json!({"rating":rating,"text":text})),
                        key,
                    )
                    .await?
            } else {
                client.action("GET", app, &["reviews"], None, None).await?
            }
        }
        Command::History { app, limit, offset } => {
            client
                .request(
                    "GET",
                    &["v1", "apps", app, "history"],
                    &[("limit", limit.to_string()), ("offset", offset.to_string())],
                    None,
                    None,
                )
                .await?
        }
        Command::Webhook { app, action } => match action {
            Webhook::Show => client.action("GET", app, &["webhook"], None, None).await?,
            Webhook::Set { url, events } => {
                client
                    .action(
                        "PUT",
                        app,
                        &["webhook"],
                        Some(json!({"url":url,"events":events})),
                        key,
                    )
                    .await?
            }
            Webhook::Rotate => {
                client
                    .action("POST", app, &["webhook", "rotate"], Some(json!({})), key)
                    .await?
            }
        },
        Command::Install {
            app,
            yes,
            archive,
            sha256,
        } => {
            let spec = app.parse()?;
            let mut switch = *yes;
            if let Some(old) = install::requires_channel_switch(&state, &spec)? {
                ensure!(
                    *yes || confirm(&format!(
                        "Switch {} from {old} to {}{}?",
                        spec.app_id,
                        spec.channel,
                        if spec.channel == "development" {
                            " experimental releases"
                        } else {
                            ""
                        }
                    ))?,
                    "channel switch cancelled; installed version was kept"
                );
                switch = true;
            }
            if let Some(old) = install::requires_source_switch(&state, &spec, &config.server)? {
                ensure!(
                    *yes || confirm(&format!(
                        "Switch {} from registry {old} to {}?",
                        spec.app_id, config.server
                    ))?,
                    "registry switch cancelled; installed version was kept"
                );
                switch = true;
            }
            if let Some(receipt) = updater::defer_self_install_archive(
                &state,
                &config,
                &spec,
                switch,
                archive.as_deref().zip(sha256.as_deref()),
            )? {
                receipt
            } else {
                let outcome = if let Some(path) = archive {
                    install::install_local(
                        &state,
                        &config,
                        &spec,
                        install::LocalArchive {
                            bytes: std::fs::read(path)?,
                            sha256: sha256.clone().context("--archive requires --sha256")?,
                        },
                        switch,
                    )
                    .await?
                } else {
                    install::install(&client, &state, &config, &spec, switch).await?
                };
                let mut result = serde_json::to_value(outcome)?;
                if let Some(notice) = result["notice"].as_str() {
                    eprintln!("{notice}");
                }
                if no_daemon() {
                    result["updater"] = json!({"status":"disabled","reason":"SILICON_APPS_NO_DAEMON=1: no updater runs in this environment. Run `silicon-apps update` when you want updates."});
                    return Ok(result);
                }
                match updater::start_configured(&state, &std::env::current_exe()?, &config).await {
                    Ok(receipt) => {
                        result["updater"] = receipt;
                    }
                    Err(error) => {
                        let previous = result["warning"].as_str().unwrap_or("");
                        result["warning"] = json!(format!(
                            "{previous} Automatic updates could not start: {error:#}. Run `silicon-apps daemon start` to retry."
                        ));
                        result["updater"] = json!({"status":"unavailable"});
                    }
                }
                result
            }
        }
        Command::Uninstall { app } => {
            if app == apps::APP_ID {
                ensure!(
                    state.installed()?.contains_key(app),
                    "apps is not installed"
                );
                if let Some(receipt) = updater::defer_self_uninstall(&state, &config)? {
                    return Ok(receipt);
                }
                updater::remove_service(&state).await?;
            }
            json!({"message":install::uninstall(&state,app)?})
        }
        Command::Installed => json!({"items":state.installed()?.into_values().collect::<Vec<_>>()}),
        Command::Update { app } => {
            let result = updater::update(&client, &state, &config, app.as_deref()).await?;
            for item in result["items"].as_array().into_iter().flatten() {
                for field in ["notice", "install_script_notice"] {
                    if let Some(notice) = item[field].as_str() {
                        eprintln!("{notice}");
                    }
                }
            }
            result
        }
        Command::Daemon { action } => match action {
            Daemon::Run { once: false, .. } | Daemon::Start | Daemon::Install if no_daemon() => {
                anyhow::bail!(
                    "SILICON_APPS_NO_DAEMON=1 is set, so this environment runs no updater. Unset it to run one, or use `silicon-apps update` (or `daemon run --once`) for a single check."
                )
            }
            Daemon::Run { once, detached } => {
                let executable = std::env::current_exe()?;
                let windows_needs_copy = cfg!(windows)
                    && !*once
                    && executable.starts_with(state.root.join("installed").join(apps::APP_ID));
                if *detached || windows_needs_copy {
                    return updater::start_process_configured(&state, &executable, &config).await;
                }
                updater::run_with_overrides(
                    &state,
                    *once,
                    cli.server.as_deref(),
                    cli.accounts_url.as_deref(),
                )
                .await?
            }
            Daemon::Start => {
                updater::start_configured(&state, &std::env::current_exe()?, &config).await?
            }
            Daemon::Stop => updater::stop(&state).await?,
            Daemon::Status => updater::status(&state)?,
            Daemon::Install => {
                state.save_config(&config)?;
                updater::reinstall_service(&state, &std::env::current_exe()?).await?
            }
            Daemon::Remove => updater::remove_service(&state).await?,
            Daemon::Definition => {
                let (path, content) =
                    updater::service_definition(&state, &std::env::current_exe()?)?;
                json!({"path":path,"content":content})
            }
        },
        Command::Login {
            slt,
            silicon,
            stk_env,
            action,
        } => {
            if action.is_some() {
                auth::status(&state, &client, token.is_some()).await?
            } else {
                let tokens = if let Some(slt) = slt {
                    auth::exchange(&client, slt).await?
                } else if let Some(id) = silicon {
                    let stk = std::env::var(stk_env).with_context(|| {
                        format!("set {stk_env} to the Silicon's STK, then retry")
                    })?;
                    auth::silicon_login(&config, &client, id, &stk).await?
                } else {
                    let challenge = auth::begin_device(&config).await?;
                    eprintln!(
                        "Open {}\nConfirm code {}. Waiting for approval (expires in {} seconds)…",
                        challenge.browser_url(),
                        challenge.user_code,
                        challenge.expires_in
                    );
                    auth::finish_device(&config, &client, &challenge).await?
                };
                let signed = client.authenticated(Some(tokens.access_token.expose().into()));
                auth::save(&state, &config, tokens)?;
                if let Ok(target) = package::current_target()
                    && let Err(error) = signed.register_platform(target).await
                {
                    eprintln!("Signed in; platform registration will retry later: {error:#}");
                }
                auth::status(&state, &signed, false).await?
            }
        }
        Command::Logout => {
            auth::logout(&state, &config, &client).await?;
            json!({"app_id":apps::APP_ID,"authenticated":false})
        }
        Command::Config { action } => {
            match action {
                ConfigAction::Show => {}
                ConfigAction::Home { location } => {
                    let new = LocalState::set_home(location)?;
                    return Ok(
                        json!({"home":new.home,"message":format!("Configured home {}. SILICON_HOME and --home still override this setting.",new.home.display())}),
                    );
                }
                ConfigAction::Server { url } => {
                    Client::new(url, None)?;
                    config.server = url.clone();
                }
                ConfigAction::Accounts { url } => {
                    let mut candidate = config.clone();
                    candidate.accounts_url = url.clone();
                    auth::accounts(&candidate)?;
                    config = candidate;
                }
                ConfigAction::Telemetry { enabled } => config.telemetry = enabled == "on",
                ConfigAction::Set { key, value } => match key.as_str() {
                    "install_script_timeout_seconds" => {
                        ensure!(
                            (1..=3600).contains(value),
                            "install script timeout must be 1–3600 seconds"
                        );
                        config.install_script_timeout_seconds = *value;
                    }
                    _ => {
                        ensure!(
                            (10..=86400).contains(value),
                            "update interval must be 10–86400 seconds"
                        );
                        config.update_interval_seconds = *value;
                    }
                },
            }
            if !matches!(action, ConfigAction::Show) {
                state.save_config(&config)?;
            }
            json!({"home":state.home,"state":state.root,"config":config})
        }
        Command::Targets => {
            client
                .request("GET", &["v1", "targets"], &[], None, None)
                .await?
        }
        Command::Report { message, pr } => client.report(message, pr.as_deref(), key).await?,
        Command::Capabilities { require } => {
            let require: Vec<&str> = require.iter().map(String::as_str).collect();
            client.capabilities(&require).await?
        }
        Command::Events {
            app,
            subscription,
            types,
            follow,
            after,
            limit,
        } => {
            let feed = match (app, subscription) {
                (Some(app), _) => apps::events::Feed::App(app.clone()),
                (None, Some(id)) => apps::events::Feed::Subscription(id.clone()),
                (None, None) => apps::events::Feed::Account,
            };
            let types: Vec<&str> = types.iter().map(String::as_str).collect();
            if *follow {
                let after = after.map(|seq| seq.to_string());
                let mut stream = client
                    .stream_events(&feed, after.as_deref(), &types)
                    .await?;
                let mut printed = 0;
                while let Some(event) = stream.next().await? {
                    println!("{}", event.data);
                    io::stdout().flush()?;
                    printed += 1;
                    if limit.is_some_and(|limit| printed >= limit) {
                        break;
                    }
                }
                return Ok(Value::Null);
            }
            client.events(&feed, *after, &types, *limit).await?
        }
        Command::Subscriptions { action } => match action {
            Subscriptions::List { status } => client.subscriptions(status.as_deref()).await?,
            Subscriptions::Create {
                app,
                types,
                channels,
                webhook,
                stream: _,
                description,
            } => {
                let request = apps::events::NewSubscription {
                    app_id: app.clone(),
                    types: types.clone(),
                    channels: channels.clone(),
                    delivery: match webhook {
                        Some(url) => apps::events::Delivery::Webhook { url: url.clone() },
                        None => apps::events::Delivery::Stream,
                    },
                    description: description.clone(),
                };
                client.create_subscription(&request, key).await?
            }
            Subscriptions::Show { id } => client.subscription(id).await?,
            Subscriptions::Update {
                id,
                types,
                channels,
                all_channels,
                webhook,
                stream,
                description,
            } => {
                let mut changes = json!({});
                if !types.is_empty() {
                    changes["types"] = json!(types);
                }
                if !channels.is_empty() {
                    changes["channels"] = json!(channels);
                }
                if *all_channels {
                    changes["channels"] = json!([]);
                }
                if let Some(url) = webhook {
                    changes["delivery"] = json!({"mode":"webhook","url":url});
                }
                if *stream {
                    changes["delivery"] = json!({"mode":"stream"});
                }
                if let Some(text) = description {
                    changes["description"] = json!(text);
                }
                ensure!(
                    changes.as_object().is_some_and(|c| !c.is_empty()),
                    "nothing to change: pass --type, --channel, --all-channels, --webhook, --stream or --description"
                );
                client.update_subscription(id, changes, key).await?
            }
            Subscriptions::Pause { id } => client.pause_subscription(id, key).await?,
            Subscriptions::Resume { id } => client.resume_subscription(id, key).await?,
            Subscriptions::Cancel { id } => client.cancel_subscription(id, key).await?,
            Subscriptions::Deliveries { id, status } => {
                client
                    .subscription_deliveries(id, status.as_deref())
                    .await?
            }
            Subscriptions::RotateSecret { id } => {
                client.rotate_subscription_secret(id, key).await?
            }
            Subscriptions::Ping { id } => client.ping_subscription(id, key).await?,
        },
        Command::Docs { .. } | Command::Accounts => unreachable!(),
    };
    updater::telemetry(
        &state,
        &config,
        "command_completed",
        "complete",
        1.0,
        json!({"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"cli_version":CLI_VERSION,"client_version":apps::VERSION}),
    );
    Ok(result)
}
fn read_json(path: &PathBuf) -> Result<Value> {
    serde_json::from_slice(&std::fs::read(path)?)
        .with_context(|| format!("{} must contain valid JSON", path.display()))
}
fn confirm(message: &str) -> Result<bool> {
    ensure!(
        io::stdin().is_terminal(),
        "{message} Confirmation requires a terminal; pass --yes for an intentional noninteractive switch."
    );
    eprint!("{message} [y/N] ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
/// `SILICON_APPS_NO_DAEMON=1`: never start or register the updater, for
/// ephemeral environments such as CI.
fn no_daemon() -> bool {
    std::env::var("SILICON_APPS_NO_DAEMON").is_ok_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
}
/// The structured form of an error for --json output: the API's or a
/// signature check's code, message, hint and details when there is one.
fn error_json(error: &anyhow::Error) -> Value {
    let chain = format!("{error:#}");
    for cause in error.chain() {
        if let Some(e) = cause.downcast_ref::<apps::ApiError>() {
            return json!({"error":{"code":e.code,"status":e.status,"message":e.message,"hint":e.hint,"details":e.details,"chain":chain}});
        }
        if let Some(e) = cause.downcast_ref::<apps::signing::VerificationError>() {
            return json!({"error":{"code":e.code,"message":e.message,"hint":e.hint,"details":e.details,"chain":chain}});
        }
    }
    json!({"error":{"code":"cli_error","message":chain}})
}
fn bundled_docs(topic: &str) -> String {
    if topic == "tree" {
        // A recursive walk retains complete parent command trees on the stack. On
        // Windows debug builds that can exhaust the default 1 MiB process stack.
        let mut pending = vec![(Cli::command(), "silicon-apps".to_owned())];
        let mut output = String::new();
        while let Some((mut command, prefix)) = pending.pop() {
            output.push_str(&format!(
                "\n=== {prefix} ===\n{}\n",
                command.render_long_help()
            ));
            let children = command.get_subcommands().cloned().collect::<Vec<_>>();
            for child in children.into_iter().rev() {
                let name = child.get_name().to_owned();
                pending.push((child, format!("{prefix} {name}")));
            }
        }
        return output;
    }
    apps::docs::guide(topic).into()
}

fn failed_result(value: &Value) -> bool {
    value.get("valid") == Some(&Value::Bool(false))
        || value.get("error").is_some_and(|error| !error.is_null())
        || value
            .get("items")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["status"] == "failed" || failed_result(item))
            })
        || value.get("result").is_some_and(failed_result)
}
