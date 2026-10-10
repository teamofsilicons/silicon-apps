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

## Signed event retirement and complete-map preflight

The final consumer preserves every stored signed event field byte-for-byte and never disables its append-only trigger. Additive migration005 keeps retirement metadata separately. Affected pending deliveries fail explicitly, and event lookup, queueing, webhook worker, JSON and SSE replay exclude retired events. Canonical unrelated events remain available; consumers refresh current catalog state after cutover. The consumer rejects any unmapped legacy account reference before updating data.

The actual consumer/HTTP/receiver regression proves original event bytes, secret preservation, old queue cancellation, manual queue-reset resistance, a valid signed fresh-UUID delivery, advancing empty-page cursors and an SSE replay containing only the new event. Full workspace **105 tests pass** (`.mig/final-uuid-fence-tests.log`), plus the final SSE extension (`.mig/event-retirement-test.log`), **5 Python migration tests** (`.mig/uuid-final-backfill-test.log`) and workspace/all-target clippy (`.mig/uuid-final-fence-clippy.log`). Earlier event-row rewrite behavior is superseded by retirement; no production or original local database had that earlier consumer applied.

## Coordinated local UUID cutover — verified

The older local Apps integration database was preserved untouched: nine cached identities match current Accounts, but one short ID belongs to a different kind in the current namespace. It was not rekeyed with an unrelated map. A separate isolated fixture used a real Accounts `developer` token and the real local service integration to create an app, then produced an actual API-signed Ed25519 release. Its archive was seeded after structural inspection; this fixture proves authentication, signing and retention, not isolated package-runner acceptance. The opt-in `uuid_cutover_fixture` test documents and repeats that controlled setup with an explicitly supplied empty directory and local token files.

Dry-run/apply/replay of the final 211-row map passed (SHA256 `750423f3117e11f5eb42025b457ff0f1bf42bd463c31b7e106d9cd10978ca4d9`). The owner and author were mapped; three old signed events were retired without altering historical bytes. Package bytes, release manifest/signature and signing key stayed identical. Replay left the logical SQLite dump unchanged; independent Ed25519 verification of the retained release passed.

The old JWT returned 401 after restart. Fresh real developer sign-in accessed the same private catalog and authenticated package download. A real app PATCH synchronized its canonical UUID owner and author into Accounts through the internal service API. Sanitized evidence: [uuid-cutover.json](evidence/uuid-cutover.json); detailed local evidence `.mig/cutover-verified.json` and `.mig/retained-signature-verification.json`. The new opt-in test and targeted strict clippy passed; full workspace 105-test and five Python migration-test results above remain applicable.

Rebuilt/repacked `.mig/candidate-uuid/apps-0.2.0-macos-aarch64.tar.gz`, SHA256 `dc13f05c2c7832602c18c530e6b754000b9efff0463694a6b029f15373431e6a`. The release verifier ran the three required discovery commands, archive validation and bundled docs in an empty home; login status was signed out. Production, original checkouts and human-owned UNDERSTANDING remain untouched. Final shutdown/restart inventory is in shared `.migration/cutover/services-waveform-mcport-apps.md`.
