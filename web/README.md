# Silicon Apps web

This React + Vite application serves the app store at `apps.teamofsilicons.com`. Creation and management belong to the shared Apps and Accounts Next.js frontend in `silicon-accounts/developer`, hosted at `developers.teamofsilicons.com`. The store never mounts the author workspace. Arc UI free components are installed as source with their MIT license and registry provenance under `vendor/uiarc/`.

## Run

```sh
npm ci
npm run dev
```

The UI runs at http://127.0.0.1:4311. Vite proxies `/v1` to the Apps service at http://127.0.0.1:4310. Override the proxy with `APPS_API_PROXY`; a separately hosted API can be set with `VITE_APPS_API_URL` at build time, with matching credentialed CORS and cookie settings on the server. Same-origin deployment is preferred.

```sh
npm run build
npm run test:e2e
```

Serve `dist/` with history fallback to `index.html`, proxy `/v1` to the service, and configure Accounts redirects on the store origin. The build also copies the reviewed `scripts/install.sh` and `scripts/install.ps1` into `dist/` for same-origin installer downloads; the scripts' network installation requires published release artifacts. The root route resolves to `/store`. Legacy `/developer` routes redirect to the plural shared portal, preserving app IDs and setup steps. Set `VITE_DEVELOPERS_URL` at build time when testing a local shared portal.

Store authentication uses the backend `/v1/auth/login` redirect, `/v1/auth/callback`, `/v1/session`, and `/v1/auth/logout` endpoints. Tokens and app credentials are never stored in browser local storage. The only persisted preferences are theme and telemetry opt-out. The shared developer portal retains its Accounts sign-in and sealed server-side session.

## Screens

- Public/private discovery with backend fuzzy search, pagination, real empty and error states.
- App details, links, media, authors, platforms, release commands, editable account reviews.
- Theme, telemetry opt-out and bug reports. Documentation lives in the shared portal at `https://developers.teamofsilicons.com/docs/apps`; legacy store `/docs` routes redirect there.

The shared portal owns creation, seven-step publishing setup, packages, releases, access, media, Accounts configuration, webhooks, authors, invitations and audit history. Developer management tests were moved to `silicon-accounts/developer/e2e`; this project's tests cover store behavior and the boundary between the two origins.

No sample listings are included in production. Browser tests stub API responses strictly in the test runner. They verify behavior against the documented HTTP contract, not a deployed Accounts configuration or native package runner.

After domain cutover, check the actual compiled site and same-origin API without fixtures:

```sh
node scripts/verify-production.mjs https://apps.teamofsilicons.com
```

This requires the development dependencies and Chrome used by Playwright. It verifies the store landing route, anonymous private-view prompt, external developer links and redirects, docs, CSP, API responses, static cache headers, missing-asset behavior and the exact hosted installer bytes. It reads live data without publishing apps or installing software.

## Telemetry

Telemetry is enabled by default and posts context to `/v1/telemetry`, where the service delivers to the configured Space Station integration. Events include source, step, progress and route context, with no app form values or secrets. Opting out in Settings prevents subsequent browser telemetry requests.
