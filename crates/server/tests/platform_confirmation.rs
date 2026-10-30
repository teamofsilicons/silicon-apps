mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::*;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use silicon_apps_server::{AppState, router};
use tower::ServiceExt;

async fn upload(
    app: &axum::Router,
    target: &str,
    confirmed: bool,
    key: &str,
    token: &str,
    bytes: Vec<u8>,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/v1/apps/test-app/packages/{target}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/gzip")
        .header("Idempotency-Key", key);
    if confirmed {
        req = req.header("X-Apps-Platform-Confirmed", "true");
    }
    let r = app
        .clone()
        .oneshot(req.body(Body::from(bytes)).unwrap())
        .await
        .unwrap();
    let status = r.status();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn nonlinux_requires_explicit_author_confirmation_and_preserves_archive_checks() {
    let dir = tempfile::tempdir().unwrap();
    let app = router(AppState::new(config(dir.path())).unwrap());
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/apps",
            Some(ALICE),
            json!({"app_id":"test-app","name":"Test"}),
            Some("create-test-app")
        )
        .await
        .0,
        StatusCode::OK
    );
    for target in [
        "windows-x86_64",
        "windows-i686",
        "windows-aarch64",
        "macos-aarch64",
        "macos-x86_64",
    ] {
        let bytes = package_bytes("test-app", target);
        let key = format!("confirm-{target}");
        let r = upload(&app, target, false, &key, ALICE, bytes.clone()).await;
        assert_eq!(r.0, StatusCode::BAD_REQUEST);
        assert_eq!(r.1["error"]["code"], "platform_confirmation_required");
        assert_eq!(
            upload(&app, target, true, &key, BOB, bytes.clone()).await.0,
            StatusCode::FORBIDDEN
        );
        let r = upload(&app, target, true, &key, ALICE, bytes.clone()).await;
        assert_eq!(r.0, StatusCode::OK, "{}", r.1);
        for check in r.1["validation"].as_array().unwrap() {
            assert_eq!(check["source"], "uploader_confirmation");
            assert_eq!(check["execution"], "not_run");
            assert_eq!(check["confirmed_by"], "alice");
            assert!(check["exit_code"].is_null());
        }
        let replay = upload(&app, target, true, &key, ALICE, bytes.clone()).await;
        assert_eq!(replay.0, StatusCode::OK, "{}", replay.1);
        assert_eq!(replay.1["id"], r.1["id"]);
        assert_eq!(
            upload(&app, target, false, &key, ALICE, bytes).await.0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            upload(
                &app,
                target,
                true,
                &format!("bad-{target}"),
                ALICE,
                b"not an archive".to_vec()
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let caps = send(&app, "GET", "/v1/capabilities", None, None, None, &[])
        .await
        .body;
    assert_eq!(
        caps["targets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["target"] == "macos-aarch64")
            .unwrap()["validation"],
        "uploader_confirmation"
    );
}
#[tokio::test]
async fn confirmation_cannot_bypass_linux_runner_validation() {
    let dir = tempfile::tempdir().unwrap();
    let app = router(AppState::new(config(dir.path())).unwrap());
    call(
        &app,
        "POST",
        "/v1/apps",
        Some(ALICE),
        json!({"app_id":"test-app","name":"Test"}),
        Some("create-test-app"),
    )
    .await;
    assert_eq!(
        upload(
            &app,
            "linux-x86_64",
            true,
            "linux-still-needs-runner",
            ALICE,
            package_bytes("test-app", "linux-x86_64")
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
