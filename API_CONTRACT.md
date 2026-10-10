# Silicon Apps API v1

Base URL: `http://127.0.0.1:4310`; every endpoint below has `/v1` prefix except `/health`.
Authorization: `Bearer <Silicon Accounts token for apps>`. Production verifies issuer, signature, audience and current account via official Accounts client. Public browsing/downloads need no token. Local-only mode `APPS_DEV_AUTH=1` accepts `Bearer dev:<uuid>:<c:id or si:id>` (example `dev:alice:c:alice`); no verified email domains are inferred from this string.

The common developer portal may send its sealed first-party `aud=developer` token through its server-side `/api/apps/*` proxy. Apps verifies that exact audience, signature, issuer and expiry, then checks live Accounts userinfo for account and token-family revocation on every request. This authoring-only bridge accepts the identity/session, targets, app list/availability/detail, authoring metadata/history, package download/media, invitations, and app authoring mutations documented below, plus sanitized telemetry. It does not create an Apps membership or accept developer tokens for reviews, install receipts, package resolution, platform registration, reports, or Apps OAuth token exchange. All existing UUID author/admin and private visibility rules still apply. The developer BFF enforces its sealed session and same-origin CSRF checks before forwarding a Bearer token; tokens and app credentials must never reach browser JavaScript.
For developer tokens, invitation matching uses currently verified contacts from the existing first-party `GET /v1/me` self-profile contract; its UUID must match both the signed token and userinfo. Apps-scoped tokens continue to receive verified emails only under their granted Email scope.
Every mutation requires `Idempotency-Key` (8–200 printable characters). Keys are scoped to account, method/path and request digest; conflicting reuse is HTTP 409. Replays have `Idempotent-Replayed: true`. JSON response always directly contains the object described. Errors: `{ "error": {"code":"…", "message":"…", "hint":"…", "details":null} }`.

## Accounts / discovery
- `GET /health` → `{status:"ok",service:"silicon-apps",version:"0.1.2"}`.
- `GET /me` → `{uuid,id,display_name,verified_emails:[]}`.
- `GET /targets?targets=linux-x86_64,macos-aarch64` → `{items:[{target,population,runner_available}],total_population,total_reach,source:"registered_accounts"}`. Populations count observed authenticated accounts; total reach deduplicates accounts across selected targets.
- `GET /apps/availability/{app_id}` → `{available:boolean}` (invalid IDs false). A historical 1–2 character Accounts ID reserved by `APPS_HISTORICAL_APP_IDS` is true only for its signed-in owner, while free; for anyone else it is an invalid ID.
- `GET /apps?q=&visibility=public|private&mine=true&limit=50&offset=0` → `{items:[App],total:n}`. `mine` requires auth and includes drafts. Otherwise published accessible apps only. Search exact ID/name ranks ahead of prefixes, substrings, then typo matches across IDs, names, tags and description words; rating breaks equal scores.
- `GET /apps/{app_id}` → App (drafts only visible to authors).
- `POST /apps` body `{app_id,name,description?:"",logo?:""}` → `{app:App,app_secret:"…"}`. `app_id` is a new 3–30 character ID, or a historical 1–2 character Accounts ID that `APPS_HISTORICAL_APP_IDS` reserves for the caller; anyone else gets 400 `invalid_input`, as for any invalid ID. Its `app.created` history then includes `historical_app_id:true`.
- `PATCH /apps/{app_id}` body any `{name,description,tags:[],logo,banner,carousel:[{url,kind:"image"|"video",alt}],links:{website,developer_docs,android,ios,custom:[{label,url,logo}]},setup_step:1..7}` → App.
- `PUT /apps/{app_id}/access` body `{visibility:"public"|"private",domains:["example.com"],account_ids:["c:alice","si:bot"]}` → App. Admin only; identities are resolved to immutable UUIDs.
- `GET /apps/{app_id}/readiness` → `{ready:boolean,errors:[{field,message}],required_commands:["--help","accounts --json","login status --json"]}`.
- `POST /apps/{app_id}/publish` body `{}` → App. Requires 200–600 character description, ≥1 accepted package in a release; immediately publishes without manual review. App can publish with development releases only; default install then says no production release.
- `POST /apps/{app_id}/secret/rotate` body `{}` → `{app_secret:"…"}`. All authors.

