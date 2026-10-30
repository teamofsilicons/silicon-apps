# Release-policy recovery, 2026-10-30

Production again requires Linux x86_64, Windows x86_64 and macOS ARM64 before a
release can be published. Linux uploads still require successful isolated-runner
execution. Windows and macOS uploads require explicit uploader confirmation and
record `execution: not_run` with no fabricated exit code. The current durable
browser-login recovery remains in place.

## Cause and source integration

The browser-session recovery rollout deployed source
`21169940a378d966e007b3de60fdf09955c5badd`, which did not include previously
deployed policy source `ce35c0f90c6ad6465da7c1e0a8f58b17ec28e6ed` or its required-target
ancestry. A correctly confirmed Windows upload therefore reached the old runner
lookup and failed with HTTP 503. This was a source-lineage regression, not a
client flag, hostname or account problem.

[PR #22](https://github.com/teamofsilicons/silicon-apps/pull/22) merges both lines.
The deployed candidate is `4a7e578db697cf50449b99337d58a2f85977c238`. Its server
authentication, vendored Accounts recovery support, and store-session sources
are byte-identical to `2116994`. It introduces no database migration or schema
change. Health continues to report API version `0.2.0`; that version alone cannot
identify this deployment.

## Native artifact and deployment

[Native build 41074003355](https://github.com/teamofsilicons/silicon-apps/actions/runs/41074003355)
built and ran the exact candidate on Linux ARM64, targeting glibc 2.34. The API
executable is 13,608,592 bytes with SHA-256
`6f4756f5f832da806901a8c4bc1ce802464c926fc953c1af2d3c95798df3cb8c`.

The rollout used the existing API host `i-0fb947706d7eff238` in `us-east-2` and
the normal verified installer. An exclusive nonblocking deployment lock and
checks against the observed release, secret version, protected-file hashes and
schema prevented a concurrent or stale update. No worker was changed.

Only the native executable and its build metadata changed in the installed
bundle. The prior bundle supplied the exact other 1,418 payload files and 28
symlinks, including the deployed frontend, Node, Caddy, units and installer. The
candidate manifest records this preserved-runtime provenance explicitly. The
seven effective unit, environment and Caddy files were byte- and mode-identical
before and after installation. The runtime secret version was unchanged.

| Artifact | SHA-256 |
|---|---|
| 6,182,576-byte API delta | `4832c0efdd883fbe69160737f83338c71bf776781fc3075488e8719210f1c360` |
| Reassembled release archive | `66609872dbcb322b04ea2636bc4ffded03bdc7f583e811a62be1dab29bc9b2a6` |
| Installed build manifest | `651293993968b42b5ae8f5240f7d4eb1dc79bf03894dcdc2b49a948b9487ec14` |
| Protected runtime backup | `9abe6bb9a7060387212070c3bdff80cfba6e1af995f8646dc7c25abc3109b52e` |

The private artifact bucket retains the delta at
`releases/4a7e578db697cf50449b99337d58a2f85977c238-api-policy-delta.tar.gz`, the
protected runtime backup at
`backups/policy-recovery-4a7e578db697cf50449b99337d58a2f85977c238-runtime.tar.gz`, and
the normal catalog/package backup at `backups/20261030T151029Z.tar.gz`.

SSM command `65de84b1-cc0e-48f3-8eff-f5ca01ffa0e1` completed successfully from
15:09:45 to 15:12:21 UTC. Postflight checked the running process's executable
hash, the full installed manifest, unchanged protected files and secret version,
SQLite integrity and foreign keys, unchanged schema, and active/enabled API,
store, Caddy and backup timer. The recovery copy of the prior release was retained;
rollback was not needed.

## Verification

Before rollout, the focused HTTP/authentication, confirmation and required-target
tests passed, as did the Rust workspace suite, strict all-target Clippy and
formatting. Deployment tests passed on the operator host with the native-platform
tests deferred to their native CI runners.

The integration CI exposed a test that used POSIX `O_NOFOLLOW` and mode assertions
on Windows for the Mac-only release-session helper. The follow-up splits that
test by its supported platform: POSIX hosts retain permission and symlink checks,
while all hosts retain and extend environment-token validation. All 22 runner
tests passed on macOS, including the actual sandbox and token-file checks. The
helper and every deployed API dependency remain unchanged by this test fix.

After rollout, public capabilities reported live validation for all four Linux
targets and uploader confirmation for Windows and macOS. OpenAPI again exposes
`X-Apps-Platform-Confirmed`. The original failed McPort Windows x86_64 upload was
retried through the ordinary authorized CLI with the same package and idempotency
key and succeeded. Its confirmation records accurately report `not_run` and null
exit codes. Linux package validation had already succeeded through the live
runner.

The ordinary author flow subsequently published McPort `0.5.0` on all eight
packaged targets, including the three required targets, as production release
`86d6903b-95a1-4916-b499-ba43a7ee39a1`. The publication receipt records matching
package checksums and verified public signatures for every target.

For future deployments, compare the candidate's ancestry and effective behavior
with the recorded live source, not only its version string or branch name. Preserve
both durable authentication recovery and the required-target/confirmation policy;
publish from the integrated branch. For a binary-only repair, verify the entire
existing bundle before copying it, preserve all non-binary payload and runtime
configuration, record the mixed-source provenance, and use the normal installer
with a fresh backup and postflight checks.
