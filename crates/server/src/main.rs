use silicon_apps_server::{AppState, config::Config, router, start_background};
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("signing-key") {
        std::process::exit(signing_key(&args[1..]));
    }
    if let Err(e) = run().await {
        eprintln!("{}: {}\n{}", e.code, e.message, e.hint);
        std::process::exit(1)
    }
}
async fn run() -> silicon_apps_server::error::Result<()> {
    let config = Config::from_env()?;
    if !config.historical_app_ids.is_empty() {
        eprintln!(
            "Historical app IDs, each creatable only by its configured owner: {}.",
            config.historical_app_ids.ids().join(", ")
        );
    }
    let bind = config.bind;
    let dev = config.dev_auth;
    let state = AppState::new(config)?;
    start_background(state.clone()).await?;
    let listener = tokio::net::TcpListener::bind(bind).await.map_err(|e| {
        silicon_apps_server::error::ApiError::unavailable(format!("Could not bind {bind}: {e}"))
    })?;
    eprintln!(
        "Silicon Apps listening on {bind}{}",
        if dev {
            " (explicit isolated development authentication enabled)"
        } else {
            ""
        }
    );
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|e| {
        silicon_apps_server::error::ApiError::unavailable(format!("HTTP server stopped: {e}"))
    })?;
    Ok(())
}

/// `apps-server signing-key generate [--key-id ID]`: print a new release
/// signing key for APPS_SIGNING_KEYS and the public key to pin in the CLI.
fn signing_key(args: &[String]) -> i32 {
    let usage = "Usage: apps-server signing-key generate [--key-id ID]\n\nPrints a new Ed25519 release signing key. Put `env` first in APPS_SIGNING_KEYS (keep older keys after it, comma separated) and pin `public_key` under `key_id` in the CLI.";
    let mut key_id = format!("apps-{}", chrono::Utc::now().format("%Y-%m-%d"));
    match args {
        [cmd] if cmd == "generate" => {}
        [cmd, flag, id] if cmd == "generate" && flag == "--key-id" => key_id = id.clone(),
        _ => {
            eprintln!("{usage}");
            return 2;
        }
    }
    let (entry, public_key) = silicon_apps_server::signing::generate(&key_id);
    // Check the ID the same way the server will when it loads the key.
    if let Err(e) = silicon_apps_server::signing::Keyring::parse(&entry, &[]) {
        eprintln!("{}: {}\n{}", e.code, e.message, e.hint);
        return 2;
    }
    println!(
        "{}",
        serde_json::json!({
            "key_id": key_id,
            "algorithm": "ed25519",
            "public_key": public_key,
            "env": format!("APPS_SIGNING_KEYS={entry}"),
            "note": "The env value is a secret. Store it only in the runtime secret."
        })
    );
    0
}
