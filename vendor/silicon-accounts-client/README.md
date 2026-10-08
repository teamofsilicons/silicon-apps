# silicon-accounts-client

The Rust package for [Silicon Accounts](https://accounts.teamofsilicons.com): one personal
account for every Carbon and Silicon, and the sign-in layer for apps.

It is **stateless**: it never writes files or reads the environment (unless you call
`Config::from_env`). You decide where tokens live. The `silicon-accounts` CLI is built only on
this package, so everything the CLI does, you can do from Rust.

```toml
[dependencies]
silicon-accounts-client = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Every call is async and returns `silicon_accounts_client::Result<T>`. Errors are typed
(`Error::Api`, `Error::OAuth`, `Error::Http`, `Error::Decode`, `Error::InvalidInput`,
`Error::PayloadTooLarge`, `Error::Token`, `Error::TimedOut`); each one has a precise
`message()` saying what went wrong and why, and a `hint()` saying what to do next.
`Display` prints both.

```rust
match client.silicon_login("si:scout", &stk, None).await {
    Ok(tokens) => { /* … */ }
    Err(err) if err.is_code("custodian_pending") => eprintln!("{err}"),
    Err(err) => return Err(err.into()),
}
```

Identifiers: store the account `uuid` (permanent) or the membership id
`{app_id}:{uuid}`; the `c:`/`si:` id is for display and can change.

## Quickstart for Silicons

```rust
use silicon_accounts_client::{AccountsClient, SiliconSelfCreate};

let client = AccountsClient::new("https://accounts.teamofsilicons.com")?;

// 1. Get an account (once): name your custodian; they have 14 days to accept.
let created = client
    .silicon_self_create(
        &SiliconSelfCreate {
            id: "si:scout".into(),
            display_name: "Scout".into(),
            custodian: "c:saket".into(),
            ..Default::default()
        },
        Some("create-si-scout-1"), // idempotency key: retries never create twice
    )
    .await?;
let stk = created.stk.expect("generated STKs are returned once"); // store it now
let decision = client
    .wait_for_custodian_decision(
        &created.request.id,
        created.request_token.expose(),
        &silicon_accounts_client::WaitOptions::custodian_default(),
        |_| {},
    )
    .await?;
assert!(decision.is_accepted());

// 2. Sign in, and 3. sign into an app with a short-lived token.
let tokens = client.silicon_login("si:scout", stk.expose(), Some("scout on build box")).await?;
let session = client.with_token(tokens.access_token.expose());
let slt = session.short_lived_token("remind").await?; // single use, 2 minutes
// hand slt.slt.expose() to the app; it calls exchange_slt
```

Refresh the first-party access token (30 minutes) with
`client.refresh_first_party(refresh_token)`; refresh tokens rotate on every use and
presenting a used one revokes the whole session, so always store the new one.

## Quickstart for apps

### Sign people in

```rust
use silicon_accounts_client::{AccountsClient, AuthorizeParams, pkce_pair, random_state};

let client = AccountsClient::new("https://accounts.teamofsilicons.com")?;
let app = client.as_app("briefcase", app_secret);

// Redirect the browser:
let pkce = pkce_pair();
let state = random_state();
let url = client.authorize_url(
    &AuthorizeParams::new("briefcase", "https://briefcase.example/auth/callback")
        .state(&state)
        .pkce(&pkce)
        .scopes(["email"]),
);
// …store state + pkce.verifier in the browser session, redirect to `url`.

// On the callback (?code=…&state=…), after checking state:
let tokens = app
    .exchange_code(&code, "https://briefcase.example/auth/callback", Some(&pkce.verifier))
    .await?;
let account = tokens.account.expect("token responses carry the account");
println!("{} signed in as membership {}", account.id, account.membership_id);

// Silicons sign in with a short-lived token instead of the browser:
let tokens = app.exchange_slt(&slt_from_the_silicon).await?;
```

### Check access tokens

```rust
let jwks = client.jwks().await?;                               // cache it; refetch on unknown kid
let claims = app.verify_access_token_locally(&jwks, &token)?;  // EdDSA, exp, aud == app id
let live = app.introspect(&token).await?;                      // also sees revocation
```

### Verify proofs (User verification / App verification)

```rust
use silicon_accounts_client::ProofVerification;

match app.verify_proof(&proof_token).await? {
    ProofVerification::Valid(proof) => {
        // proof.issuing_app, proof.user (User verification), proof.scopes, proof.expires_at
    }
    _ => { /* invalid: reject. The service answers {"valid":false,"expires_at":null} */ }
}

// Issuing (as app A): a user verification proof to act at app B for an account that consented in A.
let proof = app
    .issue_user_verification(
        &silicon_accounts_client::IssueUserVerification {
            subject_token: account_access_token,
            receiving_app: "briefcase".into(),
            scopes: vec!["files.write".into()],
            access_ttl_seconds: Some(600),
        },
        Some("user_verification-req-42"),
    )
    .await?;

// An app verification proof is for exactly one app: to talk to remind and waveform, issue one proof each.
for receiving_app in ["remind", "waveform"] {
    let proof = app
        .issue_app_verification(
            &silicon_accounts_client::IssueAppVerification {
                receiving_app: receiving_app.into(),
                ..Default::default()
            },
            None,
        )
        .await?;
    // send proof.proof_token to that app only; keep proof.proof_refresh_token
}
```

### Hosted sign-in buttons

`AuthorizeParams::method("google")` (or `apple`, `email`, `phone`) makes a direct button: Google
and Apple first show the hosted "Opening Google to sign you in to {app}…" page in your style;
email and phone open on that method's empty field. `AuthorizeParams::intent("signup")` shows the
sign-up version of the pages ("Create your {app} account"). There is no way to pass a Carbon's
email or phone (`login_hint` is ignored by the service): they always type it on our pages.

### The developer platform's public client

developers.teamofsilicons.com signs Carbons in as the first-party public client `developer`
(`DEVELOPER_APP_ID`): no secret, PKCE S256 required. `exchange_developer_code`,
`refresh_public_client("developer", …)` and `revoke_public_client("developer", …)` cover it. Its
tokens (`aud = developer`) only read the signed-in Carbon (`GET /v1/me`, `GET /v1/session`) and
manage the apps they own (`GET /v1/me/owned-apps`, `/v1/apps/{app_id}/…` through
`AccountSession::app`); everything else answers 401 `token_wrong_audience`.

### Webhooks

```rust
use silicon_accounts_client::{verify_and_parse_webhook, WebhookPayload, DEFAULT_WEBHOOK_TOLERANCE};

let event = verify_and_parse_webhook(
    &webhook_secret,                 // whsec_…, used as-is as the HMAC key
    headers["X-Accounts-Timestamp"],
    headers["X-Accounts-Signature"], // v1=<hex HMAC-SHA256(secret, "{ts}.{raw body}")>
    &raw_body,                       // the exact bytes received
    DEFAULT_WEBHOOK_TOLERANCE,       // 5 minutes
)?;
match event.payload {
    WebhookPayload::AccountIdChanged(change) => { /* update the shown id; key on change.uuid */ }
    WebhookPayload::AccountDeleted(gone) => { /* delete gone.uuid's data */ }
    _ => {}
}
// Dedupe on event.event_id: retries and replays reuse it.
```

Failed deliveries can be listed and replayed (same event id, the current URL and secret):
`app.deliveries(…)`, `app.delivery(id)` and `app.replay(…)` for an app's webhook. A Silicon's own
webhook has the same calls on its session (`my_webhook_deliveries`, `my_webhook_delivery`,
`replay_my_webhook`), and so does its custodian's (`silicon_webhook_deliveries`,
`silicon_webhook_delivery`, `replay_silicon_webhook`, by the Silicon's uuid):

```rust
use silicon_accounts_client::{DeliveriesQuery, ReplayRequest};

let session = client.with_token(silicon_access_token);
let failed = session
    .my_webhook_deliveries(&DeliveriesQuery { status: Some("failed".into()), ..Default::default() })
    .await?;
let result = session
    .replay_my_webhook(&ReplayRequest::Failed { since: None }, Some("replay-1"))
    .await?; // at most 100 per call: call again while result.extra["remaining"] > 0
```

### User base, imports, sign-in setup

```rust
use silicon_accounts_client::{ImportInput, ImportOptions, UsersQuery};

let page = app.users(&UsersQuery { q: Some("saket".into()), ..Default::default() }).await?;
let job = app
    .start_import(&ImportInput::Csv(csv_bytes.into()), &ImportOptions { dry_run: true, ..Default::default() }, Some("import-1"))
    .await?;
let job = app.wait_for_import(&job.id, std::time::Duration::from_secs(1)).await?;
let details = app.update_signin_config(&serde_json::json!({"methods": {"google": true}}), Some(7), None).await?;
// Webhook calls take an idempotency key: a retried set/rotate returns the same secret, a
// retried test queues one ping.
let hook = app.set_webhook("https://app.example.com/hooks/accounts", Some("wh-set-1")).await?;
```

An import is at most 50 MB (`MAX_IMPORT_BYTES`: the CSV itself, or the JSON the rows are
sent as) and 100,000 rows. `start_import` refuses a bigger body before sending anything, with
`Error::PayloadTooLarge` (code `payload_too_large`, the service's own code for it, and
`details()` `{"size_bytes", "limit_bytes"}`): the service would refuse it as soon as it sees
the size, often before the upload ends, so the caller could otherwise see a reset
connection instead of the reason. Split such a file into parts and import them one after
another.

`client.app_public("briefcase")` returns what a sign-in page needs (methods, branding, copy) and
`allowed_origins`, the origins that may frame the embed and use the SDK.

An app's owner can do the same through their own session without the app secret:
`client.with_token(owner_token).app("briefcase")`. Calls that need the app's own
credentials (token exchange, proof issuing and verifying) are refused in that mode with
a precise error.

## Carbons

```rust
let device = client.device_authorize(Some("my tool on laptop")).await?;
println!("Open {} and enter {}", device.verification_uri, device.user_code);
let tokens = client.wait_for_device_tokens(&device, |_| {}).await?;
let me = client.with_token(tokens.access_token.expose()).me().await?;
```

`AccountSession` covers the whole account: profile, id changes, emails and phones,
linked identities, apps you signed into, sessions, history, User verification proofs about you, the
Silicons you are custodian of (create, update, upload their photo with `set_silicon_photo`,
check an id for one with `silicon_id_available`, rotate STK, transfer, delete), custodian
requests, device approvals and the apps you own.

```rust
let s = client.with_token(carbon_token);
// Can si:scout take back its old id? (A reservation held by that Silicon is reclaimable for it.)
let check = s.silicon_id_available("si:scout_v2", "si:scout").await?;
if check.reclaimable { s.change_silicon_id("si:scout_v2", "si:scout").await?; }
let uploaded = s.set_silicon_photo("si:scout", png_bytes, "image/png", Some("photo-1")).await?;
```

`client.meta()` reports the deployment, including `docs_url` (where the docs live).

## Configuration

`AccountsClient::builder()` sets the base URL, timeouts, a User-Agent product token,
telemetry (`X-Accounts-Telemetry: off` when disabled), retries for safe requests, and
whether plain http to non-loopback hosts is allowed (off by default). `Config::from_env`
reads `ACCOUNTS_URL`, `ACCOUNTS_APP_ID`, `ACCOUNTS_APP_SECRET`, `ACCOUNTS_TELEMETRY`,
`ACCOUNTS_TIMEOUT_SECONDS` and `ACCOUNTS_ALLOW_INSECURE_HTTP` when you want that.

Secrets (`Secret`) never print in `Debug` output; call `.expose()` where a value must
leave your program.

## Links

* Docs: https://developers.teamofsilicons.com/docs/accounts (and `silicon-accounts docs` in the CLI)
* Source: https://github.com/teamofsilicons/silicon-accounts
