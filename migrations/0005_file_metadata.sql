-- Add optional metadata for uploaded/imported files so downloads preserve name/type
-- SQLite in this environment does not support IF NOT EXISTS for ADD COLUMN.
ALTER TABLE vault_entries ADD COLUMN file_name TEXT;
ALTER TABLE vault_entries ADD COLUMN mime_type TEXT;
