-- Screenshots attached to bug reports. The mail to the maintainers links to
-- GET /v1/reports/{report_id}/screenshot?key=…; only the key's SHA-256 is kept.
CREATE TABLE IF NOT EXISTS report_screenshots (
 report_id TEXT PRIMARY KEY,
 key_hash TEXT NOT NULL,
 name TEXT NOT NULL,
 content_type TEXT NOT NULL,
 data BLOB NOT NULL,
 created_at TEXT NOT NULL
);
