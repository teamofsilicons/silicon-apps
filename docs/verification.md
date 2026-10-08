# Local verification — 8 October 2026

This is the initial local implementation phase. Later deployment work and its remaining gates are recorded in [deployment status](deployment-status.md); the observations below retain their original scope.

This records observed implementation evidence. No public deployment, registry publication, or production data migration was performed.

## Automated checks

- `cargo fmt --all --check` and strict workspace Clippy pass.
- `cargo test --workspace --locked`: **52 tests pass** across package, client, CLI error contracts, install/update behavior, domain rules and HTTP integration. This includes a real Unix updater executable handoff that preserves its PID, all-error manifest validation, and all nine archive target mappings.
- Frontend TypeScript/Vite production build passes. **27 browser tests pass**, covering setup/resume, saves while typing, failed-save and browser Back recovery, retry idempotency, one-time secrets, author/admin/invite flows, release/publish, reviews, media/link rendering, telemetry opt-out, keyboard dialogs and four viewport widths (390, 768, 1024, 1440 pixels). All 26 installed UIArc source files match the retained registry payloads; the MIT notice is present. All 39 website CLI examples were checked against the actual CLI parser.
- **12 runner tests pass**, including real Apple Silicon sandbox execution, denial of process creation, private-file reads, host writes and networking; gateway credential/routing/output/redirect checks; Windows Hyper-V argument construction; and readable nested package directories despite the service's restrictive umask.
- The supporting Silicon Accounts apps, client and OAuth test suites pass, including author membership, signed update subscriptions, private mail, audience restrictions and consent-gated verified secondary emails.

The browser behavior suite uses controlled API fixtures. The following integration checks used the actual services.

## Actual local service flow

The isolated stack used the Apps API on 4310, frontend on 4311, isolated runner on 4312, Accounts site/API on 8790/8789, and its own `silicon_apps_integration` PostgreSQL database. Authentication used Accounts-issued signed tokens with development authentication disabled in Apps.

1. Imported all 16 existing Accounts registry entries without changing app IDs or Accounts memberships, including the legacy two-character `dm` identifier.
2. Completed browser PKCE sign-in and consent through Accounts and returned to the signed-in Apps store. Completed CLI short-lived-token login separately.
3. Created Local Orbit, packed its actual executable, uploaded it, and observed all three commands execute successfully in the native sandbox.
4. Created development 1.0.0, promoted it to independent production 2.0.0, and published the app immediately.
5. Installed anonymously into an isolated home, verified the command ran, and recorded its successful install. The detached daemon survived the command session and automatically upgraded production 2.0.0 to 2.1.0 on its minute interval.
6. Created, updated and removed one account review. Switching the app to private made anonymous details, releases, reviews, resolution and package downloads return 404 and removed it from search. Restored public access afterward.
7. Generated a webhook secret before configuring an endpoint; saving the endpoint and subscriptions preserved it. The UI showed the saved configuration without redisplaying the secret.
8. Built the optimized Apps 0.1.2 package, uploaded it through the final native sandbox, passed all three discovery commands, and created development and production 0.1.2 in the local store. Previous local releases 0.1.0 and 0.1.1 remain immutable.
9. Verified checksum failure rejects bootstrap before creating local state. A fresh-home 0.1.2 bootstrap registered Apps itself with its registry origin, started the updater, and self-uninstalled cleanly. Tests used `--no-startup`; no OS startup service was installed.
10. Rebuilt and opened the real UIArc-based store and author views against live data. The local preview remains available at http://127.0.0.1:4311/store and http://127.0.0.1:4311/developer.
11. Created a Silicon through the real isolated Accounts service and signed in through the official-client CLI flow. Verified pending invite invisibility, acceptance, equal detail-edit rights, rejection of non-admin visibility changes, admin transfer and private access, leaving, and last-author rejection. An accepted Silicon co-author generated an Accounts-owned webhook secret and read its configuration without redisplaying the secret. The private `author-audit` draft retains its original Carbon author afterward.

The final audit and source/test mapping are in [requirements coverage](requirements-coverage.md). Fixes include account-rename-aware invitations, typo matching across tags/descriptions, rejecting malformed mutation routes before external effects, reviewer deletion after visibility loss, updater self-reload, JSON error contracts, complete validation error reports, Windows archive path normalization, and setup/review UI completeness.

## Artifact

`dist/apps-0.1.2-macos-aarch64.tar.gz` (3,956,874 bytes)

SHA-256: `478c3c21317e553eab6bb2b647ece9bd0cb2e2a11cdcf8d0b7fa42ec5fe9936b`

The Rust client is buildable directly from this self-contained workspace. Its official Accounts dependency is vendored with source provenance. This task did not publish either client to crates.io.

## External acceptance still required

- Provision and execute native Linux, Windows and Intel macOS target workers. All nine targets have manifest/install/routing support; unavailable workers reject validation. Hyper-V execution and Windows self-update code have structural tests but have not run on a Windows host here.
- This Mac's Docker engine did not answer its local socket health probe. Windows cross-compilation stopped in cryptography dependencies because Windows SDK headers are unavailable. Neither attempt is reported as target execution evidence. Native macOS/Windows client jobs and the worker matrix are included in CI, but no remote workflow was run.
- Verify OS startup/reboot behavior on the intended deployment platforms. The real local updater process and timed automatic update were tested; permanent OS service registration was intentionally kept out of the isolated test environment.
- Configure production Accounts redirects, both public origins, DNS/TLS, private service credentials, mail delivery and a Space Station table. Mail and telemetry adapters were exercised through tests/local providers; no real-recipient mail or live Space Station table was used.
- Run remote CI and publish the desired release artifacts/crates, then deploy and verify the public domains. Deployment templates and operating instructions are in `docs/operations.md` and `deploy/`.

Supporting Accounts changes are committed locally as `00f98860ef42471434f22f019639b8a97830f209` and `ebe4b94`; its human-owned requirements were not edited.
