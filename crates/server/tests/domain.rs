use serde_json::{Value, json};
use silicon_apps_server::{
    model::*,
    store::{Mutation, Prepared, Store, hash},
};
use std::collections::BTreeMap;
fn who(id: &str) -> Identity {
    Identity {
        uuid: id.into(),
        id: format!("c:{id}"),
        display_name: id.into(),
        verified_emails: vec![],
    }
}
fn change(
    s: &mut Store,
    w: Option<&Identity>,
    method: &str,
    path: &str,
    key: &str,
    body: Value,
    p: Prepared,
) -> silicon_apps_server::error::Result<(Value, bool)> {
    let digest = hash(body.to_string().as_bytes());
    s.mutate(Mutation {
        method,
        path,
        key,
        who: w,
        body: &body,
        digest: &digest,
        prepared: p,
    })
}
fn create(s: &mut Store, w: &Identity, id: &str) -> Value {
    change(
        s,
        Some(w),
        "POST",
        "apps",
        &format!("create-{id}"),
        json!({"app_id":id,"name":id,"description":"A".repeat(200)}),
        Prepared {
            secret: Some("sa_app_test_secret".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .0
}
fn package(s: &mut Store, w: &Identity, id: &str, target: &str) -> Value {
    let pkg = Package {
        id: format!("pkg-{target}"),
        target: target.into(),
        sha256: "ab".repeat(32),
        size: 10,
        command: id.into(),
        validation: vec![json!({"passed":true})],
        created_at: now(),
    };
    change(
        s,
        Some(w),
        "POST",
        &format!("apps/{id}/packages/{target}"),
        &format!("package-{id}-{target}"),
        json!({}),
        Prepared {
            package: Some(pkg),
            ..Default::default()
        },
    )
    .unwrap()
    .0
}
fn publish(s: &mut Store, w: &Identity, id: &str) {
    let p = package(s, w, id, "linux-x86_64");
    change(
        s,
        Some(w),
        "POST",
        &format!("apps/{id}/releases"),
        &format!("release-{id}"),
        json!({"version":"1.0.0","package_ids":[p["id"]]}),
        Prepared::default(),
    )
    .unwrap();
    change(
        s,
        Some(w),
        "POST",
        &format!("apps/{id}/publish"),
        &format!("publish-{id}"),
        json!({}),
        Prepared::default(),
    )
    .unwrap();
}
#[test]
fn idempotency_replays_exact_result_and_conflicts_on_payload_route_or_actor_scope() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    let body = json!({"app_id":"test-app","name":"Test"});
    let p = || Prepared {
        secret: Some("one-time-secret".into()),
        ..Default::default()
    };
    let one = change(
        &mut s,
        Some(&a),
        "POST",
        "apps",
        "same-key",
        body.clone(),
        p(),
    )
    .unwrap();
    let two = change(&mut s, Some(&a), "POST", "apps", "same-key", body, p()).unwrap();
    assert!(!one.1);
    assert!(two.1);
    assert_eq!(one.0, two.0);
    assert_eq!(s.catalog().unwrap().apps.len(), 1);
    let e = change(
        &mut s,
        Some(&a),
        "PATCH",
        "apps/test-app",
        "same-key",
        json!({"name":"Different"}),
        p(),
    )
    .unwrap_err();
    assert_eq!(e.status, 409);
    assert_eq!(s.app("test-app").unwrap().name, "Test");
}
#[test]
fn invalid_mutation_does_not_leave_partial_app_details_or_history() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    create(&mut s, &a, "test-app");
    let before = s.app("test-app").unwrap();
    let e = change(
        &mut s,
        Some(&a),
        "PATCH",
        "apps/test-app",
        "bad-edit",
        json!({"name":"Changed","tags":[""]}),
        Prepared::default(),
    )
    .unwrap_err();
    assert_eq!(e.status, 400);
    let after = s.app("test-app").unwrap();
    assert_eq!(before.name, after.name);
    assert_eq!(before.history.len(), after.history.len());
}
#[test]
fn invitees_have_no_author_rights_until_acceptance_and_only_admin_can_remove() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    let b = who("bob");
    let outsider = who("outsider");
    create(&mut s, &a, "test-app");
    let i = change(
        &mut s,
        Some(&a),
        "POST",
        "apps/test-app/invites",
        "invite-bob",
        json!({"to":"c:bob"}),
        Prepared {
            identities: vec![b.clone()],
            ..Default::default()
        },
    )
    .unwrap()
    .0;
    assert_eq!(s.app("test-app").unwrap().authors.len(), 1);
    assert!(
        change(
            &mut s,
            Some(&b),
            "PATCH",
            "apps/test-app",
            "early-edit",
            json!({"name":"No"}),
            Prepared::default()
        )
        .is_err()
    );
    let accept = format!("invites/{}/accept", i["id"].as_str().unwrap());
    assert!(
        change(
            &mut s,
            Some(&outsider),
            "POST",
            &accept,
            "wrong-accept",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    change(
        &mut s,
        Some(&b),
        "POST",
        &accept,
        "real-accept",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    change(
        &mut s,
        Some(&b),
        "PATCH",
        "apps/test-app",
        "bob-edit",
        json!({"name":"Allowed"}),
        Prepared::default(),
    )
    .unwrap();
    assert!(
        change(
            &mut s,
            Some(&b),
            "PUT",
            "apps/test-app/access",
            "bob-access",
            json!({"visibility":"private"}),
            Prepared::default()
        )
        .is_err()
    );
    assert!(
        change(
            &mut s,
            Some(&b),
            "DELETE",
            "apps/test-app/authors/alice",
            "bob-kick",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    change(
        &mut s,
        Some(&a),
        "POST",
        "apps/test-app/authors/leave",
        "alice-leaves",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    assert_eq!(s.app("test-app").unwrap().admin_uuid, "bob");
    assert!(
        change(
            &mut s,
            Some(&b),
            "POST",
            "apps/test-app/authors/leave",
            "last-leaves",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
}
#[test]
fn private_search_details_releases_and_verified_domain_access_are_consistent() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    create(&mut s, &a, "hidden-app");
    publish(&mut s, &a, "hidden-app");
    change(
        &mut s,
        Some(&a),
        "PUT",
        "apps/hidden-app/access",
        "set-private",
        json!({"visibility":"private","domains":["example.com"],"account_ids":[]}),
        Prepared::default(),
    )
    .unwrap();
    let mut b = who("bob");
    assert!(
        s.read("apps/hidden-app", &BTreeMap::new(), Some(&b))
            .is_err()
    );
    assert!(
        s.read("apps/hidden-app/releases", &BTreeMap::new(), None)
            .is_err()
    );
    assert_eq!(s.read("apps", &BTreeMap::new(), None).unwrap()["total"], 0);
    b.verified_emails.push("bob@sub.example.com".into());
    assert!(
        s.read("apps/hidden-app", &BTreeMap::new(), Some(&b))
            .is_err()
    );
    b.verified_emails.push("bob@EXAMPLE.COM".into());
    let view = s
        .read("apps/hidden-app", &BTreeMap::new(), Some(&b))
        .unwrap();
    assert_eq!(view["app_id"], "hidden-app");
    assert!(view.get("domains").is_none());
    assert!(view.get("admin_uuid").is_none());
}
#[test]
fn immutable_releases_keep_separate_channel_versions_and_latest_semver() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    create(&mut s, &a, "test-app");
    publish(&mut s, &a, "test-app");
    let dev = s.app("test-app").unwrap().releases[0].id.clone();
    let promoted = change(
        &mut s,
        Some(&a),
        "POST",
        &format!("apps/test-app/releases/{dev}/promote"),
        "promote-key",
        json!({"version":"2.0.0"}),
        Prepared::default(),
    )
    .unwrap()
    .0;
    assert_eq!(promoted["channel"], "production");
    assert_eq!(promoted["promoted_from"], dev);
    let mut q = BTreeMap::from([("target".into(), "linux-x86_64".into())]);
    assert_eq!(
        s.read("apps/test-app/resolve", &q, None).unwrap()["release"]["version"],
        "2.0.0"
    );
    q.insert("channel".into(), "development".into());
    assert_eq!(
        s.read("apps/test-app/resolve", &q, None).unwrap()["release"]["version"],
        "1.0.0"
    );
    assert!(
        change(
            &mut s,
            Some(&a),
            "POST",
            &format!("apps/test-app/releases/{dev}/promote"),
            "duplicate-promote",
            json!({"version":"2.0.0"}),
            Prepared::default()
        )
        .is_err()
    );
}
#[test]
fn exact_search_match_beats_rating_and_typo_search_works() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    create(&mut s, &a, "briefcase");
    publish(&mut s, &a, "briefcase");
    create(&mut s, &a, "briefcase-pro");
    publish(&mut s, &a, "briefcase-pro");
    change(
        &mut s,
        Some(&a),
        "PUT",
        "apps/briefcase-pro/review",
        "review-key",
        json!({"rating":5}),
        Prepared::default(),
    )
    .unwrap();
    let q = BTreeMap::from([("q".into(), "briefcase".into())]);
    assert_eq!(
        s.read("apps", &q, None).unwrap()["items"][0]["app_id"],
        "briefcase"
    );
    let typo = BTreeMap::from([("q".into(), "brifcase".into())]);
    assert_eq!(s.read("apps", &typo, None).unwrap()["total"], 2);
}
#[test]
fn installs_are_idempotent_reviews_one_per_uuid_and_catalog_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("apps.sqlite");
    let a = who("alice");
    let mut s = Store::open(&path).unwrap();
    create(&mut s, &a, "test-app");
    publish(&mut s, &a, "test-app");
    let app = s.app("test-app").unwrap();
    let b = json!({"release_id":app.releases[0].id,"package_id":app.packages[0].id});
    change(
        &mut s,
        None,
        "POST",
        "apps/test-app/installs",
        "install-key",
        b.clone(),
        Prepared::default(),
    )
    .unwrap();
    change(
        &mut s,
        None,
        "POST",
        "apps/test-app/installs",
        "install-key",
        b,
        Prepared::default(),
    )
    .unwrap();
    for (r, k) in [(3, "review-one"), (5, "review-two")] {
        change(
            &mut s,
            Some(&a),
            "PUT",
            "apps/test-app/review",
            k,
            json!({"rating":r,"text":"Useful"}),
            Prepared::default(),
        )
        .unwrap();
    }
    drop(s);
    let s = Store::open(&path).unwrap();
    let app = s.app("test-app").unwrap();
    assert_eq!(app.installs, 1);
    assert_eq!(app.reviews.len(), 1);
    assert_eq!(app.rating(), Some(5.));
    assert!(app.history.iter().any(|h| h.kind == "app.installed"));
}
#[test]
fn immutable_id_limits_and_readiness_cannot_be_bypassed() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    create(&mut s, &a, "test-app");
    assert!(
        change(
            &mut s,
            Some(&a),
            "PATCH",
            "apps/test-app",
            "id-change",
            json!({"app_id":"hijack"}),
            Prepared::default()
        )
        .is_err()
    );
    assert!(
        change(
            &mut s,
            Some(&a),
            "POST",
            "apps/test-app/publish",
            "no-packages",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    assert!(!s.app("test-app").unwrap().published);
    assert!(version_tuple("01.2.3").is_none());
    assert!(version_tuple("1.2.3-beta").is_none());
    assert_eq!(version_tuple("1.20.3"), Some((1, 20, 3)));
}
#[test]
fn one_time_secret_replay_expires_and_plaintext_is_removed_from_storage() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    create(&mut s, &a, "test-app");
    s.connection
        .execute(
            "UPDATE idempotency SET created_at='2000-01-01T00:00:00Z'",
            [],
        )
        .unwrap();
    assert_eq!(s.expire_secret_replays().unwrap(), 1);
    let raw: String = s
        .connection
        .query_row("SELECT response FROM idempotency", [], |r| r.get(0))
        .unwrap();
    assert!(!raw.contains("sa_app_test_secret"));
    let body = json!({"app_id":"test-app","name":"test-app","description":"A".repeat(200)});
    let e = change(
        &mut s,
        Some(&a),
        "POST",
        "apps",
        "create-test-app",
        body,
        Prepared::default(),
    )
    .unwrap_err();
    assert_eq!(e.code, "secret_replay_expired");
    assert!(
        s.app("test-app")
            .unwrap()
            .history
            .iter()
            .all(|h| !h.data.to_string().contains("secret"))
    );
}
