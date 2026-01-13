-- Create webhook events table for idempotency and auditing
CREATE TABLE IF NOT EXISTS webhook_events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    received_at TEXT NOT NULL
);

-- Add last_rotated column to api_keys for key rotation tracking (nullable)
ALTER TABLE api_keys ADD COLUMN last_rotated TEXT;
