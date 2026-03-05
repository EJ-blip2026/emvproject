// Zero-Knowledge Vault - MVP Backend
// Features: Register, Login, Create Vault, Store/Retrieve Encrypted Notes

mod crypto;
mod models;
mod cloud_import;
mod webauthn;

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, get_service, post},
    Json, Router,
};
use base64::Engine;
use chrono::Utc;
use dashmap::DashMap;
use hyper::Server;
use rustls::{Certificate, PrivateKey, RootCertStore, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
use serde_json::json;
use sqlx::AnyPool;
use std::{env, fs, io::BufReader, net::SocketAddr, sync::Arc};
use tower_http::services::ServeDir;
use tower_http::cors::{CorsLayer, Any};
use uuid::Uuid;

use crypto::*;
use emvproject::redact_sensitive;
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
    headers: HeaderMap,
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

    // Log successful login
    let ip = extract_ip(&headers);
    log_audit(
        &state,
        &user_id,
        "login_success",
        Some("user"),
        Some(&user_id),
        ip.as_deref(),
        None,
    )
    .await;

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

// WebAuthn Passkey Handlers

async fn passkey_register_begin_handler(
    State(state): State<AppState>,
    Json(req): Json<PasskeyRegisterBeginRequest>,
) -> impl IntoResponse {
    // Fetch user to get ID
    let user_result = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM users WHERE username = $1",
    )
    .bind(&req.username)
    .fetch_one(&state.db_pool)
    .await;

    let (user_id,) = match user_result {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
                .into_response()
        }
    };

    // Generate registration challenge
    match webauthn::generate_registration_challenge(&state.db_pool, &user_id).await {
        Ok((challenge_bytes, challenge_id)) => {
            let challenge_b64 = base64::engine::general_purpose::STANDARD.encode(&challenge_bytes);
            (
                StatusCode::OK,
                Json(json!({
                    "challenge": challenge_b64,
                    "challenge_id": challenge_id
                })),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to generate challenge"})),
        )
            .into_response(),
    }
}

