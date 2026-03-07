-- Add file metadata columns for vault file tracking
ALTER TABLE vault_entries ADD COLUMN file_name TEXT;
ALTER TABLE vault_entries ADD COLUMN mime_type TEXT;
