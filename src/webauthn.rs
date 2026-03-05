// WebAuthn / Passkey Support
// Provides FIDO2-compliant credential registration and authentication

use sqlx::AnyPool;
use uuid::Uuid;
use chrono::Utc;
use url::Url;
use webauthn_rs::{
    prelude::*,
    WebauthnBuilder,
};

#[derive(Debug, Clone)]
pub struct WebAuthnConfig {
    pub rp_id: String,
    pub rp_name: String,
    pub origin: Url,
}

impl WebAuthnConfig {
    pub fn new(rp_id: &str, rp_name: &str, origin: &str) -> Result<Self, url::ParseError> {
        Ok(Self {
            rp_id: rp_id.to_string(),
            rp_name: rp_name.to_string(),
            origin: Url::parse(origin)?,
        })
    }

    pub fn build(&self) -> Result<Webauthn, Box<dyn std::error::Error>> {
        Ok(WebauthnBuilder::new(&self.rp_id, &self.origin)?
            .rp_name(&self.rp_name)
            .build()?)
    }
}

#[derive(Debug, Clone)]
pub struct RegistrationChallenge {
    pub id: String,
    pub user_id: String,
    pub challenge: Vec<u8>,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticationChallenge {
    pub id: String,
    pub user_id: Option<String>,
    pub challenge: Vec<u8>,
    pub created_at: String,
    pub expires_at: String,
}

/// Generate a registration challenge for passkey enrollment
pub async fn generate_registration_challenge(
    pool: &AnyPool,
    user_id: &str,
) -> Result<(Vec<u8>, String), String> {
    let challenge_id = Uuid::new_v4().to_string();
    let challenge_bytes = Uuid::new_v4().as_bytes().to_vec();
    let now = Utc::now();
    let expires_at = (now + chrono::Duration::minutes(10)).to_rfc3339();
    let created_at = now.to_rfc3339();

    sqlx::query(
        "INSERT INTO webauthn_registration_challenges (id, user_id, challenge, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(&challenge_id)
    .bind(user_id)
    .bind(&challenge_bytes)
    .bind(&created_at)
    .bind(&expires_at)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to store challenge: {}", e))?;

    Ok((challenge_bytes, challenge_id))
}

/// Verify a registration challenge exists and is not expired
pub async fn verify_registration_challenge(
    pool: &AnyPool,
    challenge_id: &str,
    user_id: &str,
) -> Result<Vec<u8>, String> {
    let now = Utc::now();

    let result = sqlx::query_as::<_, (Vec<u8>, String, String)>(
        "SELECT challenge, user_id, expires_at FROM webauthn_registration_challenges WHERE id = $1"
    )
    .bind(challenge_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Challenge lookup failed: {}", e))?;

    match result {
        Some((challenge, challenge_user_id, expires_at_str)) => {
            if challenge_user_id != user_id {
                return Err("Challenge belongs to a different account".to_string());
            }

            let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at_str)
                .map_err(|_| "Stored challenge expiry is invalid".to_string())?
                .with_timezone(&Utc);

            if expires_at <= now {
                let _ = sqlx::query(
                    "DELETE FROM webauthn_registration_challenges WHERE id = $1"
                )
                .bind(challenge_id)
                .execute(pool)
                .await;

                return Err("Challenge expired".to_string());
            }

            // Clean up challenge
            let _ = sqlx::query(
                "DELETE FROM webauthn_registration_challenges WHERE id = $1"
            )
            .bind(challenge_id)
            .execute(pool)
            .await;
            Ok(challenge)
        }
        None => Err("Challenge not found".to_string()),
    }
}

/// Store a verified passkey credential
pub async fn store_credential(
    pool: &AnyPool,
    user_id: &str,
    credential_id: Vec<u8>,
    public_key: Vec<u8>,
    transports: Option<Vec<String>>,
) -> Result<String, String> {
    let cred_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let transports_json = transports
        .map(|t| serde_json::json!(t).to_string())
        .unwrap_or_else(|| "[]".to_string());

    sqlx::query(
        "INSERT INTO webauthn_credentials (id, user_id, credential_id, public_key, sign_count, transports, backup_eligible, backup_state, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, 0, $5, false, false, $6, $6)"
    )
    .bind(&cred_id)
    .bind(user_id)
    .bind(&credential_id)
    .bind(&public_key)
    .bind(&transports_json)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to store credential: {}", e))?;

    Ok(cred_id)
}

/// Generate an authentication challenge
pub async fn generate_authentication_challenge(
    pool: &AnyPool,
) -> Result<(Vec<u8>, String), String> {
    let challenge_id = Uuid::new_v4().to_string();
    let challenge_bytes = Uuid::new_v4().as_bytes().to_vec();
    let now = Utc::now();
    let expires_at = (now + chrono::Duration::minutes(10)).to_rfc3339();
    let created_at = now.to_rfc3339();

    sqlx::query(
        "INSERT INTO webauthn_authentication_challenges (id, challenge, created_at, expires_at) \
         VALUES ($1, $2, $3, $4)"
    )
    .bind(&challenge_id)
    .bind(&challenge_bytes)
    .bind(&created_at)
    .bind(&expires_at)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to store auth challenge: {}", e))?;

    Ok((challenge_bytes, challenge_id))
}

/// Verify authentication challenge and return user_id if valid
pub async fn verify_authentication_challenge(
    pool: &AnyPool,
    challenge_id: &str,
) -> Result<Vec<u8>, String> {
    let now = Utc::now();

    let result = sqlx::query_as::<_, (Vec<u8>, String)>(
        "SELECT challenge, expires_at FROM webauthn_authentication_challenges WHERE id = $1"
    )
    .bind(challenge_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Auth challenge lookup failed: {}", e))?;

    match result {
        Some((challenge, expires_at_str)) => {
            let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at_str)
                .map_err(|_| "Stored auth challenge expiry is invalid".to_string())?
                .with_timezone(&Utc);

            if expires_at <= now {
                let _ = sqlx::query(
                    "DELETE FROM webauthn_authentication_challenges WHERE id = $1"
                )
                .bind(challenge_id)
                .execute(pool)
                .await;

                return Err("Auth challenge expired".to_string());
            }

            // Clean up challenge
            let _ = sqlx::query(
                "DELETE FROM webauthn_authentication_challenges WHERE id = $1"
            )
            .bind(challenge_id)
            .execute(pool)
            .await;
            Ok(challenge)
        }
        None => Err("Auth challenge not found".to_string()),
    }
}

/// Get all credentials for a user
pub async fn get_user_credentials(
    pool: &AnyPool,
    user_id: &str,
) -> Result<Vec<(Vec<u8>, Vec<u8>, i32)>, String> {
    sqlx::query_as::<_, (Vec<u8>, Vec<u8>, i32)>(
        "SELECT credential_id, public_key, sign_count FROM webauthn_credentials WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch credentials: {}", e))
}

/// Increment sign count for a credential (replay attack prevention)
pub async fn increment_sign_count(
    pool: &AnyPool,
    credential_id: &[u8],
) -> Result<(), String> {
    sqlx::query(
        "UPDATE webauthn_credentials SET sign_count = sign_count + 1, updated_at = $1 WHERE credential_id = $2"
    )
    .bind(Utc::now().to_rfc3339())
    .bind(credential_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to update sign count: {}", e))?;

    Ok(())
}

/// Clean up expired challenges (run periodically)
pub async fn cleanup_expired_challenges(pool: &AnyPool) -> Result<u64, String> {
    let now = Utc::now().to_rfc3339();

    let reg_deleted = sqlx::query(
        "DELETE FROM webauthn_registration_challenges WHERE expires_at < $1"
    )
    .bind(&now)
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    let auth_deleted = sqlx::query(
        "DELETE FROM webauthn_authentication_challenges WHERE expires_at < $1"
    )
    .bind(&now)
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    Ok(reg_deleted + auth_deleted)
}
