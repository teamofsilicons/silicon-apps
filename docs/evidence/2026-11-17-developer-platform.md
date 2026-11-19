# Developer platform integration — local verification

This record covers predeployment verification on 17 November 2026 across Silicon Apps, Accounts, Organizations, Interfaces, Cells and Contacts. The subsequent production release is recorded in the [deployment and acceptance report](../../deploy/verification-organization-publishing-2026-11-18.md).

## Delivered behavior

- PR #25's tagline is integrated into the model, validation, search, CLI/client, store cards and developer forms. New publications require a single line of at most 60 characters; existing publications remain compatible.
- Organizations own Apps, Interfaces and Cells independently of creators. Central account/team grants cover Create, Read, Edit, Publish and Manage, with additional grants for individual resources. Current membership and product access are checked on requests, retries and background work. Organization draft creation omits server credentials.
- Private organization apps default to members. Audience policies support included/excluded teams and individual Carbons/Silicons. Distribution, storage and events enforce current access.
- The portal uses Silicon Contacts' canonical address book through an explicit connection. People suggestions merge invitation signals across services, rank by `2 × sent + received`, then recency and contact trust, and support fuzzy matching. Saved names/photos and trust editing appear in the pickers. Resources and Cell files have sharing/contact/person filters.
- HTTP, OpenAPI, capability discovery, CLI/client operations and portal documentation describe organization publishing and interface access.
- Managers can delete apps and restore them within 30 days. Distribution and storage stop, Accounts is disabled, IDs remain reserved, and registry synchronization preserves the deletion date. The portal includes confirmation and a deleted-apps list.

## Passed checks

| Component | Validation |
| --- | --- |
| Apps | `cargo test --workspace`: 165 passed, 2 opt-in tests ignored; final CLI experience tests: 7 passed; server binary built; generated OpenAPI matches source |
| Accounts | 15 focused Apps ownership/sync/account-verification and proof-portal integration tests passed; extended deletion export/restore round trip passed |
| Organizations | Workspace tests with local PostgreSQL: 36 passed, 1 opt-in test ignored |
| Interfaces | Server tests with local PostgreSQL and `--include-ignored`: 11 passed |
| Cells | Workspace/test compilation passed; 40 library tests passed, 7 external-service tests ignored; 2 publishing tests passed with local PostgreSQL |
| Contacts | Developer connection/canonical-contact integration test passed with local PostgreSQL; workspace/test compilation passed |
| Developer portal | Typecheck, lint and production build passed; 103 unit tests; 28 selected Chromium flows including mobile organization creation, team grants, saved contact previews, file filters and deletion/recovery |
| Store | Typecheck, lint and production build passed; 7 Chromium app-page checks against an isolated Apps API |

All six repositories pass `git diff --check`. The four vendored publishing-policy sources match the canonical Organizations source (SHA-256 `bfbe425ecf1a6ff4a5f31736082bcfdff0c0e890135c074e25b024f5f89600b7`).

Backend integration tests use isolated local PostgreSQL transactions and fixture Accounts/Organizations HTTP dependencies. Portal browser tests mock BFF responses; store browser tests use an isolated Apps API with development identities. These checks do not constitute authenticated production validation. Full Cells deployment/runner integration and the unrelated Contacts scheduler fixture were not run.

## Review and rollout

The workspace's `.local/developer-platform-2026-11-17/` contains initial source snapshots, patches relative to that initial dirty workspace, a source hash manifest and validation logs. The patches exclude generated build artifacts and do not include unrelated pre-existing edits.

See [organization publishing](../organization-publishing.md) for the API contract, credential behavior, migration filenames, service configuration and rollout order. Accounts, Organizations, Apps, Interfaces, Cells and Contacts must support their respective changes before the new portal is deployed. Organization webhook subscriptions currently name an explicit app ID.
