//! OpenAPI agreement, capabilities, version negotiation, the agent card,
//! rate limits, structured errors and catalog search.
mod common;
use axum::{body::Body, http::Request};
use common::*;
use serde_json::{Value, json};
use silicon_apps_server::{AppState, discovery, router, routes::ROUTES};
use std::collections::BTreeSet;
use tower::ServiceExt;

fn documented() -> BTreeSet<(String, String)> {
    let spec: Value = serde_json::from_str(discovery::OPENAPI).unwrap();
    assert_eq!(spec["openapi"], "3.1.0");
    let mut operations = BTreeSet::new();
    for (path, item) in spec["paths"].as_object().unwrap() {
        for method in item.as_object().unwrap().keys() {
            if ["get", "post", "put", "patch", "delete"].contains(&method.as_str()) {
                operations.insert((method.to_uppercase(), path.clone()));
            }
        }
    }
    operations
}

#[test]
fn openapi_describes_exactly_the_route_table() {
    let routed: BTreeSet<_> = ROUTES
        .iter()
        .map(|(m, p)| (m.to_string(), p.to_string()))
        .collect();
    assert_eq!(routed.len(), ROUTES.len(), "duplicate route entries");
    let documented = documented();
    let missing: Vec<_> = routed.difference(&documented).collect();
    let extra: Vec<_> = documented.difference(&routed).collect();
    assert!(
        missing.is_empty(),
        "routes missing from openapi.json: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "openapi.json documents unknown routes: {extra:?}"
    );
    // Every operation has an operationId, summary and a 429 or 400 answer.
    let spec: Value = serde_json::from_str(discovery::OPENAPI).unwrap();
    let mut ids = BTreeSet::new();
    for item in spec["paths"].as_object().unwrap().values() {
        for operation in item.as_object().unwrap().values() {
            assert!(ids.insert(operation["operationId"].as_str().unwrap().to_owned()));
            assert!(operation["summary"].is_string());
        }
    }
    assert!(!discovery::OPENAPI.contains('\u{2014}') && !discovery::OPENAPI.contains('\u{2013}'));
}

/// Fresh seeded state per route, so a destructive call never hides another.
/// A fresh base64 Ed25519 public key.
fn author_public_key() -> String {
    let entry = silicon_apps_server::signing::generate("author").0;
    let ring = silicon_apps_server::signing::Keyring::parse(&entry, &[]).unwrap();
    ring.active_public_key()
}

