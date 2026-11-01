# Unsupported-target reasons, 2026-11-01

A release may now leave out one of the three required targets (`linux-x86_64`,
`windows-x86_64`, `macos-aarch64`) when it gives a public reason of 200 to 1000
characters in `unsupported_targets` ([PR #23](https://github.com/teamofsilicons/silicon-apps/pull/23)).
The store greys out that OS's mark and shows the reason on hover or focus; the
CLI and `resolve` return it. The developer portal's release form asks for it
([silicon-accounts#16](https://github.com/teamofsilicons/silicon-accounts/pull/16)).

## Source lineage

The Apps candidate is `main` at `17282b511309ad92008ab17a2380252ae7959bfa`, a
descendant of the previously live `4a7e578db697cf50449b99337d58a2f85977c238`. It
keeps durable login recovery and the required-target and uploader-confirmation
policy. `store/` is unchanged from the live `2116994` frontend apart from this
feature, and `deploy/install.py` is the one used for `ce35c0f`. No migration: the
catalog is a JSON document and releases without the field have no reasons.

The developer portal is `silicon-accounts` `main` at
`e75a05ed0c60d19f12254db98a43b2e67eccf613`. The portal previously ran
`12da103` (durable browser refresh), which was not on `main`;
[silicon-accounts#17](https://github.com/teamofsilicons/silicon-accounts/pull/17)
merged it first, so this deployment keeps it.

## Apps API and store

[Native build 41314770875](https://github.com/teamofsilicons/silicon-apps/actions/runs/41314770875)
built and ran the exact commit on Linux ARM64 (glibc 2.34). `deploy/package.py`
built the full bundle with the store built from source.

| Artifact | SHA-256 |
|---|---|
| API executable (13,635,936 bytes) | `19fc8a98a1c9248d73a3091a8f3d3549bf5a0b1b156cbf229f224474084eb02b` |
| Release archive `releases/17282b5….tar.gz` (96,767,380 bytes, 1,462 files, 38 symlinks) | `7892cd1de5bbeaf2b985aedfeda70ce7b851746009e7f6af20db1e29043e671d` |

SSM command `9948ddb1-f77b-4a50-8dc7-d55620e91fa3` installed it on
`i-0fb947706d7eff238` with the normal installer, after checking the live release
was `4a7e578`. The installer uploaded `backups/20261101T141711Z.tar.gz` before the
switch. Postflight: the running process's executable is the new release's
`bin/apps-server` with the expected hash; API, store, Caddy and the backup timer
are active; `/health` is `ok`; the store answers 200.

Public checks afterwards: capabilities list the three required targets,
`unsupported_target_reason` (200–1000 characters) and uploader confirmation for
Windows and macOS; OpenAPI has `unsupported_targets` and still has
`X-Apps-Platform-Confirmed`; `apps-2026-10` is the active signing key; app pages
render the required-platform marks.

## Developer portal

The standalone build (Next build `JU54nAOUvZbAhKrWPXC0T`) is
`releases/apps-unsupported-reasons/developer-e75a05e….tar.gz` in the Accounts
artifact bucket, SHA-256
`2576c1cd7f3033d51df5688c601d9432062698289f1a55436a9cb56f112cfdff`. It was
extracted to `/opt/accounts/browser-releases/e75a05e…` beside the previous
release, run as a candidate under the service's user and environment on its own
port and refresh state, and checked to render the updated docs before
`accounts-developer` was pointed at it (SSM `bcddface-904d-4205-9a17-f719ac1bb75f`).
The unit kept `REFRESH_STATE_PATH` and its session storage. The previous unit is
kept as `accounts-developer.service.before-e75a05e…` and the previous release
directory is unchanged.

A first attempt (SSM `acfbd328-7596-42c0-83df-35364b251346`) stopped before any
switch: `EnvironmentFile` overrides `Environment=` in systemd, so the candidate
inherited the live port and could not bind. The candidate now reads its own copy
of the environment file with its own port.

## Not yet released

The CLI's `--unsupported` and `--unsupported-file` flags reach people in the next
`silicon-apps` CLI release (after 0.3.2). Until then authors can use the portal or
the API.
