use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

/// Encode a JWT token.
///
/// Returns (token_string, expires_at_rfc3339).
pub fn encode_jwt(secret: &str, session_days: u32) -> Result<(String, String)> {
    let now = Utc::now();
    let exp = now + Duration::days(session_days as i64);

    let claims = Claims {
        sub: "admin".to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow!("encode_jwt: {}", e))?;

    Ok((token, exp.to_rfc3339()))
}

/// Decode and validate a JWT token.
pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| anyhow!("decode_jwt: {}", e))?;

    Ok(data.claims)
}