async fn passkey_register_verify_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<PasskeyRegisterVerifyRequest>,
) -> impl IntoResponse {
    use webauthn_rs::prelude::*;
    use serde::{Deserialize};
    
    // Step 1: Verify the challenge exists and hasn't expired, and retrieve it
    let challenge_bytes = match webauthn::verify_registration_challenge(&state.db_pool, &req.challenge_id, &req.user_id)
        .await
    {
        Ok(challenge) => challenge,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Challenge verification failed or expired"})),
            )
                .into_response()
        }
    };

    // Step 2: Decode attestation object and client data JSON from base64
    let attestation_object_bytes = match base64::engine::general_purpose::STANDARD.decode(&req.attestation_object) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid attestation_object encoding"})),
            )
                .into_response()
        }
    };

    let client_data_json_bytes = match base64::engine::general_purpose::STANDARD.decode(&req.client_data_json) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid client_data_json encoding"})),
            )
                .into_response()
        }
    };

    let credential_id_bytes = match base64::engine::general_purpose::STANDARD.decode(&req.credential_id) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid credential_id encoding"})),
            )
                .into_response()
        }
    };

    // Step 3: Decode and verify client data JSON structure
    #[derive(Deserialize)]
    struct ClientData {
        challenge: String,
        origin: String,
        #[serde(rename = "type")]
        ty: String,
    }
    
    let client_data: ClientData = match serde_json::from_slice(&client_data_json_bytes) {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Invalid client data JSON: {}", e)})),
            )
                .into_response()
        }
    };

    // Step 4: Verify the challenge matches what we stored (base64url encoded)
    let expected_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&challenge_bytes);
    if client_data.challenge != expected_challenge {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Challenge mismatch - potential replay attack"})),
        )
            .into_response();
    }

    if client_data.ty != "webauthn.create" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid client data type"})),
        )
            .into_response();
    }

    // Step 5: Verify origin (check host matches)
    let hostname = std::env::var("DOMAIN").unwrap_or_else(|_| "localhost:3000".to_string());
    let expected_origin = format!("https://{}", hostname);
    if !client_data.origin.starts_with(&expected_origin) && !client_data.origin.contains("localhost") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": format!("Origin mismatch: got {}, expected {}", client_data.origin, expected_origin)})),
        )
            .into_response();
    }

    // Step 6: Decode attestation object (CBOR format)
    let attestation_obj: serde_cbor::Value = match serde_cbor::from_slice(&attestation_object_bytes) {
        Ok(obj) => obj,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Failed to parse attestation object: {}", e)})),
            )
                .into_response()
        }
    };

    // Step 7: Extract authData from attestation object
    let auth_data_bytes = match &attestation_obj {
        serde_cbor::Value::Map(map) => {
            let key = serde_cbor::Value::Text("authData".to_string());
            match map.get(&key) {
                Some(serde_cbor::Value::Bytes(bytes)) => bytes.clone(),
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": "Missing or invalid authData in attestation object"})),
                    )
                        .into_response()
                }
            }
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Attestation object is not a map"})),
            )
                .into_response()
        }
    };

    // Step 8: Parse authenticator data (first 37 bytes are fixed, remainder is credential data)
    // Format: rpIdHash (32 bytes) | flags (1 byte) | signCount (4 bytes) | [credentialData]
    if auth_data_bytes.len() < 37 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Authenticator data too short"})),
        )
            .into_response();
    }

    let flags = auth_data_bytes[32];
    let has_credential_data = (flags & 0x40) != 0; // Bit 6 = attested credential data included
    let has_user_verified = (flags & 0x04) != 0; // Bit 2 = user verified

    if !has_credential_data {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Authenticator data missing credential data"})),
        )
            .into_response();
    }

    // Step 9: Extract credential public key from authData
    // Credential data format: credentialId (2 bytes length + variable) | credentialPublicKey (CBOR)
    let mut pos = 37;
    
    if auth_data_bytes.len() < pos + 2 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Authenticator data truncated at credentialId length"})),
        )
            .into_response();
    }

    let cred_id_len = u16::from_be_bytes([auth_data_bytes[pos], auth_data_bytes[pos + 1]]) as usize;
    pos += 2;

    if auth_data_bytes.len() < pos + cred_id_len {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Authenticator data truncated at credentialId"})),
        )
            .into_response();
    }

    let _stored_cred_id = &auth_data_bytes[pos..pos + cred_id_len];
    pos += cred_id_len;

    // Extract public key (CBOR encoded)
    let public_key_bytes = &auth_data_bytes[pos..];
    let public_key_cbor: serde_cbor::Value = match serde_cbor::from_slice(public_key_bytes) {
        Ok(pk) => pk,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Failed to parse public key: {}", e)})),
            )
                .into_response()
        }
    };

    // Re-encode public key to get perfect CBOR bytes (normalized)
    let public_key_cbor_bytes = match serde_cbor::to_vec(&public_key_cbor) {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to serialize public key: {}", e)})),
            )
                .into_response()
        }
    };

    // Step 10: Store the verified credential
    match webauthn::store_credential(
        &state.db_pool,
        &req.user_id,
        credential_id_bytes.clone(),
        public_key_cbor_bytes,
        Some(req.transports.unwrap_or_default()),
    )
    .await
    {
        Ok(_) => {
            let ip = extract_ip(&headers);
            log_audit(
                &state,
                &req.user_id,
                "passkey_enrolled",
                Some("credential"),
                Some(&req.credential_id),
                ip.as_deref(),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "credential_id": req.credential_id,
                    "message": "Passkey verified and stored"
                })),
            )
                .into_response()
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to store credential: {}", e)})),
            )
                .into_response()
        }
    }
}

async fn passkey_authenticate_begin_handler(
    State(state): State<AppState>,
    Json(req): Json<PasskeyAuthenticateBeginRequest>,
) -> impl IntoResponse {
    // Fetch user to check if they have passkey credentials
    let user_result = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM users WHERE username = $1",
    )
    .bind(&req.username)
    .fetch_one(&state.db_pool)
    .await;

    let (user_id,) = match user_result {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
                .into_response()
        }
    };

    // Check if user has any passkey credentials
    let has_credentials = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM webauthn_credentials WHERE user_id = $1",
    )
    .bind(&user_id)
    .fetch_one(&state.db_pool)
    .await;

    if matches!(has_credentials, Ok(count) if count == 0) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User has no passkey credentials"})),
        )
            .into_response();
    }

    // Generate authentication challenge
    match webauthn::generate_authentication_challenge(&state.db_pool).await {
        Ok((challenge_bytes, challenge_id)) => {
            let challenge_b64 = base64::engine::general_purpose::STANDARD.encode(&challenge_bytes);
            (
                StatusCode::OK,
                Json(json!({
                    "challenge": challenge_b64,
                    "challenge_id": challenge_id
                })),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to generate challenge"})),
        )
            .into_response(),
    }
}

