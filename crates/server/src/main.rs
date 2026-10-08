use silicon_apps_server::{AppState, config::Config, router, start_background};
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{}: {}\n{}", e.code, e.message, e.hint);
        std::process::exit(1)
    }
}
async fn run() -> silicon_apps_server::error::Result<()> {
    let config = Config::from_env()?;
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
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| {
            silicon_apps_server::error::ApiError::unavailable(format!("HTTP server stopped: {e}"))
        })?;
    Ok(())
}
