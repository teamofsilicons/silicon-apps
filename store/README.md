# Silicon Apps store

The store at `apps.teamofsilicons.com`: a server-rendered Next.js App Router site in front of the Apps API
(`crates/server`). Every page renders its real content on the server and works with JavaScript off. Script is only
used for small islands: the theme switch, copy buttons, the carousel buttons and the review character counter.

The store is for finding and using apps. Making an app and adding sign-in live on the developer portal: "Make an app"
links to https://developers.teamofsilicons.com/docs/apps and "Add sign-in" to
https://developers.teamofsilicons.com/docs/accounts. There is no app authoring here.

## Pages

| Path | What it is |
| --- | --- |
| `/` | What Silicon Apps is, search, featured, popular and new apps, tags, how to get the CLI, what Silicons can call, questions |
| `/search?q=&tag=&target=&visibility=&page=` | Search and filters, one plain GET form. Signed out, `visibility=private` says `Log in to see private apps` |
| `/apps/{app_id}` | Everything about one app: banner, logo, carousel, description, tags, links, authors, platforms, releases, signatures, withdrawn releases, rating, installs, `silicon-apps install {app_id}` with a copy button, reviews and the review form |
| `/authors/{uuid}` | An author and their published apps |
| `/settings` | Session (sign in or out), appearance, telemetry, report a problem |
| `/sign-in`, `/sign-out` | Sign in through Silicon Accounts with the API's browser routes (`/v1/auth/login`), sign out with a plain form post |
| `/robots.txt`, `/sitemap.xml`, `/llms.txt`, `/llms-full.txt`, `/.well-known/security.txt`, `/manifest.webmanifest` | Agent and crawler files |
| `/mcp` | MCP server (Streamable HTTP, stateless): `search_apps`, `get_app`, `list_releases`, `get_install_command`, `list_reviews`; 60 requests a minute per address, then 429 with `Retry-After` |
| `/og.png`, `/apps/{app_id}/og.png` | Open Graph images (the store's, and one per public app) |

Every page has its own title, description, canonical link, Open Graph and Twitter tags, and JSON-LD: `Organization` and
`WebSite` (with a `SearchAction`) everywhere, `SoftwareApplication` and `BreadcrumbList` on app pages, `ProfilePage` on
author pages, `CollectionPage` or `SearchResultsPage` on search, `FAQPage` on the home page.

Reviews are 1 to 5 stars and up to 600 characters, one per account per app. The form is a server action behind a plain
form, so it works without script. Apps and authors the visitor cannot see answer a real 404 page rendered on the server
(`proxy.ts` checks with the API first, with the visitor's own session).

Old store addresses keep working: `/store` and `/store?q=` go to `/` and `/search`, `/store/{app_id}` to
`/apps/{app_id}`, `/apps` to `/search`, and `/developer/*` and `/docs/*` to the developer portal.

`llms.txt` and `llms-full.txt` are both `llms/llms.md`, served exactly as written (bundled at build time by
`scripts/build-llms.mjs`). That file is the Carbon's own: never edit it from code.

## What Caddy answers before the store

In production Caddy sends these paths to the Apps API (or serves them from disk) before they reach the store:

- `/v1/*`, including the SSE streams (`/v1/events/stream`, `/v1/apps/{app_id}/events/stream`) and the browser sign-in
  routes (`/v1/auth/login`, `/v1/auth/callback`, `/v1/auth/logout`)
- `/health`, `/openapi.json`
- `/.well-known/agent.json`, `/.well-known/agent-card.json`, `/.well-known/silicon-apps-keys.json`
- `/install.sh` and `/install.ps1` (served from disk)

Everything else goes to the store. The store itself calls the API server to server at `APPS_API_URL`. Under `next dev`,
or with `STORE_DEV_API_PASSTHROUGH=1`, `proxy.ts` forwards the API paths above to `APPS_API_URL` itself, standing in for
Caddy. Never set it in production.

The browser's session is the API's HttpOnly `apps_session` cookie on this origin. The store forwards it to the API with
the visitor's own requests and never shows it to page scripts. Caddy must keep the `Host` header and set
`X-Forwarded-For` from the connection (its default), which the MCP rate limit and the API's own limits use.

## Environment

Everything is read at request time, never baked into the build, so one build serves any stack.

| Variable | Default | What it is |
| --- | --- | --- |
| `APPS_API_URL` | `http://127.0.0.1:4310` | Where the store server reaches the Apps API, server to server |
| `STORE_PUBLIC_URL` | `https://apps.teamofsilicons.com` | The origin browsers use for the store: the `Origin` sent with a visitor's writes (it must be in the API's `APPS_ALLOWED_ORIGINS`), the same-origin check of the store's own forms, and whether its cookies are `Secure` |
| `PORT` | `3000` (`8710` with `pnpm dev` and `pnpm start`) | The port the server listens on |
| `HOSTNAME` | `0.0.0.0` (`127.0.0.1` with `pnpm dev` and `pnpm start`) | The address the server listens on; set `127.0.0.1` behind Caddy |
| `DEVELOPERS_URL` | `https://developers.teamofsilicons.com` | Where the old `/developer/*` and `/docs/*` addresses redirect |
| `STORE_DEV_API_PASSTHROUGH` | unset | `1` forwards the API's paths to `APPS_API_URL` (local checks only; `next dev` always does) |

Canonical links, Open Graph, JSON-LD and the sitemap always name `https://apps.teamofsilicons.com`.

## Run it locally

Requirements: Node.js 24 or newer and pnpm 10. From this directory:

```sh
pnpm install
pnpm dev                    # http://127.0.0.1:8710, API at APPS_API_URL (default http://127.0.0.1:4310)
```

With no Apps API running, pages say the catalog is not answering. For a full local stack with a fixture catalog,
build the API from the repository root, then start the fixture API on port 4510 and the store against it:

```sh
CARGO_TARGET_DIR=target/integration cargo build -p silicon-apps-server --bin apps-server   # repository root
node scripts/e2e-api.mjs                                                                   # store/, keeps running
APPS_API_URL=http://127.0.0.1:4510 STORE_PUBLIC_URL=http://127.0.0.1:8710 pnpm dev            # store/, another terminal
```

`scripts/e2e-api.mjs` wipes and re-creates `../.dev/store-e2e-fixture`, runs the API with `APPS_DEV_AUTH=1` (fixture
accounts, loopback only), seeds it with `scripts/seed-fixture.mjs` (public, private and draft apps, signed and
withdrawn releases, reviews, pictures) and restarts it so it signs the seeded releases. To browse as a fixture account,
set the cookie `apps_session=e2e-mira-session` (c:mira, who can see the private Team Vault), `e2e-nova-session`
(si:nova) or `e2e-ada-session` (c:ada).

## Build and run in production

```sh
pnpm install --frozen-lockfile
pnpm build
```

`pnpm build` bundles `llms/llms.md`, runs `next build` with `output: "standalone"` and copies `public/` and
`.next/static` into `.next/standalone` (`scripts/standalone.mjs`). That directory is the whole server: copy it
anywhere with Node.js 24 and start it with

```sh
cd .next/standalone
NODE_ENV=production PORT=8710 HOSTNAME=127.0.0.1 \
APPS_API_URL=http://127.0.0.1:4310 STORE_PUBLIC_URL=https://apps.teamofsilicons.com \
node server.js
```

`pnpm start` does the same from this directory. Caddy then proxies everything except the API paths above to
`127.0.0.1:8710`.

## Check it

```sh
pnpm typecheck
pnpm lint
pnpm build
pnpm test:e2e               # needs the API binary above; starts the fixture API on 4510 and the build on 8710
node scripts/screens.mjs    # with both running: home, search and an app page, light and dark, 320 and 1440 wide, in .screens/
```

The Playwright suite (`e2e/`) checks browsing, search and filters, the app page, signatures and withdrawn releases, the
sign-in redirect through the API to Silicon Accounts, sign-out, writing, editing and removing a review with JavaScript
off, the 600-character limit, private apps signed in and out, 404s, old addresses, no sideways scrolling at 320 pixels,
the agent files, the API paths, that pages register no browser tools, and the MCP server (tools, errors, SSE answers,
bearer tokens and the rate limit). It reuses servers already listening on those ports outside CI. `E2E_STORE_PORT` and `E2E_API_PORT` change the
ports, and `E2E_DEV=1` tests `next dev` instead of the build.

Copy on these pages follows the Carbon's voice in `llms/llms.md`: plain words, Carbons and Silicons, and no em or en
dashes anywhere.