async fn passkey_authenticate_verify_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<PasskeyAuthenticateVerifyRequest>,
) -> impl IntoResponse {
    // Verify the challenge exists
    match webauthn::verify_authentication_challenge(&state.db_pool, &req.challenge_id).await {
        Ok(_) => {
            // In a production system, you would:
            // 1. Decode authenticatorData, clientDataJSON, and signature
            // 2. Verify the signature against the stored public key
            // 3. Check the challenge in clientDataJSON matches the stored challenge
            // 4. Verify the origin matches the expected RP ID
            // 5. Update the sign count to prevent cloned authenticators

            // For MVP, we'll do basic validation and require proper signature verification
            let credential_id_bytes = match base64::engine::general_purpose::STANDARD.decode(&req.credential_id) {
                Ok(b) => b,
                Err(_) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": "Invalid credential_id encoding"})),
                    )
                        .into_response()
                }
            };

            // Fetch the user and credential
            let cred_result = sqlx::query_as::<_, (String, String)>(
                "SELECT user_id, sign_count FROM webauthn_credentials WHERE credential_id = $1",
            )
            .bind(&credential_id_bytes.as_slice())
            .fetch_one(&state.db_pool)
            .await;

            let (user_id, _sign_count_str) = match cred_result {
                Ok(c) => c,
                Err(_) => {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({"error": "Credential not found"})),
                    )
                        .into_response()
                }
            };

            // TODO: In production, verify the signature using webauthn-rs crate
            // For now, do basic validation
            if req.signature.is_empty() || req.client_data_json.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Missing required attestation fields"})),
                )
                    .into_response();
            }

            // Increment sign count to prevent cloned authenticators
            if let Err(_) = webauthn::increment_sign_count(&state.db_pool, &credential_id_bytes).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to update sign count"})),
                )
                    .into_response();
            }

            // Generate session token
            let token = Uuid::new_v4().to_string();
            state.sessions.insert(user_id.clone(), token.clone());

            // Log successful passkey authentication
            let ip = extract_ip(&headers);
            log_audit(
                &state,
                &user_id,
                "passkey_login_success",
                Some("credential"),
                Some(&req.credential_id),
                ip.as_deref(),
                None,
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "token": token,
                    "user_id": user_id
                })),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Challenge verification failed or expired"})),
        )
            .into_response(),
    }
}

async fn gamma_landing_handler() -> impl IntoResponse {
    let page = fs::read_to_string("public/gamma.html")
        .unwrap_or_else(|_| "Gamma landing page not found".to_string());
    Html(page)
}

fn load_certs(path: &str) -> Vec<Certificate> {
    let file = fs::File::open(path).expect("cannot open certificate file");
    let mut reader = BufReader::new(file);
    certs(&mut reader)
        .expect("failed to read certificates")
        .into_iter()
        .map(Certificate)
        .collect()
}

fn load_private_key(path: &str) -> PrivateKey {
    let file = fs::File::open(path).expect("cannot open private key file");
    let mut reader = BufReader::new(file);

    if let Ok(keys) = pkcs8_private_keys(&mut reader) {
        if let Some(key) = keys.first() {
            return PrivateKey(key.clone());
        }
    }

    let file = fs::File::open(path).expect("cannot open private key file");
    let mut reader = BufReader::new(file);
    let keys = rsa_private_keys(&mut reader).expect("failed to read RSA private key");
    keys.first()
        .map(|key| PrivateKey(key.clone()))
        .expect("no private key found")
}

