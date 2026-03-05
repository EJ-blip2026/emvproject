// Zero-Knowledge Vault Backend
// Handles encrypted storage, key derivation, and sharing

use argon2::{
    password_hash::{rand_core::OsRng, SaltString, PasswordHasher},
    Argon2, Params,
};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::Rng;
use std::fmt;

// Security hardening constants for Argon2id.
const M_COST: u32 = 65536; // 64 MB
const T_COST: u32 = 3; // 3 iterations
const P_COST: u32 = 4; // parallelism

#[derive(Debug)]
pub enum CryptoError {
    EncryptionFailed,
    DecryptionFailed,
    KeyDerivationFailed,
    InvalidNonce,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::EncryptionFailed => write!(f, "Encryption failed"),
            Self::DecryptionFailed => write!(f, "Decryption failed"),
            Self::KeyDerivationFailed => write!(f, "Key derivation failed"),
            Self::InvalidNonce => write!(f, "Invalid nonce"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Derive encryption key from password using Argon2id
/// Returns (key: [u8; 32], salt: String)
pub fn derive_key(password: &str) -> Result<([u8; 32], String), CryptoError> {
    // Generate random salt
    let salt = SaltString::generate(&mut OsRng);
    let salt_str = salt.to_string();

    // Derive a 32-byte key directly into the buffer
    let params = Params::new(M_COST, T_COST, P_COST, None)
        .map_err(|_| CryptoError::KeyDerivationFailed)?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key_bytes = [0u8; 32];
    argon2
        .hash_password_into(
            password.as_bytes(),
            salt.as_salt().as_ref().as_bytes(),
            &mut key_bytes,
        )
        .map_err(|_| CryptoError::KeyDerivationFailed)?;

    Ok((key_bytes, salt_str))
}

/// Verify password against a known salt
#[allow(dead_code)]
pub fn verify_password(password: &str, salt_str: &str) -> Result<[u8; 32], CryptoError> {
    let salt = SaltString::from_b64(salt_str).map_err(|_| CryptoError::KeyDerivationFailed)?;

    let params = Params::new(M_COST, T_COST, P_COST, None)
        .map_err(|_| CryptoError::KeyDerivationFailed)?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key_bytes = [0u8; 32];
    argon2
        .hash_password_into(
            password.as_bytes(),
            salt.as_salt().as_ref().as_bytes(),
            &mut key_bytes,
        )
        .map_err(|_| CryptoError::KeyDerivationFailed)?;

    Ok(key_bytes)
}

pub fn hash_vault_key(key: &str) -> Result<String, CryptoError> {
    let salt = SaltString::generate(&mut OsRng);
    let params = Params::new(M_COST, T_COST, P_COST, None)
        .map_err(|_| CryptoError::KeyDerivationFailed)?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    argon2
        .hash_password(key.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| CryptoError::KeyDerivationFailed)
}

/// Encrypt plaintext with XChaCha20-Poly1305
/// Returns (ciphertext: Vec<u8>, nonce: String)
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<(Vec<u8>, String), CryptoError> {
    // Generate random 24-byte nonce for XChaCha20
    let mut nonce_bytes = [0u8; 24];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let nonce_str = hex::encode(&nonce_bytes);

    let key = Key::from_slice(key);
    let cipher = XChaCha20Poly1305::new(key);

    // Encrypt with authenticated encryption (tamper-evident)
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    Ok((ciphertext, nonce_str))
}

/// Decrypt ciphertext with XChaCha20-Poly1305
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce_str: &str) -> Result<Vec<u8>, CryptoError> {
    let nonce_bytes = hex::decode(nonce_str).map_err(|_| CryptoError::InvalidNonce)?;

    if nonce_bytes.len() != 24 {
        return Err(CryptoError::InvalidNonce);
    }

    let nonce = XNonce::from_slice(&nonce_bytes);
    let key = Key::from_slice(key);
    let cipher = XChaCha20Poly1305::new(key);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Generate a random API key (for programmatic access)
pub fn generate_api_key() -> String {
    use rand::distributions::Alphanumeric;
    use rand::Rng;

    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation() {
        let password = "my_secure_password";
        let (key, salt) = derive_key(password).expect("Key derivation failed");
        assert_eq!(key.len(), 32);
        assert!(!salt.is_empty());
    }

    #[test]
    fn test_encrypt_decrypt() {
        let plaintext = b"Secret message";
        let key = [0u8; 32]; // Test key

        let (ciphertext, nonce) = encrypt(plaintext, &key).expect("Encryption failed");
        let decrypted = decrypt(&ciphertext, &key, &nonce).expect("Decryption failed");

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_api_key_generation() {
        let key = generate_api_key();
        assert_eq!(key.len(), 64);
    }
}
