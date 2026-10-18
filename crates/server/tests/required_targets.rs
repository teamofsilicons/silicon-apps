//! Release requirements apply to new writes; historical downloads remain available.
mod common;
use common::*;
use serde_json::json;
use silicon_apps_server::model::REQUIRED_RELEASE_TARGETS;

#[tokio::test]
async fn every_required_target_must_be_in_the_same_release() {
    let (_dir, state, app) = fresh();
    seed_app(&state, "alice", "ring", &[], "linux-aarch64", false);
    let ids: Vec<_> = REQUIRED_RELEASE_TARGETS
        .iter()
        .map(|t| format!("ring-{t}-pkg"))
        .collect();
    for (index, target) in REQUIRED_RELEASE_TARGETS.iter().enumerate() {
        let mut selected = ids.clone();
        selected.remove(index);
        // An optional target and an already uploaded required package do not fill the gap.
        selected.push("ring-linux-aarch64-pkg".into());
        let (status, error) = call(
            &app,
            "POST",
            "/v1/apps/ring/releases",
            Some(ALICE),
            json!({"version":"0.2.0","package_ids":selected}),
            Some(&format!("missing-{index}")),
        )
        .await;
        assert_eq!(status, 400, "{error}");
        assert_eq!(error["error"]["code"], "required_targets_missing");
        assert_eq!(
            error["error"]["details"]["missing_targets"],
            json!([target])
        );
        assert_eq!(
            error["error"]["details"]["required_targets"],
            json!(REQUIRED_RELEASE_TARGETS)
        );
    }
    let (status, error) = call(
        &app,
        "POST",
        "/v1/apps/ring/releases",
        Some(ALICE),
        json!({"version":"0.2.0","package_ids":[]}),
        Some("empty-release"),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(
        error["error"]["details"]["missing_targets"],
        json!(REQUIRED_RELEASE_TARGETS)
    );
    assert_eq!(
        state
            .store
            .lock()
            .unwrap()
            .app("ring")
            .unwrap()
            .releases
            .len(),
        1
    );
    let mut selected = ids.clone();
    selected.push("ring-linux-aarch64-pkg".into());
    let (status, release) = call(
        &app,
        "POST",
        "/v1/apps/ring/releases",
        Some(ALICE),
        json!({"version":"0.2.0","package_ids":selected}),
        Some("complete-release"),
    )
    .await;
    assert_eq!(status, 200, "{release}");
    let (status, promoted) = call(
        &app,
        "POST",
        &format!(
            "/v1/apps/ring/releases/{}/promote",
            release["id"].as_str().unwrap()
        ),
        Some(ALICE),
        json!({"version":"1.0.0"}),
        Some("complete-promotion"),
    )
    .await;
    assert_eq!(status, 200, "{promoted}");
    let ready = send(
        &app,
        "GET",
        "/v1/apps/ring/readiness",
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await
    .body;
    assert_eq!(ready["ready"], true);
    assert_eq!(ready["required_targets"], json!(REQUIRED_RELEASE_TARGETS));
    let (status, published) = call(
        &app,
        "POST",
        "/v1/apps/ring/publish",
        Some(ALICE),
        json!({}),
        Some("complete-publish"),
    )
    .await;
    assert_eq!(status, 200, "{published}");
    let capabilities = send(&app, "GET", "/v1/capabilities", None, None, None, &[])
        .await
        .body;
    assert_eq!(
        capabilities["required_release_targets"],
        json!(REQUIRED_RELEASE_TARGETS)
    );
}

#[tokio::test]
async fn historical_incomplete_releases_cannot_be_promoted_or_published_but_still_resolve() {
    let (_dir, state, app) = fresh();
    let (package, release) = seed_app(&state, "alice", "ring", &[], "linux-x86_64", true);
    {
        let store = state.store.lock().unwrap();
        let mut catalog = store.catalog().unwrap();
        catalog.apps.get_mut("ring").unwrap().releases[0].package_ids = vec![package];
        store
            .connection
            .execute(
                "UPDATE catalog SET document=?1 WHERE id=1",
                [serde_json::to_string(&catalog).unwrap()],
            )
            .unwrap();
    }
    let ready = send(
        &app,
        "GET",
        "/v1/apps/ring/readiness",
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await
    .body;
    assert_eq!(ready["ready"], false);
    assert_eq!(
        ready["errors"][0]["missing_targets"],
        json!(["windows-x86_64", "macos-aarch64"])
    );
    let (status, error) = call(
        &app,
        "POST",
        &format!("/v1/apps/ring/releases/{release}/promote"),
        Some(ALICE),
        json!({"version":"1.0.0"}),
        Some("legacy-promotion"),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(error["error"]["code"], "required_targets_missing");
    let (status, _) = call(
        &app,
        "POST",
        "/v1/apps/ring/publish",
        Some(ALICE),
        json!({}),
        Some("legacy-publish"),
    )
    .await;
    assert_eq!(status, 400);
    let resolved = send(
        &app,
        "GET",
        "/v1/apps/ring/resolve?target=linux-x86_64&channel=development",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(resolved.status, 200, "{}", resolved.body);
    assert_eq!(resolved.body["release"]["id"], release);
}

#[test]
fn publishing_checks_latest_releases_on_both_channels_and_ignores_withdrawn_ones() {
    let (_dir, state, _) = fresh();
    seed_app(&state, "alice", "ring", &[], "linux-x86_64", false);
    let mut app = state.store.lock().unwrap().app("ring").unwrap();
    let mut old = app.releases[0].clone();
    old.id = "old-production".into();
    old.channel = "production".into();
    old.package_ids.retain(|id| id.contains("linux-x86_64"));
    app.releases.push(old);
    assert_eq!(
        app.readiness()["ready"],
        false,
        "a complete development release cannot hide incomplete production"
    );
    let mut fixed = app.releases[0].clone();
    fixed.id = "fixed-production".into();
    fixed.channel = "production".into();
    fixed.version = "1.0.0".into();
    app.releases.push(fixed);
    assert_eq!(
        app.readiness()["ready"],
        true,
        "superseded history does not block publishing"
    );
    app.releases[2].withdrawn = Some(silicon_apps_server::model::Withdrawal {
        reason: "broken".into(),
        at: silicon_apps_server::model::now(),
        by_uuid: "alice".into(),
        by_id: "c:alice".into(),
    });
    assert_eq!(
        app.readiness()["ready"],
        false,
        "withdrawal exposes the incomplete fallback"
    );
}
