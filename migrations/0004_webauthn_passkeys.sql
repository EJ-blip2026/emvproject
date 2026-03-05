-- WebAuthn/Passkey Support Schema

-- Stores WebAuthn credentials (public keys) for each user
CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    credential_id BYTEA NOT NULL,
    public_key BYTEA NOT NULL,
    sign_count INTEGER NOT NULL DEFAULT 0,
    transports TEXT, -- JSON array: ["internal", "usb", "ble", "nfc"]
    backup_eligible BOOLEAN DEFAULT false,
    backup_state BOOLEAN DEFAULT false,
    execution_time INTEGER, -- milliseconds
    user_verified BOOLEAN DEFAULT true,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Index for efficient lookups
CREATE INDEX IF NOT EXISTS idx_webauthn_user ON webauthn_credentials(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_webauthn_credential_id ON webauthn_credentials(credential_id);

-- Stores registration challenges (temporary, expires after 10 minutes)
CREATE TABLE IF NOT EXISTS webauthn_registration_challenges (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    challenge BYTEA NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_webauthn_reg_user ON webauthn_registration_challenges(user_id, expires_at);

-- Stores authentication challenges (temporary, expires after 10 minutes)
CREATE TABLE IF NOT EXISTS webauthn_authentication_challenges (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    challenge BYTEA NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webauthn_auth_user ON webauthn_authentication_challenges(user_id, expires_at);
