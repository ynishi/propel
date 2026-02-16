use crate::error::SdkError;

/// Application state that loads configuration from environment variables.
///
/// Locally reads from `.env` via dotenvy, in production reads from
/// Cloud Run environment variables (injected via Secret Manager).
#[derive(Debug, Clone)]
pub struct PropelState {
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub supabase_jwt_secret: String,
}

impl PropelState {
    /// Load state from environment variables.
    ///
    /// Call this in your `main()`:
    /// ```ignore
    /// let state = PropelState::load().expect("failed to load config");
    /// ```
    pub fn load() -> Result<Self, SdkError> {
        // Attempt to load .env file (silently ignore if not found)
        let dotenv_loaded = dotenvy::dotenv().is_ok();
        tracing::debug!(dotenv = dotenv_loaded, "loading PropelState");

        let state = Self {
            supabase_url: required_env("SUPABASE_URL")?,
            supabase_anon_key: required_env("SUPABASE_ANON_KEY")?,
            supabase_jwt_secret: required_env("SUPABASE_JWT_SECRET")?,
        };

        tracing::debug!(supabase_url = %state.supabase_url, "PropelState loaded");
        Ok(state)
    }
}

fn required_env(key: &str) -> Result<String, SdkError> {
    std::env::var(key).map_err(|_| SdkError::MissingEnvVar(key.to_owned()))
}
