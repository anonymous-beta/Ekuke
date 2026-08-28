use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use rand::RngCore;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const PBKDF2_ITERATIONS: u32 = 600_000;

pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);
    
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, PBKDF2_ITERATIONS, &mut key);
    
    let cipher = Aes256Gcm::new_from_slice(&key)
        .context("Failed to initialize cipher")?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher.encrypt(nonce, data)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
    
    let mut result = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    
    Ok(result)
}

pub fn decrypt(encrypted_data: &[u8], password: &str) -> Result<Vec<u8>> {
    if encrypted_data.len() < SALT_LEN + NONCE_LEN {
        anyhow::bail!("Invalid encrypted data: too short");
    }
    
    let salt = &encrypted_data[..SALT_LEN];
    let nonce_bytes = &encrypted_data[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &encrypted_data[SALT_LEN + NONCE_LEN..];
    
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    
    let cipher = Aes256Gcm::new_from_slice(&key)
        .context("Failed to initialize cipher")?;
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed (wrong password?): {}", e))?;
    
    Ok(plaintext)
}
