use anyhow::{Context, Result, ensure};
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{Value, json};
use silicon_apps_client::{self as apps, Client, LocalState, auth, install, package, updater};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

const HELP: &str = "Silicon Apps is the developer platform, app store and sole app updater.\n\nQUICK START\n  apps search terminal\n  apps install briefcase\n  apps login\n  apps create ring --name Ring\n  apps setup ring details --description-file description.txt\n  apps validate ./package\n  apps pack ./package --output ring.tar.gz\n  apps upload ring --target macos-aarch64 ring.tar.gz\n  apps release ring --version 0.1.0 --package PACKAGE_ID\n  apps promote ring RELEASE_ID --version 1.0.0\n  apps publish ring\n\nTraverse every branch with --help. `apps docs` includes the complete guide;\n`apps docs tree` prints every command and flag. Public browsing and installs need no login.\nUse --json for machine output and --idempotency-key KEY to safely retry a mutation.\n\nState: $SILICON_HOME/.apps or ~/.apps. Every published package must implement\n--help, accounts --json, and login status --json. Only isolated server runners\nexecute upload validation. Local install scripts require --allow-install-script.\n\nSource: https://github.com/teamofsilicons/silicon-apps\nDocs: https://apps.teamofsilicons.com/docs\nRust: https://docs.rs/silicon-apps-client";

