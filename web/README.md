# Silicon Apps web

The public store and author workspace share one React + Vite application. Arc UI free components are installed as source with their MIT license and registry provenance under `vendor/uiarc/`.

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

Serve `dist/` with history fallback to `index.html`, proxy `/v1` to the service, and configure Accounts redirects on the public origins. The root route resolves to `/developer` on `developer.*`, and `/store` on other hosts. Explicit `/developer` and `/store` paths work locally and on either host.

Browser authentication uses the backend `/v1/auth/login` redirect, `/v1/auth/callback`, `/v1/session`, and `/v1/auth/logout` endpoints. Tokens and app credentials are never stored in browser local storage. The only persisted preferences are theme and telemetry opt-out. One-time app and webhook secrets live in component memory until their dialog closes.

## Screens

- Public/private discovery with backend fuzzy search, pagination, real empty and error states.
- App details, links, media, authors, platforms, release commands, editable account reviews.
- Author workspace, app ID availability, creation and one-time secret display.
- Freely navigable seven-step setup with debounced saves, save/error state, explicit flush before changing steps and management tabs, and a browser exit warning for unsaved work.
- Raw package upload, exact validation output, observed platform reach, development releases and independent production promotion.
- Public/private sharing, domains, links, media uploads and alternative text.
- Accounts-owned webhook configuration, one-time webhook secret display and rotation.
- Author invitations, incoming invitation accept/decline, leave, administrator transfer/removal, app secret rotation, durable history.
- Theme, telemetry opt-out, bug reports, and task-oriented CLI documentation.

No sample listings are included in production. Browser tests stub API responses strictly in the test runner. They verify behavior against the documented HTTP contract, not a deployed Accounts configuration or native package runner.

## Telemetry

Telemetry is enabled by default and posts context to `/v1/telemetry`, where the service delivers to the configured Space Station integration. Events include source, step, progress and route context, with no app form values or secrets. Opting out in Settings prevents subsequent browser telemetry requests.
