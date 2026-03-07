-- Add file metadata columns - complete solution for vault file tracking
-- IF NOT EXISTS supported in both PostgreSQL 9.6+ and SQLite 3.35.0+

ALTER TABLE vault_entries ADD COLUMN IF NOT EXISTS file_name TEXT;
ALTER TABLE vault_entries ADD COLUMN IF NOT EXISTS mime_type TEXT;
