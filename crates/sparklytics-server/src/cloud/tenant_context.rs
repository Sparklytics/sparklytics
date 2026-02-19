use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::json;

/// Role within a Clerk Organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrgRole {
    Admin,
    Member,
    Viewer,
}

impl OrgRole {
    fn from_str(s: &str) -> Self {
        match s {
            "admin" | "org:admin" => OrgRole::Admin,
            "viewer" | "org:viewer" => OrgRole::Viewer,
            _ => OrgRole::Member,
        }
    }
}

/// Extracted from a Clerk JWT on every cloud-mode request.
///
/// The Clerk JWT payload contains:
/// ```json
/// { "sub": "user_abc", "o": { "id": "org_xyz", "slg": "acme", "rol": "admin" } }
/// ```
///
/// `FromRequestParts` returns:
/// - `401` if the Authorization header is missing or the token is malformed.
/// - `403 "Organization context required"` if the token has no `o` (org) claim.
#[derive(Debug, Clone)]
pub struct TenantContext {
    /// Clerk Organization ID (`o.id`), used as `tenant_id` in all queries.
    pub tenant_id: String,
    /// Clerk User ID (`sub`).
    pub user_id: String,
    /// Role within the organization.
    pub role: OrgRole,
}

/// Rejection returned by [`TenantContext`]'s `FromRequestParts` impl.
pub enum TenantError {
    Unauthenticated,
    Forbidden(String),
}

impl IntoResponse for TenantError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match self {
            TenantError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Unauthorized".to_string(),
            ),
            TenantError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg),
        };
        (
            status,
            Json(json!({ "error": { "code": code, "message": msg, "field": null } })),
        )
            .into_response()
    }
}

impl<S> FromRequestParts<S> for TenantContext
where
    S: Send + Sync,
{
    type Rejection = TenantError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract Authorization: Bearer <token>
        let auth = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(TenantError::Unauthenticated)?;

        let token = auth
            .strip_prefix("Bearer ")
            .ok_or(TenantError::Unauthenticated)?;

        // Decode the JWT payload without re-verifying the signature.
        // ClerkLayer (applied as a router layer) has already verified the token.
        // We only need the payload claims.
        let claims = decode_payload(token).ok_or(TenantError::Unauthenticated)?;

        let user_id = claims
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or(TenantError::Unauthenticated)?
            .to_string();

        let org = claims
            .get("o")
            .and_then(|v| v.as_object())
            .ok_or_else(|| TenantError::Forbidden("Organization context required".to_string()))?;

        let tenant_id = org
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TenantError::Forbidden("Organization context required".to_string()))?
            .to_string();

        let role_str = org.get("rol").and_then(|v| v.as_str()).unwrap_or("member");

        Ok(TenantContext {
            tenant_id,
            user_id,
            role: OrgRole::from_str(role_str),
        })
    }
}

/// Decode the payload section of a JWT (base64url, middle segment) without
/// verifying the signature.  ClerkLayer is responsible for verification.
fn decode_payload(token: &str) -> Option<serde_json::Value> {
    let payload_b64 = token.split('.').nth(1)?;
    // URL_SAFE_NO_PAD handles the base64url alphabet and missing padding.
    let bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    serde_json::from_slice(&bytes).ok()
}
