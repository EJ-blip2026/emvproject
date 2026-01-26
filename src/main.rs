// Zero-Knowledge Vault - MVP Backend
// Features: Register, Login, Create Vault, Store/Retrieve Encrypted Notes

mod crypto;
mod models;
mod cloud_import;

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, get_service, post},
    Json, Router,
};
use base64::Engine;
use chrono::Utc;
use dashmap::DashMap;
use serde_json::json;
use sqlx::AnyPool;
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::services::ServeDir;
use tower_http::cors::{CorsLayer, Any};
use uuid::Uuid;

use crypto::*;
use models::*;

#[derive(Clone)]
struct AppState {
    db_pool: AnyPool,
    _admin_token: String,
    // Session tokens (user_id -> token)
    sessions: Arc<DashMap<String, String>>,
}

// ============================================================================
// AUTH HANDLERS
// ============================================================================

async fn register_handler(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    if req.username.trim().is_empty() || req.password.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Username and password are required"})),
        )
            .into_response();
    }

    // Derive encryption key from password
    let (_key, salt) = match derive_key(&req.password) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("derive_key failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Hash password with Argon2id for storage
    let password_hash = match argon2::password_hash::PasswordHasher::hash_password(
        &argon2::Argon2::default(),
        req.password.as_bytes(),
        &argon2::password_hash::SaltString::generate(rand::thread_rng()),
    ) {
        Ok(h) => h.to_string(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to hash password"})),
            )
                .into_response()
        }
    };

    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Determine subscription tier (default to Starter)
    let tier = req.subscription_tier
        .as_ref()
        .map(|t| t.as_str())
        .unwrap_or(models::TIER_STARTER);
    
    // Validate tier
    let tier = match tier {
        models::TIER_PRO | models::TIER_ENTERPRISE => tier,
        _ => models::TIER_STARTER,
    };
    
    let storage_limit = models::get_storage_limit(tier);

    // Insert user into DB
    let result = sqlx::query(
        "INSERT INTO users (id, username, password_hash, encryption_key_salt, subscription_tier, storage_limit_gb, storage_used_gb, created_at, updated_at) 
         VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $7)"
    )
    .bind(&user_id)
    .bind(&req.username)
    .bind(&password_hash)
    .bind(&salt)
    .bind(tier)
    .bind(storage_limit)
    .bind(&now)
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_) => (
            StatusCode::CREATED,
            Json(json!({"user_id": user_id, "message": "User created"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Registration failed: {}", e)})),
        )
            .into_response(),
    }
}

async fn login_handler(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    // Fetch user from DB
    let user_result = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, username, password_hash, encryption_key_salt FROM users WHERE username = $1",
    )
    .bind(&req.username)
    .fetch_one(&state.db_pool)
    .await;

    let (user_id, username, password_hash, _salt) = match user_result {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid credentials"})),
            )
                .into_response()
        }
    };

    // Verify password with Argon2id
    let parsed_hash = match argon2::PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Invalid hash"})),
            )
                .into_response()
        }
    };

    if argon2::PasswordVerifier::verify_password(
        &argon2::Argon2::default(),
        req.password.as_bytes(),
        &parsed_hash,
    )
    .is_err()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid credentials"})),
        )
            .into_response();
    }

    // Generate session token
    let token = Uuid::new_v4().to_string();
    state.sessions.insert(user_id.clone(), token.clone());

    (
        StatusCode::OK,
        Json(json!({
            "token": token,
            "user_id": user_id,
            "username": username
        })),
    )
        .into_response()
}

// ============================================================================
// VAULT HANDLERS
// ============================================================================

async fn create_vault_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateVaultRequest>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response()
        }
    };

    let vault_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        "INSERT INTO vaults (id, user_id, name, description, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $5)"
    )
    .bind(&vault_id)
    .bind(&user_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&now)
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_) => (
            StatusCode::CREATED,
            Json(json!({"vault_id": vault_id, "name": req.name})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create vault: {}", e)})),
        )
            .into_response(),
    }
}

