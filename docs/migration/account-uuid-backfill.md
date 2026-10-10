# Coordinated Accounts UUID backfill

This is an **offline data migration**, using the one immutable CSV exported by Silicon Accounts. The exact header is `old_uuid,new_uuid,kind`; targets are canonical lowercase UUIDv4 values, kinds are `carbon` or `silicon`, and the mapping is one-to-one. Do not generate another mapping for Apps. Retired subjects are never accepted as authentication aliases.

Stop the Apps API, outbox/subscription workers, and the CLI updater on each affected home. Back up the Apps data directory, SQLite database including its WAL, runtime secrets, and CLI homes together with the coordinated Accounts snapshot. No production action is implicit in these commands.

From the candidate checkout, preview:

```sh
python3 scripts/migrate_account_uuids.py accounts-uuid-export.csv --database /absolute/path/apps.sqlite
python3 scripts/migrate_account_uuids.py accounts-uuid-export.csv --cli-home /absolute/path/home
```

After reviewing the reports, apply the **same file** during the shared cutover outage:

```sh
python3 scripts/migrate_account_uuids.py accounts-uuid-export.csv --database /absolute/path/apps.sqlite --apply
python3 scripts/migrate_account_uuids.py accounts-uuid-export.csv --cli-home /absolute/path/home --apply
```

Repeat the CLI command for each actual configured home; it does not discover or follow a home pointer. Each database transaction and local mapping write is atomic. A replay reports zero new mappings. A changed mapping, duplicate target, chain/cycle, existing target reference, or stored kind mismatch fails without merging. Additive SQL migrations are included in the same transaction, so previews on older schemas also roll back their DDL. Restart the matching candidate service only after all dependent stores have applied the coordinated mapping.

Update every owner UUID in the runtime secret's `APPS_HISTORICAL_APP_IDS` with
that same mapping before restart. For example, `dm:<old-owner>` becomes
`dm:<mapped-owner>` while the app ID stays `dm`. This setting is configuration,
not part of the SQLite backfill. Preserve unrelated secret fields. The server
accepts canonical lowercase UUIDs and legacy IDs during preparation, but never
aliases an old configured owner to a new signed-in identity automatically.

## What moves and what stays

The consumer changes declared account references in:

- Catalog authors/admin, private account access, reviews, invitations, per-account platform registrations, reports, package signer metadata, release withdrawal metadata, history actors and identity-specific history data.
- Subscription owners and author signing-key owners.
- Idempotency owners and identity-bearing response objects, pending-secret owners, and invitation/report outbox references.

Every stored event field and its append-only guard remain unchanged. Events containing mapped actors, recipients or declared identity fields are marked in `event_identity_retirements`; their pending deliveries fail with `account_uuid_migrated`. Event lookup, webhook queue/worker and all JSON/SSE replay feeds exclude them, including after a manual delivery reset. Delivery history, IDs, sequence numbers, timestamps, cursors, attempt counts and original payload bytes remain available for audit. Unrelated canonical-identity events remain deliverable. Consumers refresh the current app catalog after the coordinated restart. The mapping ledger records the historical identity relation. Fingerprints are retained; if a retried request contains a changed account UUID in its path/body, use a new logical operation only after inspecting the old result, since the original idempotency key correctly detects the changed input.

Every declared legacy account reference must occur in the shared export or an already persisted mapping. Unmapped references fail the preview before any data is changed; already canonical accounts are allowed. The platform's `system` and `anonymous` actor literals are preserved, and a mapping that collides with either requires explicit provenance review.

Package and media files, object keys, package/release/app IDs, private signing seeds, public signing keys, author/release signature bytes, webhook signing secrets, application secrets and private secret hashes are unchanged. Account UUIDs are not part of the package/release signature messages. Local installed binaries/manifests, trusted key pins and `.apps/keys` remain unchanged.

Accounts invalidates the old account token families. Affected stored Apps browser sessions are deleted; incomplete browser authorization attempts expire. The explicit CLI-home pass removes its old access/refresh session and retains signing keys and installed data. Fresh new-UUID sessions survive an identical replay. Users sign in again; no refresh result is fabricated. The server denies every old subject listed in its mapping ledger even if a stale JWT is still correctly signed. The Accounts developer portal's own session invalidation belongs to the Accounts cutover.

The Apps identity, rate-limit, runner-probe and SDK caches are memory-only and disappear with the stopped service. There are no persisted account-bound Briefcase proofs. Apps server/client models use strings and its store author route already accepts hyphenated 36-character UUIDs; public app IDs and key IDs retain their existing formats.

## Verification and recovery

Run `python3 scripts/test_uuid_backfill.py`, then `cargo test --workspace --locked`. The HTTP regression invokes the actual consumer against a real private catalog and tests a correctly signed old JWT (401), a fresh canonical UUID JWT (200), and preserved signed releases/package bytes. Verify the new account's authorship, private access, reviews, invitation acceptance, event feeds and subscription ownership after coordinated restart. Existing author private keys must still sign successfully; installed artifacts must remain usable.

Before accepting new traffic, rollback requires restoring **Accounts and every dependent Apps database/home from the same coordinated snapshot**. Do not invert the CSV against a live store or restore only Apps. Keep the immutable mapping ledger for replay protection and audit.
