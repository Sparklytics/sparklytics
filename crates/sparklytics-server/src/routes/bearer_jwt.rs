use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::error::AppError;

const JWKS_CACHE_TTL_SECONDS: u64 = 300;
const JWKS_HTTP_TIMEOUT_SECONDS: u64 = 3;

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Clone)]
struct CachedJwksKeys {
    keys_by_kid: HashMap<String, DecodingKey>,
    expires_at: Instant,
}

fn jwks_cache() -> &'static RwLock<Option<CachedJwksKeys>> {
    static CACHE: OnceLock<RwLock<Option<CachedJwksKeys>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(JWKS_HTTP_TIMEOUT_SECONDS))
            .build()
            .expect("failed to build JWKS http client")
    })
}

async fn fetch_jwks_keys(jwks_url: &str) -> Result<HashMap<String, DecodingKey>, AppError> {
    let response = http_client()
        .get(jwks_url)
        .send()
        .await
        .map_err(|_| AppError::Unauthorized)?;
    if !response.status().is_success() {
        return Err(AppError::Unauthorized);
    }

    let payload: JwksResponse = response.json().await.map_err(|_| AppError::Unauthorized)?;
    let mut keys_by_kid = HashMap::new();
    for key in payload.keys {
        if key.kty.as_deref() != Some("RSA") {
            continue;
        }
        let Some(kid) = key.kid else {
            continue;
        };
        let Some(n) = key.n else {
            continue;
        };
        let Some(e) = key.e else {
            continue;
        };
        let decoding_key =
            DecodingKey::from_rsa_components(&n, &e).map_err(|_| AppError::Unauthorized)?;
        keys_by_kid.insert(kid, decoding_key);
    }

    if keys_by_kid.is_empty() {
        return Err(AppError::Unauthorized);
    }
    Ok(keys_by_kid)
}

async fn get_decoding_key(kid: &str) -> Result<DecodingKey, AppError> {
    let jwks_url =
        std::env::var("SPARKLYTICS_CLOUD_JWKS_URL").map_err(|_| AppError::Unauthorized)?;
    let parsed_jwks_url = reqwest::Url::parse(&jwks_url).map_err(|_| AppError::Unauthorized)?;
    if parsed_jwks_url.scheme() != "https" {
        let is_localhost = matches!(
            parsed_jwks_url.host_str(),
            Some("localhost") | Some("127.0.0.1")
        );
        if !is_localhost {
            return Err(AppError::Unauthorized);
        }
    }

    let now = Instant::now();

    {
        let guard = jwks_cache().read().await;
        if let Some(cache) = guard.as_ref() {
            if cache.expires_at > now {
                return cache
                    .keys_by_kid
                    .get(kid)
                    .cloned()
                    .ok_or(AppError::Unauthorized);
            }
        }
    }

    let refreshed_keys = fetch_jwks_keys(&jwks_url).await?;
    let expires_at = Instant::now() + Duration::from_secs(JWKS_CACHE_TTL_SECONDS);
    let selected = refreshed_keys
        .get(kid)
        .cloned()
        .ok_or(AppError::Unauthorized)?;

    let mut write_guard = jwks_cache().write().await;
    if let Some(cache) = write_guard.as_ref() {
        if cache.expires_at > now {
            return cache
                .keys_by_kid
                .get(kid)
                .cloned()
                .ok_or(AppError::Unauthorized);
        }
    }
    *write_guard = Some(CachedJwksKeys {
        keys_by_kid: refreshed_keys,
        expires_at,
    });
    Ok(selected)
}

pub async fn verify_and_decode_bearer_claims(headers: &HeaderMap) -> Result<Value, AppError> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = auth.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;

    let header = decode_header(token).map_err(|_| AppError::Unauthorized)?;
    if header.alg != Algorithm::RS256 {
        return Err(AppError::Unauthorized);
    }
    let kid = header.kid.ok_or(AppError::Unauthorized)?;
    let key = get_decoding_key(&kid).await?;

    let issuer = std::env::var("SPARKLYTICS_CLOUD_JWT_ISSUER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(AppError::Unauthorized)?;
    let audience = std::env::var("SPARKLYTICS_CLOUD_JWT_AUDIENCE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(AppError::Unauthorized)?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.leeway = 30;
    validation.required_spec_claims.clear();
    validation.required_spec_claims.extend([
        "exp".to_string(),
        "iss".to_string(),
        "aud".to_string(),
    ]);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);

    let token_data =
        decode::<Value>(token, &key, &validation).map_err(|_| AppError::Unauthorized)?;
    Ok(token_data.claims)
}
