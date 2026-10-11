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
