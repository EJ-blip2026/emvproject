// Vault data models for zero-knowledge encrypted storage
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// Subscription tier definitions
pub const TIER_STARTER: &str = "Starter";
pub const TIER_PRO: &str = "Pro";
pub const TIER_ENTERPRISE: &str = "Enterprise";

pub const STARTER_STORAGE_GB: i32 = 5;
pub const PRO_STORAGE_GB: i32 = 100;
pub const ENTERPRISE_STORAGE_GB: i32 = 1024;

pub const STARTER_PRICE: &str = "Free";
pub const PRO_PRICE: &str = "$9.99/month";
pub const ENTERPRISE_PRICE: &str = "$49.99/month";

pub fn get_storage_limit(tier: &str) -> i32 {
    match tier {
        TIER_PRO => PRO_STORAGE_GB,
        TIER_ENTERPRISE => ENTERPRISE_STORAGE_GB,
        _ => STARTER_STORAGE_GB,
    }
}

pub fn get_tier_price(tier: &str) -> &'static str {
    match tier {
        TIER_PRO => PRO_PRICE,
        TIER_ENTERPRISE => ENTERPRISE_PRICE,
        _ => STARTER_PRICE,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub encryption_key_salt: String,
    pub subscription_tier: String,
    pub storage_limit_gb: i32,
    pub storage_used_gb: f32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub id: String,
    pub vault_id: String,
    pub entry_type: String, // "note", "password", "file"
    pub encrypted_content: Vec<u8>,
    pub nonce: String,
    pub file_size_bytes: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Password {
    pub id: String,
    pub vault_entry_id: String,
    pub service_name: Option<String>,
    pub encrypted_username: Vec<u8>,
    pub encrypted_password: Vec<u8>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedVault {
    pub id: String,
    pub vault_id: String,
    pub shared_with_user_id: String,
    pub permissions: String, // "read" or "read-write"
    pub shared_key: String,  // Encrypted with recipient's key
    pub accepted: bool,
    pub created_at: String,
}

// Request/Response DTOs

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub subscription_tier: Option<String>, // "Starter", "Pro", or "Enterprise" - defaults to Starter
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateVaultRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNoteRequest {
    pub encrypted_content: String, // Base64-encoded encrypted content
    pub nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePasswordRequest {
    pub service_name: String,
    pub encrypted_username: String, // Base64
    pub encrypted_password: String, // Base64
    pub nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadFileRequest {
    pub encrypted_content: String, // Base64-encoded
    pub nonce: String,
    pub file_size_bytes: i32,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShareVaultRequest {
    pub share_with_username: String,
    pub permissions: String, // "read" or "read-write"
    pub shared_key: String,  // Vault encryption key, encrypted for recipient
}

#[derive(Debug, Serialize)]
pub struct VaultResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct VaultEntryResponse {
    pub id: String,
    pub entry_type: String,
    pub encrypted_content: String, // Base64
    pub nonce: String,
    pub file_size_bytes: Option<i32>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct UsageResponse {
    pub storage_used_gb: f32,
    pub storage_limit_gb: i32,
    pub subscription_tier: String,
    pub vault_count: i32,
    pub entry_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct CloudImportRequest {
    pub provider: String, // "google_drive", "onedrive", "s3"
    pub access_token: String,
    pub file_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CloudFileResponse {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct AuditLog {
    pub id: String,
    pub user_id: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub details: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQueryParams {
    pub start_date: Option<String>, // ISO 8601
    pub end_date: Option<String>,   // ISO 8601
    pub action: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct AuditLogsResponse {
    pub logs: Vec<AuditLog>,
    pub total_count: i32,
}

// WebAuthn/Passkey DTOs
#[derive(Debug, Deserialize)]
pub struct PasskeyRegisterBeginRequest {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct PasskeyRegisterBeginResponse {
    pub challenge: String, // Base64-encoded challenge bytes
    pub challenge_id: String, // UUID to correlate with verification
}

#[derive(Debug, Deserialize)]
pub struct PasskeyRegisterVerifyRequest {
    pub user_id: String,
    pub challenge_id: String,
    pub credential_id: String,        // Base64
    pub attestation_object: String,   // Base64 encoded attestation
    pub client_data_json: String,     // Base64 encoded client data
    pub transports: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct PasskeyRegisterVerifyResponse {
    pub success: bool,
    pub credential_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PasskeyAuthenticateBeginRequest {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct PasskeyAuthenticateBeginResponse {
    pub challenge: String, // Base64-encoded challenge bytes
    pub challenge_id: String, // UUID to correlate with verification
}

#[derive(Debug, Deserialize)]
pub struct PasskeyAuthenticateVerifyRequest {
    pub challenge_id: String,
    pub credential_id: String, // Base64
    pub authenticator_data: String, // Base64
    pub client_data_json: String, // Base64
    pub signature: String, // Base64
}

#[derive(Debug, Serialize)]
pub struct PasskeyAuthenticateVerifyResponse {
    pub success: bool,
    pub token: String,
    pub user_id: String,
}
