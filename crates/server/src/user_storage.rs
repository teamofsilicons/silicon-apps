//! Guarded app settings. Never pass proofs through the author/session middleware,
//! or put user values in catalog views, events, or the authoring replay namespace.
use crate::{
    Shared,
    error::{ApiError, Result},
    model::{APP_ID, now},
    store::hash,
};
use axum::{
    Json,
    body::Bytes,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use silicon_accounts_client::{ProofKind, ValidProof};
use std::collections::BTreeMap;

const MAX_BYTES: usize = 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    #[serde(rename = "type")]
    pub kind: String,
    pub default: Value,
}
pub type Schema = BTreeMap<String, Field>;

pub fn schema(value: &Value) -> Result<Schema> {
    let fields: Schema = serde_json::from_value(value.clone())
        .map_err(|_| ApiError::bad("user_storage must map keys to {type, default}."))?;
    if fields.len() > 128 || value.to_string().len() > MAX_BYTES {
        return Err(ApiError::bad(
            "Storage allows at most 128 keys and 1 MiB of defaults.",
        ));
    }
    for (key, field) in &fields {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            || ["__proto__", "prototype", "constructor"].contains(&key.as_str())
        {
            return Err(ApiError::bad(
                "Storage keys must be 1–128 letters, digits, underscores, dots or hyphens; prototype names are reserved.",
            ));
        }
        validate_value(key, field, &field.default)?;
    }
    Ok(fields)
}

fn validate_value(key: &str, field: &Field, value: &Value) -> Result<()> {
    let valid = match field.kind.as_str() {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        _ => {
            return Err(ApiError::bad(
                "Storage types are string, number, boolean, object and array.",
            ));
        }
    };
    if !valid || value.to_string().len() > 64 * 1024 {
        return Err(ApiError::bad(format!(
            "Storage key `{key}` must match its declared type and fit within 64 KiB."
        )));
    }
    Ok(())
}

/// Add keys without changing existing users' starting values or discarding data.
pub fn extend_schema(old: Option<&Schema>, next: &Schema) -> Result<()> {
    if old.is_some_and(|old| old.iter().any(|(key, field)| next.get(key) != Some(field))) {
        return Err(ApiError::conflict(
            "Existing storage keys, types and defaults cannot be removed or changed. Add a new key instead.",
        ));
    }
    Ok(())
}

pub fn check_existing_records(
    connection: &rusqlite::Connection,
    app: &str,
    schema: &Schema,
) -> Result<()> {
    let mut statement =
        connection.prepare("SELECT values_json FROM app_user_storage WHERE app_id=?1")?;
    let rows = statement.query_map([app], |r| r.get::<_, String>(0))?;
    for row in rows {
        let mut values: Map<String, Value> = schema
            .iter()
            .map(|(k, f)| (k.clone(), f.default.clone()))
            .collect();
        let saved: Map<String, Value> = serde_json::from_str(&row?)
            .map_err(|_| ApiError::unavailable("Stored user settings could not be read."))?;
        values.extend(saved);
        if serde_json::to_vec(&values).unwrap().len() > MAX_BYTES {
            return Err(ApiError::conflict(
                "Adding these defaults would exceed an existing user's 1 MiB storage limit. Use smaller defaults.",
            ));
        }
    }
    Ok(())
}

fn denied() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "storage_access_denied",
        "The proof does not authorize this storage operation.",
        "Use a current proof addressed to silicon-apps with the storage scope for this app and user. Writes also require the owning app's App verification.",
    )
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str> {
    let value = headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(denied)?;
    if name == "authorization" {
        value.strip_prefix("Proof ").ok_or_else(denied)
    } else {
        Ok(value)
    }
}

async fn verify(s: &Shared, token: &str) -> Result<ValidProof> {
    if !token.starts_with("sap_") || token.len() > 256 || token.trim() != token {
        return Err(denied());
    }
    let secret = s.config.accounts_app_secret.as_deref().ok_or_else(|| {
        ApiError::unavailable(
            "Accounts app credentials are required for storage proof verification.",
        )
    })?;
    // Deliberately uncached: revocation must affect reads, writes and replays.
    let result = s
        .accounts
        .as_app(APP_ID, secret)
        .verify_proof(token)
        .await
        .map_err(crate::integrations::accounts_error)?;
    let proof = result.valid().ok_or_else(denied)?;
    if proof.receiving_app.app_id != APP_ID
        || proof.expires_at.unix_timestamp() <= chrono::Utc::now().timestamp()
    {
        return Err(denied());
    }
    Ok(proof.clone())
}

fn has(proof: &ValidProof, scope: &str) -> bool {
    proof.scopes.iter().any(|s| s == scope)
}

