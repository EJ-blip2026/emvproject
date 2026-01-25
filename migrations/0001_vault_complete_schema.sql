-- Complete Zero-Knowledge Vault Schema (SQLite & Postgres compatible)

-- Users with password hashing and encryption key management
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    encryption_key_salt TEXT NOT NULL,
    subscription_tier TEXT DEFAULT 'Starter',
    storage_limit_gb INTEGER DEFAULT 5,
    storage_used_gb REAL DEFAULT 0.0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Vaults (containers for encrypted content)
CREATE TABLE IF NOT EXISTS vaults (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Vault entries (encrypted notes, passwords, files)
CREATE TABLE IF NOT EXISTS vault_entries (
    id TEXT PRIMARY KEY,
    vault_id TEXT NOT NULL,
    entry_type TEXT NOT NULL CHECK(entry_type IN ('note', 'password', 'file')),
    encrypted_content BYTEA NOT NULL,
    nonce TEXT NOT NULL,
    file_size_bytes INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(vault_id) REFERENCES vaults(id) ON DELETE CASCADE
);

-- Password manager specific fields
CREATE TABLE IF NOT EXISTS passwords (
    id TEXT PRIMARY KEY,
    vault_entry_id TEXT NOT NULL,
    service_name TEXT,
    encrypted_username BYTEA,
    encrypted_password BYTEA,
    created_at TEXT NOT NULL,
    FOREIGN KEY(vault_entry_id) REFERENCES vault_entries(id) ON DELETE CASCADE
);

-- Vault sharing (encrypted key sharing between users)
CREATE TABLE IF NOT EXISTS shared_vaults (
    id TEXT PRIMARY KEY,
    vault_id TEXT NOT NULL,
    shared_with_user_id TEXT NOT NULL,
    permissions TEXT CHECK(permissions IN ('read', 'read-write')),
    shared_key TEXT NOT NULL,
    accepted BOOLEAN DEFAULT false,
    created_at TEXT NOT NULL,
    FOREIGN KEY(vault_id) REFERENCES vaults(id) ON DELETE CASCADE,
    FOREIGN KEY(shared_with_user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Subscriptions (Stripe billing)
CREATE TABLE IF NOT EXISTS subscriptions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    stripe_subscription_id TEXT,
    stripe_customer_id TEXT,
    plan TEXT NOT NULL,
    status TEXT NOT NULL,
    current_period_end TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- API keys for programmatic access
CREATE TABLE IF NOT EXISTS api_keys (
    key TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    revoked BOOLEAN DEFAULT false,
    created_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Webhook events (Stripe)
CREATE TABLE IF NOT EXISTS webhook_events (
    id TEXT PRIMARY KEY,
    stripe_event_id TEXT UNIQUE,
    event_type TEXT,
    processed BOOLEAN DEFAULT false,
    created_at TEXT NOT NULL
);

-- Usage tracking
CREATE TABLE IF NOT EXISTS usage (
    id TEXT PRIMARY KEY,
    api_key TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    count INTEGER NOT NULL,
    window_start TEXT NOT NULL
);

-- Key rotation audit
CREATE TABLE IF NOT EXISTS key_rotations (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    old_key TEXT,
    new_key TEXT,
    admin_token TEXT,
    reason TEXT,
    created_at TEXT
);

-- Notifications
CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    channel TEXT,
    message TEXT,
    created_at TEXT
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_vaults_user ON vaults(user_id);
CREATE INDEX IF NOT EXISTS idx_vault_entries_vault ON vault_entries(vault_id);
CREATE INDEX IF NOT EXISTS idx_passwords_entry ON passwords(vault_entry_id);
CREATE INDEX IF NOT EXISTS idx_shared_vaults_vault ON shared_vaults(vault_id);
CREATE INDEX IF NOT EXISTS idx_shared_vaults_user ON shared_vaults(shared_with_user_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_user ON subscriptions(user_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_user ON api_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_webhook_events_stripe_id ON webhook_events(stripe_event_id);
CREATE INDEX IF NOT EXISTS idx_usage_api_key ON usage(api_key);
CREATE INDEX IF NOT EXISTS idx_key_rotations_user ON key_rotations(user_id);
