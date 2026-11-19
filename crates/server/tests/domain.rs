use serde_json::{Value, json};
use silicon_apps_server::{
    model::*,
    store::{Mutation, Prepared, Store, hash},
};
use std::collections::BTreeMap;
fn who(id: &str) -> Identity {
    Identity {
        publishing: Default::default(),
        publishing_error: None,
        app_audience: String::new(),
        uuid: id.into(),
        id: format!("c:{id}"),
        display_name: id.into(),
        pfp_url: None,
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
        json!({"app_id":id,"name":id,"description":"A".repeat(200),"tagline":"A test app"}),
        Prepared {
            secret: Some("sa_app_test_secret".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .0
}

#[test]
fn catalog_signin_observes_sharing_without_inheriting_the_users_author_permissions() {
    let mut store = Store::memory().unwrap();
    let owner = who("alice");
    create(&mut store, &owner, "catalog-test");
    let mut app = store.app("catalog-test").unwrap();
    let mut catalog_user = owner.clone();
    catalog_user.app_audience = silicon_apps_server::model::CATALOG_AUDIENCE.into();
    assert!(app.visible(Some(&owner)));
    assert!(!app.visible(Some(&catalog_user)));
    app.published = true;
    app.visibility = "private".into();
    assert!(!app.visible(Some(&catalog_user)));
    app.access_uuids.push(owner.uuid.clone());
    assert!(app.visible(Some(&catalog_user)));
    assert!(!app.is_admin(Some(&catalog_user)));
    let view = app.view(Some(&catalog_user));
    assert!(view.get("account_ids").is_none());
    assert!(view.get("organization_access").is_none());
    app.deleted_at = Some("2026-11-18T00:00:00Z".into());
    assert!(!app.visible(Some(&catalog_user)));
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
        install_script: None,
        inspected: true,
        author_signature: None,
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
    change(
        s,
        Some(w),
        "PATCH",
        &format!("apps/{id}"),
        &format!("interface-docs-{id}"),
        json!({"interface_api_documentation_url":"https://example.com/interface-api"}),
        Prepared::default(),
    )
    .unwrap();
    let packages: Vec<_> = REQUIRED_RELEASE_TARGETS
        .into_iter()
        .map(|target| package(s, w, id, target)["id"].clone())
        .collect();
    change(
        s,
        Some(w),
        "POST",
        &format!("apps/{id}/releases"),
        &format!("release-{id}"),
        json!({"version":"1.0.0","package_ids":packages}),
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
fn author_profiles_use_stable_identity_paginate_and_never_expose_drafts_or_private_apps() {
    let mut s = Store::memory().unwrap();
    let alice = who("alice");
    let bob = who("bob");
    for id in ["alpha", "bravo", "hidden", "draft"] {
        create(&mut s, &alice, id);
        if id != "draft" {
            publish(&mut s, &alice, id);
        }
    }
    change(
        &mut s,
        Some(&alice),
        "PUT",
        "apps/hidden/access",
        "hide-author-app",
        json!({"visibility":"private","domains":[],"account_ids":[]}),
        Prepared::default(),
    )
    .unwrap();
    create(&mut s, &bob, "unrelated");
    publish(&mut s, &bob, "unrelated");
    let invitation = change(
        &mut s,
        Some(&alice),
        "POST",
        "apps/alpha/invites",
        "invite-coauthor",
        json!({"to":"c:bob"}),
        Prepared {
            identities: vec![bob.clone()],
            ..Default::default()
        },
    )
    .unwrap()
    .0;
    assert_eq!(
        s.read("authors/bob", &BTreeMap::new(), None).unwrap()["total"],
        1
    );
    change(
        &mut s,
        Some(&bob),
        "POST",
        &format!("invites/{}/accept", invitation["id"].as_str().unwrap()),
        "accept-coauthor",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    let coauthored = s.read("authors/bob", &BTreeMap::new(), None).unwrap();
    assert_eq!(coauthored["total"], 2);
    assert_eq!(coauthored["items"][0]["app_id"], "alpha");
    let all = s.read("authors/alice", &BTreeMap::new(), None).unwrap();
    assert_eq!(all["uuid"], "alice");
    assert_eq!(all["total"], 2);
    assert_eq!(all["items"][0]["app_id"], "alpha");
    assert_eq!(all["items"][1]["app_id"], "bravo");
    let q = BTreeMap::from([("limit".into(), "1".into()), ("offset".into(), "1".into())]);
    let page = s.read("authors/alice", &q, None).unwrap();
    assert_eq!(page["total"], 2);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["app_id"], "bravo");
    let owner = s
        .read(
            "authors/alice",
            &BTreeMap::from([("mine".into(), "true".into())]),
            Some(&alice),
        )
        .unwrap();
    assert_eq!(owner["total"], 3);
    assert!(
        owner["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|app| app["app_id"] != "draft")
    );
    assert_eq!(
        s.read("authors/unknown", &BTreeMap::new(), None)
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(
        s.read("authors/c:alice", &BTreeMap::new(), None)
            .unwrap_err()
            .status,
        404
    );
    let private = who("private-author");
    create(&mut s, &private, "private-only");
    publish(&mut s, &private, "private-only");
    change(
        &mut s,
        Some(&private),
        "PUT",
        "apps/private-only/access",
        "hide-private-only",
        json!({"visibility":"private","domains":[],"account_ids":[]}),
        Prepared::default(),
    )
    .unwrap();
    assert_eq!(
        s.read("authors/private-author", &BTreeMap::new(), None)
            .unwrap_err()
            .status,
        404
    );
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
    let mut renamed = b.clone();
    renamed.id = "c:bob-renamed".into();
    assert_eq!(
        change(
            &mut s,
            Some(&a),
            "POST",
            "apps/test-app/invites",
            "duplicate-renamed-invite",
            json!({"to":"c:bob-renamed"}),
            Prepared {
                identities: vec![renamed],
                ..Default::default()
            }
        )
        .unwrap_err()
        .status,
        409
    );
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
    // A direct co-author of a personal app can invite either kind of author.
    assert!(!s.app("test-app").unwrap().is_admin(Some(&b)));
    let (organization_invite, _) = change(
        &mut s,
        Some(&b),
        "POST",
        "apps/test-app/invites",
        "invite-studio",
        json!({"to":"org:studio"}),
        Prepared {
            resolved_organization: Some("studio".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(organization_invite["to"], "org:studio");

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
fn invitations_can_be_cancelled_or_declined_and_adminship_transfers_to_an_accepted_author() {
    let mut s = Store::memory().unwrap();
    let owner = who("owner");
    let coauthor = who("coauthor");
    let invitee = who("invitee");
    create(&mut s, &owner, "team-app");
    let invite = |store: &mut Store, recipient: &Identity, key: &str| {
        change(
            store,
            Some(&owner),
            "POST",
            "apps/team-app/invites",
            key,
            json!({"to":recipient.id}),
            Prepared {
                identities: vec![recipient.clone()],
                ..Default::default()
            },
        )
        .unwrap()
        .0["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let joined = invite(&mut s, &coauthor, "invite-coauthor");
    change(
        &mut s,
        Some(&coauthor),
        "POST",
        &format!("invites/{joined}/accept"),
        "accept-coauthor",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    let cancelled = invite(&mut s, &invitee, "invite-cancel");
    change(
        &mut s,
        Some(&coauthor),
        "DELETE",
        &format!("apps/team-app/invites/{cancelled}"),
        "coauthor-cancels",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    assert_eq!(
        change(
            &mut s,
            Some(&invitee),
            "POST",
            &format!("invites/{cancelled}/accept"),
            "cancelled-cannot-accept",
            json!({}),
            Prepared::default()
        )
        .unwrap_err()
        .status,
        409
    );
    let declined = invite(&mut s, &invitee, "invite-decline");
    assert_eq!(
        change(
            &mut s,
            Some(&coauthor),
            "POST",
            &format!("invites/{declined}/decline"),
            "wrong-account-decline",
            json!({}),
            Prepared::default()
        )
        .unwrap_err()
        .status,
        404
    );
    change(
        &mut s,
        Some(&invitee),
        "POST",
        &format!("invites/{declined}/decline"),
        "recipient-declines",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    assert!(
        s.read("invites", &BTreeMap::new(), Some(&invitee)).unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    change(
        &mut s,
        Some(&owner),
        "POST",
        "apps/team-app/admin",
        "transfer-to-coauthor",
        json!({"uuid":"coauthor"}),
        Prepared::default(),
    )
    .unwrap();
    assert_eq!(
        change(
            &mut s,
            Some(&owner),
            "PUT",
            "apps/team-app/access",
            "old-admin-cannot-change",
            json!({"visibility":"private"}),
            Prepared::default()
        )
        .unwrap_err()
        .status,
        403
    );
    change(
        &mut s,
        Some(&coauthor),
        "DELETE",
        "apps/team-app/authors/owner",
        "new-admin-removes",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    let app = s.app("team-app").unwrap();
    assert_eq!(app.authors.len(), 1);
    assert_eq!(app.admin_uuid, "coauthor");
    assert!(
        app.history
            .iter()
            .all(|event| event.idempotency_key.is_some())
    );
    for kind in [
        "author.invite_cancelled",
        "author.invite_declined",
        "author.admin_transferred",
        "author.removed",
    ] {
        assert!(app.history.iter().any(|event| event.kind == kind));
    }
}
#[test]
fn immutable_releases_keep_separate_channel_versions_and_latest_semver() {
    let mut s = Store::memory().unwrap();
    // Releases are signed when they are created, as in the service.
    s.signer = Some(std::sync::Arc::new(
        silicon_apps_server::signing::Keyring::parse(
            &silicon_apps_server::signing::generate("domain-test").0,
            &[],
        )
        .unwrap(),
    ));
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
fn search_handles_typos_in_names_tags_and_description_without_rating_overriding_relevance() {
    let mut s = Store::memory().unwrap();
    let owner = who("owner");
    for id in ["name-app", "tag-app", "description-app", "private-app"] {
        create(&mut s, &owner, id);
        publish(&mut s, &owner, id);
    }
    for (id, body) in [
        ("name-app", json!({"name":"Image Editor"})),
        ("tag-app", json!({"tags":["photography"]})),
        (
            "description-app",
            json!({"description":format!("Synchronizes calendars. {}", "x".repeat(200))}),
        ),
        ("private-app", json!({"tags":["photography"]})),
    ] {
        change(
            &mut s,
            Some(&owner),
            "PATCH",
            &format!("apps/{id}"),
            &format!("edit-{id}"),
            body,
            Prepared::default(),
        )
        .unwrap();
    }
    change(
        &mut s,
        Some(&owner),
        "PUT",
        "apps/private-app/access",
        "make-private",
        json!({"visibility":"private"}),
        Prepared::default(),
    )
    .unwrap();
    for (query, expected) in [
        ("image edtor", "name-app"),
        ("photgraphy", "tag-app"),
        ("calndars", "description-app"),
    ] {
        let results = s
            .read("apps", &BTreeMap::from([("q".into(), query.into())]), None)
            .unwrap();
        assert_eq!(
            results["items"][0]["app_id"], expected,
            "{query}: {results}"
        );
        assert_eq!(results["total"], 1);
    }
}
#[test]
fn reviewer_can_remove_own_review_after_losing_private_access_without_reading_app() {
    let mut s = Store::memory().unwrap();
    let owner = who("owner");
    let mut reviewer = who("reviewer");
    create(&mut s, &owner, "review-app");
    publish(&mut s, &owner, "review-app");
    change(
        &mut s,
        Some(&reviewer),
        "PUT",
        "apps/review-app/review",
        "review-before-private",
        json!({"rating":4,"text":"My review"}),
        Prepared::default(),
    )
    .unwrap();
    change(
        &mut s,
        Some(&owner),
        "PUT",
        "apps/review-app/access",
        "make-app-private",
        json!({"visibility":"private"}),
        Prepared::default(),
    )
    .unwrap();
    assert_eq!(
        s.read("apps/review-app", &BTreeMap::new(), Some(&reviewer))
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(
        change(
            &mut s,
            Some(&who("outsider")),
            "DELETE",
            "apps/review-app/review",
            "outsider-delete",
            json!({}),
            Prepared::default()
        )
        .unwrap_err()
        .status,
        404
    );
    reviewer.id = "c:renamed-reviewer".into();
    let removed = change(
        &mut s,
        Some(&reviewer),
        "DELETE",
        "apps/review-app/review",
        "remove-own-review",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    assert_eq!(removed.0, json!({"status":"removed"}));
    assert!(s.app("review-app").unwrap().reviews.is_empty());
    assert_eq!(
        s.read("apps/review-app/reviews", &BTreeMap::new(), Some(&reviewer))
            .unwrap_err()
            .status,
        404
    );
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
fn only_a_create_the_api_found_historical_may_use_a_short_id() {
    let mut s = Store::memory().unwrap();
    let a = who("alice");
    let body = |id: &str| json!({"app_id":id,"name":"DM"});
    let prepared = |historical_app_id| Prepared {
        secret: Some("sa_app_test_secret".into()),
        historical_app_id,
        ..Default::default()
    };
    // Without the API's finding, a two-character ID is invalid.
    let error = change(
        &mut s,
        Some(&a),
        "POST",
        "apps",
        "create-dm-plain",
        body("dm"),
        prepared(false),
    )
    .unwrap_err();
    assert_eq!(error.status, 400);
    // The finding never makes an invalid ID valid.
    for id in ["", "DM", "d.m", "dm/x"] {
        let error = change(
            &mut s,
            Some(&a),
            "POST",
            "apps",
            &format!("create-invalid-{id}"),
            body(id),
            prepared(true),
        )
        .unwrap_err();
        assert_eq!(error.status, 400, "{id}");
    }
    assert!(s.catalog().unwrap().apps.is_empty());
    // With it, dm is created and its history says why.
    change(
        &mut s,
        Some(&a),
        "POST",
        "apps",
        "create-dm",
        body("dm"),
        prepared(true),
    )
    .unwrap();
    let dm = s.app("dm").unwrap();
    assert_eq!(dm.history[0].kind, "app.created");
    assert_eq!(
        dm.history[0].data,
        json!({"name":"DM","historical_app_id":true})
    );
    // On an ordinary ID the finding changes nothing.
    change(
        &mut s,
        Some(&a),
        "POST",
        "apps",
        "create-ring",
        body("ring"),
        prepared(true),
    )
    .unwrap();
    assert_eq!(s.app("ring").unwrap().history[0].data, json!({"name":"DM"}));
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
    let body = json!({"app_id":"test-app","name":"test-app","description":"A".repeat(200),"tagline":"A test app"});
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

#[test]
fn platform_id_migration_keeps_release_history_and_refuses_collisions() {
    let mut store = Store::memory().unwrap();
    create(&mut store, &who("owner"), "fixture");
    let mut catalog = store.catalog().unwrap();
    let mut app = catalog.apps.remove("fixture").unwrap();
    app.app_id = "apps".into();
    app.name = "Silicon Apps".into();
    app.installs = 42;
    app.releases.push(Release {
        id: "release".into(),
        app_id: "apps".into(),
        channel: "production".into(),
        version: "0.1.8".into(),
        package_ids: vec!["original-package".into()],
        notes: "Original".into(),
        created_at: now(),
        promoted_from: None,
        unsupported_targets: Default::default(),
        signatures: Default::default(),
        signed_by_author: false,
        withdrawn: None,
    });
    let before = app.clone();
    catalog.apps.insert("apps".into(), app);
    store.connection.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,expires_at INTEGER); INSERT INTO sessions VALUES('browser',9999999999)").unwrap();
    store
        .connection
        .execute(
            "UPDATE catalog SET document=?1 WHERE id=1",
            [serde_json::to_string(&catalog).unwrap()],
        )
        .unwrap();
    assert!(store.migrate_silicon_apps_id().unwrap());
    let after = store.app("silicon-apps").unwrap();
    assert_eq!(after.secret_hash, before.secret_hash);
    assert_eq!(after.admin_uuid, before.admin_uuid);
    assert_eq!(after.installs, 42);
    assert_eq!(after.releases[0].app_id, "silicon-apps");
    assert_eq!(
        after.releases[0].package_ids,
        before.releases[0].package_ids
    );
    assert_eq!(after.history.len(), before.history.len() + 1);
    assert!(store.app("apps").is_err());
    assert!(!store.migrate_silicon_apps_id().unwrap());
    let expires: i64 = store
        .connection
        .query_row("SELECT expires_at FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(expires, 0);
    let mut conflict = store.catalog().unwrap();
    conflict.apps.insert("apps".into(), before);
    let raw = serde_json::to_string(&conflict).unwrap();
    store
        .connection
        .execute("UPDATE catalog SET document=?1 WHERE id=1", [&raw])
        .unwrap();
    assert!(store.migrate_silicon_apps_id().is_err());
    let unchanged: String = store
        .connection
        .query_row("SELECT document FROM catalog WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw, unchanged);
}
fn patch(s: &mut Store, w: &Identity, id: &str, key: &str, body: Value) -> Result<Value, String> {
    change(
        s,
        Some(w),
        "PATCH",
        &format!("apps/{id}"),
        key,
        body,
        Prepared::default(),
    )
    .map(|(v, _)| v)
    .map_err(|e| e.message)
}
#[test]
fn tagline_is_one_trimmed_plain_line_of_at_most_sixty_characters() {
    let mut s = Store::memory().unwrap();
    let alice = who("alice");
    create(&mut s, &alice, "card-app");
    let app = patch(
        &mut s,
        &alice,
        "card-app",
        "tagline-trim",
        json!({"tagline":"  Calendars that sync themselves \u{a0}"}),
    )
    .unwrap();
    assert_eq!(app["tagline"], "Calendars that sync themselves");
    let sixty = "é".repeat(TAGLINE_MAX_CHARS);
    let app = patch(
        &mut s,
        &alice,
        "card-app",
        "tagline-60",
        json!({"tagline":sixty}),
    )
    .unwrap();
    assert_eq!(
        app["tagline"], sixty,
        "the limit counts characters, not bytes"
    );
    let too_long = patch(
        &mut s,
        &alice,
        "card-app",
        "tagline-61",
        json!({"tagline":"x".repeat(TAGLINE_MAX_CHARS + 1)}),
    )
    .unwrap_err();
    assert!(too_long.contains("60 characters"), "{too_long}");
    for (key, text) in [
        ("tagline-newline", "Two\nlines"),
        ("tagline-tab", "Tab\there"),
    ] {
        let err = patch(&mut s, &alice, "card-app", key, json!({ "tagline": text })).unwrap_err();
        assert!(err.contains("one line"), "{err}");
    }
    assert!(
        patch(
            &mut s,
            &alice,
            "card-app",
            "tagline-number",
            json!({"tagline":5})
        )
        .is_err()
    );
    assert_eq!(
        s.app("card-app").unwrap().tagline,
        sixty,
        "rejected edits change nothing"
    );
    let created = change(
        &mut s,
        Some(&alice),
        "POST",
        "apps",
        "create-with-tagline",
        json!({"app_id":"tagged","name":"Tagged","tagline":" Set at creation "}),
        Prepared {
            secret: Some("sa_app_test_secret".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .0;
    assert_eq!(created["app"]["tagline"], "Set at creation");
}
#[test]
fn a_new_app_needs_a_tagline_to_publish() {
    let mut s = Store::memory().unwrap();
    let alice = who("alice");
    create(&mut s, &alice, "untagged");
    patch(
        &mut s,
        &alice,
        "untagged",
        "clear-tagline",
        json!({"tagline":"   ", "interface_api_documentation_url":"https://example.com/api"}),
    )
    .unwrap();
    let readiness = s
        .read("apps/untagged/readiness", &BTreeMap::new(), Some(&alice))
        .unwrap();
    assert!(
        readiness["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["field"] == "tagline"),
        "{readiness}"
    );
    let packages: Vec<_> = REQUIRED_RELEASE_TARGETS
        .into_iter()
        .map(|target| package(&mut s, &alice, "untagged", target)["id"].clone())
        .collect();
    change(
        &mut s,
        Some(&alice),
        "POST",
        "apps/untagged/releases",
        "release-untagged",
        json!({"version":"1.0.0","package_ids":packages}),
        Prepared::default(),
    )
    .unwrap();
    let refused = change(
        &mut s,
        Some(&alice),
        "POST",
        "apps/untagged/publish",
        "publish-untagged",
        json!({}),
        Prepared::default(),
    )
    .unwrap_err();
    assert_eq!(
        refused.details["errors"][0]["field"], "tagline",
        "{:?}",
        refused.details
    );
    assert!(!s.app("untagged").unwrap().published);
    patch(
        &mut s,
        &alice,
        "untagged",
        "add-tagline",
        json!({"tagline":"Now with a tagline"}),
    )
    .unwrap();
    change(
        &mut s,
        Some(&alice),
        "POST",
        "apps/untagged/publish",
        "publish-tagged",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    assert!(s.app("untagged").unwrap().published);
}
#[test]
fn apps_published_before_taglines_stay_published_and_editable_without_one() {
    let mut s = Store::memory().unwrap();
    let alice = who("alice");
    create(&mut s, &alice, "legacy");
    publish(&mut s, &alice, "legacy");
    // Catalog documents written before taglines existed have no `tagline` key at all.
    s.connection
        .execute(
            "UPDATE catalog SET document=json_remove(document,'$.apps.legacy.tagline') WHERE id=1",
            [],
        )
        .unwrap();
    let raw: String = s
        .connection
        .query_row("SELECT document FROM catalog", [], |r| r.get(0))
        .unwrap();
    assert!(!raw.contains("tagline"));
    let legacy = s.app("legacy").unwrap();
    assert!(legacy.published);
    assert_eq!(legacy.tagline, "");
    let page = s.read("apps/legacy", &BTreeMap::new(), None).unwrap();
    assert_eq!(page["tagline"], "", "readers always get the field");
    let readiness = s
        .read("apps/legacy/readiness", &BTreeMap::new(), Some(&alice))
        .unwrap();
    assert_eq!(readiness["ready"], true, "{readiness}");
    let edited = patch(
        &mut s,
        &alice,
        "legacy",
        "legacy-tags",
        json!({"tags":["calendar"]}),
    )
    .unwrap();
    assert_eq!(edited["published"], true);
    change(
        &mut s,
        Some(&alice),
        "POST",
        "apps/legacy/publish",
        "republish-legacy",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    let tagged = patch(
        &mut s,
        &alice,
        "legacy",
        "legacy-tagline",
        json!({"tagline":"Finally a card line"}),
    )
    .unwrap();
    assert_eq!(tagged["tagline"], "Finally a card line");
    let cleared = patch(
        &mut s,
        &alice,
        "legacy",
        "legacy-clear",
        json!({"tagline":""}),
    )
    .unwrap();
    assert_eq!(cleared["tagline"], "");
    assert_eq!(cleared["published"], true);
}
#[test]
fn search_matches_tagline_words() {
    let mut s = Store::memory().unwrap();
    let owner = who("owner");
    for id in ["tagline-app", "other-app"] {
        create(&mut s, &owner, id);
        publish(&mut s, &owner, id);
    }
    patch(
        &mut s,
        &owner,
        "tagline-app",
        "tagline-search",
        json!({"tagline":"Spreadsheets everyone can edit"}),
    )
    .unwrap();
    let results = s
        .read(
            "apps",
            &BTreeMap::from([("q".into(), "spreadsheets".into())]),
            None,
        )
        .unwrap();
    assert_eq!(results["total"], 1, "{results}");
    assert_eq!(results["items"][0]["app_id"], "tagline-app");
}

#[test]
fn deletion_hides_distribution_reserves_id_and_rechecks_receipts() {
    let mut store = Store::memory().unwrap();
    let owner = who("owner");
    let stranger = who("other");
    create(&mut store, &owner, "delete-me");
    publish(&mut store, &owner, "delete-me");
    let path = "apps/delete-me";
    assert!(
        change(
            &mut store,
            Some(&stranger),
            "DELETE",
            path,
            "bad-delete",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    let deleted = change(
        &mut store,
        Some(&owner),
        "DELETE",
        path,
        "delete",
        json!({}),
        Prepared::default(),
    )
    .unwrap()
    .0;
    assert!(deleted["deleted_at"].is_string());
    assert!(deleted["restore_until"].is_string());
    assert!(!store.app("delete-me").unwrap().visible(Some(&owner)));
    assert!(store.read(path, &BTreeMap::new(), None).is_err());
    assert!(
        store
            .read("apps/delete-me/packages", &BTreeMap::new(), Some(&owner))
            .is_err()
    );
    assert!(
        change(
            &mut store,
            Some(&owner),
            "PATCH",
            path,
            "edit-deleted",
            json!({"name":"Gone"}),
            Prepared::default()
        )
        .is_err()
    );
    assert!(change(&mut store,Some(&owner),"POST","apps","create-delete-me",json!({"app_id":"delete-me","name":"delete-me","description":"A".repeat(200),"tagline":"A test app"}),Prepared::default()).is_err());
    let q = BTreeMap::from([
        ("mine".into(), "true".into()),
        ("deleted".into(), "true".into()),
    ]);
    assert_eq!(store.read("apps", &q, Some(&owner)).unwrap()["total"], 1);
    assert!(
        change(
            &mut store,
            Some(&stranger),
            "POST",
            "apps/delete-me/restore",
            "bad-restore",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    change(
        &mut store,
        Some(&owner),
        "POST",
        "apps/delete-me/restore",
        "restore",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    assert!(store.app("delete-me").unwrap().visible(None));
}

#[test]
fn organization_drafts_live_permissions_audience_and_departure() {
    use silicon_publishing_policy::{Context, OrganizationContext, Permission, ResourceKind};
    use std::collections::BTreeSet;
    let org = |id: &str, admin: bool, permissions: Vec<Permission>| {
        let mut w = who(id);
        w.publishing = Context {
            account_uuid: id.into(),
            organizations: vec![OrganizationContext {
                org_id: "studio".into(),
                name: "Studio".into(),
                admin,
                permissions: BTreeMap::from([(
                    ResourceKind::Apps,
                    BTreeSet::from_iter(permissions),
                )]),
                ..Default::default()
            }],
        };
        w
    };
    let mut creator = org("maker", false, vec![Permission::Create]);
    let manager = org("manager", true, vec![]);
    let member = org("member", false, vec![]);
    let outsider = who("outsider");
    let mut store = Store::memory().unwrap();
    let created=change(&mut store,Some(&creator),"POST","apps","org-create",json!({"app_id":"org-app","name":"Org app","description":"A".repeat(200),"tagline":"For the studio","organization_id":"studio"}),Prepared{secret:Some("secret".into()),..Default::default()}).unwrap();
    assert!(created.0.get("app_secret").is_none());
    let draft = store.app("org-app").unwrap();
    assert_eq!(draft.visibility, "private");
    assert!(draft.is_author(Some(&creator)));
    assert!(!draft.is_admin(Some(&creator)));
    assert!(!draft.visible(Some(&member)));
    assert!(
        change(
            &mut store,
            Some(&creator),
            "POST",
            "apps/org-app/publish",
            "no-publish",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    assert!(
        change(
            &mut store,
            Some(&creator),
            "DELETE",
            "apps/org-app",
            "no-delete",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    publish(&mut store, &manager, "org-app");
    assert!(store.app("org-app").unwrap().visible(Some(&member)));
    assert!(!store.app("org-app").unwrap().visible(Some(&outsider)));
    change(&mut store,Some(&manager),"PUT","apps/org-app/organization-access","restrict",json!({"grants":[],"audience":{"mode":"organization","include":[],"exclude":[{"type":"account","id":"member"}]}}),Prepared::default()).unwrap();
    assert!(!store.app("org-app").unwrap().visible(Some(&member)));
    creator.publishing.organizations.clear();
    assert!(!store.app("org-app").unwrap().is_author(Some(&creator)));
    assert!(change(&mut store,Some(&creator),"POST","apps","org-create",json!({"app_id":"org-app","name":"Org app","description":"A".repeat(200),"tagline":"For the studio","organization_id":"studio"}),Prepared::default()).is_err());
    change(
        &mut store,
        Some(&manager),
        "DELETE",
        "apps/org-app",
        "org-delete",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    assert_eq!(
        store.app("org-app").unwrap().organization_id.as_deref(),
        Some("studio")
    );
}

#[test]
fn organization_invitation_grants_live_author_access_without_transferring_ownership() {
    use silicon_publishing_policy::{Permission, ResourceKind};
    let host = who("host");
    let mut manager = who("org-manager");
    manager.publishing =
        serde_json::from_value(json!({"account_uuid":manager.uuid,"organizations":[{
            "org_id":"guest","name":"Guest","admin":false,"permissions":{"apps":["manage"]}
        }]}))
        .unwrap();
    let mut contributor = who("editor");
    contributor.publishing =
        serde_json::from_value(json!({"account_uuid":contributor.uuid,"organizations":[{
            "org_id":"guest","name":"Guest","admin":false,"permissions":{"apps":["edit"]}
        }]}))
        .unwrap();
    let mut store = Store::memory().unwrap();
    create(&mut store, &host, "invite-project");
    let path = "apps/invite-project/invites";
    let target = json!({"to":"org:guest"});
    let destination = || Prepared {
        resolved_organization: Some("guest".into()),
        ..Default::default()
    };
    assert!(
        change(
            &mut store,
            Some(&host),
            "POST",
            path,
            "unknown",
            target.clone(),
            Prepared::default()
        )
        .is_err()
    );
    let invitation = change(
        &mut store,
        Some(&host),
        "POST",
        path,
        "invite",
        target.clone(),
        destination(),
    )
    .unwrap()
    .0;
    assert!(
        change(
            &mut store,
            Some(&host),
            "POST",
            path,
            "duplicate",
            target,
            destination()
        )
        .is_err()
    );
    let id = invitation["id"].as_str().unwrap();
    let accept = format!("invites/{id}/accept");
    let inbox = |db: &Store, user: &Identity| {
        db.read("invites", &BTreeMap::new(), Some(user)).unwrap()["items"]
            .as_array()
            .unwrap()
            .len()
    };
    assert_eq!(inbox(&store, &manager), 1);
    assert_eq!(inbox(&store, &contributor), 0);
    assert!(
        change(
            &mut store,
            Some(&contributor),
            "POST",
            &accept,
            "wrong-role",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    change(
        &mut store,
        Some(&manager),
        "POST",
        &accept,
        "accept",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    let app = store.app("invite-project").unwrap();
    assert_eq!(app.author_organizations, ["guest"]);
    assert_eq!(app.authors.len(), 1);
    assert_eq!(app.admin_uuid, host.uuid);
    assert!(app.permits(Some(&contributor), Permission::Edit));
    assert!(!app.permits(Some(&contributor), Permission::Publish));
    assert!(app.permits(Some(&manager), Permission::Publish));
    assert!(!app.is_admin(Some(&manager)));
    assert_eq!(
        change(
            &mut store,
            Some(&manager),
            "POST",
            path,
            "guest-cannot-invite",
            json!({"to":"org:third-studio"}),
            Prepared {
                resolved_organization: Some("third-studio".into()),
                ..Default::default()
            },
        )
        .unwrap_err()
        .status,
        403
    );
    assert!(
        change(
            &mut store,
            Some(&manager),
            "DELETE",
            "apps/invite-project",
            "cannot-delete",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    manager.publishing.organizations[0]
        .permissions
        .remove(&ResourceKind::Apps);
    assert!(
        change(
            &mut store,
            Some(&manager),
            "POST",
            &accept,
            "accept",
            json!({}),
            Prepared::default()
        )
        .is_err()
    );
    assert!(!app.visible(Some(&manager)));
    change(
        &mut store,
        Some(&host),
        "DELETE",
        "apps/invite-project/organization-authors/guest",
        "remove",
        json!({}),
        Prepared::default(),
    )
    .unwrap();
    assert!(
        !store
            .app("invite-project")
            .unwrap()
            .is_author(Some(&contributor))
    );
}
