-- Add file metadata columns - complete solution for vault file tracking
-- Simple ALTER TABLE statements compatible with both SQLite 3.35+ and PostgreSQL

ALTER TABLE vault_entries ADD COLUMN file_name TEXT;
ALTER TABLE vault_entries ADD COLUMN mime_type TEXT;
