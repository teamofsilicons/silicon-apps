mod common;

use serde_json::{Value, json};
use silicon_apps_server::catalog_presentation;

#[test]
fn icon_correction_preserves_user_edits_and_retries_atomically() {
    let (_dir, state, _router) = common::fresh();
    common::seed_app(&state, "alice", "template", &[], "linux-x86_64", true);
    let mut store = state.store.lock().unwrap();
    let mut catalog = store.catalog().unwrap();
    let template = catalog.apps["template"].clone();
    let manifest: Value =
        serde_json::from_str(include_str!("../data/catalog-presentation-2026-11.json")).unwrap();
    let ids = [
        "briefcase",
        "dm",
        "ring",
        "silicon-writings",
        "ting",
        "write",
    ];
    for id in ids {
        let mut app = template.clone();
        app.app_id = id.into();
        app.tagline = "An author edited this after the first rollout".into();
        let logo = &manifest[id]["logo"];
        // Ring already has its corrected artwork in this partially updated fixture.
        app.logo = logo["previous_rollout_url"]
            .as_str()
            .unwrap_or_else(|| logo["url"].as_str().unwrap())
            .into();
        catalog.apps.insert(id.into(), app);
    }
    let old_write = std::mem::replace(
        &mut catalog.apps.get_mut("write").unwrap().logo,
        "an-author-changed-this".into(),
    );
    let saved = serde_json::to_string(&catalog).unwrap();
    store
        .connection
        .execute("UPDATE catalog SET document=?1 WHERE id=1", [&saved])
        .unwrap();
    store.connection.execute_batch("CREATE TABLE catalog_presentation_updates(revision TEXT PRIMARY KEY, applied_at TEXT NOT NULL);
        INSERT INTO catalog_presentation_updates VALUES ('2026-11-taglines-and-logos','original');").unwrap();
    let pending = |store: &silicon_apps_server::store::Store| {
        store
            .connection
            .query_row(
                "SELECT count(*) FROM outbox WHERE kind='accounts.sync'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    let queued = pending(&store);

    assert!(catalog_presentation::apply(&mut store).is_err());
    assert_eq!(
        serde_json::to_string(&store.catalog().unwrap()).unwrap(),
        saved
    );
    assert_eq!(pending(&store), queued);

    catalog.apps.get_mut("write").unwrap().logo = old_write;
    store
        .connection
        .execute(
            "UPDATE catalog SET document=?1 WHERE id=1",
            [serde_json::to_string(&catalog).unwrap()],
        )
        .unwrap();
    let result = catalog_presentation::apply(&mut store).unwrap();
    assert_eq!(result["taglines"], 0);
    assert_eq!(result["logos"], 5);
    let updated = store.catalog().unwrap();
    assert_eq!(updated.apps["ting"].logo, "");
    assert!(updated.apps["ring"].logo.contains("/app-logos/ring-"));
    for id in ids {
        assert_eq!(
            updated.apps[id].logo,
            manifest[id]["logo"]["url"].as_str().unwrap()
        );
        assert_eq!(updated.apps[id].tagline, catalog.apps[id].tagline);
    }
    assert_eq!(
        catalog_presentation::apply(&mut store).unwrap(),
        json!({"already_applied":true})
    );
    assert_eq!(pending(&store), queued + 5);
}
