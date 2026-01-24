// Zero-Knowledge Vault - MVP Backend
// Features: Register, Login, Create Vault, Store/Retrieve Encrypted Notes

mod crypto;
mod models;

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post, get_service},
    Json, Router,
};
use base64::Engine;
use dashmap::DashMap;
use serde_json::json;
use sqlx::AnyPool;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::{Mutex, RwLock};
use tower_http::services::ServeDir;
use uuid::Uuid;
use chrono::Utc;

use models::*;
use crypto::*;

#[derive(Clone)]
struct AppState {
    db_pool: AnyPool,
    admin_token: String,
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
    // Derive encryption key from password
    let (key, salt) = match derive_key(&req.password) {
        Ok(k) => k,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    // Hash password with Argon2id for storage
    let password_hash = match argon2::password_hash::PasswordHasher::hash_password(
        &argon2::Argon2::default(),
        req.password.as_bytes(),
        &argon2::password_hash::SaltString::generate(rand::thread_rng()),
    ) {
        Ok(h) => h.to_string(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to hash password"}))).into_response(),
    };

    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Insert user into DB
    let result = sqlx::query(
        "INSERT INTO users (id, username, password_hash, encryption_key_salt, subscription_tier, storage_limit_gb, storage_used_gb, created_at, updated_at) 
         VALUES ($1, $2, $3, $4, 'Starter', 5, 0, $5, $5)"
    )
    .bind(&user_id)
    .bind(&req.username)
    .bind(&password_hash)
    .bind(&salt)
    .bind(&now)
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_) => (StatusCode::CREATED, Json(json!({"user_id": user_id, "message": "User created"}))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Registration failed: {}", e)}))).into_response(),
    }
}

async fn login_handler(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    // Fetch user from DB
    let user_result = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, username, password_hash, encryption_key_salt FROM users WHERE username = $1"
    )
    .bind(&req.username)
    .fetch_one(&state.db_pool)
    .await;

    let (user_id, username, password_hash, salt) = match user_result {
        Ok(u) => u,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"}))).into_response(),
    };

    // Verify password with Argon2id
    let parsed_hash = match argon2::PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Invalid hash"}))).into_response(),
    };

    if argon2::PasswordVerifier::verify_password(&argon2::Argon2::default(), req.password.as_bytes(), &parsed_hash).is_err() {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"}))).into_response();
    }

    // Generate session token
    let token = Uuid::new_v4().to_string();
    state.sessions.insert(user_id.clone(), token.clone());

    (StatusCode::OK, Json(json!({
        "token": token,
        "user_id": user_id,
        "username": username
    }))).into_response()
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
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response(),
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
        Ok(_) => (StatusCode::CREATED, Json(json!({"vault_id": vault_id, "name": req.name}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to create vault: {}", e)}))).into_response(),
    }
}

async fn list_vaults_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response(),
    };

    let vaults_result = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, name, description, created_at FROM vaults WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(&user_id)
    .fetch_all(&state.db_pool)
    .await;

    match vaults_result {
        Ok(vaults) => {
            let vault_list: Vec<VaultResponse> = vaults.into_iter().map(|(id, name, description, created_at)| {
                VaultResponse { id, name, description, created_at }
            }).collect();
            (StatusCode::OK, Json(json!({"vaults": vault_list}))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to fetch vaults: {}", e)}))).into_response(),
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
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response(),
    };

    // Verify vault ownership
    let vault_check = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM vaults WHERE id = $1 AND user_id = $2"
    )
    .bind(&vault_id)
    .bind(&user_id)
    .fetch_one(&state.db_pool)
    .await;

    if vault_check.unwrap_or(0) == 0 {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Vault not found or access denied"}))).into_response();
    }

    // Decode base64 encrypted content
    let encrypted_content = match base64::engine::general_purpose::STANDARD.decode(&req.encrypted_content) {
        Ok(c) => c,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid base64 content"}))).into_response(),
    };

    let entry_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        "INSERT INTO vault_entries (id, vault_id, entry_type, encrypted_content, nonce, file_size_bytes, created_at, updated_at) 
         VALUES ($1, $2, 'note', $3, $4, NULL, $5, $5)"
    )
    .bind(&entry_id)
    .bind(&vault_id)
    .bind(&encrypted_content)
    .bind(&req.nonce)
    .bind(&now)
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_) => (StatusCode::CREATED, Json(json!({"entry_id": entry_id, "message": "Note created"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to create note: {}", e)}))).into_response(),
    }
}

async fn list_entries_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(vault_id): AxumPath<String>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers).await {
        Some(uid) => uid,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response(),
    };

    // Verify vault ownership
    let vault_check = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM vaults WHERE id = $1 AND user_id = $2"
    )
    .bind(&vault_id)
    .bind(&user_id)
    .fetch_one(&state.db_pool)
    .await;

    if vault_check.unwrap_or(0) == 0 {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Vault not found or access denied"}))).into_response();
    }

    let entries_result = sqlx::query_as::<_, (String, String, Vec<u8>, String, String)>(
        "SELECT id, entry_type, encrypted_content, nonce, created_at FROM vault_entries WHERE vault_id = $1 ORDER BY created_at DESC"
    )
    .bind(&vault_id)
    .fetch_all(&state.db_pool)
    .await;

    match entries_result {
        Ok(entries) => {
            let entry_list: Vec<VaultEntryResponse> = entries.into_iter().map(|(id, entry_type, encrypted_content, nonce, created_at)| {
                VaultEntryResponse {
                    id,
                    entry_type,
                    encrypted_content: base64::engine::general_purpose::STANDARD.encode(&encrypted_content),
                    nonce,
                    created_at,
                }
            }).collect();
            (StatusCode::OK, Json(json!({"entries": entry_list}))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to fetch entries: {}", e)}))).into_response(),
    }
}

// ============================================================================
// HEALTH & UTILITY
// ============================================================================

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok", "service": "vault-api"}))).into_response()
}