## Ownership and history
- `GET /authors/{uuid}?limit=24&offset=0` → `{uuid,id,display_name,items:[App],total:n}`. The stable profile URL uses the Accounts UUID; public ID and display name are refreshed from Accounts. Lists published apps the caller may see, ordered by name then app ID, with a maximum page size of 100. Drafts never appear, even for their authors. Unknown authors and authors with no visible published apps return 404.
- `GET /apps/{app_id}/authors` → `{items:[{uuid,id,display_name,joined_at}]}`; admin isn't marked publicly.
- `POST /apps/{app_id}/invites` body `{to:"c:alice"|"si:bot"|"alice@example.com"}` → Invite; author only.
- `GET /apps/{app_id}/invites` → `{items:[Invite]}`; author only.
- `GET /invites` → `{items:[Invite]}`; only invitations matching your UUID or verified email.
- `POST /invites/{invite_id}/accept` or `/decline` body `{}` → `{status:"accepted"|"declined"}`.
- `DELETE /apps/{app_id}/invites/{invite_id}` body `{}` → `{status:"cancelled"}`; author only.
- `POST /apps/{app_id}/authors/leave` body `{}` → `{status:"left"}`; last author prohibited, admin passes to oldest remaining author.
- `POST /apps/{app_id}/admin` body `{uuid:"…"}` → `{status:"transferred"}`; admin only, destination must already be an author.
- `DELETE /apps/{app_id}/authors/{uuid}` body `{}` → `{status:"removed"}`; admin only, cannot remove self.
- `GET /apps/{app_id}/history?limit=100&offset=0` → `{items:[{id,at,actor_uuid,kind,data}],total:n}`; author only.

## Packages and releases
- `POST /apps/{app_id}/packages/{target}` body raw `.tar.gz`, `Content-Type: application/gzip` → Package. Server validates archive and invokes configured isolated runner. Failures return 422 with exact command results in `error.details`, and persist failed validation in history. Missing runner returns 503. Accepted object `{id,target,sha256,size,command,validation:[{command,exit_code,stdout,stderr,passed,expected}],created_at}`. `apps.yaml` manifest format is shared with Rust package crate.
- `GET /apps/{app_id}/packages` → `{items:[Package]}`; authors only.
- `POST /apps/{app_id}/releases` body `{version:"1.2.3",package_ids:["…"],notes?:""}` → Release; channel always development, package IDs must all belong to this app, no duplicate targets.
- `GET /apps/{app_id}/releases?channel=production|development` → `{items:[Release]}`; app visibility enforced.
- `POST /apps/{app_id}/releases/{release_id}/promote` body `{version:"2.0.0"}` → Release; creates immutable production release using development package bytes, versions independent per channel.
- `GET /apps/{app_id}/resolve?channel=production|development&version=1.2.3&target=macos-aarch64` → `{app_id,release:Release,package:Package,download_path:"/v1/apps/.../packages/.../download"}`. Default channel production, optional version, target required.
- `GET /apps/{app_id}/packages/{package_id}/download` → raw gzip. Visibility is rechecked for every download.
- `POST /apps/{app_id}/installs` body `{release_id,package_id}` → `{installs:n}`. Anonymous allowed with Idempotency-Key. Count only after client completes install.

## Reviews and webhooks
- `GET /apps/{app_id}/reviews` → `{items:[{uuid,id,rating,text,updated_at}],rating:number|null,count:n}`.
- `PUT /apps/{app_id}/review` body `{rating:1..5,text?:""}` → Review; signed-in accessible app only, max 600 chars, one per UUID.
- `DELETE /apps/{app_id}/review` body `{}` → `{status:"removed"}`. The original reviewer may remove their own review after losing access to a private app; this does not grant access to app details or other reviews.
- `GET /apps/{app_id}/webhook` → Accounts-owned configuration; author only.
- `PUT /apps/{app_id}/webhook` body `{url,events:["id_change",...]}` → Accounts configuration (secret shown only when generated).
- `POST /apps/{app_id}/webhook/rotate` body `{}` → `{webhook_secret:"whsec_…"}`.
- `POST /reports` body `{message,pr?:""}` → `{id,status:"queued"}`; durable mail outbox for the three specified recipients. Requires configured delivery transport; unavailable transport returns 503.

