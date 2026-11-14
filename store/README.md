# Silicon Apps store

The store at `apps.teamofsilicons.com`: a server-rendered Next.js App Router site in front of the Apps API
(`crates/server`). Every page renders its real content on the server and works with JavaScript off. Script is only
used for small islands: copy buttons, the filter row, the carousel buttons and the review character counter.

The store is for finding and using apps. Making an app and adding Silicon Accounts to it live on the developer portal: "Make a Silicon App"
links to https://developers.teamofsilicons.com/docs/apps and "Add Silicon Accounts to your app" to
https://developers.teamofsilicons.com/docs/accounts. There is no app authoring here.

## Pages

**Cleared to the layout (2026-11-14).** At Anil's request every page was emptied down to the account site's layout
(the shell: top bar with search, the section list's column, the page box), to be designed again one page at a time.
The routes below still answer, with their titles, 404s and redirects, but their pages are empty white boxes for now.

| Path | What it is |
| --- | --- |
| `/` | The home page, designed again one element at a time (a page's first element is level with the sidebar's first row). Now (2026-11-14): a banner first (`components/home/hero.tsx`, 3:1, 2:1 on a phone): what every app here shares, one command, as a single white terminal window on the account site's pink and blue gradient (`silicon-apps install briefcase`, its output, `briefcase --help`, and a blinking cursor, still with reduced motion). The earlier banners, a wall of app icons (`icon-hero.tsx`) and two Cosmos pictures cross-fading (`picture-hero.tsx`), are kept in `/playground`, then "Core Apps to Try". The banner carousel (`components/home/banner-carousel.tsx`, its banners in `banners.ts`; the "below" style with the words under the picture, and the "overlay" style with white words on it) is off the page for now and kept in `/playground`. "Core Apps to Try": three apps as the account site's picture cards (`components/parts/picture-cards.tsx`, copied from its Home introduction, without its dismiss button, which needs script), each with its own picture from Cosmos (`public/home/briefcase.webp`, `dm.webp`, `ring.webp`: a frosted glass folder, a pink envelope and for Ring a man taking a call on an old mobile phone (Anil's pick, cosmos.so/e/70297960; a recognisable person, so it also needs a model release before shipping), other people's uploads, so check the rights before shipping) pictures ready for Silicon Accounts, Silicon Apps and Silicon Present (`silicon-accounts.webp`, `silicon-apps.webp`, `present.webp`, same caveat) for when they join the row, or, for any other app, one of the account site's pictures (`public/banner`), the app's name and the first sentence of its description (at most 15 words), the whole card a link to the app. The three are picked by hand (`CORE_APPS` in `app/page.tsx`: Briefcase, DM and Ring). Under it, "Categories" (2026-11-14, `components/home/category-cards.tsx`): the six categories other than Essentials as cards with no box: each one's banner from its own page (`public/categories/{id}.webp`; for AI and Agents `ai-agents.svg`, the Silicon gradient), then its name and an "Explore apps" button to it on one line. "Install an App" and the scrolling "All Apps" list came off the home page (2026-11-14); the app rows (`components/home/all-apps.tsx`: logo, name, first sentence, rating and the systems' marks, `os-icons.tsx`) are still used by search and a category's list view. The first layout's side-by-side feature (`components/home/feature.tsx`) is kept, unused, to compare. A search (`/?q=`, from the sidebar) shows its results instead: "Apps matching …" and the count, any category whose name or line matches as a link, then the matching apps as rows in the API's order, 24 at a time with Show More |
| `/apps` | All Apps (2026-11-14), in the sidebar: every app the visitor can see, organised by category. Under the title and count, chips link to each category (`lib/categories.ts`: Essentials, Communication, Productivity, Business, AI and Agents, Developer Tools, Media, and More Apps for any app in none), each a chapter with its title, line, its first six picture cards (`components/gallery/gallery.tsx`), most installed first, and "See all N", so the page stays an overview however many apps there are. A card is a 16:9 cover (the app's own picture, banner or first picture; else its icon, large, on one of the account site's soft pictures), its icon rising into the cover's corner, its name, one line, and its rating and systems; the whole card is a link. Old `/apps?…` search links still go to `/` (`proxy.ts`) |
| `/categories/{id}` | One category, whole (2026-11-14), each also a row in the sidebar's Categories group with its icon (`components/parts/category-icon.tsx`) and named on its apps' pages: the page bar (All Apps above its title), a picture-only banner (a Cosmos picture per category in `public/categories/`, Silicon UI's gradient for AI and Agents, 5:1, 5:2 on a phone), its line, then a results toolbar: the platform as a dropdown on the left, Sort by as a dropdown and the layout as two icons on the right (dropdowns are `components/parts/choice-menu.tsx`, a native popover of links on a Button, so all of it works without script and each view has an address): Sort by (Most popular; Newest, by latest release; Top rated, apps with at least 3 reviews first), the platform (All platforms, macOS, Linux, Windows) and the layout (Grid view, or List view: the account site's rows), then its apps 24 at a time with Show More (`?page=N`). An unknown category answers 404 |
| `/apps/{app_id}` | One app, on Anil's sketch (2026-11-14; the live store's content and flow in the account site's style): a cover across the column at 3:1 (2:1 on a phone: the app's own picture from `lib/app-pictures.ts`, else its banner, else its first picture, else one of the account site's), the app's icon (96px, 72 on a phone; Anil's icon where there is one) rising into the cover's lower-left corner on a white ring, and beside it, below the cover's edge, the name (display) and "By" its authors, each a link to their page; "Try it" with Open Website (where it has one), the install command with a copy button (`components/app/command.tsx`), and Manage App for its authors; the facts in a row between hairlines (rating, installs, version, platforms as the systems' marks; two by two on a phone); About (the description and tags); then the live store's details as labelled rows between hairlines (Links; Releases, with production and development, their notes, who signed them, withdrawn releases and other ways to install; Available on, each system with its architectures; Authors); then Ratings and reviews (the average, the spread by stars, every review, and Sign in to write a review; the review form itself comes back next). The "By" line carries the authors' pictures and the app's id, and "Get the CLI" sits under the command. Apps the visitor may not see still answer 404 (`proxy.ts` asks the API first) |
| `/authors/{uuid}` | An author's page. Empty. Unknown authors still answer 404 |
| `/settings` | Settings. Empty; the top bar's account button opens it. Sign out (`/sign-out`) has no button on a page yet |
| `/sign-in`, `/sign-out` | Sign in through Silicon Accounts with the API's browser routes (`/v1/auth/login`), sign out with a plain form post |
| `/robots.txt`, `/sitemap.xml`, `/llms.txt`, `/llms-full.txt`, `/.well-known/security.txt`, `/manifest.webmanifest` | Agent and crawler files |
| `/playground` | Development only (a 404 in production builds, never indexed): designs kept to compare: the banner carousel in both styles (words under the picture, and words on it over an overlay) |
| `/og.png`, `/apps/{app_id}/og.png` | Open Graph images (the store's, and one per public app) |

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
`X-Forwarded-For` from the connection (its default), which the API's own rate limits use.

## Environment

Everything is read at request time, never baked into the build, so one build serves any stack.

| Variable | Default | What it is |
| --- | --- | --- |
| `APPS_API_URL` | `http://127.0.0.1:4310` | Where the store server reaches the Apps API, server to server |
| `STORE_PUBLIC_URL` | `https://apps.teamofsilicons.com` | The origin browsers use for the store: the `Origin` sent with a visitor's writes (it must be in the API's `APPS_ALLOWED_ORIGINS`), the same-origin check of the store's own forms, and whether its cookies are `Secure` |
| `PORT` | `3000` (`8710` with `pnpm dev` and `pnpm start`) | The port the server listens on |
| `HOSTNAME` | `0.0.0.0` (`127.0.0.1` with `pnpm dev` and `pnpm start`) | The address the server listens on; set `127.0.0.1` behind Caddy |
| `DEVELOPERS_URL` | `https://developers.teamofsilicons.com` | Where the old `/developer/*` and `/docs/*` addresses redirect |
| `STORE_PAGE_SIZE` | `24` | Apps per batch on the home page (1 to 100), for local checks of "Show more apps" |
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
pnpm lint                   # ESLint with Silicon UI's guardrails, then silicon-ui-check on the CSS
pnpm build
pnpm test:e2e               # needs the API binary above; starts the fixture API on 4510 and the build on 8710
node scripts/screens.mjs    # with both running: home, search and an app page, 320 and 1440 wide, in .screens/
```

The Playwright suite (`e2e/`) checks browsing, search and filters, the app page, signatures and withdrawn releases, the
sign-in redirect through the API to Silicon Accounts, sign-out, writing, editing and removing a review with JavaScript
off, the 600-character limit, private apps signed in and out, 404s, old addresses, no sideways scrolling at 320 pixels,
the agent files, the API paths, that pages register no browser tools, and that `/mcp` answers 404 (the store runs no
MCP server). It reuses servers already listening on those ports outside CI. `E2E_STORE_PORT` and `E2E_API_PORT` change
the ports, and `E2E_DEV=1` tests `next dev` instead of the build.

## The look (2026-11-13)

The store is on the account site's design (silicon-accounts web, branch `compact`), as Silicon Starter and Silicon
Organizations are:

- **Silicon UI from npm**: `@teamofsilicons/silicon-ui@0.5.0`, unpatched (0.5.0 carries the account site's former
  local patch, teamofsilicons/silicon-ui#28 to #40). `@teamofsilicons/silicon-ui/styles.css` loads
  first, then `styles/tokens.css`.
- **Tokens** (`styles/tokens.css`, the account site's, copied through Starter): the system font, the type roles
  (display, h1, h2, h3, body, label, small, caption, hint, each with its `-tracking`), compact spacing and controls (32,
  36, 40), a neutral #FCFCFC page, near-black ink and the blue accent. Light only. Change it with the account site,
  never per screen. `styles/base.css` and `styles/squircle.css` (the native half only) are the account site's too.
- **Shell** (`components/shell`, 2026-11-14): the account site's. From 900px there is no top bar: the sidebar and
  the page (a white box that scrolls on its own) fill the window's height, and the sidebar is the account site's in its
  inset form: the wordmark "Team of Silicons **Apps**" at its head, search under it (a plain GET form to `/`, so Enter
  searches without script), the section list (NavList: Home, more as pages are designed), then at its foot the profile
  across its width (the signed-in account, which opens Settings, or Sign In) over a hairline, and Developers and Docs
  as small grey chips under it. A page without a page bar starts level with the sidebar's search field (`--page-top`).
  Under 900px a top bar holds the mark and the menu button, which opens the same sidebar as a native popover sheet,
  and pages start 24px under it.
- **Page parts** (`components/parts`, kept for the pages to come): the account site's page column (`Page`, 896px, or
  `wide` across 1120px), page bar, sections and rows, and Starter's Box.
- **Guardrails** (`pnpm lint`): Silicon UI's ESLint rules (no raw `<button>`, `<input>`, `<select>`, `<textarea>` or
  `<dialog>`, no borrowed component CSS, no local component copies; NavList and ButtonLink's soft chips opted in as on
  the account site) and `silicon-ui-check`, which keeps spacing, radius, colour, font size and layers on tokens in
  every `*.module.css`. The dev tools (development only) may use Button's and SegmentedControl's experimental sizes.
- **Without script**: the shell renders on the server and its search is a plain GET form, so it works with JavaScript
  off. Pages designed from here must keep that.

- **Dev tools** (`components/dev/`, 2026-11-14): the account site's (silicon-accounts web,
  `components/foundation/dev/`), in `next dev` only: a round flask button at the lower right of every page opens a
  panel. **Look**: Density (Compact, the store's; Comfortable, the account site's earlier roomier scale, to compare),
  Typography (the type lab, ⌥F, `lib/type-lab.ts`, with the account site's faces: Hanken Grotesk, Geist, Inter, the
  system font, Work Sans, TeX Gyre Heros, Liberation Sans, Cooper Hewitt and BDO Grotesk, loaded by `app/fonts.ts`
  on <html> in development only and never preloaded; the local ones are in `assets/fonts` with their licences) and Text edits (`lib/text-edits.ts`: count, Copy CSS, Clear). **Inspect**: Edit text (⌥I) and Spacing
  (⌥S). Everything is kept in this browser (`store.dev.*`) until Reset All. The account site's Data group is left out:
  its states are the account site's own. Its experimental Button and SegmentedControl sizes are allowed in
  `components/dev/**` only (`eslint.config.mjs`).

The Open Graph images (`lib/og.tsx`, BDO Grotesk from `assets/og/`) and the app icons keep the earlier look for now.

Copy on these pages follows the Carbon's voice in `llms/llms.md`: plain words, Carbons and Silicons, and no em or en
dashes anywhere.

## Telemetry (2026-10-11)

The store records no telemetry. It sends no page view events, and every call it makes to the Apps API carries
`X-Apps-Telemetry: off` (`lib/api.ts`, and the proxy's quick reads), so the API records nothing for the store's visitors.
There is no telemetry setting.
