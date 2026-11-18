# Sign-in state per attempt, and production back on main

On 18 November 2026 production moved to main at `af06bd9`. That revision is the live organization-authors release
(`ddd5044`, from `codex/developer-organization-authors`, now fast-forwarded into main) plus teamofsilicons/silicon-apps#26.
With #26, each browser sign-in attempt keeps its state in its own cookie, so a second tab or click no longer fails the
first attempt with `oauth_state_mismatch`.

## Release

| | |
| --- | --- |
| Revision | `af06bd9c2ce3d6a89cb05af0a682b4d4c90b9d4a` (main) |
| Archive | `releases/af06bd9c2ce3d6a89cb05af0a682b4d4c90b9d4a.tar.gz`, SHA-256 `0d5854b7d0f4c4b4cb7a1008d05b1232ec720370ba29f81177aad1e9d9e58047` |
| API binary | SHA-256 `e7dcadd7a684a06ed1560d4281bca91e2912d5508f524c04ed209144c30938de` (the running process) |
| Store build | `4Sm7KXm7qtoA7jWXahdFe` |
| Installer | `deploy/install.py`, SSM command `149e9bf5-992e-44ed-9e7b-1a3c71f7bb64` |
| Backup before cutover | `backups/20261118T140557Z.tar.gz` |
| Previous release | `ddd50442e49957538caf5fc6d35c69f8ca0adcc4` |

The earlier overlay release had left `/opt/silicon-apps/previous-release` as a symlink. The installer refuses one, so the
install command first rewrote it as the plain file holding the same path. The installer writes it in that form too.

## Verification

- On main, CI passed `Rust check and tests` (including the 3 new `oauth_state` tests), `Web build and browser behavior`
  and the Linux and Windows worker safety tests.
- On the host: `current` is `af06bd9`, `deployment.json` records it, the API, store, Caddy and backup timer are active,
  and `/health` and the store's `GET /` answer.
- Public: `/health`, `/openapi.json`, `/`, `/.well-known/silicon-apps-keys.json`, `/v1/capabilities` and `/v1/apps` all
  return 200. The catalog lists 30 apps, every one with a tagline.
- Two sign-ins in a row each set their own `apps_oauth_state_<state>=1` cookie (Secure, HttpOnly, Path `/v1/auth`) and
  redirect to Silicon Accounts. The first attempt's callback, sent after the second started, passed the browser check
  and reached the code exchange. A callback without its own cookie, or with only the other attempt's cookie, got 403
  `oauth_state_mismatch`.
- In Chrome, the store, an app page and Silicon Accounts' account chooser for Silicon Apps loaded with no console
  errors, and all 14 store images decoded. No sign-in was completed in the browser.

Rollback: point `current` at `previous-release` (`ddd5044`) and restart `silicon-apps-api`, `silicon-apps-store` and
`caddy`.