`App`: `{app_id,name,description,logo,banner,tags,visibility,domains,account_ids,links,carousel,published,setup_step,created_at,updated_at,authors:[...],targets:[],latest_production:Release|null,latest_development:Release|null,rating:number|null,review_count,installs,is_author,is_admin}`. `domains` and `account_ids` only returned to authors. `is_admin` only indicates the caller's own capability, never marks an author. `Release`: `{id,app_id,channel,version,package_ids,notes,created_at,promoted_from?:id}`. `Invite`: `{id,app_id,to,account_uuid?:uuid,status,created_at}`; only authors/invitees may inspect pending invites.

## Browser authentication, telemetry and media

- `GET /session` returns `{authenticated,account:Identity|null}` from a secure server-side session.
- `GET /auth/login?return_to=/developer` redirects to Silicon Accounts with PKCE and a browser-bound one-use state. `GET /auth/callback` verifies state, exchanges the code and sets an opaque HttpOnly SameSite=Lax cookie. `APPS_ALLOWED_ORIGINS` supports both developer and store origins; each registered callback must match Accounts configuration.
- `POST /auth/exchange {slt}` and `POST /auth/refresh {refresh_token}` return the official Accounts `TokenResponse`. `POST /auth/logout {token?}` revokes the app token and clears the browser session. Auth endpoints follow Accounts one-use token semantics and are exempt from the catalog Idempotency-Key requirement; do not automatically retry consumed SLTs/codes/refresh tokens.
- Browser mutations require an `Origin` from `APPS_ALLOWED_ORIGINS` whenever an Apps session cookie is present. Bearer-only CLI requests need no Origin.
- `POST /apps/{app_id}/media` accepts raw PNG/JPEG/WebP/GIF/MP4/WebM up to 100 MiB, with its matching Content-Type. Returns `{url,id,kind,size,content_type}`; save returned URL in the relevant app field. Reads at that URL recheck app visibility. SVG uploads are rejected. Package and media objects are atomically published without replacing existing bytes. Duplicate media retains its first Content-Type; conflicting types for the same bytes return 409. Existing Accounts base64 logos are preserved on import.
- App fields `logo_alt` and `banner_alt` support up to 10,000 characters, alongside each carousel item's `alt`.
- `POST /platforms {target}` records the authenticated account's observed platform. `/targets?targets=linux-x86_64,macos-aarch64` returns `source:"registered_accounts"`, per-target `population`, `total_population`, and distinct `total_reach` for selected targets. These are actual registered account counts, beginning at zero; they do not estimate unknown ecosystem users. Authenticated successful install receipts register their target too.
- `POST /telemetry {step,progress,event?,path?,target?,status_code?,duration_ms?,item_count?,byte_count?,error_code?}` records a sanitized Space Station event. `X-Apps-Telemetry: off` opts out. Requests return `{accepted:false,reason:"not_configured"}` if no operator telemetry destination exists, without accumulating an undeliverable outbox. Arbitrary properties, credentials, raw app IDs and user identities are excluded.
- `POST /apps/{app_id}/webhook/rotate` generates or replaces a secret even before a URL is configured. `PUT /webhook` preserves an existing secret; it returns `{url,secret?}` with `secret` only if none existed. `GET /webhook` returns `{url:string|null,secret_set:boolean,events:string[]|null}`.
- Create/rotate/webhook secret responses can be replayed with the same key for 10 minutes. After that, plaintext is removed and retry returns `409 secret_replay_expired` without repeating the operation. Normal idempotency records remain durable. Every mutation history event includes its idempotency key; failed package validation is also durable and replayed without executing twice.
- Current author, reviewer and known-account invite labels refresh through Accounts using immutable UUIDs; an ID-only lookup cannot erase a saved display name. A renamed account cannot receive a duplicate pending invitation under its new ID.
- Unknown routes, extra path segments and unsupported mutation methods return 404 before any external webhook, package-runner or media side effect.