#[derive(Parser)]
#[command(name="apps",version,about="Create, publish, discover and install Silicon Apps",long_about=HELP,subcommand_required=true,arg_required_else_help=true)]
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
    Show { app: String },
    /// Check whether an immutable app ID (3–30 characters, a-z 0-9 - _) is available.
    Availability { app: String },
    /// Create an app and show its secret once. Next: setup, upload, release, publish.
    Create {
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
    Upload {
        app: String,
        #[arg(long)]
        target: String,
        file: PathBuf,
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
    /// Review the saved app and all missing requirements before publishing.
    Readiness { app: String },
    /// Make a ready app immediately live; no manual verification or review gate.
    Publish { app: String },
    /// Set your one review, remove it, or list reviews. Example: apps review ring --rating 5.
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
        #[arg(long)]
        allow_install_script: bool,
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
    Update {
        app: Option<String>,
        #[arg(long)]
        allow_install_script: bool,
    },
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let json_output = cli.json;
    match execute(&cli).await {
        Ok(value) => {
            let failed = value.get("valid") == Some(&Value::Bool(false))
                || value.get("error").is_some()
                || value
                    .get("items")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|item| item["status"] == "failed"));
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
                eprintln!("{}", json!({"error":{"message":format!("{error:#}")}}));
            } else {
                eprintln!("error: {error:#}");
            }
            std::process::exit(1);
        }
    }
}
async fn execute(cli: &Cli) -> Result<Value> {
    if let Command::Docs { topic } = &cli.command {
        show_docs(topic)?;
        return Ok(Value::Null);
    }
    if let Command::Accounts = &cli.command {
        return Ok(
            json!({"app_id":apps::APP_ID,"version":apps::VERSION,"accounts_url":cli.accounts_url.as_deref().unwrap_or("https://accounts.teamofsilicons.com"),"client":"silicon-accounts-client"}),
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
        Command::Show { app } => client.app(app).await?,
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
        Command::Upload { app, target, file } => {
            client
                .upload(
                    app,
                    target,
                    std::fs::read(file)
                        .with_context(|| format!("cannot read {}", file.display()))?,
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
            allow_install_script,
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
                *allow_install_script,
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
                        *allow_install_script,
                    )
                    .await?
                } else {
                    install::install(
                        &client,
                        &state,
                        &config,
                        &spec,
                        switch,
                        *allow_install_script,
                    )
                    .await?
                };
                let mut result = serde_json::to_value(outcome)?;
                match updater::start_configured(&state, &std::env::current_exe()?, &config).await {
                    Ok(receipt) => {
                        result["updater"] = receipt;
                    }
                    Err(error) => {
                        let previous = result["warning"].as_str().unwrap_or("");
                        result["warning"] = json!(format!(
                            "{previous} Automatic updates could not start: {error:#}. Run `apps daemon start` to retry."
                        ));
                        result["updater"] = json!({"status":"unavailable"});
                    }
                }
                result
            }
        }
        Command::Uninstall { app } => {
            if app == "apps" {
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
        Command::Update {
            app,
            allow_install_script,
        } => {
            updater::update(
                &client,
                &state,
                &config,
                app.as_deref(),
                *allow_install_script,
            )
            .await?
        }
        Command::Daemon { action } => match action {
            Daemon::Run { once, detached } => {
                let executable = std::env::current_exe()?;
                let windows_needs_copy = cfg!(windows)
                    && !*once
                    && executable.starts_with(state.root.join("installed/apps"));
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
                updater::remove_service(&state).await?;
                updater::install_service(&state, &std::env::current_exe()?)?
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
        Command::Docs { .. } | Command::Accounts => unreachable!(),
    };
    updater::telemetry(
        &state,
        &config,
        "command_completed",
        "complete",
        1.0,
        json!({"os":std::env::consts::OS,"architecture":std::env::consts::ARCH}),
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
fn show_docs(topic: &str) -> Result<()> {
    if topic == "tree" {
        fn walk(mut command: clap::Command, prefix: String) {
            println!("\n=== {prefix} ===\n{}", command.render_long_help());
            for child in command.get_subcommands().cloned().collect::<Vec<_>>() {
                let name = child.get_name().to_owned();
                walk(child, format!("{prefix} {name}"));
            }
        }
        walk(Cli::command(), "apps".into());
        return Ok(());
    }
    println!(
        "{}",
        match topic {
            "start" =>
                "Install: apps search QUERY; apps show APP; apps install APP.\nSign in for private apps or authoring: apps login.\nPublish: apps create APP --name NAME; apps setup APP details --description-file description.txt; apps pack ./package -o package.tar.gz; apps upload APP --target TARGET package.tar.gz; apps release APP --version 0.1.0 --package PACKAGE_ID; apps promote APP RELEASE_ID --version 1.0.0; apps readiness APP; apps publish APP.\nEnable updates: apps daemon install. Run apps docs publish, manifest, install, auth or why for details.",
            "publish" =>
                "1. Create with an immutable 3–30-character app ID. Save the app_secret shown once.\n2. Save a 200–600-character description and up to 20 tags with setup details.\n3. Use setup access for public/private access. Only admin changes visibility.\n4. Prepare apps.yaml and at least one target binary. Every binary must support --help, accounts --json (app_id), login status --json (authenticated true/false and identity).\n5. Validate, pack, upload each supported target. Upload validation runs in isolated target runners; unavailable runners fail closed.\n6. Create a development release with accepted package IDs. Promote with an independent x.y.z production version.\n7. Optional: setup links, setup media, webhook set. Save secrets when displayed.\n8. Check readiness and publish. Drafts remain visible to authors under apps list --mine. No manual review delays publication.\nInvite co-authors through apps authors APP invite c:person; accept using apps invites accept INVITE_ID. History records every mutation. See apps docs why.",
            "manifest" =>
                "Example apps.yaml:\n\nschema_version: 1\napp_id: ring\nversion: 0.1.0\ncommand: ring\ntargets:\n  macos-aarch64:\n    binary: bin/ring\n    # install_script: scripts/install.sh\n\nPaths are relative to package root. No symlinks, hardlinks, parent paths or special files. Use apps targets for all nine targets; unknown market data is shown as null. Run apps validate DIR to see all errors. Pack output should be outside DIR. Install scripts run locally after installation only when explicitly allowed, with a timeout and rollback on failure. Scripts may have external side effects that package rollback cannot undo.",
            "install" =>
                "apps install APP installs latest production. Quote 'APP>dev' for development, 'APP@1.2.3' or 'APP>dev@1.2.3' for an exact version. Changing channel prompts for confirmation; --yes approves noninteractive switching. An exact version chooses the initial release; the updater still follows its channel.\nAll bytes are checksum verified before bounded safe extraction. Apps commands live in ~/.apps/bin (or SILICON_HOME/.apps/bin); add that directory to PATH.\nUse apps update [APP] now, or apps daemon install for updates at login, once a minute. apps is updated through the same mechanism when installed from the store. Other apps must not run their own updater.\nUninstall with apps uninstall APP; leave a review with apps review APP --rating 5 --text '…'.",
            "auth" =>
                "apps login starts the official Silicon Accounts device sign-in. Approve its code in your browser. Apps exchanges a single-use Apps SLT on its backend; no application secret ships in the CLI.\nSilicons: apps login --silicon si:NAME reads the STK from SILICON_STK (override with --stk-env). Or provide an Accounts-generated Apps token using apps login --slt TOKEN.\napps login status --json returns authenticated and the current Carbon/Silicon identity. apps logout revokes the session. Rotating tokens are stored atomically with owner-only permissions in the configured .apps directory. APPS_TOKEN optionally provides an externally managed bearer token.",
            "why" =>
                "Silicon Apps uses account UUIDs for ownership and authorization because public c:id and si:id can change. Accepted authors have equal authoring rights; the oldest member initially administers membership and visibility.\nPackages must implement three discovery commands so Silicons can navigate any CLI without bespoke knowledge. Package validation never runs on the API host.\nDevelopment and production versions are independent immutable histories. Retried mutations use Idempotency-Key to avoid duplication. Search relevance precedes ratings; private apps are filtered before search.\nTelemetry uses Space Station when APPS_TELEMETRY_KEY is configured, with source, step, progress and context. It defaults on; apps config telemetry off disables it. Secrets and argument payloads are never included.\nSee apps docs tree for the complete API-accessible command surface.",
            _ =>
                "Repository: https://github.com/teamofsilicons/silicon-apps\nOnline docs: https://apps.teamofsilicons.com/docs\nRust package: https://docs.rs/silicon-apps-client\nDeveloper platform: https://developer.teamofsilicons.com\nStore: https://apps.teamofsilicons.com",
        }
    );
    Ok(())
}
