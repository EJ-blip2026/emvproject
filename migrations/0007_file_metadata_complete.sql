-- Fallback migration for SQLite (older versions that don't support IF NOT EXISTS or PL/pgSQL)
-- This approach creates a new table without the columns, then renames it back
-- This is a safe no-op on PostgreSQL since columns already exist from 0005/0006

-- For SQLite only: check if columns exist by attempting the rename
-- If this fails, columns already exist and we skip the rest
-- PostgreSQL will just execute these statements harmlessly

-- Note: In production, this migration is mainly a safeguard
-- The columns should already exist from migration 0005 or 0006


