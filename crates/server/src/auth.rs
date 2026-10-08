use crate::{
    Shared,
    error::{ApiError, Result},
    integrations::accounts_error,
    model::Identity,
};
use axum::{
    Json,
    body::Bytes,
    http::{HeaderMap, Method},
    response::{IntoResponse, Redirect, Response},
};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use silicon_accounts_client::{AuthorizeParams, VerifyOptions, verify_access_token};
use std::collections::BTreeMap;

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get("Cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (k, v) = part.trim().split_once('=')?;
            (k == name).then(|| v.to_owned())
        })
}
fn session_cookie(s: &Shared, id: &str, max_age: i64) -> String {
    format!(
        "apps_session={id}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{}",
        if s.config.public_url.starts_with("https:") {
            "; Secure"
        } else {
            ""
        }
    )
}
fn state_cookie(s: &Shared, state: &str, max_age: i64) -> String {
    format!(
        "apps_oauth_state={state}; Path=/v1/auth; HttpOnly; SameSite=Lax; Max-Age={max_age}{}",
        if s.config.public_url.starts_with("https:") {
            "; Secure"
        } else {
            ""
        }
    )
}
pub fn check_csrf(s: &Shared, headers: &HeaderMap) -> Result<()> {
    if cookie(headers, "apps_session").is_none() {
        return Ok(());
    }
    let origin = headers.get("Origin").and_then(|v| v.to_str().ok());
    if !origin.is_some_and(|origin| {
        s.config
            .allowed_origins
            .iter()
            .any(|allowed| allowed == origin)
    }) {
        return Err(ApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            "origin_mismatch",
            "Browser mutation origin does not match the configured Apps origin.",
            "Send the request from the configured application website.",
        ));
    }
    Ok(())
}
fn request_origin(s: &Shared, headers: &HeaderMap) -> String {
    for name in ["X-Forwarded-Host", "Host"] {
        if let Some(host) = headers.get(name).and_then(|v| v.to_str().ok())
            && let Some(origin) = s.config.allowed_origins.iter().find(|origin| {
                url::Url::parse(origin).ok().is_some_and(|url| {
                    url[url::Position::BeforeHost..url::Position::AfterPort] == *host
                })
            })
        {
            return origin.clone();
        }
    }
    url::Url::parse(&s.config.public_url)
        .unwrap()
        .origin()
        .ascii_serialization()
}
pub async fn bearer(s: &Shared, headers: &HeaderMap) -> Result<Option<String>> {
    if let Some(auth) = headers.get("Authorization") {
        return auth
            .to_str()
            .ok()
            .and_then(|v| v.strip_prefix("Bearer "))
            .filter(|v| !v.is_empty())
            .map(|v| Some(v.to_owned()))
            .ok_or_else(ApiError::auth);
    }
    let Some(id) = cookie(headers, "apps_session") else {
        return Ok(None);
    };
    let row: Option<(String, Option<String>, i64)> = s
        .store
        .lock()
        .unwrap()
        .connection
        .query_row(
            "SELECT access_token,refresh_token,expires_at FROM sessions WHERE id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((access, refresh, expiry)) = row else {
        return Ok(None);
    };
    if expiry > chrono::Utc::now().timestamp() + 30 {
        return Ok(Some(access));
    }
    let Some(refresh) = refresh else {
        return Ok(None);
    };
    let secret = s.config.accounts_app_secret.as_deref().ok_or_else(|| {
        ApiError::unavailable("APPS_ACCOUNTS_APP_SECRET is required for browser sign-in.")
    })?;
    // Serialize rotating refreshes so concurrent browser calls cannot replay a retired token.
    let _guard = s.mutation_gate.lock().await;
    let current: Option<(String, i64)> = s
        .store
        .lock()
        .unwrap()
        .connection
        .query_row(
            "SELECT access_token,expires_at FROM sessions WHERE id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((current, exp)) = current
        && exp > chrono::Utc::now().timestamp() + 30
    {
        return Ok(Some(current));
    }
    let tokens = s
        .accounts
        .as_app("apps", secret)
        .refresh(&refresh)
        .await
        .map_err(accounts_error)?;
    let access = tokens.access_token.expose().to_owned();
    s.store.lock().unwrap().connection.execute(
        "UPDATE sessions SET access_token=?1,refresh_token=?2,expires_at=?3 WHERE id=?4",
        params![
            access,
            tokens.refresh_token.as_ref().map(|r| r.expose()),
            chrono::Utc::now().timestamp() + tokens.expires_in as i64,
            id
        ],
    )?;
    Ok(Some(access))
}
pub async fn identity(s: &Shared, token: Option<&str>) -> Result<Option<Identity>> {
    let Some(token) = token else { return Ok(None) };
    if s.config.dev_auth && token.starts_with("dev:") {
        let parts: Vec<_> = token.splitn(3, ':').collect();
        if parts.len() != 3
            || parts[1].is_empty()
            || !matches!(parts[2].split(':').next(), Some("c" | "si"))
        {
            return Err(ApiError::auth());
        }
        return Ok(Some(Identity {
            uuid: parts[1].into(),
            id: parts[2].into(),
            display_name: parts[2].into(),
            verified_emails: vec![],
        }));
    }
    let jwks = s.accounts.jwks().await.map_err(accounts_error)?;
    let claims = verify_access_token(
        &jwks,
        token,
        &VerifyOptions::for_app("apps").with_issuer(s.config.accounts_url.trim_end_matches('/')),
    )
    .map_err(|e| {
        ApiError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "invalid_token",
            e.to_string(),
            "Sign in to Silicon Apps again.",
        )
    })?;
    let secret = s.config.accounts_app_secret.as_deref().unwrap_or("");
    let info = s
        .accounts
        .as_app("apps", secret)
        .userinfo(token)
        .await
        .map_err(accounts_error)?;
    let user = info.account;
    if user.uuid != claims.sub {
        return Err(ApiError::auth());
    }
    Ok(Some(Identity {
        uuid: user.uuid,
        id: user.id,
        display_name: user.display_name,
        verified_emails: info.verified_emails.unwrap_or_else(|| {
            if user.email_verified == Some(true) {
                user.email.into_iter().collect()
            } else {
                vec![]
            }
        }),
    }))
}
pub async fn handle_auth(
    s: &Shared,
    path: &str,
    method: &Method,
    q: &BTreeMap<String, String>,
    headers: &HeaderMap,
    raw: &Bytes,
) -> Result<Response> {
    let secret = s.config.accounts_app_secret.as_deref();
    if path == "auth/login" && *method == Method::GET {
        secret.ok_or_else(|| {
            ApiError::unavailable("Browser sign-in requires APPS_ACCOUNTS_APP_SECRET.")
        })?;
        let return_to = q.get("return_to").map(String::as_str).unwrap_or("/");
        if !return_to.starts_with('/')
            || return_to.starts_with("//")
            || return_to.contains(['\\', '\r', '\n'])
        {
            return Err(ApiError::bad(
                "return_to must be a local path beginning with one slash.",
            ));
        }
        let origin = request_origin(s, headers);
        let state = silicon_accounts_client::random_state();
        let pkce = silicon_accounts_client::pkce_pair();
        {
            let store = s.store.lock().unwrap();
            store.connection.execute(
                "DELETE FROM oauth_pending WHERE expires_at<?1",
                [chrono::Utc::now().timestamp()],
            )?;
            store.connection.execute("INSERT INTO oauth_pending(state,verifier,return_to,expires_at,origin) VALUES(?1,?2,?3,?4,?5)",params![state,pkce.verifier,return_to,chrono::Utc::now().timestamp()+600,origin])?;
        }
        let params = AuthorizeParams::new("apps", format!("{origin}/v1/auth/callback"))
            .state(&state)
            .pkce(&pkce)
            .scopes(["profile", "email"]);
        let url = s.accounts.authorize_url(&params);
        return Ok((
            [("Set-Cookie", state_cookie(s, &state, 600))],
            Redirect::to(url.as_str()),
        )
            .into_response());
    }
    if path == "auth/callback" && *method == Method::GET {
        let state = q
            .get("state")
            .ok_or_else(|| ApiError::bad("The sign-in callback is missing state."))?;
        if cookie(headers, "apps_oauth_state").as_deref() != Some(state) {
            return Err(ApiError::new(
                axum::http::StatusCode::FORBIDDEN,
                "oauth_state_mismatch",
                "Sign-in state does not match this browser.",
                "Start sign-in again from Silicon Apps.",
            ));
        }
        let pending:Option<(String,String,String)>=s.store.lock().unwrap().connection.query_row("DELETE FROM oauth_pending WHERE state=?1 AND expires_at>?2 RETURNING verifier,return_to,origin",params![state,chrono::Utc::now().timestamp()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let (verifier, return_to, origin) = pending.ok_or_else(|| {
            ApiError::bad("The sign-in attempt expired or was already completed.")
        })?;
        let code = q.get("code").ok_or_else(|| {
            ApiError::bad("Sign-in was declined or the callback is missing its code.")
        })?;
        let tokens = s
            .accounts
            .as_app(
                "apps",
                secret.ok_or_else(|| {
                    ApiError::unavailable("APPS_ACCOUNTS_APP_SECRET is required.")
                })?,
            )
            .exchange_code(
                code,
                &format!(
                    "{}/v1/auth/callback",
                    if origin.is_empty() {
                        &s.config.public_url
                    } else {
                        &origin
                    }
                ),
                Some(&verifier),
            )
            .await
            .map_err(accounts_error)?;
        identity(s, Some(tokens.access_token.expose())).await?;
        let id = silicon_accounts_client::random_token(32);
        s.store.lock().unwrap().connection.execute(
            "INSERT INTO sessions(id,access_token,refresh_token,expires_at) VALUES(?1,?2,?3,?4)",
            params![
                id,
                tokens.access_token.expose(),
                tokens.refresh_token.as_ref().map(|v| v.expose()),
                chrono::Utc::now().timestamp() + tokens.expires_in as i64
            ],
        )?;
        let mut response = Redirect::to(&return_to).into_response();
        response.headers_mut().append(
            "Set-Cookie",
            session_cookie(s, &id, 30 * 24 * 3600).parse().unwrap(),
        );
        response
            .headers_mut()
            .append("Set-Cookie", state_cookie(s, "", 0).parse().unwrap());
        return Ok(response);
    }
    if *method != Method::POST {
        return Err(ApiError::missing());
    }
    let body: Value = if raw.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(raw).map_err(|_| ApiError::bad("Expected JSON auth request."))?
    };
    if path == "auth/logout" {
        check_csrf(s, headers)?;
        let session_id = cookie(headers, "apps_session");
        let token = body["token"]
            .as_str()
            .map(str::to_owned)
            .or(bearer(s, headers).await?);
        if let Some(token) = token
            && (!s.config.dev_auth || !token.starts_with("dev:"))
        {
            s.accounts
                .as_app(
                    "apps",
                    secret.ok_or_else(|| {
                        ApiError::unavailable("APPS_ACCOUNTS_APP_SECRET is required.")
                    })?,
                )
                .revoke(&token)
                .await
                .map_err(accounts_error)?;
        }
        if let Some(id) = session_id {
            s.store
                .lock()
                .unwrap()
                .connection
                .execute("DELETE FROM sessions WHERE id=?1", [id])?;
        }
        return Ok((
            [("Set-Cookie", session_cookie(s, "", 0))],
            Json(json!({"authenticated":false})),
        )
            .into_response());
    }
    let app = s.accounts.as_app(
        "apps",
        secret.ok_or_else(|| {
            ApiError::unavailable("APPS_ACCOUNTS_APP_SECRET is required for token exchange.")
        })?,
    );
    let tokens = match path {
        "auth/exchange" => {
            app.exchange_slt(crate::store::str_field(&body, "slt")?)
                .await
        }
        "auth/refresh" => {
            app.refresh(crate::store::str_field(&body, "refresh_token")?)
                .await
        }
        _ => return Err(ApiError::missing()),
    }
    .map_err(accounts_error)?;
    Ok(Json(json!(tokens)).into_response())
}
