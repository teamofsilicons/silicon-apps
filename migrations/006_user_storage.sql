CREATE TABLE IF NOT EXISTS app_user_storage (
 app_id TEXT NOT NULL,
 user_uuid TEXT NOT NULL,
 values_json TEXT NOT NULL CHECK(json_valid(values_json)),
 revision INTEGER NOT NULL,
 updated_at TEXT NOT NULL,
 PRIMARY KEY(app_id,user_uuid)
);
CREATE TABLE IF NOT EXISTS app_storage_replays (
 actor TEXT NOT NULL,
 key TEXT NOT NULL,
 fingerprint TEXT NOT NULL,
 response TEXT NOT NULL CHECK(json_valid(response)),
 created_at TEXT NOT NULL,
 PRIMARY KEY(actor,key)
);
