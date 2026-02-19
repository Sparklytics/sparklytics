use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};

/// Hash a password with Argon2id.
///
/// `m_cost` is the memory cost in KB (default 65536 = 64MB per CLAUDE.md).
pub fn hash_password(password: &str, m_cost: u32) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let params =
        Params::new(m_cost, 3, 1, Some(32)).map_err(|e| anyhow!("argon2 params: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("hash_password: {}", e))?;
    Ok(hash.to_string())
}

/// Verify a password against an Argon2id hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Validate password strength: minimum 8 characters.
pub fn validate_password_strength(password: &str) -> Result<()> {
    if password.len() < 8 {
        return Err(anyhow!("password must be at least 8 characters"));
    }
    Ok(())
}
