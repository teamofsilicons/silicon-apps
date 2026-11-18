//! Browser sign-in keeps one state cookie per attempt, so starting sign-in again (another tab, a second click) never
//! breaks an attempt that is still on its way back from Silicon Accounts.
mod common;
use common::*;
use silicon_apps_server::{AppState, router};

fn app(dir: &std::path::Path) -> axum::Router {
    let mut cfg = config(dir);
    cfg.dev_auth = false;
    cfg.accounts_app_secret = Some("test-secret".into());
    router(AppState::new(cfg).unwrap())
}

/// Start a sign-in; returns its state and the cookie pair the browser keeps.
async fn start(app: &axum::Router) -> (String, String) {
    let r = send(
        app,
        "GET",
        "/v1/auth/login?return_to=%2F",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, 303);
    let location = url::Url::parse(r.headers["Location"].to_str().unwrap()).unwrap();
    let state = location
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let set = r.headers["Set-Cookie"].to_str().unwrap();
    let pair = set.split(';').next().unwrap().to_owned();
    assert_eq!(pair, format!("apps_oauth_state_{state}=1"));
    assert!(
        set.contains("Path=/v1/auth") && set.contains("HttpOnly") && set.contains("SameSite=Lax")
    );
    (state, pair)
}

async fn callback(app: &axum::Router, state: &str, cookies: &str) -> Response {
    let path = format!("/v1/auth/callback?state={state}&code=c");
    let extra: Vec<(&str, &str)> = if cookies.is_empty() {
        vec![]
    } else {
        vec![("Cookie", cookies)]
    };
    send(app, "GET", &path, None, None, None, &extra).await
}

fn mismatch(r: &Response) -> bool {
    r.status == 403 && r.body["error"]["code"] == "oauth_state_mismatch"
}

#[tokio::test]
async fn an_earlier_attempt_survives_a_later_one() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (first, first_cookie) = start(&app).await;
    let (second, second_cookie) = start(&app).await;
    let both = format!("{first_cookie}; {second_cookie}");

    // The first attempt comes back after the second started: it passes the browser check and goes on to the code
    // exchange (which fails here only because no Accounts server is running).
    let r = callback(&app, &first, &both).await;
    assert!(!mismatch(&r), "{:?}", r.body);
    // Each attempt runs once.
    let r = callback(&app, &first, &both).await;
    assert!(!mismatch(&r) && r.status == 400, "{:?}", r.body);
    // The second still works.
    assert!(!mismatch(&callback(&app, &second, &both).await));
}

#[tokio::test]
async fn a_state_without_its_cookie_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (state, _) = start(&app).await;
    let (_, other_cookie) = start(&app).await;
    assert!(mismatch(&callback(&app, &state, "").await));
    assert!(mismatch(&callback(&app, &state, &other_cookie).await));
    assert!(mismatch(
        &callback(&app, &state, &format!("apps_oauth_state_{state}=0")).await
    ));
}

#[tokio::test]
async fn the_single_cookie_from_before_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let (state, _) = start(&app).await;
    assert!(!mismatch(
        &callback(&app, &state, &format!("apps_oauth_state={state}")).await
    ));
}
