-- Keep historical signed event bodies immutable while excluding retired
-- account identities from every delivery and replay surface.
CREATE TABLE IF NOT EXISTS event_identity_retirements (
 event_seq INTEGER PRIMARY KEY NOT NULL REFERENCES events(seq),
 retired_at TEXT NOT NULL,
 reason TEXT NOT NULL CHECK(reason = 'account_uuid_migrated')
);
