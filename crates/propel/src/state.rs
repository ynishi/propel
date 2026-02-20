use std::fmt;

use secrecy::SecretString;

use crate::error::SdkError;

/// Application state that loads configuration from environment variables.
///
/// Locally reads from `.env` via dotenvy, in production reads from
/// Cloud Run environment variables (injected via Secret Manager).
///
/// Sensitive fields (`supabase_anon_key`, `supabase_jwt_secret`,
/// `server_key`) are wrapped in [`SecretString`] to prevent accidental
/// logging or debug output.
#[derive(Clone)]
pub struct PropelState {
    pub supabase_url: String,
    pub supabase_anon_key: SecretString,
    pub supabase_jwt_secret: SecretString,
    /// Optional pre-shared key for server-to-server authentication.
    /// Set `PROPEL_SERVER_KEY` environment variable to enable `X-Server-Key` header auth.
    pub server_key: Option<SecretString>,
}

impl fmt::Debug for PropelState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropelState")
            .field("supabase_url", &self.supabase_url)
            .field("supabase_anon_key", &"[REDACTED]")
            .field("supabase_jwt_secret", &"[REDACTED]")
            .field(
                "server_key",
                &self.server_key.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl PropelState {
    /// Load state from environment variables.
    ///
    /// Call this in your `main()`:
    /// ```rust,no_run
    /// use propel::PropelState;
    /// let state = PropelState::load().expect("failed to load config");
    /// ```
    pub fn load() -> Result<Self, SdkError> {
        // Attempt to load .env file (silently ignore if not found)
        let dotenv_loaded = dotenvy::dotenv().is_ok();
        tracing::debug!(dotenv = dotenv_loaded, "loading PropelState");

        let state = Self {
            supabase_url: required_env("SUPABASE_URL")?,
            supabase_anon_key: SecretString::from(required_env("SUPABASE_ANON_KEY")?),
            supabase_jwt_secret: SecretString::from(required_env("SUPABASE_JWT_SECRET")?),
            server_key: std::env::var("PROPEL_SERVER_KEY")
                // arch-lint: allow(no-silent-result-drop) reason="env var absence means server key is not configured — a valid operational state"
                .ok()
                .filter(|k| !k.trim().is_empty())
                .map(SecretString::from),
        };

        tracing::debug!(
            supabase_url = %state.supabase_url,
            server_key_configured = state.server_key.is_some(),
            "PropelState loaded",
        );
        Ok(state)
    }
}

fn required_env(key: &str) -> Result<String, SdkError> {
    std::env::var(key).map_err(|_| SdkError::MissingEnvVar(key.to_owned()))
}