async fn list_vaults_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response()
        }
    };

    let vaults_result = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, name, description, created_at FROM vaults WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(&user_id)
    .fetch_all(&state.db_pool)
    .await;

    match vaults_result {
        Ok(vaults) => {
            let vault_list: Vec<VaultResponse> = vaults
                .into_iter()
                .map(|(id, name, description, created_at)| VaultResponse {
                    id,
                    name,
                    description,
                    created_at,
                })
                .collect();
            (StatusCode::OK, Json(json!({"vaults": vault_list}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to fetch vaults: {}", e)})),
        )
            .into_response(),
    }
}

// ============================================================================
// NOTES HANDLERS (Encrypted)
// ============================================================================

async fn create_note_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(vault_id): AxumPath<String>,
    Json(req): Json<CreateNoteRequest>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response()
        }
    };

    // Verify vault ownership
    let vault_check =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vaults WHERE id = $1 AND user_id = $2")
            .bind(&vault_id)
            .bind(&user_id)
            .fetch_one(&state.db_pool)
            .await;

    if vault_check.unwrap_or(0) == 0 {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Vault not found or access denied"})),
        )
            .into_response();
    }

    // Decode base64 encrypted content
    let encrypted_content =
        match base64::engine::general_purpose::STANDARD.decode(&req.encrypted_content) {
            Ok(c) => c,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid base64 content"})),
                )
                    .into_response()
            }
        };

        // Enforce storage quota: reserve then insert, with compensation on failure
        let size_bytes = encrypted_content.len() as f64;
        let additional_gb = size_bytes / (1024.0 * 1024.0 * 1024.0);

        let reserve = sqlx::query(
            "UPDATE users SET storage_used_gb = storage_used_gb + $1 WHERE id = $2 AND storage_used_gb + $1 <= storage_limit_gb"
        )
        .bind(additional_gb)
        .bind(&user_id)
        .execute(&state.db_pool)
        .await;

        match reserve {
            Ok(res) if res.rows_affected() > 0 => {
                let entry_id = Uuid::new_v4().to_string();
                let now = Utc::now().to_rfc3339();

                let insert_res = sqlx::query(
                    "INSERT INTO vault_entries (id, vault_id, entry_type, encrypted_content, nonce, file_size_bytes, created_at, updated_at) \
                     VALUES ($1, $2, 'note', $3, $4, NULL, $5, $5)"
                )
                .bind(&entry_id)
                .bind(&vault_id)
                .bind(&encrypted_content)
                .bind(&req.nonce)
                .bind(&now)
                .execute(&state.db_pool)
                .await;

                match insert_res {
                    Ok(_) => (
                        StatusCode::CREATED,
                        Json(json!({"entry_id": entry_id, "message": "Note created"})),
                    )
                        .into_response(),
                    Err(e) => {
                        // compensate reserved usage
                        let _ = sqlx::query(
                            "UPDATE users SET storage_used_gb = storage_used_gb - $1 WHERE id = $2"
                        )
                        .bind(additional_gb)
                        .bind(&user_id)
                        .execute(&state.db_pool)
                        .await;
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Failed to create note: {}", e)})),
                        )
                            .into_response()
                    }
                }
            }
            Ok(_) => (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "Storage limit exceeded"})),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to check quota: {}", e)})),
            )
                .into_response(),
        }
}

