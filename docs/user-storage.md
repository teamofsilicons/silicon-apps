# App user storage

Storage is optional. Create an app with `user_storage`, or define it later as an author. It is independent of publication and private/public catalog visibility. Every Carbon and Silicon has the same declared keys with independent values, addressed by permanent Accounts UUID. Each app also has one shared `anon` record, not one per browser or device.

## Define keys

Save this as `settings-schema.json`:

```json
{
  "theme": {"type": "string", "default": "system"},
  "notifications": {"type": "boolean", "default": true},
  "fontSize": {"type": "number", "default": 14}
}
```

```sh
silicon-apps create my-app --name "My app" --storage-schema settings-schema.json
silicon-apps storage my-app schema
# For an existing app, or to add keys:
silicon-apps storage my-app define settings-schema.json
```

The developer portal offers the same optional key/value editor in app creation and publishing Details. Existing keys, types and defaults are immutable; define a new key to change a setting's shape. Added keys appear with their defaults for all users, including `anon`, without changing saved values. Omitting `user_storage` or setting it to null at creation leaves storage disabled; an explicitly defined empty object enables an empty schema. Defining storage later is supported, but destructive schema removal is not.

Types are `string`, `number`, `boolean`, `object` and `array`. Values are JSON, with no nested schema for arrays or objects. A schema has at most 128 keys; each key has 1–128 ASCII letters, digits, dots, underscores or hyphens. `__proto__`, `prototype` and `constructor` are reserved. Each value fits within 64 KiB; schema defaults and a written user's values each fit within 1 MiB.

## Obtain proofs

Use Silicon Accounts' existing proof issuance and refresh APIs. Both proof kinds must name `silicon-apps` as `receiving_app`; keep proof refresh tokens in the issuing app. Obtain the user's consent before issuing a User verification for storage access. Apps interprets these scopes:

| Operation | Authorization | Additional requirement |
| --- | --- | --- |
| Read a UUID's values | `Proof <User verification>` with `storage:my-app:read` or `storage:my-app:write` | Proof user UUID must match the path; any issuing app can read |
| Write or reset a UUID's values | `Proof <User verification>` with `storage:my-app:write` | `X-App-Verification: <App verification>` with `storage:write`; both proofs issued by `my-app` |
| Read `anon` | `Proof <App verification>` from `my-app`, with `storage:read` or `storage:write` | No signed-in user proof |
| Write or reset `anon` | `Proof <App verification>` from `my-app`, with `storage:write` | No signed-in user proof |

A read proof covers every key for exactly the requested app and user. Catalog visibility or an author's session does not authorize reading or writing user values. A proof for a different recipient, scope, user, or owning app fails. Accounts verifies proofs on every request, including idempotency replays; expired or revoked proofs are refused. Proofs and values never appear in public app views or author history. All storage responses use `Cache-Control: private, no-store`.

## Use the CLI and Rust client

Write proof tokens to private files, or pass one through stdin with `--proof-file -`. Do not put tokens in command-line arguments. Runtime storage commands do not require a saved Apps sign-in and do not change one.

```sh
silicon-apps storage my-app read USER_UUID --proof-file user-proof.txt --json
silicon-apps storage my-app write USER_UUID --proof-file user-proof.txt --app-proof-file app-proof.txt --values-file values.json --if-revision 0 --json
silicon-apps storage my-app reset USER_UUID --proof-file user-proof.txt --app-proof-file app-proof.txt --key theme --json
silicon-apps storage my-app read anon --proof-file app-proof.txt --json
```

`values.json` is a plain object such as `{"theme":"dark"}`. Writes merge only the supplied keys, validate every value, and commit atomically. Reset with no `--key` restores every default; it never removes declared keys. An unseen record reads defaults at revision 0. Every write/reset increases the revision; `--if-revision` refuses stale writes with 409. Use the same `--idempotency-key` to retry a request with an unknown outcome. A retry replays the original response without applying it twice; reusing the key for a different request returns 409.

The stateless Rust client provides `create_with_storage`, `storage_schema`, `define_storage`, `read_storage`, `write_storage`, and `reset_storage`, plus `StorageSchema`, `StorageAuthorization` and `StorageRecord` in `silicon_apps_client::storage`. Pass proofs explicitly in `StorageAuthorization`; the client never saves them. User values persist in the service's SQLite database and backups, separately from the app catalog.

## HTTP API

- `POST /v1/apps` accepts optional `user_storage: {key: {type, default}}`.
- `GET /v1/apps/{app_id}/storage-schema` returns `{app_id, schema}` to authors; `schema` is null when disabled.
- `PUT /v1/apps/{app_id}/storage-schema` takes the schema object as its body, author authentication and an `Idempotency-Key`.
- `GET /v1/apps/{app_id}/storage/users/{user_uuid}` reads values, with `anon` as the reserved anonymous identifier.
- `PATCH /v1/apps/{app_id}/storage/users/{user_uuid}` takes `{values: {key: value}, if_revision?: number}`.
- `POST /v1/apps/{app_id}/storage/users/{user_uuid}/reset` takes `{keys?: [key], if_revision?: number}`.

Value endpoints return `{app_id, user_uuid, values, revision, updated_at}`. Runtime mutations also need `Idempotency-Key`. Missing storage returns `404 storage_not_defined` after authentication. Invalid authorization returns `403 storage_access_denied`; missing proof-verifier configuration returns 503 and never falls back to author or development authentication.

## Service configuration

The Apps service must have `APPS_ACCOUNTS_URL` and its own `silicon-apps` credential in `APPS_ACCOUNTS_APP_SECRET` configured to verify proofs with Accounts. Supply the secret through the deployment's secret mechanism. The authoring service token is not a substitute for this app credential. The storage tables are created automatically by migration 006 in the existing Apps SQLite database; include that database in normal backups.
