-- Add file metadata columns using standard SQL syntax
-- Compatible with SQLite (all versions) and PostgreSQL
-- This replaces migration 0007 which attempted to use IF NOT EXISTS (unsupported on older SQLite)

-- These statements will succeed on fresh databases (SQLite in-memory on Railway)
-- On PostgreSQL where columns might already exist, the migration system tracks state
ALTER TABLE vault_entries ADD COLUMN file_name TEXT;
ALTER TABLE vault_entries ADD COLUMN mime_type TEXT;
