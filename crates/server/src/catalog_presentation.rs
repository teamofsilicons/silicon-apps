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
}

pub fn apply(store: &mut Store) -> Result<Value> {
    let entries: BTreeMap<String, Listing> =
        serde_json::from_str(include_str!("../data/catalog-presentation-2026-11.json"))
            .map_err(|_| ApiError::bad("The embedded catalog update is invalid."))?;
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS catalog_presentation_updates (revision TEXT PRIMARY KEY, applied_at TEXT NOT NULL)")?;
    let revision = "2026-11-taglines-and-logos";
    if tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalog_presentation_updates WHERE revision=?1)",
        [revision],
        |r| r.get::<_, bool>(0),
    )? {
        return Ok(json!({"already_applied":true}));
    }
    let raw: String = tx.query_row("SELECT document FROM catalog WHERE id=1", [], |r| r.get(0))?;
    let mut catalog: Catalog =
        serde_json::from_str(&raw).map_err(|_| ApiError::bad("Unable to read the catalog."))?;
    let marks = events::marks(&catalog);
    let mut taglines = 0;
    let mut logos = 0;
    for (id, entry) in entries {
        let app = catalog
            .apps
            .get_mut(&id)
            .filter(|a| a.published && a.deleted_at.is_none())
            .ok_or_else(|| ApiError::conflict(format!("Expected published listing {id}.")))?;
        if !app.tagline.is_empty() && app.tagline != entry.tagline {
            return Err(ApiError::conflict(format!(
                "The tagline for {id} changed since preflight."
            )));
        }
        let mut changed = Vec::new();
        if app.tagline != entry.tagline {
            app.tagline = entry.tagline;
            changed.push("tagline");
            taglines += 1;
        }
        if let Some(logo) = entry.logo
            && app.logo != logo.url
        {
            if hash(app.logo.as_bytes()) != logo.previous_sha256 {
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
        "INSERT INTO catalog_presentation_updates(revision,applied_at) VALUES(?1,?2)",
        params![revision, now()],
    )?;
    tx.commit()?;
    Ok(json!({"taglines":taglines,"logos":logos,"revision":revision}))
}
