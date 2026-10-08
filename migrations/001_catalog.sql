PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS catalog (
 id INTEGER PRIMARY KEY CHECK(id=1),
 document TEXT NOT NULL CHECK(json_valid(document))
);
INSERT OR IGNORE INTO catalog(id,document) VALUES(1,'{"apps":{},"invites":[],"reports":[]}');
CREATE TABLE IF NOT EXISTS idempotency (
 actor TEXT NOT NULL,
 key TEXT NOT NULL,
 fingerprint TEXT NOT NULL,
 response TEXT NOT NULL,
 created_at TEXT NOT NULL,
 PRIMARY KEY(actor,key)
);
CREATE TABLE IF NOT EXISTS outbox (
 id TEXT PRIMARY KEY,
 kind TEXT NOT NULL,
 body TEXT NOT NULL CHECK(json_valid(body)),
 created_at TEXT NOT NULL,
 delivered_at TEXT,
 attempts INTEGER NOT NULL DEFAULT 0,
 last_error TEXT
);
CREATE TABLE IF NOT EXISTS pending_secrets (
 actor TEXT NOT NULL,
 key TEXT NOT NULL,
 fingerprint TEXT NOT NULL,
 app_id TEXT NOT NULL UNIQUE,
 secret TEXT NOT NULL,
 created_at TEXT NOT NULL,
 PRIMARY KEY(actor,key)
);
