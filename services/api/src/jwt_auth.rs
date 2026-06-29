use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use warp::{Filter, Rejection};

/// JWT claims for WebSocket authentication.
#[derive(Debug, Serialize, Deserialize)]
pub struct WsClaims {
    /// Subject (client identifier, e.g. "web-ui").
    pub sub: String,
    /// Issued-at (unix ms).
    pub iat: usize,
    /// Expiry (unix ms).
    pub exp: usize,
}

/// Auth rejection types.
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ExpiredToken,
}

impl warp::reject::Reject for AuthError {}

/// Returns the JWT secret from the `API_JWT_SECRET` env var, or `None` if auth
/// is disabled (development mode).
fn secret_bytes() -> Option<Vec<u8>> {
    std::env::var("API_JWT_SECRET").ok().map(|s| s.into_bytes())
}

/// Whether WS auth is enabled (API_JWT_SECRET is set).
pub fn is_enabled() -> bool {
    secret_bytes().is_some()
}

/// Verify a JWT token and return its claims.
pub fn verify_ws_token(token: &str) -> Result<WsClaims, AuthError> {
    let secret = secret_bytes().ok_or(AuthError::InvalidToken)?;
    decode::<WsClaims>(
        token,
        &DecodingKey::from_secret(&secret),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
        _ => AuthError::InvalidToken,
    })
}

/// Issue a short-lived JWT for WebSocket access.
pub fn issue_ws_token(sub: &str, ttl_seconds: u64) -> Result<String, String> {
    let secret = secret_bytes().ok_or("API_JWT_SECRET not set".to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;
    let claims = WsClaims {
        sub: sub.to_string(),
        iat: now,
        exp: now + ttl_seconds as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&secret),
    )
    .map_err(|e| format!("JWT issuance failed: {}", e))
}

/// Warp filter that extracts a valid JWT token from the `token` query param.
/// If `API_JWT_SECRET` is not set, allows all requests (dev mode).
///
/// Usage:
/// ```ignore
/// warp::path("ws")
///     .and(jwt_auth::ws_auth_filter())
///     .and(warp::ws())
///     ...
/// ```
pub fn ws_auth_filter() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    // When auth is disabled, skip the filter entirely (extract nothing, never reject).
    if !is_enabled() {
        return warp::any().boxed();
    }

    warp::any()
        .and(warp::query::<HashMap<String, String>>())
        .and_then(|params: HashMap<String, String>| async move {
            let token = params
                .get("token")
                .ok_or_else(|| warp::reject::custom(AuthError::MissingToken))?;
            verify_ws_token(token).map_err(|_| warp::reject::custom(AuthError::InvalidToken))?;
            Ok::<(), warp::Rejection>(())
        })
        .untuple_one()
        .boxed()
}