async fn upload_file_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(vault_id): AxumPath<String>,
    Json(req): Json<UploadFileRequest>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response()
        }
    };

    // Ensure the vault belongs to the authenticated user
    let vault_check =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vaults WHERE id = $1 AND user_id = $2")
            .bind(&vault_id)
            .bind(&user_id)
            .fetch_one(&state.db_pool)
            .await;

    if vault_check.unwrap_or(0) == 0 {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Vault not found or access denied"})),
        )
            .into_response();
    }

    let encrypted_content = match base64::engine::general_purpose::STANDARD.decode(&req.encrypted_content) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid base64 content"})),
            )
                .into_response()
        }
    };

    // Enforce storage quota: reserve then insert, with compensation on failure
    let size_bytes = encrypted_content.len() as f64;
    let additional_gb = size_bytes / (1024.0 * 1024.0 * 1024.0);

    let reserve = sqlx::query(
        "UPDATE users SET storage_used_gb = storage_used_gb + $1 WHERE id = $2 AND storage_used_gb + $1 <= storage_limit_gb"
    )
    .bind(additional_gb)
    .bind(&user_id)
    .execute(&state.db_pool)
    .await;

    match reserve {
        Ok(res) if res.rows_affected() > 0 => {
            let entry_id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();

            let insert_res = sqlx::query(
                "INSERT INTO vault_entries (id, vault_id, entry_type, encrypted_content, nonce, file_size_bytes, created_at, updated_at) \
                 VALUES ($1, $2, 'file', $3, $4, $5, $6, $6)"
            )
            .bind(&entry_id)
            .bind(&vault_id)
            .bind(&encrypted_content)
            .bind(&req.nonce)
            .bind(req.file_size_bytes)
            .bind(&now)
            .execute(&state.db_pool)
            .await;

            match insert_res {
                Ok(_) => (
                    StatusCode::CREATED,
                    Json(json!({"entry_id": entry_id, "message": "File stored"})),
                )
                    .into_response(),
                Err(e) => {
                    // compensate reserved usage
                    let _ = sqlx::query(
                        "UPDATE users SET storage_used_gb = storage_used_gb - $1 WHERE id = $2"
                    )
                    .bind(additional_gb)
                    .bind(&user_id)
                    .execute(&state.db_pool)
                    .await;
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to save file: {}", e)})),
                    )
                        .into_response()
                }
            }
        }
        Ok(_) => (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Storage limit exceeded"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to check quota: {}", e)})),
        )
            .into_response(),
    }
}

async fn list_entries_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(vault_id): AxumPath<String>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response()
        }
    };

    // Verify vault ownership
    let vault_check =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vaults WHERE id = $1 AND user_id = $2")
            .bind(&vault_id)
            .bind(&user_id)
            .fetch_one(&state.db_pool)
            .await;

    if vault_check.unwrap_or(0) == 0 {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Vault not found or access denied"})),
        )
            .into_response();
    }

    let entries_result = sqlx::query_as::<_, (String, String, Vec<u8>, String, Option<i32>, String)>(
        "SELECT id, entry_type, encrypted_content, nonce, file_size_bytes, created_at FROM vault_entries WHERE vault_id = $1 ORDER BY created_at DESC"
    )
    .bind(&vault_id)
    .fetch_all(&state.db_pool)
    .await;

    match entries_result {
        Ok(entries) => {
            let entry_list: Vec<VaultEntryResponse> = entries
                .into_iter()
                .map(|(id, entry_type, encrypted_content, nonce, file_size_bytes, created_at)| {
                    VaultEntryResponse {
                        id,
                        entry_type,
                        encrypted_content: base64::engine::general_purpose::STANDARD
                            .encode(&encrypted_content),
                        nonce,
                        file_size_bytes,
                        created_at,
                    }
                })
                .collect();
            (StatusCode::OK, Json(json!({"entries": entry_list}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to fetch entries: {}", e)})),
        )
            .into_response(),
    }
}

// ============================================================================
// HEALTH & UTILITY
// ============================================================================

async fn health_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({"status": "ok", "service": "vault-api"})),
    )
        .into_response()
}

