-- Add optional metadata for uploaded/imported files so downloads preserve name/type
ALTER TABLE vault_entries ADD COLUMN IF NOT EXISTS file_name TEXT;
ALTER TABLE vault_entries ADD COLUMN IF NOT EXISTS mime_type TEXT;
