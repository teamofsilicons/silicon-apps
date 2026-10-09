-- Append-only event log, release subscriptions and their webhook deliveries.
-- Every statement is idempotent so the service can apply it on each start.
CREATE TABLE IF NOT EXISTS events (
 seq INTEGER PRIMARY KEY AUTOINCREMENT,
 id TEXT NOT NULL UNIQUE,
 type TEXT NOT NULL,
 app_id TEXT,
 actor_uuid TEXT NOT NULL,
 -- public: anyone who can see the app; authors: the app's current authors;
 -- direct: only the subscription named in its delivery (test pings).
 visibility TEXT NOT NULL CHECK(visibility IN ('public','authors','direct')),
 recipient_uuids TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(recipient_uuids)),
 recipient_emails TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(recipient_emails)),
 data TEXT NOT NULL CHECK(json_valid(data)),
 idempotency_key TEXT,
 occurred_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS events_by_app ON events(app_id, seq);
CREATE TRIGGER IF NOT EXISTS events_are_append_only_update BEFORE UPDATE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;
CREATE TRIGGER IF NOT EXISTS events_are_append_only_delete BEFORE DELETE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;

CREATE TABLE IF NOT EXISTS subscriptions (
 id TEXT PRIMARY KEY,
 owner_uuid TEXT NOT NULL,
 owner_id TEXT NOT NULL,
 owner_emails TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(owner_emails)),
 app_id TEXT,
 types TEXT NOT NULL CHECK(json_valid(types)),
 channels TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(channels)),
 delivery TEXT NOT NULL CHECK(delivery IN ('webhook','stream')),
 url TEXT,
 secret TEXT,
 status TEXT NOT NULL CHECK(status IN ('active','paused','cancelled')),
 description TEXT NOT NULL DEFAULT '',
 cursor INTEGER NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 cancelled_at TEXT,
 CHECK(delivery = 'stream' OR (url IS NOT NULL AND secret IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS subscriptions_by_owner ON subscriptions(owner_uuid, created_at);
CREATE INDEX IF NOT EXISTS subscriptions_by_app ON subscriptions(app_id, status);

CREATE TABLE IF NOT EXISTS subscription_deliveries (
 id TEXT PRIMARY KEY,
 subscription_id TEXT NOT NULL REFERENCES subscriptions(id),
 event_seq INTEGER NOT NULL REFERENCES events(seq),
 status TEXT NOT NULL CHECK(status IN ('pending','delivered','failed')),
 attempts INTEGER NOT NULL DEFAULT 0,
 next_attempt_ms INTEGER NOT NULL,
 created_ms INTEGER NOT NULL,
 created_at TEXT NOT NULL,
 delivered_at TEXT,
 last_attempt_at TEXT,
 last_status INTEGER,
 last_error TEXT,
 UNIQUE(subscription_id, event_seq)
);
CREATE INDEX IF NOT EXISTS deliveries_due ON subscription_deliveries(status, next_attempt_ms);
CREATE INDEX IF NOT EXISTS deliveries_by_subscription ON subscription_deliveries(subscription_id, created_ms);