// ============================================================================
// AUTHENTICATION MIDDLEWARE
// ============================================================================

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let token = headers.get("authorization")?
        .to_str().ok()?
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

    // Add SSL mode for Postgres on Railway if not already present
    if (database_url.starts_with("postgres://") || database_url.starts_with("postgresql://")) && !database_url.contains("sslmode=") {
        database_url = format!("{}?sslmode=prefer", database_url);
    }
    
    println!("Connecting to database: {}", database_url.replace(|c: char| c.is_whitespace(), "").chars().take(50).collect::<String>());

    // Try to connect with exponential backoff for Postgres startup
    let mut pool = None;
    let mut retry_count = 0;
    let max_retries = 10;
    
    while pool.is_none() && retry_count < max_retries {
        match AnyPool::connect(&database_url).await {
            Ok(p) => pool = Some(p),
            Err(err) => {
                if database_url.starts_with("sqlite://") {
                    eprintln!("Failed to open sqlite database '{}': {err}. Falling back to in-memory sqlite.", database_url);
                    pool = Some(AnyPool::connect("sqlite::memory:").await.expect("failed to connect to in-memory sqlite"));
                } else {
                    retry_count += 1;
                    if retry_count < max_retries {
                        let wait_time = std::time::Duration::from_secs(2_u64.pow(retry_count as u32 - 1));
                        eprintln!("DB connection failed (attempt {}/{}): {err}. Retrying in {:?}...", retry_count, max_retries, wait_time);
                        tokio::time::sleep(wait_time).await;
                    }
                }
            }
        }
    }

    if pool.is_none() {
        eprintln!("DB connection failed after {} attempts. Falling back to sqlite::memory: for this run.", max_retries);
        pool = Some(AnyPool::connect("sqlite::memory:").await.expect("failed to connect to in-memory sqlite"));
    }

    let mut pool = pool.expect("pool not initialized");

    // Run migrations
    println!("Applying migrations...");
    let migrator = sqlx::migrate!("./migrations");
    let migration_result = migrator.run(&pool).await;

    // If the database has a stale migrations table (e.g., old versions), drop and retry once
    if let Err(sqlx::migrate::MigrateError::VersionMissing(_)) = migration_result {
        eprintln!("Detected stale migration history. Dropping _sqlx_migrations and retrying...");
        // Best-effort drop; ignore errors so we can retry
        let _ = sqlx::query("DROP TABLE IF EXISTS _sqlx_migrations").execute(&pool).await;
        migrator.run(&pool).await.expect("migrations failed after reset");
    } else {
        migration_result.expect("migrations failed");
    }
    println!("Migrations applied.");

    let admin_token = env::var("ADMIN_TOKEN").unwrap_or_else(|_| "admintoken".to_string());

    let state = AppState {
        db_pool: pool,
        admin_token,
        sessions: Arc::new(DashMap::new()),
    };

    let app = Router::new()
        // Health check (must come before static files)
        .route("/health", get(health_handler))
        // Auth API
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler))
        // Vaults API
        .route("/vaults/create", post(create_vault_handler))
        .route("/vaults/list", get(list_vaults_handler))
        // Notes API
        .route("/vaults/:vault_id/notes", post(create_note_handler))
        .route("/vaults/:vault_id/entries", get(list_entries_handler))
        .with_state(state.clone())
        // Serve static files (frontend) - must come last
        .fallback_service(get_service(ServeDir::new("public")).handle_error(|_| async {
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serve file")
        }));

    let port = env::var("PORT").ok().and_then(|s| s.parse::<u16>().ok()).unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("🔒 Vault API listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
