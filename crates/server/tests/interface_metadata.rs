mod common;
use common::*;
use serde_json::{Value, json};

#[tokio::test]
async fn authoring_publication_and_public_discovery() {
    let (_dir, state, app) = fresh();
    seed_app(&state, "alice", "ring", &[], "linux-x86_64", false);
    let path = "/v1/apps/ring";
    let clear = json!({"interface_api_documentation_url":null});
    assert_eq!(
        call(&app, "PATCH", path, Some(ALICE), clear, Some("clear-docs"))
            .await
            .0,
        200
    );
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
    assert!(
        ready["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["field"] == "interface_api_documentation_url")
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/apps/ring/publish",
            Some(ALICE),
            json!({}),
            Some("not-ready")
        )
        .await
        .0,
        400
    );
    let material = json!({
        "interface_api_documentation_url":"https://example.com/interface-api",
        "interface_use_cases":[{"name":"Write together","files":[{"name":"one.md","content":"# Hello\n"},{"name":"two.md","content":"More"}]}],
        "interface_license":{"kind":"preset","preset":"attribution_unless_paid"}
    });
    assert_eq!(
        call(
            &app,
            "PATCH",
            path,
            Some(BOB),
            material.clone(),
            Some("not-author")
        )
        .await
        .0,
        403
    );
    let (status, _) = call(
        &app,
        "PATCH",
        path,
        Some(ALICE),
        material.clone(),
        Some("update-interface"),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/apps/ring/publish",
            Some(ALICE),
            json!({}),
            Some("publish-ready")
        )
        .await
        .0,
        200
    );
    let public = send(&app, "GET", path, None, None, None, &[]).await.body;
    for field in [
        "interface_api_documentation_url",
        "interface_use_cases",
        "interface_license",
    ] {
        assert_eq!(public[field], material[field]);
    }
    assert_eq!(
        call(
            &app,
            "PATCH",
            path,
            Some(ALICE),
            json!({"interface_api_documentation_url":null}),
            Some("clear-published")
        )
        .await
        .0,
        400
    );
    // Upload content is not copied into the durable history for every revision.
    let app_data = state.store.lock().unwrap().app("ring").unwrap();
    let event = app_data
        .history
        .iter()
        .find(|h| h.kind == "app.details_changed" && h.data.get("interface_use_cases").is_some())
        .unwrap();
    assert_eq!(event.data["interface_use_cases"], "updated");
}

#[tokio::test]
async fn legacy_records_stay_grandfathered_and_malformed_uploads_are_atomic() {
    let (_dir, state, app) = fresh();
    seed_app(&state, "alice", "old-app", &[], "linux-x86_64", false);
    // Simulate a persisted pre-feature row: all new serde fields are absent.
    {
        let store = state.store.lock().unwrap();
        let raw: String = store
            .connection
            .query_row("SELECT document FROM catalog WHERE id=1", [], |r| r.get(0))
            .unwrap();
        let mut catalog: Value = serde_json::from_str(&raw).unwrap();
        let item = catalog["apps"]["old-app"].as_object_mut().unwrap();
        for field in [
            "interface_documentation_required",
            "interface_api_documentation_url",
            "interface_use_cases",
            "interface_license",
        ] {
            item.remove(field);
        }
        store
            .connection
            .execute(
                "UPDATE catalog SET document=?1 WHERE id=1",
                [catalog.to_string()],
            )
            .unwrap();
    }
    let path = "/v1/apps/old-app";
    let ready = send(
        &app,
        "GET",
        "/v1/apps/old-app/readiness",
        Some(ALICE),
        None,
        None,
        &[],
    )
    .await
    .body;
    assert_eq!(ready["ready"], true);
    assert_eq!(call(&app, "PATCH", path, Some(ALICE), json!({"interface_api_documentation_url":"https://example.com/docs", "interface_license":{"kind":"file", "name":"LICENSE.txt", "content":"Terms"}}), Some("old-edit")).await.0, 200);
    assert_eq!(
        call(
            &app,
            "PATCH",
            path,
            Some(ALICE),
            json!({"interface_api_documentation_url":null}),
            Some("old-clear")
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/apps/old-app/publish",
            Some(ALICE),
            json!({}),
            Some("old-publish")
        )
        .await
        .0,
        200
    );
    let (status, _) = call(&app, "PATCH", path, Some(ALICE), json!({"interface_api_documentation_url":"https://example.com/new", "interface_use_cases":[{"name":"Bad", "files":[{"name":"bad.html","content":"x"}]}]}), Some("invalid-file")).await;
    assert_eq!(status, 400);
    let old = send(&app, "GET", path, None, None, None, &[]).await.body;
    assert!(old["interface_api_documentation_url"].is_null());
    assert_eq!(old["interface_license"]["content"], "Terms");
    for (i, value) in [
        json!("http://example.com"),
        json!("https://user:pass@example.com"),
        json!("javascript:alert(1)"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            call(
                &app,
                "PATCH",
                path,
                Some(ALICE),
                json!({"interface_api_documentation_url":value}),
                Some(&format!("invalid-url-{i}"))
            )
            .await
            .0,
            400
        );
    }
}
