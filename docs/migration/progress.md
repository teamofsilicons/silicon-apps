# Apps account UUID migration progress

## 2026-10-10 — local implementation and verification

Created isolated worktree `.worktrees/apps-uuid`, branch `migrate/accounts-uuid-20261010`, from clean `main` at `480b529`. Original checkout and human-owned `understanding/UNDERSTANDING.md` remain unchanged. No publication, deployment, or production backfill has run.

Implemented the strict persisted Accounts CSV consumer with atomic dry-run/apply/replay, collision/kind/history checks, declared catalog/SQL/JSON references, preserved signing material/artifacts/credentials, browser token invalidation, and an explicit CLI-home session pass. The service adds an immutable UUID ledger and rejects old JWT subjects before reading their profile. [The cutover runbook](account-uuid-backfill.md) inventories every storage surface and the coordinated rollback boundary.

Validation:

- `python3 scripts/test_uuid_backfill.py`: **4 passed**, covering full additive SQL steps, dry-run/apply/replay, conflict rollback, event trigger restoration, custody-independent author/access/review/invite/platform metadata, signing/secret preservation and CLI old-session deletion/new-session replay safety. Evidence `.mig/uuid-backfill-test.log`.
- `cargo test --workspace --locked`: **104 passed**, including an HTTP regression with real Ed25519 JWTs that executes the actual consumer, rejects the retired subject, accepts the new UUID for its private app and retains release signatures/package bytes. Evidence `.mig/full-workspace-tests.log` and `.mig/uuid-http-test.log`.
- Workspace/all-target clippy with warnings denied: pass, `.mig/clippy.log`.
- Consumer against a **backup** of the existing local integration database: **18 apps, 10 mapped accounts, 5 package references and 10 release signature sets preserved**; preview left the original logical SQL dump unchanged, apply succeeded, and replay changed nothing. Evidence `.mig/populated-backfill.log`. The source local database was not changed.

The store author URL validator, browser/client types and vendored Accounts client already accept canonical hyphenated UUID strings; no app-ID or public-key-ID validation was widened. The shared developer portal belongs to the Accounts worktree and is coordinated separately. Await the single Accounts export and coordinated local cutover before applying this consumer to an active shared stack.
