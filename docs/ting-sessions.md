# Ting sessions in Silicon Apps

Apps uses the official Ting **0.4.0** Rust client, pinned to release commit
`ce35675ce153f85a26c98983945bfa3fbaa644ce`. Ting delivers live wake-up hints for
account, author and subscription event feeds. Apps' committed event log remains
the authoritative history for releases, invitations, validation progress and
other catalog activity.

## Session lifecycle

1. The caller opens an authenticated Apps event stream. Apps checks feed access
   and applies its existing concurrent-stream limit before contacting Ting.
2. Apps captures the event cursor, opens a new anonymous Ting receiver and
   registers only `silicon-apps` on it. The server owns the socket and binds it
   to that caller's feed and filters in memory. Callers cannot supply another
   receiver's ID, and they need no separate Ting login or account subscription.
3. A background worker observes committed events, checks each watcher's current
   feed permissions, and coalesces matching batches into
   `silicon-apps.eventsAvailable` hints. One shared publishing WebSocket and a
   short-lived Accounts App verification proof serve all active receivers.
4. Each hint targets exactly one `session_id`, omits `for`, and contains empty
   `data` and `metadata`. No app IDs, account IDs, package output, event payloads,
   user-storage data or credentials cross Ting. Session IDs and deterministic
   idempotency keys are transport metadata; they grant no Apps access.
5. On a hint, the watcher reconciles the Apps log using its original filters.
   Initial catch-up and a five-second reconciliation scan repair handshake gaps
   and lost hints. Access and subscription status are checked again before
   queued events are emitted. Apps revalidates the login every 15 seconds;
   current account identity is passed back to the worker. An upstream auth
   failure closes the stream rather than continuing with stale authority.
6. Closing a stream removes its binding and drops its Ting socket. The shared
   publisher closes when no bindings remain. A reconnect gets a new receiver
   and resumes with the **Apps event sequence**, never an old Ting session ID.
   The existing 30-minute stream lifetime also bounds idle receivers.

The binding is deliberately ephemeral: it is not written into subscriptions or
stored as a user's default notification destination. There is no account-wide
fallback, notification consent change, hook takeover, or read acknowledgement.
Offline automation can use existing durable signed webhooks and event replay.
Ting's transport acknowledgements never count as an Apps workflow completing.

## API and clients

Both `/v1/events/stream` and `/v1/apps/{app_id}/events/stream` accept:

| `transport` | Behavior |
| --- | --- |
| `auto` (default) | Use Ting when configured; downgrade to direct Apps events if Ting fails. |
| `ting` | Require a Ting receiver; return 503 if it cannot open, or close if delivery later fails. Reconnect with the saved Apps cursor. |
| `direct` | Read the same Apps log without allocating a Ting receiver. |

Event payloads, filters, `Last-Event-ID`, and subscription cursors stay the same.
The opening comment is `: ready cursor=N transport=MODE`.
`X-Apps-Event-Transport` identifies the initial mode and `X-Apps-Ting-Session`
identifies its receiver when present. Auto-mode failure emits
`: transport=direct reason=ting_unavailable`; replay remains available throughout.
`GET /v1/capabilities` reports supported transports and whether Ting is configured
(configuration is not a claim of live upstream reachability).

```sh
silicon-apps events --follow
silicon-apps events --app example --type 'package.*' --follow --transport ting
silicon-apps events --follow --transport direct --after 123
```

The CLI reconnects with bounded exponential backoff and jitter, preserving the
opening cursor even if the connection closes before its first event. It stops
on permanent HTTP errors and Ctrl-C. The Rust client exposes
`stream_events_with_transport(..., EventTransport::Ting)` and
`EventStream::last_event_id()` for callers implementing their own reconnect loop.
`stream_events(...)` uses `Auto`.

The store opens one account stream per visible signed-in tab, coalesces page
refreshes, reconciles after reconnect, closes hidden-tab connections, and
suppresses automatic refresh after form edits until navigation. Anonymous store
browsing does not allocate sessions. The developer portal can use these API
streams with an Apps-scoped credential; this does not broaden its existing
developer-audience token permissions.

## Enable on a deployment

Use the API service's existing protected environment and app credentials.
Set `APPS_TING_URL=https://backend.ting.teamofsilicons.com`, then run the new
release's executable with that environment:

```sh
apps-server ting-register
```

This issues a short-lived `types.manage` App verification proof and registers
the empty-object schema. Exact re-registration is safe; a conflicting definition
fails without being overwritten. Runtime publishing requests only `tings.send`,
caches proofs briefly in memory, and does not rotate any user login. Neither
operation prints credentials. Accounts must permit those scopes for the
`silicon-apps` → `ting` app proof, and Ting must run the 0.4 session protocol.

Restart the API and deploy the matching store/CLI builds. Verify an authenticated
`--follow --transport ting` stream, trigger a test-owned catalog mutation, confirm
the corresponding event, and reconnect from its cursor. Only then treat the
deployment as verified. Removing `APPS_TING_URL` restores direct delivery; no
database migration, notification history deletion or credential rotation is
needed. An unset URL leaves Ting disabled. Invalid origins and a configured URL
without the Apps secret are rejected at startup.

## Verification

Tests use isolated Accounts and Ting protocol fixtures and real Apps routes.
They exercise two accounts with distinct receivers, a shared publisher/proof,
empty hint payloads, disconnect cleanup, replay across new session IDs, failure
fallback, required-Ting failure, and pausing access while a hint is in flight.
Rust-client and CLI tests cover losing the connection before the first event and
resuming from the ready cursor. Store checks include TypeScript and lint.
