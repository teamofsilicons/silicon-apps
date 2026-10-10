-- Offline account UUID exports are data/audit mappings, never authentication aliases.
CREATE TABLE IF NOT EXISTS account_uuid_migrations (
 old_uuid TEXT PRIMARY KEY NOT NULL,
 new_uuid TEXT UNIQUE NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('carbon','silicon')),
 migrated_at TEXT NOT NULL,
 CHECK(old_uuid <> new_uuid)
);