async fn seeded() -> (tempfile::TempDir, axum::Router, Value) {
    let (dir, state, app) = fresh();
    let (package_id, release_id) = seed_app(
        &state,
        "alice",
        "demo-app",
        &["tools"],
        "linux-x86_64",
        true,
    );
    let (status, invite) = call(
        &app,
        "POST",
        "/v1/apps/demo-app/invites",
        Some(ALICE),
        json!({"to":"si:bob"}),
        Some("invite-bob"),
    )
    .await;
    assert_eq!(status, 200, "{invite}");
    let (status, _) = call(
        &app,
        "POST",
        &format!("/v1/invites/{}/accept", invite["id"].as_str().unwrap()),
        Some("dev:bob:si:bob"),
        json!({}),
        Some("accept-bob"),
    )
    .await;
    assert_eq!(status, 200);
    let (_, pending) = call(
        &app,
        "POST",
        "/v1/apps/demo-app/invites",
        Some(ALICE),
        json!({"to":"si:carol"}),
        Some("invite-carol"),
    )
    .await;
    let png = b"\x89PNG\r\n\x1a\nfixture".to_vec();
    let media = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/apps/demo-app/media")
                .header("Authorization", format!("Bearer {ALICE}"))
                .header("Content-Type", "image/png")
                .header("Idempotency-Key", "media-upload")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(media.status(), 200);
    let media: Value = serde_json::from_slice(
        &http_body_util::BodyExt::collect(media.into_body())
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let (status, sub) = call(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(ALICE),
        json!({"app_id":"demo-app","delivery":{"mode":"webhook","url":"http://127.0.0.1:9/hook"}}),
        Some("seed-subscription"),
    )
    .await;
    assert_eq!(status, 201, "{sub}");
    let (status, author_key) = call(
        &app,
        "POST",
        "/v1/keys",
        Some(ALICE),
        json!({"public_key":author_public_key(),"name":"laptop"}),
        Some("seed-author-key"),
    )
    .await;
    assert_eq!(status, 201, "{author_key}");
    let ids = json!({
        "app_id":"demo-app","target":"linux-x86_64","package_id":package_id,"release_id":release_id,
        "invite_id":pending["id"],"media_id":media["id"],"subscription_id":sub["subscription"]["id"],"uuid":"bob",
        "key_id":author_key["key"]["key_id"]
    });
    (dir, app, ids)
}

#[tokio::test]
async fn every_documented_operation_is_served() {
    for (method, template) in ROUTES {
        let (_dir, app, ids) = seeded().await;
        let mut path = template.to_string();
        for (name, value) in ids.as_object().unwrap() {
            path = path.replace(&format!("{{{name}}}"), value.as_str().unwrap());
        }
        if *template == "/v1/authors/{uuid}" {
            path = "/v1/authors/alice".into();
        }
        if *template == "/v1/apps/{app_id}/resolve" {
            path.push_str("?target=linux-x86_64&channel=development");
        }
        assert!(!path.contains('{'), "unsubstituted {path}");
        let token = if template.starts_with("/v1/invites/{invite_id}") {
            CAROL
        } else {
            ALICE
        };
        let body = match (*method, *template) {
            ("POST", "/v1/apps") => json!({"app_id":"another-app","name":"Another"}),
            ("POST", "/v1/apps/{app_id}/releases/{release_id}/promote") => {
                json!({"version":"1.0.0"})
            }
            ("POST", "/v1/apps/{app_id}/installs") => {
                json!({"release_id":ids["release_id"],"package_id":ids["package_id"]})
            }
            ("POST", "/v1/apps/{app_id}/releases/{release_id}/withdraw") => {
                json!({"reason":"Crashes on start."})
            }
            ("POST", "/v1/keys") => json!({"public_key":author_public_key()}),
            _ => json!({}),
        };
        let request = Request::builder()
            .method(*method)
            .uri(&path)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Idempotency-Key", "route-coverage")
            .body(if *method == "GET" {
                Body::empty()
            } else {
                Body::from(body.to_string())
            })
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        // Streams stay open; only their status matters here.
        if !path.ends_with("/stream") {
            let _ = http_body_util::BodyExt::collect(response.into_body()).await;
        }
        assert!(
            status != 404 && status != 405,
            "{method} {path} answered {status}"
        );
    }
}

#[tokio::test]
async fn openapi_and_agent_card_are_served_from_the_api() {
    let (_dir, _state, app) = fresh();
    for path in ["/openapi.json", "/v1/openapi.json"] {
        let r = send(&app, "GET", path, None, None, None, &[]).await;
        assert_eq!(r.status, 200);
        assert_eq!(r.headers["content-type"], "application/json");
        assert_eq!(
            r.body,
            serde_json::from_str::<Value>(discovery::OPENAPI).unwrap()
        );
    }
    for path in ["/.well-known/agent.json", "/.well-known/agent-card.json"] {
        let r = send(&app, "GET", path, None, None, None, &[]).await;
        assert_eq!(r.status, 200);
        let card = r.body;
        assert_eq!(card["name"], "Silicon Apps");
        assert_eq!(card["url"], "http://127.0.0.1:4311");
        assert_eq!(card["provider"]["organization"], "Team of Silicons");
        assert_eq!(card["capabilities"]["streaming"], true);
        assert_eq!(card["capabilities"]["pushNotifications"], true);
        let skills: Vec<_> = card["skills"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            skills,
            [
                "search-apps",
                "get-app",
                "install-app",
                "publish-release",
                "subscribe-to-releases"
            ]
        );
        assert_eq!(
            card["links"]["openapi"],
            "http://127.0.0.1:4311/openapi.json"
        );
        assert_eq!(card["links"]["llms_txt"], "http://127.0.0.1:4311/llms.txt");
        assert_eq!(card["links"]["mcp"], "http://127.0.0.1:4311/mcp");
        assert_eq!(
            card["links"]["docs"],
            "https://developers.teamofsilicons.com/docs/apps"
        );
    }
}

#[tokio::test]
async fn capabilities_answer_requirements_and_report_live_workers() {
    let (_dir, _state, app) = fresh();
    let r = send(&app, "GET", "/v1/capabilities", None, None, None, &[]).await;
    assert_eq!(r.status, 200);
    let c = r.body;
    assert_eq!(c["api"]["current"], "2026-10-09");
    assert_eq!(c["targets"].as_array().unwrap().len(), 9);
    assert!(
        c["targets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["validation"] == "not_configured")
    );
    assert!(c["requirements"].is_null());
    assert_eq!(c["streaming"]["sse"], true);
    assert_eq!(
        c["links"]["agent_card"],
        "http://127.0.0.1:4311/.well-known/agent.json"
    );
    // Everything required is there: 200 with the answers.
    let r = send(
        &app,
        "GET",
        "/v1/capabilities?require=streaming,subscriptions,signing,withdrawal,version:2026-10-09",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["requirements"]["satisfied"], true);
    // Something missing: 422 that names it and says why.
    let r = send(&app, "GET", "/v1/capabilities?require=streaming,subscriptions,version:2026-10-09,auth:bearer,event:release.promoted,target:linux-x86_64,teleportation", None, None, None, &[]).await;
    assert_eq!(r.status, 422, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "capabilities_missing");
    let results = &r.body["error"]["details"];
    let missing: Vec<_> = results["missing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["requirement"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(missing, ["target:linux-x86_64", "teleportation"]);
    let by_name = |name: &str| {
        results["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["requirement"] == name)
            .unwrap()
            .clone()
    };
    for name in [
        "streaming",
        "subscriptions",
        "version:2026-10-09",
        "auth:bearer",
        "event:release.promoted",
    ] {
        assert_eq!(by_name(name)["satisfied"], true, "{name}");
    }
    assert_eq!(by_name("target:linux-x86_64")["satisfied"], false);
    assert!(
        by_name("target:linux-x86_64")["reason"]
            .as_str()
            .unwrap()
            .contains("not configured")
    );
    assert_eq!(by_name("teleportation")["satisfied"], false);

    // A configured, reachable worker is live; an unreachable one is not.
    let (runner, task) = fake_runner("demo-app").await;
    for (url, live) in [
        (runner.clone(), true),
        ("http://127.0.0.1:9".to_owned(), false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.runner_url = Some(url);
        cfg.runner_token = Some("runner-token-runner-token-runner-token".into());
        cfg.runner_targets = vec!["linux-x86_64".into()];
        let app = router(AppState::new(cfg).unwrap());
        let r = send(
            &app,
            "GET",
            "/v1/capabilities?require=target:linux-x86_64,target:macos-aarch64",
            None,
            None,
            None,
            &[],
        )
        .await;
        assert_eq!(r.status, 422, "{}", r.body);
        let results = r.body["error"]["details"]["results"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(results[0]["satisfied"], live, "{results:?}");
        assert_eq!(results[1]["satisfied"], false);
        let r = send(&app, "GET", "/v1/capabilities", None, None, None, &[]).await;
        assert_eq!(r.body["validation_runner"]["reachable"], live);
        let linux = r.body["targets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["target"] == "linux-x86_64")
            .unwrap()
            .clone();
        assert_eq!(
            linux["validation"],
            if live { "live" } else { "unreachable" }
        );
    }
    task.abort();
}

#[tokio::test]
async fn apps_version_is_negotiated_echoed_and_unknown_versions_are_refused() {
    let (_dir, _state, app) = fresh();
    let plain = send(&app, "GET", "/v1/apps", None, None, None, &[]).await;
    assert_eq!(plain.status, 200);
    assert_eq!(plain.headers["apps-version"], "2026-10-09");
    assert!(
        plain.headers["vary"]
            .to_str()
            .unwrap()
            .contains("Apps-Version")
    );
    let pinned = send(
        &app,
        "GET",
        "/v1/apps",
        None,
        None,
        None,
        &[("Apps-Version", "2026-10-09")],
    )
    .await;
    assert_eq!(pinned.body, plain.body);
    assert_eq!(pinned.headers["apps-version"], "2026-10-09");
    let preferred = send(
        &app,
        "GET",
        "/health",
        None,
        None,
        None,
        &[("Apps-Version", "2030-01-01, 2026-10-09")],
    )
    .await;
    assert_eq!(preferred.status, 200);
    assert_eq!(preferred.headers["apps-version"], "2026-10-09");
    for path in ["/v1/apps", "/health", "/openapi.json"] {
        let refused = send(
            &app,
            "GET",
            path,
            None,
            None,
            None,
            &[("Apps-Version", "2030-01-01")],
        )
        .await;
        assert_eq!(refused.status, 400);
        assert_eq!(refused.body["error"]["code"], "unsupported_api_version");
        assert_eq!(
            refused.body["error"]["details"]["supported"],
            json!(["2026-10-09"])
        );
        assert_eq!(
            refused.body["error"]["details"]["requested"],
            json!(["2030-01-01"])
        );
    }
}

#[tokio::test]
async fn rate_limits_return_429_with_retry_after_and_structured_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.rate_limit_reads_per_minute = 3;
    cfg.rate_limit_writes_per_minute = 1;
    cfg.rate_limit_streams = 1;
    let state = AppState::new(cfg).unwrap();
    let app = router(state.clone());
    for remaining in ["2", "1", "0"] {
        let r = send(&app, "GET", "/v1/apps", Some(ALICE), None, None, &[]).await;
        assert_eq!(r.status, 200);
        assert_eq!(r.headers["ratelimit-limit"], "3");
        assert_eq!(r.headers["ratelimit-remaining"], remaining);
    }
    let limited = send(&app, "GET", "/v1/apps", Some(ALICE), None, None, &[]).await;
    assert_eq!(limited.status, 429);
    let retry: u64 = limited.headers["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&retry));
    assert_eq!(limited.body["error"]["code"], "rate_limited");
    assert_eq!(limited.body["error"]["details"]["limit"], 3);
    assert_eq!(limited.body["error"]["details"]["scope"], "read");
    assert_eq!(limited.headers["apps-version"], "2026-10-09");
    // Other clients and the health check are unaffected.
    assert_eq!(
        send(&app, "GET", "/v1/apps", Some(BOB), None, None, &[])
            .await
            .status,
        200
    );
    assert_eq!(
        send(&app, "GET", "/health", Some(ALICE), None, None, &[])
            .await
            .status,
        200
    );
    // Writes have their own bucket.
    let first = send(
        &app,
        "POST",
        "/v1/platforms",
        Some(BOB),
        Some(json!({"target":"linux-x86_64"})),
        Some("platform-1"),
        &[],
    )
    .await;
    assert_eq!(first.status, 200);
    let second = send(
        &app,
        "POST",
        "/v1/platforms",
        Some(BOB),
        Some(json!({"target":"linux-x86_64"})),
        Some("platform-2"),
        &[],
    )
    .await;
    assert_eq!(second.status, 429);
    assert_eq!(second.body["error"]["details"]["scope"], "write");

    // One open stream per client.
    let (base, task) = serve(state).await;
    let first = reqwest::Client::new()
        .get(format!("{base}/v1/events/stream"))
        .bearer_auth(CAROL)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    let second = reqwest::Client::new()
        .get(format!("{base}/v1/events/stream"))
        .bearer_auth(CAROL)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), 429);
    // Every 429 says when to come back.
    assert_eq!(second.headers()["retry-after"], "5");
    assert_eq!(
        second.json::<Value>().await.unwrap()["error"]["code"],
        "too_many_streams"
    );
    drop(first);
    let mut reopened = false;
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let again = reqwest::Client::new()
            .get(format!("{base}/v1/events/stream"))
            .bearer_auth(CAROL)
            .send()
            .await
            .unwrap();
        if again.status() == 200 {
            reopened = true;
            break;
        }
    }
    assert!(reopened, "closing a stream frees its slot");
    task.abort();
}

#[tokio::test]
async fn unknown_paths_and_methods_get_structured_errors() {
    let (_dir, _state, app) = fresh();
    for (method, path, status, code) in [
        ("GET", "/nope", 404, "not_found"),
        ("GET", "/v1/nope", 404, "not_found"),
        ("POST", "/openapi.json", 405, "method_not_allowed"),
        (
            "DELETE",
            "/.well-known/agent.json",
            405,
            "method_not_allowed",
        ),
    ] {
        let r = send(&app, method, path, None, None, None, &[]).await;
        assert_eq!(r.status, status, "{method} {path}");
        assert_eq!(r.body["error"]["code"], code, "{method} {path}");
        assert!(r.body["error"]["hint"].is_string());
    }
}

#[tokio::test]
async fn search_filters_by_tags_target_and_visibility_sorts_and_paginates() {
    let (_dir, state, app) = fresh();
    seed_app(
        &state,
        "alice",
        "alpha-tool",
        &["Tools", "cli"],
        "linux-x86_64",
        true,
    );
    seed_app(
        &state,
        "alice",
        "beta-tool",
        &["tools"],
        "macos-aarch64",
        true,
    );
    seed_app(&state, "bob", "gamma-chat", &["chat"], "linux-x86_64", true);
    seed_app(
        &state,
        "bob",
        "delta-draft",
        &["tools"],
        "linux-x86_64",
        false,
    );
    let app_view = |id: &str| state.store.lock().unwrap().app(id).unwrap();
    for i in 0..3 {
        let a = app_view("gamma-chat");
        let release = a.releases[0].clone();
        let r = send(
            &app,
            "POST",
            "/v1/apps/gamma-chat/installs",
            None,
            Some(json!({"release_id":release.id,"package_id":release.package_ids[0]})),
            Some(&format!("install-gamma-{i}")),
            &[],
        )
        .await;
        assert_eq!(r.status, 200);
    }
    let ids = |body: &Value| {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["app_id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let r = send(&app, "GET", "/v1/apps?tags=tools", None, None, None, &[]).await;
    assert_eq!(ids(&r.body), ["alpha-tool", "beta-tool"]);
    let r = send(
        &app,
        "GET",
        "/v1/apps?tags=tools,cli",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&r.body), ["alpha-tool"]);
    let r = send(
        &app,
        "GET",
        "/v1/apps?target=linux-x86_64",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&r.body), ["alpha-tool", "gamma-chat"]);
    let r = send(&app, "GET", "/v1/apps?sort=installs", None, None, None, &[]).await;
    assert_eq!(ids(&r.body)[0], "gamma-chat");
    let r = send(
        &app,
        "GET",
        "/v1/apps?sort=name&limit=2",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&r.body), ["alpha-tool", "beta-tool"]);
    assert_eq!(r.body["total"], 3);
    assert_eq!(r.body["next_offset"], 2);
    assert_eq!(r.body["sort"], "name");
    let r = send(
        &app,
        "GET",
        "/v1/apps?sort=name&limit=2&offset=2",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&r.body), ["gamma-chat"]);
    assert_eq!(r.body["next_offset"], Value::Null);
    let r = send(&app, "GET", "/v1/apps?q=gama", None, None, None, &[]).await;
    assert_eq!(ids(&r.body), ["gamma-chat"]);
    let r = send(
        &app,
        "GET",
        "/v1/apps?visibility=public&tags=chat",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&r.body), ["gamma-chat"]);
    for (query, field) in [
        ("sort=random", "sort"),
        ("target=plan9-mips", "target"),
        ("limit=0", "limit"),
        ("limit=abc", "limit"),
        ("offset=-1", "offset"),
        ("visibility=secret", "visibility"),
    ] {
        let r = send(
            &app,
            "GET",
            &format!("/v1/apps?{query}"),
            None,
            None,
            None,
            &[],
        )
        .await;
        assert_eq!(r.status, 400, "{query}");
        assert_eq!(r.body["error"]["code"], "invalid_input");
        assert!(
            r.body["error"]["message"].as_str().unwrap().contains(field),
            "{query}: {}",
            r.body
        );
    }
    let r = send(
        &app,
        "GET",
        "/v1/apps?visibility=private",
        None,
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, 401);
}
