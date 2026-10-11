# Production verification, 2026-10-11

## Store release b54d810: the Silicon Writings look

Why: the store moves to the look of Silicon Writings (warm paper and ink, a warm dark mode, Hedvig Letters Serif
titles, BDO Grotesk text, Geist Pixel Square labels) with a reworked home page (PR #4). Store only.

Before: the live release was `profile-df47d7accb43e2ecae71317d36dda137215212f8` (API and store from `df47d7a`,
installed as an overlay; `deployment.json` still named `4721f65`). `git diff df47d7a f92ea57` is empty, so `main`
was exactly what was live.

API: to keep the API byte-identical, the archive carries the CI artifact of `df47d7a` (run 38072582642,
`apps-server` SHA-256 `27dbdc6f16c46409d7f75b9c70b54d10d9dc56c605b21a774eaaef26f0eacd71`, the binary already live)
instead of a new build. No migration, runtime secret or unit change.

Build: `deploy/package.py` with the pinned Caddy 2.11.7 and Node.js 24.21.0 archives and `--api` set to that
artifact. Archive `releases/b54d8101f21ec93e804ab0a5ec815ae0385cf563.tar.gz`, SHA-256
`d97ea02f9ac2a2c155553bbbd227f0d47d56e3d923c749e10da69207792957c0`, 97,081,625 bytes, S3 version
`.Am5bp41t.03aIGhDR7jPXXQL6OPc6ZB`; store build `XT_kKhS2uS1a5ef6vb-aN`. Checks before packaging: CI green on PR #4,
`pnpm typecheck`, `pnpm lint`, `pnpm test:e2e` (32 passed) and the deploy unit tests (67, 8 skipped).

Install: SSM `8fac0506-ab4c-4055-839c-d980adf3fc74` downloaded the archive, checked its SHA-256 (`OK`) before
extracting anything, extracted only `deploy/install.py` and ran it with `--role api`. The installer uploaded
`backups/20261011T111542Z.tar.gz`, switched `current` to `releases/b54d8101…` (previous release
`profile-df47d7a…` retained) and recorded `deployment.json` with revision `b54d810`. API, store and Caddy active.

Public checks (`https://apps.teamofsilicons.com`): `/` 200 (the new home page: "Most installed", "Just arrived",
paper `#FFFDF9`), `/search` 200, `/apps/silicon-apps` 200, `/apps/silicon-apps/og.png` 200, `/health` 200,
`/llms.txt`, `/robots.txt`, `/sitemap.xml` and `/.well-known/silicon-apps-keys.json` 200, both new fonts 200 as
`font/woff2`, `/developer` 308. No console errors in a browser.

## Store release d4a83b9: the minimal store

Why: `b54d810` was busy and broke in places (stray dots on wrapped meta lines, install commands cut off in scrolling
boxes, empty featured panels, descriptions truncated mid-word). PR #5 makes every page one article-width column,
lists apps one way everywhere and boxes almost nothing. Store only; the archive again carries the live API binary
(`27dbdc6f…`, the `df47d7a` CI artifact).

Archive `releases/d4a83b95ba0783032fb72dd9c37b063a20111ba7.tar.gz`, SHA-256
`a81e953126422e4b0da17ff2088c5e81e941b99d3a7a5693f83f03ea784084bf`, 97,078,411 bytes, S3 version
`BLrOAsAxVr2rox9AJaaDfo.TLi4s5uYs`; store build `kVKYj5ddXXLR4fs9gTWQ1`. Checks: CI green on PR #5, `pnpm typecheck`,
`pnpm lint`, `pnpm test:e2e` (32 passed).

Install: SSM `ea0b3a7e-3d08-4994-ac6f-055ea0ceafb2` checked the archive (`OK`) before extracting, ran
`deploy/install.py --role api`, uploaded `backups/20261011T124445Z.tar.gz`, switched `current` to `releases/d4a83b9…`
(previous `b54d810` retained) and recorded `deployment.json`. API, store and Caddy active.

Public checks: `/`, `/search`, `/apps/silicon-apps`, its `og.png`, `/health`, `/llms.txt`, `/robots.txt`,
`/sitemap.xml`, `/.well-known/silicon-apps-keys.json` and the serif font 200; `/developer` 308. No console errors.

## Store release 5fa09f5: polish, light only, no tag filters

PR #6: looser serif titles, the nav pill's padding restored (base.css's `ul[role="list"]` reset had removed it),
light only (`lib/boot.ts` pins `data-theme="light"`; the theme switch and Settings appearance are gone), author photos
by the byline, a View all apps button ending the home list, and tags no longer links or filters (`/search?tag=x`
answers 308 to `/search?q=x`; tag pages left the sitemap). Store only; the archive carried the live API binary
(`27dbdc6f…`).

Archive `releases/5fa09f54368837711f9084a4505f3d792487d6ff.tar.gz`, SHA-256
`452bb29bbe6bc401a240b690b3967044e859936559824a8b6e480670411f104e`, S3 version `NmDsqwIHwg9qhFDl8oKJ10aiiL03gzg6`.
SSM `c526ed46-9a38-43a5-88c0-446eab9821e1`, backup `backups/20261011T131625Z.tar.gz`. Checks: CI green, e2e 32 passed;
live `/`, `/search`, `/apps/briefcase`, `/settings`, `/sitemap.xml` 200, `/search?tag=cli` 308 to `/search?q=cli`,
no `search?tag=` links and no tag entries in the sitemap, the boot script pins light.

## API release 0861354: tags are never a search filter

PR #7: `GET /v1/apps` drops `tags`; tag words are matched through `q`, and a request that still sends `tags` gets
`400 invalid_input` with `details {"field":"tags","use":"q"}`. Agent card, capabilities and OpenAPI updated;
UNDERSTANDING.md edited at the Carbon's request. No migration.

API binary: the CI artifact of `9f0cd51` (run 38142896295, SHA-256
`6240eac88346bd9d331e69212aab35920c6110907821ac6b4955ec5255283ac2`; the merge commit's crates are identical). Archive
`releases/08613547eb955bc339f984eefbafaf76ce6d8337.tar.gz`, SHA-256
`9b6d8c756dbeb8acb6ab8094e219129015563c600e1e626ff12612699c4943cf`, S3 version `un2azpg9CIQoCXDuuISvFHaRj9ndLZPp`.
Checks: CI green (including the ARM64 build), `cargo test -p silicon-apps-server`, store e2e 32 passed against the new
binary.

Install: SSM `f2696b2c-e014-4a9e-9c93-d7fdd3d5ed24` checked the archive (`OK`), uploaded
`backups/20261011T132920Z.tar.gz`, switched `current` to `releases/0861354…` (previous `5fa09f5` retained), recorded
`deployment.json`; API, store and Caddy active. Live: `/v1/apps?tags=cli` 400 with the message pointing to `q`;
`/v1/apps?q=cli` 15 results; typo `q=productivty` 8 results; OpenAPI parameters `q,target,visibility,mine,sort,limit,
offset`; agent card says tags are matched through q; `/health` ok (0.1.4), store pages and signing keys 200.

Follow-up for the Carbon: `silicon-accounts/developer/llms/llms-full.md` (around line 1753) still lists `tags=` in the
`/v1/apps` route; that file is Carbon-edited only.

## Store release a14c5d1: search is just search, filters behind Filters

PR #8: the search page shows only the search field; a native "Filters" disclosure (closed until one is set, with a
count) holds platform, visibility, added, last release, minimum installs, minimum rating and sort. Sort goes to the
API; the rest narrow its results in the store. The page redirects the form's address to its tidy form; filtered views
are noindex with `/search` as canonical. Store only; the archive carried the live API binary (`6240eac8…`).

Archive `releases/a14c5d1a14c7a6e82174d52e0f8c69b670de0467.tar.gz`, SHA-256
`4a6b802bba65497bbffdafef95ed78c6f467f27980ed5f07b0f17a7e2c91bbff`, S3 version `8cw9gGOO.op87UgjM9ssYKyGCh6VuZNl`.
SSM `c9f687eb-2952-4e8b-8b0e-a426d0735a4a`, backup `backups/20261011T134137Z.tar.gz`. Checks: CI green, e2e 33 passed.
Live: `/search` 200 with Filters closed; `sort=installs` puts silicon-apps, silicon-accounts, briefcase first;
`installs=10` gives silicon-accounts and silicon-apps; `released=week` 18 apps; an empty-valued address 307s to
`/search`; `/search?tag=cli` still 308s to `/search?q=cli`; filtered views noindex with canonical `/search`.

## Store release 90460e8: Arc filter toolbar, silicon-apps hover card, no font flash, no FAQ

PR #9: search filters are Arc's filter toolbar (vendored from the Silicon UI registry), with the plain form kept for
no script; `silicon-apps` in install commands opens Arc's hover card with the installer for the visitor's system and a
toggle for the other (`@radix-ui/react-popover` 1.2.0 added); app pages warn when there is no package for the
visitor's system; results dim while a filter change loads; `/` focuses search; the four first-paint fonts are
preloaded with `font-display: block`; the FAQ section and its FAQPage data are gone; "Make a Silicon App" and "Add
Silicon Accounts to your app". Store only; the archive carried the live API binary (`6240eac8…`).

Archive `releases/90460e8eac85784a37c737de5173b75c66796954.tar.gz`, SHA-256
`b244cad5b0c41957e2e6aa8ac3cd7c2cad7d21eb9fc4a4bad841646cefce94e5`, S3 version `ljNOCk97tmcD.Pm3qXVGgzlnD89ZwixJ`.
SSM `001344a6-1218-4c07-82c0-d5574d077cf8`, backup `backups/20261011T140932Z.tar.gz`. Checks: CI green, e2e 35 passed
(new tests for the hover card and the toolbar). Live: four font preloads in the home page, no FAQ, new wording, the
hover card opens on `/apps/dm` with the installer, the toolbar and the no-script form both present on `/search`, no
console errors; `/`, `/search`, `/apps/dm`, `/health`, `/sitemap.xml` 200.