// Seed a free admin account (Enterprise tier) if configured and not already present
async fn seed_admin_user(pool: &AnyPool) {
    let admin_username = env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
    let admin_password = env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "change_me_admin".to_string());

    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = $1")
        .bind(&admin_username)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let (_key, salt) = match derive_key(&admin_password) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Failed to derive key for admin user: {}", e);
            return;
        }
    };

    let password_hash = match argon2::password_hash::PasswordHasher::hash_password(
        &argon2::Argon2::default(),
        admin_password.as_bytes(),
        &argon2::password_hash::SaltString::generate(rand::thread_rng()),
    ) {
        Ok(h) => h.to_string(),
        Err(e) => {
            eprintln!("Failed to hash admin password: {}", e);
            return;
        }
    };

    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let subscription_tier = models::TIER_ENTERPRISE;
    let storage_limit_gb = models::ENTERPRISE_STORAGE_GB;

    if exists > 0 {
        // Update password hash, salt, and ensure Enterprise tier
        let res = sqlx::query(
            "UPDATE users SET password_hash = $1, encryption_key_salt = $2, subscription_tier = $3, storage_limit_gb = $4 WHERE username = $5"
        )
        .bind(&password_hash)
        .bind(&salt)
        .bind(subscription_tier)
        .bind(storage_limit_gb)
        .bind(&admin_username)
        .execute(pool)
        .await;

        match res {
            Ok(_) => eprintln!(
                "✅ Updated admin user '{}' credentials and tier {} (limit {} GB)",
                admin_username, subscription_tier, storage_limit_gb
            ),
            Err(e) => eprintln!("Failed to update admin user '{}': {}", admin_username, e),
        }
        return;
    }

    let res = sqlx::query(
        "INSERT INTO users (id, username, password_hash, encryption_key_salt, subscription_tier, storage_limit_gb, storage_used_gb, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $7)"
    )
    .bind(&user_id)
    .bind(&admin_username)
    .bind(&password_hash)
    .bind(&salt)
    .bind(subscription_tier)
    .bind(storage_limit_gb)
    .bind(&now)
    .execute(pool)
    .await;

    match res {
        Ok(_) => eprintln!(
            "✅ Seeded admin user '{}' with tier {} (limit {} GB)",
            admin_username, subscription_tier, storage_limit_gb
        ),
        Err(e) => eprintln!("Failed to seed admin user '{}': {}", admin_username, e),
    }
}

// ============================================================================
// ACCOUNT USAGE & UPGRADE
// ============================================================================

async fn get_usage_handler(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response()
        }
    };

    let usage = sqlx::query_as::<_, (f64, i32, String)>(
        "SELECT storage_used_gb, storage_limit_gb, subscription_tier FROM users WHERE id = $1"
    )
    .bind(&user_id)
    .fetch_one(&state.db_pool)
    .await;

    let (storage_used_gb, storage_limit_gb, subscription_tier) = match usage {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to fetch usage: {}", e)})),
            )
                .into_response()
        }
    };

    let vault_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM vaults WHERE user_id = $1"
    )
    .bind(&user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let entry_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM vault_entries WHERE vault_id IN (SELECT id FROM vaults WHERE user_id = $1)"
    )
    .bind(&user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let resp = UsageResponse {
        storage_used_gb,
        storage_limit_gb,
        subscription_tier,
        vault_count: vault_count as i32,
        entry_count: entry_count as i32,
    };

    (StatusCode::OK, Json(resp)).into_response()
}

async fn upgrade_enterprise_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response()
        }
    };

    let current_tier = sqlx::query_scalar::<_, String>(
        "SELECT subscription_tier FROM users WHERE id = $1"
    )
    .bind(&user_id)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or_else(|_| models::TIER_STARTER.to_string());

    if current_tier == models::TIER_ENTERPRISE {
        return (StatusCode::OK, Json(json!({"message": "Already Enterprise"}))).into_response();
    }

    let limit = models::ENTERPRISE_STORAGE_GB;
    let res = sqlx::query(
        "UPDATE users SET subscription_tier = $1, storage_limit_gb = $2 WHERE id = $3"
    )
    .bind(models::TIER_ENTERPRISE)
    .bind(limit)
    .bind(&user_id)
    .execute(&state.db_pool)
    .await;

    match res {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({"message": "Upgraded to Enterprise", "storage_limit_gb": limit})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Upgrade failed: {}", e)})),
        )
            .into_response(),
    }
}

// ============================================================================
// CLOUD IMPORT HANDLERS
// ============================================================================