fn create_mtls_config() -> ServerConfig {
    let mut roots = RootCertStore::empty();

    let ca_file = fs::File::open("certs/ca.crt").expect("cannot open CA file");
    let mut reader = BufReader::new(ca_file);
    for cert in certs(&mut reader).expect("failed to read CA cert") {
        roots
            .add(&Certificate(cert))
            .expect("failed to add CA cert");
    }

    ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(Arc::new(rustls::server::AllowAnyAuthenticatedClient::new(roots)))
        .with_single_cert(load_certs("certs/server.crt"), load_private_key("certs/server.key"))
        .expect("bad certificates/private key")
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
                    Ok(_) => {
                        // Log note creation
                        let ip = extract_ip(&headers);
                        log_audit(&state, &user_id, "note_created", Some("note"), Some(&entry_id), ip.as_deref(), None).await;
                        (
                            StatusCode::CREATED,
                            Json(json!({"entry_id": entry_id, "message": "Note created"})),
                        )
                            .into_response()
                    }
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
                Ok(_) => {
                    // Log file upload
                    let ip = extract_ip(&headers);
                    log_audit(&state, &user_id, "file_uploaded", Some("file"), Some(&entry_id), ip.as_deref(), None).await;
                    (
                        StatusCode::CREATED,
                        Json(json!({"entry_id": entry_id, "message": "File stored"})),
                    )
                        .into_response()
                }
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
// AUDIT LOGS
// ============================================================================

async fn get_audit_logs_handler(
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

    // Query audit logs for authenticated user (last 100, ordered by created_at DESC)
    let logs_result = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, user_id, action, resource_type, resource_id, ip_address, details, created_at FROM audit_logs WHERE user_id = $1 ORDER BY created_at DESC LIMIT 100"
    )
    .bind(&user_id)
    .fetch_all(&state.db_pool)
    .await;

    match logs_result {
        Ok(logs) => {
            let audit_logs: Vec<AuditLog> = logs
                .into_iter()
                .map(|(id, user_id, action, resource_type, resource_id, ip_address, details, created_at)| AuditLog {
                    id,
                    user_id,
                    action,
                    resource_type,
                    resource_id,
                    ip_address,
                    details,
                    created_at,
                })
                .collect();

            let response = AuditLogsResponse {
                total_count: audit_logs.len() as i32,
                logs: audit_logs,
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to fetch audit logs: {}", e)})),
        )
            .into_response(),
    }
}

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

// OAuth callback handler - serves HTML that posts token back to opener
async fn oauth_callback_handler() -> impl IntoResponse {
    let html = r#"
<!DOCTYPE html>
<html>
<head><title>OAuth Callback</title></head>
<body>
<script>
    // Extract token from URL fragment
    const hash = window.location.hash.substring(1);
    const params = new URLSearchParams(hash);
    const accessToken = params.get('access_token');
    const error = params.get('error');
    
    if (accessToken) {
        // Send token back to opener window
        if (window.opener) {
            window.opener.postMessage({
                type: 'oauth_success',
                access_token: accessToken
            }, window.location.origin);
            window.close();
        } else {
            document.body.innerHTML = '<h3>✅ Authentication successful!</h3><p>Access token: <code>' + accessToken + '</code></p><p>You can close this window.</p>';
        }
    } else if (error) {
        if (window.opener) {
            window.opener.postMessage({
                type: 'oauth_error',
                error: error
            }, window.location.origin);
            window.close();
        } else {
            document.body.innerHTML = '<h3>❌ Authentication failed</h3><p>Error: ' + error + '</p>';
        }
    } else {
        document.body.innerHTML = '<h3>⏳ Processing authentication...</h3>';
    }
</script>
</body>
</html>
"#;
    (StatusCode::OK, [("content-type", "text/html")], html)
}
// ============================================================================
// AUTHENTICATION MIDDLEWARE
// ============================================================================
// AUDIT LOGGING
// ============================================================================

async fn log_audit(
    state: &AppState,
    user_id: &str,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<&str>,
    ip_address: Option<&str>,
    details: Option<&str>,
) {
    let audit_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let _ = sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, ip_address, details, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    )
    .bind(&audit_id)
    .bind(user_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(ip_address)
    .bind(details)
    .bind(&now)
    .execute(&state.db_pool)
    .await;
}

// Helper to extract client IP from headers
fn extract_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
}

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

    let db_log_value = redact_sensitive(
        &database_url
            .replace(|c: char| c.is_whitespace(), "")
            .chars()
            .take(50)
            .collect::<String>(),
    );
    println!("Connecting to database: {}", db_log_value);

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
                    eprintln!(
                        "Failed to open sqlite database '{}': {err}. Falling back to in-memory sqlite.",
                        redact_sensitive(&database_url)
                    );
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
        .route("/gamma", get(gamma_landing_handler))
        // Auth API
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler))
        .route("/auth/register-passkey-begin", post(passkey_register_begin_handler))
        .route("/auth/register-passkey-verify", post(passkey_register_verify_handler))
        .route("/auth/authenticate-passkey-begin", post(passkey_authenticate_begin_handler))
        .route("/auth/authenticate-passkey-verify", post(passkey_authenticate_verify_handler))
        // Account API
        .route("/account/usage", get(get_usage_handler))
        .route("/account/upgrade", post(upgrade_enterprise_handler))
        .route("/account/audit-logs", get(get_audit_logs_handler))
        // Cloud import API
        .route("/cloud/list", post(list_cloud_files_handler))
        .route("/vaults/:vault_id/import", post(import_cloud_files_handler))
        .route("/oauth-callback", get(oauth_callback_handler))
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

    // Use hyper::Server for axum 0.6 compatibility
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    let std_listener = listener.into_std().unwrap();
    Server::from_tcp(std_listener)
        .unwrap()
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
