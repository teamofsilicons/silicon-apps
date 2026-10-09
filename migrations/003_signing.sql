-- Release signing keys the API has published, author keys, and their
-- endorsements. Every statement is idempotent so the service can apply it on
-- each start.
CREATE TABLE IF NOT EXISTS signing_keys (
 key_id TEXT PRIMARY KEY,
 public_key TEXT NOT NULL UNIQUE,
 first_seen_at TEXT NOT NULL,
 -- Signatures by older keys over this key: [{"key_id","signature"}].
 endorsements TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(endorsements))
);
CREATE TABLE IF NOT EXISTS author_keys (
 key_id TEXT PRIMARY KEY,
 owner_uuid TEXT NOT NULL,
 owner_id TEXT NOT NULL,
 name TEXT NOT NULL DEFAULT '',
 public_key TEXT NOT NULL UNIQUE,
 created_at TEXT NOT NULL,
 revoked_at TEXT,
 revoked_reason TEXT
);
CREATE INDEX IF NOT EXISTS author_keys_by_owner ON author_keys(owner_uuid, created_at);