async fn list_cloud_files_handler(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let provider = req["provider"].as_str().unwrap_or("");
    let access_token = req["access_token"].as_str().unwrap_or("");

    if access_token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "access_token required"})),
        )
            .into_response();
    }

    let files = match provider {
        "google_drive" => cloud_import::google_drive::list_files(access_token).await,
        "onedrive" => cloud_import::onedrive::list_files(access_token).await,
        _ => Err(format!("Unsupported provider: {}", provider)),
    };

    match files {
        Ok(files) => {
            let response: Vec<models::CloudFileResponse> = files
                .into_iter()
                .map(|f| models::CloudFileResponse {
                    id: f.id,
                    name: f.name,
                    size: f.size,
                    mime_type: f.mime_type,
                })
                .collect();
            (StatusCode::OK, Json(json!({"files": response}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        )
            .into_response(),
    }
}

async fn import_cloud_files_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(vault_id): AxumPath<String>,
    Json(req): Json<models::CloudImportRequest>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized"})),
            )
                .into_response()
        }
    };

    // Verify vault ownership
    let vault_check =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vaults WHERE id = $1 AND user_id = $2")
            .bind(&vault_id)
            .bind(&user_id)
            .fetch_one(&state.db_pool)
            .await;

    if vault_check.unwrap_or(0) == 0 {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Vault not found or access denied"})),
        )
            .into_response();
    }

    let mut imported_count = 0;
    let mut failed_count = 0;

    for file_id in &req.file_ids {
        let content = match req.provider.as_str() {
            "google_drive" => {
                cloud_import::google_drive::download_file(&req.access_token, file_id).await
            }
            "onedrive" => {
                // OneDrive needs download URL from file metadata
                Err("OneDrive import needs download URL".to_string())
            }
            _ => Err(format!("Unsupported provider: {}", req.provider)),
        };

        match content {
            Ok(bytes) => {
                // Encrypt and store (simple base64 for MVP)
                let encrypted = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let nonce = base64::engine::general_purpose::STANDARD
                    .encode(uuid::Uuid::new_v4().to_string());

                // Check quota and insert
                let size_bytes = bytes.len() as f64;
                let additional_gb = size_bytes / (1024.0 * 1024.0 * 1024.0);

                let reserve = sqlx::query(
                    "UPDATE users SET storage_used_gb = storage_used_gb + $1 WHERE id = $2 AND storage_used_gb + $1 <= storage_limit_gb"
                )
                .bind(additional_gb)
                .bind(&user_id)
                .execute(&state.db_pool)
                .await;

                if let Ok(res) = reserve {
                    if res.rows_affected() > 0 {
                        let entry_id = Uuid::new_v4().to_string();
                        let now = Utc::now().to_rfc3339();

                        let insert_res = sqlx::query(
                            "INSERT INTO vault_entries (id, vault_id, entry_type, encrypted_content, nonce, file_size_bytes, created_at, updated_at) \
                             VALUES ($1, $2, 'file', $3, $4, $5, $6, $6)"
                        )
                        .bind(&entry_id)
                        .bind(&vault_id)
                        .bind(encrypted.as_bytes())
                        .bind(&nonce)
                        .bind(bytes.len() as i32)
                        .bind(&now)
                        .execute(&state.db_pool)
                        .await;

                        if insert_res.is_ok() {
                            imported_count += 1;
                        } else {
                            // Compensate
                            let _ = sqlx::query(
                                "UPDATE users SET storage_used_gb = storage_used_gb - $1 WHERE id = $2"
                            )
                            .bind(additional_gb)
                            .bind(&user_id)
                            .execute(&state.db_pool)
                            .await;
                            failed_count += 1;
                        }
                    } else {
                        failed_count += 1; // quota exceeded
                    }
                } else {
                    failed_count += 1;
                }
            }
            Err(_) => {
                failed_count += 1;
            }
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "imported": imported_count,
            "failed": failed_count,
            "message": format!("Imported {} files, {} failed", imported_count, failed_count)
        })),
    )
        .into_response()
}
// ============================================================================
// AUTHENTICATION MIDDLEWARE
// ============================================================================

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let token = headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;

    // Find user_id by token in sessions
    for entry in state.sessions.iter() {
        if entry.value() == token {
            return Some(entry.key().clone());
        }
    }
    None
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Register sqlx any drivers
    sqlx::any::install_default_drivers();

    // Database setup
    let mut database_url = env::var("DATABASE_URL")
        .or_else(|_| env::var("RAILWAY_DATABASE_URL"))
        .unwrap_or_else(|_| "sqlite:///app/data/vault.db".to_string());

    // Force SSL for Railway Postgres
    if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
        if database_url.contains("sslmode=") {
            database_url = database_url.replace("sslmode=prefer", "sslmode=require");
        } else {
            database_url = format!("{}?sslmode=require", database_url);
        }
    }

    println!(
        "Connecting to database: {}",
        database_url
            .replace(|c: char| c.is_whitespace(), "")
            .chars()
            .take(50)
            .collect::<String>()
    );

    // Try to connect with exponential backoff for Postgres startup
    let mut pool = None;
    let mut retry_count = 0;
    let max_retries = 6; // shorten startup delay
    let mut used_postgres = false;

    while pool.is_none() && retry_count < max_retries {
        match AnyPool::connect(&database_url).await {
            Ok(p) => {
                if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
                    eprintln!("✅ Successfully connected to Postgres!");
                    used_postgres = true;
                } else {
                    eprintln!("✅ Successfully connected to SQLite file database");
                }
                pool = Some(p);
            }
            Err(err) => {
                if database_url.starts_with("sqlite://") {
                    eprintln!("Failed to open sqlite database '{}': {err}. Falling back to in-memory sqlite.", database_url);
                    pool = Some(
                        AnyPool::connect("sqlite::memory:")
                            .await
                            .expect("failed to connect to in-memory sqlite"),
                    );
                } else {
                    retry_count += 1;
                    if retry_count < max_retries {
                        let wait_time =
                            std::time::Duration::from_secs(2_u64.pow(retry_count as u32));
                        eprintln!(
                            "DB connection failed (attempt {}/{}): {err}. Retrying in {:?}...",
                            retry_count, max_retries, wait_time
                        );
                        tokio::time::sleep(wait_time).await;
                    }
                }
            }
        }
    }

    if pool.is_none() {
        eprintln!(
            "❌ DB connection failed after {} attempts. Falling back to sqlite::memory: for this run.",
            max_retries
        );
        pool = Some(
            AnyPool::connect("sqlite::memory:")
                .await
                .expect("failed to connect to in-memory sqlite"),
        );
        used_postgres = false;
    }

    if used_postgres {
        eprintln!("✅ Using Postgres (pooled connection ready)");
    } else {
        eprintln!("✅ Using SQLite (in-memory)");
    }

    let pool = pool.expect("pool not initialized");

    // Run migrations
    println!("Applying migrations...");
    let migrator = sqlx::migrate!("./migrations");
    let migration_result = migrator.run(&pool).await;

    // If the database has a stale migrations table (e.g., old versions), drop and retry once
    if let Err(sqlx::migrate::MigrateError::VersionMissing(_)) = migration_result {
        eprintln!("Detected stale migration history. Dropping _sqlx_migrations and retrying...");
        // Best-effort drop; ignore errors so we can retry
        let _ = sqlx::query("DROP TABLE IF EXISTS _sqlx_migrations")
            .execute(&pool)
            .await;
        migrator
            .run(&pool)
            .await
            .expect("migrations failed after reset");
    } else {
        migration_result.expect("migrations failed");
    }
    println!("Migrations applied.");

    // Seed a free admin Enterprise account if configured
    seed_admin_user(&pool).await;

    let _admin_token = env::var("ADMIN_TOKEN").unwrap_or_else(|_| "admintoken".to_string());

    let state = AppState {
        db_pool: pool,
        _admin_token,
        sessions: Arc::new(DashMap::new()),
    };

    let app = Router::new()
        // Health check (must come before static files)
        .route("/health", get(health_handler))
        // Auth API
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler))
        // Account API
        .route("/account/usage", get(get_usage_handler))
        .route("/account/upgrade", post(upgrade_enterprise_handler))
        // Cloud import API
        .route("/cloud/list", post(list_cloud_files_handler))
        .route("/vaults/:vault_id/import", post(import_cloud_files_handler))
        // Vaults API
        .route("/vaults/create", post(create_vault_handler))
        .route("/vaults/list", get(list_vaults_handler))
        // Notes API
        .route("/vaults/:vault_id/notes", post(create_note_handler))
        .route("/vaults/:vault_id/entries", get(list_entries_handler))
        // Files API
        .route("/vaults/:vault_id/files", post(upload_file_handler))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state.clone())
        // Serve static files (frontend) - must come last
        .fallback_service(
            get_service(ServeDir::new("public")).handle_error(|_| async {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serve file")
            }),
        );

    let port = env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("🔒 Vault API listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
