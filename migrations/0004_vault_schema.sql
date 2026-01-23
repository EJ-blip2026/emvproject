-- Vault Schema: Zero-Knowledge Encrypted Storage
-- Features: Text notes, password manager, file storage, sharing
BEGIN;

-- Drop old haikus-related tables (if any)
-- (none in current schema)

-- Users table (extend existing if you have one)
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,  -- Argon2id hash
    encryption_key_salt TEXT NOT NULL,  -- Salt for deriving encryption key from password
    subscription_tier TEXT DEFAULT 'Starter',  -- Starter, Professional, Enterprise
    storage_limit_gb INTEGER DEFAULT 5,
    storage_used_gb REAL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Vaults: Top-level encrypted containers
CREATE TABLE IF NOT EXISTS vaults (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,  -- Plaintext for listing; user can see their own vault names
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vaults_user ON vaults(user_id);

-- Vault Entries: Individual encrypted items (notes, passwords, files)
CREATE TABLE IF NOT EXISTS vault_entries (
    id TEXT PRIMARY KEY,
    vault_id TEXT NOT NULL,
    entry_type TEXT NOT NULL,  -- 'note', 'password', 'file'
    encrypted_content BYTEA NOT NULL,  -- XChaCha20-Poly1305 encrypted
    nonce TEXT NOT NULL,  -- Unique per entry for encryption
    file_size_bytes INTEGER,  -- Only for files
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vault_entries_vault ON vault_entries(vault_id);
CREATE INDEX IF NOT EXISTS idx_vault_entries_type ON vault_entries(entry_type);

-- Passwords: Subtype of vault_entries with service metadata
CREATE TABLE IF NOT EXISTS passwords (
    id TEXT PRIMARY KEY,
    vault_entry_id TEXT NOT NULL UNIQUE,
    service_name TEXT,  -- e.g., "Gmail", "GitHub" (plaintext for user filtering)
    encrypted_username BYTEA NOT NULL,  -- XChaCha20-Poly1305 encrypted
    encrypted_password BYTEA NOT NULL,  -- XChaCha20-Poly1305 encrypted
    created_at TEXT NOT NULL,
    FOREIGN KEY (vault_entry_id) REFERENCES vault_entries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_passwords_vault_entry ON passwords(vault_entry_id);

-- Shared Vaults: Manage sharing permissions and re-encrypted keys
CREATE TABLE IF NOT EXISTS shared_vaults (
    id TEXT PRIMARY KEY,
    vault_id TEXT NOT NULL,
    shared_with_user_id TEXT NOT NULL,
    permissions TEXT DEFAULT 'read',  -- 'read' or 'read-write'
    shared_key TEXT NOT NULL,  -- Vault's encryption key, re-encrypted with recipient's Argon2id key
    accepted BOOLEAN DEFAULT false,
    created_at TEXT NOT NULL,
    FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE,
    FOREIGN KEY (shared_with_user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_shared_vaults_vault ON shared_vaults(vault_id);
CREATE INDEX IF NOT EXISTS idx_shared_vaults_user ON shared_vaults(shared_with_user_id);

-- Subscriptions: Track active subscriptions (linked to Stripe)
CREATE TABLE IF NOT EXISTS subscriptions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    stripe_customer_id TEXT,
    stripe_subscription_id TEXT,
    tier TEXT NOT NULL,  -- Starter, Professional, Enterprise
    status TEXT NOT NULL,  -- active, canceled, past_due
    current_period_start TEXT,
    current_period_end TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_subscriptions_user ON subscriptions(user_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_stripe ON subscriptions(stripe_customer_id);

-- API Keys: For programmatic access to vault API
CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    key TEXT NOT NULL UNIQUE,  -- hashed API key
    name TEXT,  -- user-friendly name
    last_used TEXT,
    created_at TEXT NOT NULL,
    last_rotated TEXT,
    revoked BOOLEAN DEFAULT false,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_api_keys_user ON api_keys(user_id);

-- Webhook Events: Track Stripe webhook deliveries
CREATE TABLE IF NOT EXISTS webhook_events (
    id TEXT PRIMARY KEY,
    stripe_event_id TEXT UNIQUE,
    event_type TEXT,
    processed BOOLEAN DEFAULT false,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webhook_events_stripe_id ON webhook_events(stripe_event_id);

COMMIT;
