use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

use crate::PropelState;

/// JWT claims from Supabase Auth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseClaims {
    pub sub: String,
    pub aud: String,
    pub email: Option<String>,
    pub role: Option<String>,
    pub exp: usize,
    pub iat: usize,
}

/// Axum middleware that verifies Supabase JWT tokens.
///
/// Usage:
/// ```rust,no_run
/// use axum::{Router, middleware, routing::get};
/// use propel_sdk::{PropelState, PropelAuth};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let state = PropelState::load().unwrap();
/// let app: Router = Router::new()
///     .route("/api/protected", get(handler))
///     .layer(middleware::from_fn_with_state(state.clone(), PropelAuth::verify))
///     .with_state(state);
/// ```
pub struct PropelAuth;

impl PropelAuth {
    pub async fn verify(
        State(state): State<PropelState>,
        mut request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let auth_header = request
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                tracing::warn!(path = %request.uri(), "missing Authorization header");
                StatusCode::UNAUTHORIZED
            })?;

        let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
            tracing::warn!(path = %request.uri(), "malformed Authorization header");
            StatusCode::UNAUTHORIZED
        })?;

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["authenticated"]);

        let key = DecodingKey::from_secret(state.supabase_jwt_secret.as_bytes());

        let token_data = decode::<SupabaseClaims>(token, &key, &validation).map_err(|e| {
            tracing::warn!(path = %request.uri(), error = %e, "JWT verification failed");
            StatusCode::UNAUTHORIZED
        })?;

        tracing::debug!(sub = %token_data.claims.sub, "authenticated");

        // Attach claims to request extensions for downstream handlers
        request.extensions_mut().insert(token_data.claims);

        Ok(next.run(request).await)
    }
}
