//! Optional per-app settings. Proofs are explicit inputs and never saved by this client.
use crate::{Client, check_response, network};
use anyhow::{Context, Result};
use reqwest::{Method, header::HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageField {
    #[serde(rename = "type")]
    pub kind: String,
    pub default: Value,
}
pub type StorageSchema = BTreeMap<String, StorageField>;

/// `proof` is a User verification for a UUID, or the owner's App verification for `anon`.
/// Signed-in writes also need `app_verification`. Values are intentionally not Debug.
pub struct StorageAuthorization<'a> {
    pub proof: &'a str,
    pub app_verification: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageRecord {
    pub app_id: String,
    pub user_uuid: String,
    pub values: Map<String, Value>,
    pub revision: u64,
    pub updated_at: Option<String>,
}

impl Client {
    pub async fn create_with_storage(
        &self,
        id: &str,
        name: &str,
        description: &str,
        logo: Option<&str>,
        schema: Option<&StorageSchema>,
        key: Option<&str>,
    ) -> Result<Value> {
        self.request("POST", &["v1", "apps"], &[], Some(json!({"app_id":id,"name":name,"description":description,"logo":logo.unwrap_or(""),"user_storage":schema})), key).await
    }

    pub async fn storage_schema(&self, app: &str) -> Result<Value> {
        self.action("GET", app, &["storage-schema"], None, None)
            .await
    }

    /// Define storage, or add new keys. Existing keys, types and defaults are immutable.
    pub async fn define_storage(
        &self,
        app: &str,
        schema: &StorageSchema,
        key: Option<&str>,
    ) -> Result<Value> {
        self.action(
            "PUT",
            app,
            &["storage-schema"],
            Some(serde_json::to_value(schema)?),
            key,
        )
        .await
    }

    pub async fn read_storage(
        &self,
        app: &str,
        user: &str,
        auth: &StorageAuthorization<'_>,
    ) -> Result<StorageRecord> {
        self.storage_request(Method::GET, app, user, false, auth, None, None)
            .await
    }

    pub async fn write_storage(
        &self,
        app: &str,
        user: &str,
        auth: &StorageAuthorization<'_>,
        values: &Map<String, Value>,
        if_revision: Option<u64>,
        key: Option<&str>,
    ) -> Result<StorageRecord> {
        self.storage_request(
            Method::PATCH,
            app,
            user,
            false,
            auth,
            Some(json!({"values":values,"if_revision":if_revision})),
            key,
        )
        .await
    }

    pub async fn reset_storage(
        &self,
        app: &str,
        user: &str,
        auth: &StorageAuthorization<'_>,
        keys: Option<&[String]>,
        if_revision: Option<u64>,
        key: Option<&str>,
    ) -> Result<StorageRecord> {
        self.storage_request(
            Method::POST,
            app,
            user,
            true,
            auth,
            Some(json!({"keys":keys,"if_revision":if_revision})),
            key,
        )
        .await
    }

    async fn storage_request(
        &self,
        method: Method,
        app: &str,
        user: &str,
        reset: bool,
        auth: &StorageAuthorization<'_>,
        body: Option<Value>,
        key: Option<&str>,
    ) -> Result<StorageRecord> {
        let mut path = vec!["v1", "apps", app, "storage", "users", user];
        if reset {
            path.push("reset");
        }
        let mut proof = HeaderValue::from_str(&format!("Proof {}", auth.proof))
            .context("Invalid proof header")?;
        proof.set_sensitive(true);
        let mut request = self
            .http
            .request(method.clone(), self.url(&path)?)
            .header("Authorization", proof);
        if let Some(token) = auth.app_verification {
            let mut value =
                HeaderValue::from_str(token).context("Invalid App verification header")?;
            value.set_sensitive(true);
            request = request.header("X-App-Verification", value);
        }
        let operation_key = (method != Method::GET).then(|| {
            key.map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
        });
        if let Some(key) = &operation_key {
            request = request.header("Idempotency-Key", key);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let result = async {
            check_response(
                network::send(request)
                    .await
                    .context("Could not reach Silicon Apps storage")?,
            )
            .await?
            .json()
            .await
            .context("Silicon Apps returned invalid storage JSON")
        }
        .await;
        match operation_key {
            Some(key) => result.with_context(|| format!("Operation Idempotency-Key: {key}. Retry the same request with --idempotency-key {key}.")),
            None => result,
        }
    }
}