async fn authorize(
    s: &Shared,
    headers: &HeaderMap,
    app: &str,
    user: &str,
    write: bool,
) -> Result<String> {
    let proof = verify(s, header(headers, "authorization")?).await?;
    if user == "anon" {
        if proof.kind != ProofKind::AppVerification
            || proof.issuing_app.app_id != app
            || !(has(&proof, "storage:write") || !write && has(&proof, "storage:read"))
        {
            return Err(denied());
        }
    } else {
        if proof.kind != ProofKind::UserVerification
            || proof.user.as_ref().is_none_or(|u| u.uuid != user)
            || !(has(&proof, &format!("storage:{app}:write"))
                || !write && has(&proof, &format!("storage:{app}:read")))
        {
            return Err(denied());
        }
        if s.store.lock().unwrap().account_uuid_retired(user)? {
            return Err(denied());
        }
        if write {
            let app_proof = verify(s, header(headers, "x-app-verification")?).await?;
            if app_proof.kind != ProofKind::AppVerification
                || app_proof.issuing_app.app_id != app
                || proof.issuing_app.app_id != app
                || !has(&app_proof, "storage:write")
            {
                return Err(denied());
            }
        }
    }
    Ok(format!("storage:{app}:{user}:{}", proof.issuing_app.app_id))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    values: Map<String, Value>,
    if_revision: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reset {
    keys: Option<Vec<String>>,
    if_revision: Option<u64>,
}

pub async fn handle(
    s: &Shared,
    method: &Method,
    app: &str,
    user: &str,
    reset: bool,
    headers: &HeaderMap,
    raw: &Bytes,
) -> Result<Response> {
    let write = *method != Method::GET;
    // Authenticate before testing existence or returning an idempotent response.
    let actor = authorize(s, headers, app, user, write).await?;
    let _gate = s.mutation_gate.lock().await;
    let mut store = s.store.lock().unwrap();
    let app_record = store.app(app)?;
    let schema = app_record.user_storage.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "storage_not_defined",
            "This app has not defined user storage.",
            "An app author can define an optional storage schema.",
        )
    })?;
    let tx = store
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let row: Option<(String, i64, String)> = tx.query_row(
        "SELECT values_json,revision,updated_at FROM app_user_storage WHERE app_id=?1 AND user_uuid=?2",
        params![app, user], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    let mut values: Map<String, Value> = schema
        .iter()
        .map(|(k, f)| (k.clone(), f.default.clone()))
        .collect();
    let (mut revision, mut updated_at) = (0i64, None);
    if let Some((raw, rev, at)) = row {
        let saved: Map<String, Value> = serde_json::from_str(&raw)
            .map_err(|_| ApiError::unavailable("Stored user settings could not be read."))?;
        values.extend(saved);
        revision = rev;
        updated_at = Some(at);
    }
    let mut replayed = false;
    let response = if write {
        let key = crate::idempotency_key(headers)?;
        let fingerprint = hash(format!("{method}:{reset}:{}", hash(raw)).as_bytes());
        let replay: Option<(String, String)> = tx
            .query_row(
                "SELECT fingerprint,response FROM app_storage_replays WHERE actor=?1 AND key=?2",
                params![actor, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((previous, response)) = replay {
            if previous != fingerprint {
                return Err(ApiError::conflict(
                    "This idempotency key was already used for a different storage request.",
                ));
            }
            replayed = true;
            serde_json::from_str(&response)
                .map_err(|_| ApiError::unavailable("Stored response could not be read."))?
        } else {
            let expected = if reset {
                let body: Reset = serde_json::from_slice(raw).map_err(|_| {
                    ApiError::bad("Reset expects {keys?: [key], if_revision?: number}.")
                })?;
                for key in body
                    .keys
                    .unwrap_or_else(|| schema.keys().cloned().collect())
                {
                    let field = schema.get(&key).ok_or_else(|| {
                        ApiError::bad(format!("Storage key `{key}` is not defined."))
                    })?;
                    values.insert(key, field.default.clone());
                }
                body.if_revision
            } else {
                let body: Patch = serde_json::from_slice(raw).map_err(|_| {
                    ApiError::bad(
                        "Storage writes expect {values: {key: value}, if_revision?: number}.",
                    )
                })?;
                for (key, value) in body.values {
                    let field = schema.get(&key).ok_or_else(|| {
                        ApiError::bad(format!("Storage key `{key}` is not defined."))
                    })?;
                    validate_value(&key, field, &value)?;
                    values.insert(key, value);
                }
                body.if_revision
            };
            if expected.is_some_and(|expected| expected != revision as u64) {
                return Err(ApiError::conflict(
                    "These settings changed since they were read. Read the current revision and retry.",
                ));
            }
            let serialized = serde_json::to_string(&values).unwrap();
            if serialized.len() > MAX_BYTES {
                return Err(ApiError::bad("A user's settings must fit within 1 MiB."));
            }
            revision += 1;
            updated_at = Some(now());
            tx.execute("INSERT INTO app_user_storage(app_id,user_uuid,values_json,revision,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(app_id,user_uuid) DO UPDATE SET values_json=excluded.values_json,revision=excluded.revision,updated_at=excluded.updated_at",
                params![app,user,serialized,revision,updated_at])?;
            let result = json!({"app_id":app,"user_uuid":user,"values":values,"revision":revision,"updated_at":updated_at});
            tx.execute("INSERT INTO app_storage_replays(actor,key,fingerprint,response,created_at) VALUES(?1,?2,?3,?4,?5)", params![actor,key,fingerprint,result.to_string(),now()])?;
            result
        }
    } else {
        json!({"app_id":app,"user_uuid":user,"values":values,"revision":revision,"updated_at":updated_at})
    };
    tx.commit()?;
    let mut response = Json(response).into_response();
    if replayed {
        response
            .headers_mut()
            .insert("Idempotent-Replayed", "true".parse().unwrap());
    }
    Ok(response)
}
