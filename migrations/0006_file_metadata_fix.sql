-- Ensure file metadata columns exist (failsafe for migration 0005)
-- This uses a simple approach: attempt to add column, ignore if it already exists in error handling
-- For PostgreSQL only; SQLite doesn't support this syntax but migration 0005 handles it there

DO $$
BEGIN
    -- Add file_name column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name='vault_entries' AND column_name='file_name'
    ) THEN
        ALTER TABLE vault_entries ADD COLUMN file_name TEXT;
    END IF;
    
    -- Add mime_type column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name='vault_entries' AND column_name='mime_type'
    ) THEN
        ALTER TABLE vault_entries ADD COLUMN mime_type TEXT;
    END IF;
END $$;

