//! Operator-invoked, one-time update of the November 2026 public listings.
use crate::{
    error::{ApiError, Result},
    events,
    model::{Catalog, new_id, now},
    store::{Store, hash},
};
use rusqlite::{TransactionBehavior, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Listing {
    tagline: String,
    logo: Option<Logo>,
}
#[derive(Deserialize)]
struct Logo {
    url: String,
    previous_sha256: String,
    previous_rollout_url: Option<String>,
}

pub fn apply(store: &mut Store) -> Result<Value> {
    let entries: BTreeMap<String, Listing> =
        serde_json::from_str(include_str!("../data/catalog-presentation-2026-11.json"))
            .map_err(|_| ApiError::bad("The embedded catalog update is invalid."))?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS catalog_presentation_updates (revision TEXT PRIMARY KEY, applied_at TEXT NOT NULL)")?;
    let original = "2026-11-taglines-and-logos";
    let revision = "2026-11-18-maskable-icons";
    let (original_applied, corrected) = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalog_presentation_updates WHERE revision=?1),
                EXISTS(SELECT 1 FROM catalog_presentation_updates WHERE revision=?2)",
        [original, revision],
        |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
    )?;
    if corrected {
        return Ok(json!({"already_applied":true}));
    }
    let raw: String = tx.query_row("SELECT document FROM catalog WHERE id=1", [], |r| r.get(0))?;
    let mut catalog: Catalog =
        serde_json::from_str(&raw).map_err(|_| ApiError::bad("Unable to read the catalog."))?;
    let marks = events::marks(&catalog);
    let mut taglines = 0;
    let mut logos = 0;
    for (id, entry) in entries {
        // The correction only touches the six affected logos on an upgraded catalog.
        if original_applied && entry.logo.is_none() {
            continue;
        }
        let app = catalog
            .apps
            .get_mut(&id)
            .filter(|a| a.published && a.deleted_at.is_none())
            .ok_or_else(|| ApiError::conflict(format!("Expected published listing {id}.")))?;
        if !original_applied && !app.tagline.is_empty() && app.tagline != entry.tagline {
            return Err(ApiError::conflict(format!(
                "The tagline for {id} changed since preflight."
            )));
        }
        let mut changed = Vec::new();
        if !original_applied && app.tagline != entry.tagline {
            app.tagline = entry.tagline;
            changed.push("tagline");
            taglines += 1;
        }
        if let Some(logo) = entry.logo
            && app.logo != logo.url
        {
            let expected = if original_applied {
                logo.previous_rollout_url.as_deref()
            } else {
                None
            };
            let matches_previous = match expected {
                Some(url) => app.logo == url,
                None => hash(app.logo.as_bytes()) == logo.previous_sha256,
            };
            if !matches_previous {
                return Err(ApiError::conflict(format!(
                    "The logo for {id} changed since preflight."
                )));
            }
            app.logo = logo.url;
            changed.push("logo");
            logos += 1;
            tx.execute(
                "INSERT INTO outbox(id,kind,body,created_at) VALUES(?1,'accounts.sync',?2,?3)",
                params![new_id(), json!({"app_id":id}).to_string(), now()],
            )?;
        }
        if !changed.is_empty() {
            app.event(
                "system",
                "app.listing_updated",
                json!({"release":revision,"fields":changed}),
            );
        }
    }
    Store::save_catalog(&tx, &catalog, &marks)?;
    tx.execute(
        "INSERT OR IGNORE INTO catalog_presentation_updates(revision,applied_at) VALUES(?1,?3),(?2,?3)",
        params![original, revision, now()],
    )?;
    tx.commit()?;
    Ok(json!({"taglines":taglines,"logos":logos,"revision":revision}))
}
