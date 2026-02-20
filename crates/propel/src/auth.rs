use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use secrecy::ExposeSecret;
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

/// Authenticated identity attached to request extensions.
///
/// Handlers extract this to determine how the request was authenticated:
///
/// ```rust,no_run
/// use axum::extract::Request;
/// use axum::body::Body;
/// use propel::AuthIdentity;
///
/// async fn handler(req: Request<Body>) -> String {
///     match req.extensions().get::<AuthIdentity>().unwrap() {
///         AuthIdentity::User(claims) => format!("user: {}", claims.sub),
///         AuthIdentity::ServiceRole(claims) => format!("service: {}", claims.sub),
///         AuthIdentity::ServerKey => "server".to_owned(),
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub enum AuthIdentity {
    /// Authenticated via Supabase user JWT (`role` is not `"service_role"`).
    User(SupabaseClaims),
    /// Authenticated via Supabase service_role JWT.
    ServiceRole(SupabaseClaims),
    /// Authenticated via pre-shared server key (`X-Server-Key` header).
    ServerKey,
}

/// Axum middleware that verifies Supabase JWT tokens and server keys.
///
/// Authentication methods (checked in order):
///
/// 1. **Server Key** — `X-Server-Key` header matching `PROPEL_SERVER_KEY` env var
/// 2. **Supabase JWT** — `Authorization: Bearer <token>` with HS256 verification
///    - `role: "service_role"` → [`AuthIdentity::ServiceRole`]
///    - Other roles → [`AuthIdentity::User`]
///
/// Usage:
/// ```rust,no_run
/// use axum::{Router, middleware, routing::get};
/// use propel::{PropelState, PropelAuth};
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
        // 1. Try X-Server-Key header first (cheap constant-time check)
        if let Some(key) = request
            .headers()
            .get("x-server-key")
            // arch-lint: allow(no-silent-result-drop) reason="non-ASCII HeaderValue is invalid for server key; treating as absent"
            .and_then(|v| v.to_str().ok())
        {
            let expected = state.server_key.as_ref().ok_or_else(|| {
                tracing::warn!(
                    path = %request.uri(),
                    "X-Server-Key header sent but PROPEL_SERVER_KEY not configured",
                );
                StatusCode::UNAUTHORIZED
            })?;

            if !constant_time_eq(key.as_bytes(), expected.expose_secret().as_bytes()) {
                tracing::warn!(path = %request.uri(), "invalid server key");
                return Err(StatusCode::UNAUTHORIZED);
            }

            tracing::debug!(path = %request.uri(), "authenticated via server key");
            request.extensions_mut().insert(AuthIdentity::ServerKey);
            return Ok(next.run(request).await);
        }

        // 2. Fall back to Authorization: Bearer JWT
        let auth_header = request
            .headers()
            .get("authorization")
            // arch-lint: allow(no-silent-result-drop) reason="non-ASCII Authorization header is malformed; treating as absent triggers 401"
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                tracing::warn!(path = %request.uri(), "missing authentication");
                StatusCode::UNAUTHORIZED
            })?;

        let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
            tracing::warn!(path = %request.uri(), "malformed Authorization header");
            StatusCode::UNAUTHORIZED
        })?;

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["authenticated"]);

        let key = DecodingKey::from_secret(state.supabase_jwt_secret.expose_secret().as_bytes());

        let token_data = decode::<SupabaseClaims>(token, &key, &validation).map_err(|e| {
            tracing::warn!(path = %request.uri(), error = %e, "JWT verification failed");
            StatusCode::UNAUTHORIZED
        })?;

        let claims = token_data.claims;
        let identity = if claims.role.as_deref() == Some("service_role") {
            tracing::debug!(sub = %claims.sub, "authenticated as service_role");
            AuthIdentity::ServiceRole(claims.clone())
        } else {
            tracing::debug!(sub = %claims.sub, "authenticated as user");
            AuthIdentity::User(claims.clone())
        };

        // Attach both AuthIdentity and SupabaseClaims (backward compat)
        request.extensions_mut().insert(identity);
        request.extensions_mut().insert(claims);

        Ok(next.run(request).await)
    }
}

/// Constant-time byte comparison to prevent timing attacks on server key validation.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
