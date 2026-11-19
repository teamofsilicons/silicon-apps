# Store redesign and report screenshots in production

On 19 November 2026 production moved to main at `ec4dd4c`: teamofsilicons/silicon-apps#27, Anil's redesign of the store
on the Silicon Accounts layout, with optional screenshots on bug reports and the gallery-wall 404. Before merging, report
screenshots were limited to signed-in visitors (`c8b1f36`): each is up to 5 MiB in the production database, and anonymous
reports allow 120 writes a minute per address. The API answers 401 to an anonymous screenshot; a text report stays open
to anyone, and the store shows the screenshot field only when signed in.

## Release

| | |
| --- | --- |
| Revision | `ec4dd4cfefacd916d6883d50f88466f99b1f7c40` (main) |
| Archive | `releases/ec4dd4cfefacd916d6883d50f88466f99b1f7c40.tar.gz`, SHA-256 `2670c98abf22f426769ca10faa310646278a509f75243593995ddab4ae2712bb` |
| API binary | SHA-256 `b9c5e229541e0e4132c710cdfb84b551810aac644cf5b29b355dba31a2cf8393` (the running process) |
| Store build | `-RlbBJvRo8r_jYmC1E85x` |
| Runtime secret | version `afd95141-d19f-41f5-819e-907e7548c9e7` (`AWSCURRENT`) |
| Installer | `deploy/install.py`, SSM command `84c62376-fde5-44de-b946-844b2b25f0de` |
| Backup before cutover | `backups/20261119T190104Z.tar.gz` |
| Previous release | `af06bd9c2ce3d6a89cb05af0a682b4d4c90b9d4a` |
| New migration | `007_report_screenshots` (a new table; older binaries ignore it) |

## Verification

- On the PR at `c8b1f36` (the same tree as `ec4dd4c`), CI passed all eight jobs. Locally: the server's report tests,
  including the new `only_a_signed_in_caller_may_attach_a_screenshot`, store typecheck and lint, and the 87 deploy tests.
- Before cutover the host was on `af06bd9` with the recorded binary, no recovery receipt, and 21 GB free.
- On the host: `current` is `ec4dd4c`, `deployment.json` records it, and the API, store, Caddy and backup timer are active.
- Public: `/health`, `/openapi.json`, `/`, `/apps`, `/.well-known/silicon-apps-keys.json`, `/v1/capabilities` and
  `/v1/apps` return 200, with 30 apps listed. An unknown path returns 404 with the gallery wall. An anonymous report with
  a screenshot returns 401 and a screenshot link with a wrong key returns 404.
- In Chrome, the home page, an app page and the 404 rendered with no console errors.

Not checked: a signed-in report with a real screenshot, and screen capture in Chrome (the PR notes Attach Image works).

Rollback: point `current` at `previous-release` (`af06bd9`) and restart `silicon-apps-api`, `silicon-apps-store` and
`caddy`. The `report_screenshots` table stays and the old binary ignores it.
