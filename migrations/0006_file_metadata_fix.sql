-- Fix for file metadata columns - adding with conditional checks for PostgreSQL
-- Migration 5 had a checksum issue due to SQLite compatibility changes
-- This migration ensures the file_name and mime_type columns exist
ALTER TABLE vault_entries ADD COLUMN IF NOT EXISTS file_name TEXT;
ALTER TABLE vault_entries ADD COLUMN IF NOT EXISTS mime_type TEXT;
