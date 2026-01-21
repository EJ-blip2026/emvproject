-- Add revoked flag to api_keys and create audit/notification tables
BEGIN;

ALTER TABLE api_keys ADD COLUMN revoked BOOLEAN DEFAULT false;

CREATE TABLE IF NOT EXISTS key_rotations (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    old_key TEXT,
    new_key TEXT,
    admin_token TEXT,
    reason TEXT,
    created_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_key_rotations_user ON key_rotations(user_id);

CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    channel TEXT,
    message TEXT,
    created_at TEXT
);

COMMIT;
