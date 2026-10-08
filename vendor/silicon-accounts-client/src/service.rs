//! Privileged Silicon Apps integration. Never distribute the internal credential to end users.
use crate::Result;
use crate::client::{AccountsClient, Auth, Request};
use reqwest::Method;
use serde_json::Value;

impl AccountsClient {
    /// Queues a Silicon Apps invitation or bug report through Accounts' private delivery bridge.
    /// Carbon email addresses are resolved inside Accounts and are never returned to Apps.
    pub async fn service_send_apps_mail(
        &self,
        token: &str,
        kind: &str,
        body: &Value,
        key: &str,
    ) -> Result<Value> {
        let request = Request::new(
            Method::POST,
            self.endpoint(&["v1", "internal", "apps", "mail"]),
            Auth::Bearer(token),
        )
        .json(&serde_json::json!({"kind":kind,"body":body}))?
        .idempotency_key(Some(key))?;
        self.execute(request).await?.json()
    }
    /// Upserts the Silicon Apps registry through Accounts' internal service API.
    /// Only server operators may call this; `token` is ACCOUNTS_INTERNAL_TOKEN.
    /// Repeating the same document preserves app users and sign-in configuration.
    pub async fn service_sync_apps(&self, token: &str, document: &Value) -> Result<Value> {
        let request = Request::new(
            Method::POST,
            self.endpoint(&["v1", "internal", "apps", "sync"]),
            Auth::Bearer(token),
        )
        .json(document)?;
        self.execute(request).await?.json()
    }

    /// Reads registry identities for migration into Silicon Apps, without secrets or user data.
    pub async fn service_list_apps(&self, token: &str) -> Result<Value> {
        self.get(
            self.endpoint(&["v1", "internal", "apps"]),
            Auth::Bearer(token),
        )
        .await
    }
}
