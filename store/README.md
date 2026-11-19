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
| `/` | The home page, designed again one element at a time (a page's first element is level with the sidebar's first row). Now (2026-11-14): a banner first (`components/home/hero.tsx`, 3:1, 2:1 on a phone): what every app here shares, one command, as a single white terminal window on the account site's pink and blue gradient (`silicon-apps install briefcase`, its output, `briefcase --help`, and a blinking cursor, still with reduced motion). The earlier banners, a wall of app icons (`icon-hero.tsx`) and two Cosmos pictures cross-fading (`picture-hero.tsx`), are kept in `/playground`, then "Core Apps to Try". The banner carousel (`components/home/banner-carousel.tsx`, its banners in `banners.ts`; the "below" style with the words under the picture, and the "overlay" style with white words on it) is off the page for now and kept in `/playground`. "Core Apps to Try": three apps as the account site's picture cards (`components/parts/picture-cards.tsx`, copied from its Home introduction, without its dismiss button, which needs script), each with its own picture from Cosmos (`public/home/briefcase.webp`, `dm.webp`, `ring.webp`: a frosted glass folder, a pink envelope and for Ring a man taking a call on an old mobile phone (Anil's pick, cosmos.so/e/70297960; a recognisable person, so it also needs a model release before shipping), other people's uploads, so check the rights before shipping) pictures ready for Silicon Accounts, Silicon Apps and Silicon Present (`silicon-accounts.webp`, `silicon-apps.webp`, `present.webp`, same caveat) for when they join the row, or, for any other app, one of the account site's pictures (`public/banner`), the app's name and the first sentence of its description (at most 15 words), the whole card a link to the app. The three are picked by hand (`CORE_APPS` in `app/page.tsx`: Briefcase, DM and Ring). Under it, "Categories" (2026-11-14, `components/home/category-cards.tsx`): the six categories other than Essentials as cards with no box: each one's banner from its own page (`public/categories/{id}.webp`; for AI and Agents `ai-agents.svg`, the Silicon gradient), then its name, the whole card one link to it. "Install an App" and the scrolling "All Apps" list came off the home page (2026-11-14); the app rows (`components/home/all-apps.tsx`: logo, name, first sentence, rating and the systems' marks, `os-icons.tsx`) are still used by search and a category's list view. The first layout's side-by-side feature (`components/home/feature.tsx`) is kept, unused, to compare. A search (`/?q=`, from the sidebar) shows its results instead: "Apps matching …" and the count, any category whose name or line matches as a link, then the matching apps as rows in the API's order, 24 at a time with Show More |
| `/apps` | All Apps (2026-11-14), in the sidebar: every app the visitor can see, organised by category. Under the title and count, chips link to each category (`lib/categories.ts`: Essentials, Communication, Productivity, Business, AI and Agents, Developer Tools, Media, and More Apps for any app in none), each a chapter with its title, line, its first six app cards (`components/gallery/gallery.tsx`), most installed first, and "See all N", so the page stays an overview however many apps there are. The cards (2026-11-17, also a category's grid view) are built for scanning: as many columns as fit at 20rem or more (three across a wide page, one on a phone) of open rows with no box or cover, each the app's icon, its name (base size, regular weight, primary colour), its tagline (small, soft) (the authors' own line for cards, at most 60 characters, a new API field on branch `claude/app-tagline-369121`; until an app has one, the first sentence of its description, whole and wrapping as it needs, never cut off), a hairline under each; the whole row is a link. Ratings and the systems' marks are left off the cards for now (most apps have no rating yet). (Until 2026-11-17 each card had a 16:9 cover, three across, which Anil found distracted from the app.) Old `/apps?…` search links still go to `/` (`proxy.ts`) |
| `/categories/{id}` | One category, whole (2026-11-14), each also a row in the sidebar's Categories group with its icon (`components/parts/category-icon.tsx`) and named on its apps' pages: the page bar (All Apps above its title), a picture-only banner (a Cosmos picture per category in `public/categories/`, Silicon UI's gradient for AI and Agents, 5:1, 5:2 on a phone), its line, then a results toolbar: the platform as a dropdown on the left, Sort by as a dropdown and the layout as two icons on the right (dropdowns are `components/parts/choice-menu.tsx`, a native popover of links on a Button, so all of it works without script and each view has an address): Sort by (Most popular; Newest, by latest release; Top rated, apps with at least 3 reviews first), the platform (All platforms, macOS, Linux, Windows) and the layout (Grid view, or List view: the account site's rows), then its apps 24 at a time with Show More (`?page=N`). An unknown category answers 404 |
| `/apps/{app_id}` | One app. From `main` (merged 2026-11-19), fitted to the redesign: Build an interface after Compatibility (`components/app/interface.tsx`, only where the app publishes any: the interface API docs, each use case's Markdown files in folds with Download, the interface license), and the developer card lists the publishing organization and author organizations first (`organizationsOf`, not links yet). Signed-in visitors get live catalog changes (`components/shell/live-events.tsx`, in the root layout). The rest: Cleared on 2026-11-18 to be designed again one element at a time (Anil). Laid out as Sidebar (`components/app/app-layout.tsx`, picked in `/playground/app-page` on 2026-11-18, after npm, GitHub and the VS Code Marketplace): the header short (icon, name, tagline); then a reading column (screenshots, About, What's new, Compatibility, reviews, half again a page's room apart) and a 20rem sidebar beside it kept in view as you scroll (Open Website and the install command, the facts one under another, the developer card); More by across the foot; under 1024px one column with the sidebar first. The parts below are described as they were built; the sidebar takes the header's actions and facts and Developer's card. So far the header, Editorial (`components/app/app-header.tsx`, picked in `/playground/app-header`): no box and no ground; the icon (72px, 56 on a phone) on the left with the name (display) and the tagline (else the description's first sentence) as a lede (h2's size, regular) beside it, and on the right of that row (under it when narrow) Open Website (where it has one), the install command with a copy button, Manage App for its authors, and a line saying what it installs and Get the CLI; then a hairline and labelled facts across the page: Developer (each a pill with their picture and name, a link to their page), Category (a pill), Version, Systems (the marks and names) and Signed by (Silicon Apps, and the developer where they signed too; a link to how signing works; left out until there is a release). Rating and installs are left out while most apps have none. Then the screenshots (`components/app/screenshots.tsx`, 2026-11-18): the carousel as one strip that scrolls sideways and snaps, each picture or video in a frame by the screen it shows, desktop 16:10 or mobile 9:19.5, every frame the same height (320px, 210 on a phone) so a mix lines up, the picture cropped to its frame; which frame comes from each item's `form` (`desktop` or `mobile`, proposed for the API and not there yet, so every item is a desktop frame until it is); no section when there are none (no app in the catalog has any yet; `/playground/app-header` shows stand-ins). Then About (`components/app/about.tsx`): the description at 70ch and a 1.5 line height (`--leading-read`), a paragraph per blank line, and the tags as pills, each a search for that tag. Then What's new (`components/app/whats-new.tsx`, Store, picked in `/playground/whats-new` on 2026-11-18, after the App Store's): "Version 5.3.1" and how long ago, the notes cut to three lines with "more" (a details, so no script), then folds between hairlines, each title after Lucide's triangle (right, down when open): Version history (every release from `GET /v1/apps/{id}/releases`, `lib/releases.ts`), Withdrawn releases (where there are any) and Other ways to install (the development channel, saying when it is newer than production, and this exact version). No section until there is a release. Then Compatibility (`components/app/compatibility.tsx`, 2026-11-18): a line saying the CLI picks the right build, then a row per system (macOS, Linux, Windows) between hairlines, its mark and name (greyed when the app has nothing for it) and every processor the store builds for on it as chips: filled with a green check when the app has a package, faint and dashed when not; under a row, the developer's public reason for a required target the release leaves out (`unsupported_targets`). Then Developer (`components/app/developer.tsx`, Profile, picked in `/playground/developer` on 2026-11-18, after Google Play's About the developer and GitHub's sidebar): "Developer" (or "Developers"), then two columns (one when the section is under 44rem wide, a container query): a card on a soft glow (the accent's wash fading into the raised surface, with a hairline), with each author linked to their page (picture, name over id and how many apps they have published) and under a hairline the links (website, developer docs, store pages and the authors' own; each its icon, label over address, in a new tab); beside it More by the developer as a compact list in columns (a 36px icon, the name over one line), every author's together, at most six, with See all to the author's page when there is one author (`GET /v1/authors/{uuid}`, `lib/developers.ts`). Then Ratings and reviews (`components/app/reviews.tsx`, 2026-11-18): the average large with its stars and how many ratings, beside a bar for each star count; after a write, a notice (saved, removed, or what went wrong, from `?review=` and `&code=`); signed out, "Used X? Sign in to write a review"; signed in, one row, "Write a review" (or "Edit your review"), that opens into the review form (a details, open by itself after an error; server actions in `app/apps/[appId]/actions.ts`, plain forms that work without script; the app id and an idempotency key bound to the action): five stars to pick (native radios drawn as Lucide stars), the words with Silicon UI's Textarea and how many of the 600 characters are left, Post Review, or "Edit your review" holding theirs with Save Changes and "Remove your review" under it; then the reviews, theirs first and marked You, the rest newest first as cards on the quiet ground, as many columns as fit at 18rem: stars, how long ago, the words, the reviewer's id; the newest six, the rest behind "N more reviews". No app has a review yet, so `/playground/app-header` shows eight made-up stand-ins, with `?as=` for signed out, signed in, or signed in with a review (a preview form that never posts, as the playground reads the live catalog). The page before (cover, Try it, facts, About, details, reviews) is in git at `e7748ac`; its styles (`components/app/app-page.module.css`) are kept for reuse. Apps the visitor may not see still answer 404 (`proxy.ts` asks the API first) |
| `/authors/{uuid}` | A developer's page (2026-11-18), where the app page's developer card and More by's See all lead, built like the app page's header: their picture (72px, 56 on a phone), name (display), id and a Carbon or Silicon badge; a hairline, then the labelled facts across the page: Apps, Installs (all their apps' together), Systems (as chips), Works with (everyone they publish with, most shared apps first, each a chip linked to their page) and Publishing since (when they first became an author of one of these apps); then their apps in the store's grid, most installed first (`GET /v1/authors/{uuid}?limit=100`). Unknown authors, and authors with nothing visible, answer 404 |
| `/settings` | Retired (2026-11-19): a permanent redirect to `/`. What it held is in the sidebar: the signed-in account opens the account menu (`components/shell/account-menu.tsx`, a native popover above the button: picture, name and "c:ada · Carbon", Manage your account on Silicon Accounts, Sign Out, a form post to `/sign-out` that comes back to the same page), and Report a problem sits in the sidebar's foot beside Developers and Docs, opening a dialog (`components/shell/report.tsx`, a native popover; posted by `components/shell/report-action.ts`, the same report as `silicon-apps report`; with an optional screenshot, `components/shell/report-screenshot.tsx`: a "Screenshot (optional)" field like the others (no box of its own) with Take Screenshot of This Page (the browser's screen sharing, this tab first; the dialog steps aside, one frame is kept and sharing stops) or Attach Image, then a preview with Retake and Remove; PNG, JPEG or WebP up to 5 MB, sent as `screenshot` {name, content_type, data (base64)}, which the API must accept before it reaches anyone (server actions allow 6 MB, `next.config.ts`); without script a plain file field; back to the page with `?report=`, shown as a notice at the top of the page). Both work without script. External rows (`LinkItem external`) end in an arrow instead of the chevron |
| Not found, error | The 404 (`app/not-found.tsx`, 2026-11-19) is a small gallery wall (`components/parts/art-wall.tsx`), a way to show art the Team likes: on the quiet ground, a piece from `lib/art.ts` hung whole on a white mat with a long soft shadow, beside the note ("404 · Not here", "This page stepped out of frame.", what happened, Go Home, Browse All Apps, Report a Problem) and under it a museum-style wall label crediting the artist with links to the work and their site. Now: Austin Augie, *Untitled I*, Japan, 2025 (`public/art/austin-augie-untitled-i.jpg`, from austinaugie.nyc). Add a piece with its picture in `public/art/` and a line in `lib/art.ts`. The error page (`app/error.tsx`, on `components/parts/status-page.tsx`) is a centred column: an icon tile, "Error" over the h1 ("This page couldn’t load"), a line, Try Again, Go Home and Report a Problem, and the digest as a reference to quote, never the message. `/playground/error` throws on purpose to show it |
| `/sign-in`, `/sign-out` | Sign in through Silicon Accounts with the API's browser routes (`/v1/auth/login`), sign out with a plain form post |
| `/robots.txt`, `/sitemap.xml`, `/llms.txt`, `/llms-full.txt`, `/.well-known/security.txt`, `/manifest.webmanifest` | Agent and crawler files |
| `/playground` | Development only (a 404 in production builds, never indexed): designs kept to compare: the banner carousel in both styles (words under the picture, and words on it over an overlay) |
| `/playground/app-header` | Development only: the app page's header (Now · Editorial) beside B · Cover, the two kept of four designs tried on 2026-11-18 (the account site's lab way; Spotlight, a centred gradient panel, and Install card, a buy box on the right, were dropped and are in git), on a real app (`?app=`, Ring by default; Browser has no picture, so Cover shows its gradient), with tabs (`?v=now`, `b`) and, for Now, four ways to show the developer (`?dev=chips`, the page's; `profile`, a 36px picture with the name over the id; `faces`, the pictures overlapping with the first name and how many others; `plain`, a picture and the name with no box). Plain links, so it works without script. The shared pieces are in `components/app/header-lab/parts.tsx`, Cover in `headers.tsx` |
| `/playground/whats-new` | Development only: the app page's What's new (Now · Store, picked 2026-11-18, after the App Store) beside the other three designs tried after similar works, on a real app's release history (`?app=`, Briefcase by default): B · Timeline (after GitHub Releases and Linear's changelog: a dated rail, the three newest, the rest behind Show all), C · Notes window (after Sparkle on the Mac: versions down the left, the chosen one's notes on the right, radio buttons so it needs no script), D · Pulse (after the Chrome Web Store and npm: "Updated today", how many releases since when, a dot per release in time, then the latest notes). Tabs `?v=now`, `b`, `c`, `d`; plain links. The other designs are in `components/app/whats-new-lab/` |
| `/playground/developer` | Dev only (a 404 in production builds): the app page's Developer (`?v=now`, Profile, with `?card=` glow (the page's), raised, outline or tint for the card's ground) beside the other designs on a live app (`?app=`, Briefcase by default): A Grid (the page's first: people as app rows on the store's grid, More by as the app grid), B Shelf (people as chips, More by as a sideways shelf of tiles, after the App Store), D Byline (one sentence for who, links as words, More by as a row of icons, after Raycast); in `components/app/developer-lab/designs.tsx` |
| `/playground/sitemap` | Dev only (a 404 in production builds): the store's whole flow (2026-11-19), from `app/playground/sitemap/flow.ts` (every page, the steps that aren't pages, the other sites, and every link between them, read from the pages' code; keep it in step when a page gains or loses a link): the sidebar on every page as a band, a diagram with an arrow per link (`?focus=` lights up one box's links in and out; the diagram scrolls sideways on a phone), a card per page with where it goes and what leads to it, the old addresses that redirect, and what is served for machines |
| `/playground/app-page` | Dev only (a 404 in production builds): the whole app page in four layouts on a live app (`?app=`, Ring by default), the same components with only their places changed (2026-11-18, after the header and Developer ran the full width while the reading sections didn't): `?layout=before` (the page before Sidebar was picked), `sidebar` (the page's now and the default: a reading column with the install command, the facts and the developer card in a sidebar kept in view, More by across the foot; the sidebar first on a phone), `labels` (every section's title in a 9rem label column, its content beside it) and `column` (everything at 70ch). Stand-in screenshots and reviews from `components/app/samples.ts` |
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
- **Tokens** (`styles/tokens.css`, the account site's, copied through Starter): Allo on trial as the site's face (2026-11-18: Josh Puckett's variable font, SIL OFL 1.1, self-hosted from `assets/fonts/allo` with its licence and loaded on every page by `app/fonts.ts`; its roundness axis set to 20 of 100 in `styles/base.css`, on every element, since the `font` shorthand resets it; the system font is the fallback), where it was the system font, the type roles
  (display, h1, h2, h3, body, label, small, caption, hint, each with its `-tracking`), compact spacing and controls (32,
  36, 40), a neutral #FCFCFC page, near-black ink and the blue accent. Light only. Change it with the account site,
  never per screen. `styles/base.css` and `styles/squircle.css` (the native half only) are the account site's too.
- **Shell** (`components/shell`, 2026-11-14): the account site's. From 900px there is no top bar: the sidebar and
  the page (a white box that scrolls on its own) fill the window's height, and the sidebar is the account site's in its
  inset form: the wordmark "Team of Silicons **Apps**" at its head, search under it (a plain GET form to `/`, so Enter
  searches without script), the section list (NavList: Home, more as pages are designed), then at its foot the profile
  across its width (the signed-in account, which opens Settings, or Sign In) over a hairline, and Developers and Docs
  as small grey chips under it. A page without a page bar starts level with the sidebar's search field (`--page-top`).
  The sidebar's width can be changed (2026-11-17, `sidebar-resize.tsx`, the account site's handle): drag the page box's
  left edge, where a grip forms on hover, and it snaps between 200, 248 and 296px; arrow keys step, Home and End jump,
  a double-click goes back to 248, and the choice is remembered in this browser (`store.sidebar-step`). It needs
  script, so the handle only appears once script runs; without it the sidebar stays at 248px.
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
