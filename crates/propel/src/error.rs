#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    #[error("missing environment variable: {0}")]
    MissingEnvVar(String),

    #[error("invalid JWT: {0}")]
    InvalidJwt(String),

    #[error("JWT verification failed")]
    JwtVerification(#[from] jsonwebtoken::errors::Error),

    #[error("failed to fetch JWKS: {0}")]
    JwksFetch(String),
}
