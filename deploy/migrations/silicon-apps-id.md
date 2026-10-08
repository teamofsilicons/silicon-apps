# Silicon Apps app ID correction

The canonical app ID is `silicon-apps`. CLI 0.1.9 and client 0.1.7 use it for Accounts login, package manifests and local installation records. This is a one-time platform correction. Ordinary app IDs remain immutable.

## Deployment order

1. Check that `apps` is the existing Silicon Apps record and `silicon-apps` does not already exist in either database. Stop if either identity is ambiguous. Preserve a Postgres dump and a consistent Apps SQLite backup.
2. Pause the Apps API while installing Accounts migration 0010 and the matching Accounts API. The migration retains the secret, owner, authors, settings, memberships, rotating refresh tokens, verification requests and notification records. It updates all app foreign keys in a single transaction. It sends no messages.
3. Install the Apps API. Startup moves the existing catalog entry, release associations and pending work to `silicon-apps`; package IDs and archive bytes stay unchanged. A collision aborts startup. Restarting does not rerun the move.
4. Publish the verified CLI archives, update the catalog, and deploy the matching docs. Check the shared portal, hosted authorization, old-link redirects, authenticated author access and a fresh registry install.

The shared portal keeps its `developer` Accounts audience. The `accounts` identity also stays unchanged. The retired `apps` ID is reserved so it cannot be claimed by someone else.

## Existing clients

The public installer obtains the app ID from the checksum-verified executable, so pinned old releases remain installable. Upgrading an official `apps` installation to 0.1.9 transfers command ownership to `silicon-apps` in the same installation transaction. A failure restores the previous directory, command and installed metadata. Other registries do not receive this migration.

Old registry URLs redirect to the canonical ID. Old clients whose update logic requires an exact ID must run the public installer once. Unchanged pre-0.1.9 archives remain available by exact version; current clients accept their legacy manifest only for this official app, its known old versions and its official registry. New uploads require the canonical manifest ID.

Refresh tokens remain usable. The updated CLI refreshes legacy saved sessions before use, and the Apps API refreshes browser sessions on first access. Existing external access JWTs with the old audience must be refreshed. They do not gain access to the new audience automatically.

## Rollback

Both services must agree on the ID. If the migration fails, transactions leave the original data in place. If a later deployment check fails, keep the Apps API paused and restore the matching pre-cutover Postgres dump, SQLite backup and both previous service releases before resuming traffic. A binary-only rollback cannot undo an identity migration. Do not rewrite released archives or overwrite a new app that already owns the requested ID.
