//! Axum middleware for Supabase Auth on Google Cloud Run.
//!
//! # Quick start
//!
//! ```toml
//! [dependencies]
//! propel = "0.2"
//! ```
//!
//! ```rust,no_run
//! use axum::{routing::get, middleware, Router};
//! use propel::{PropelState, PropelAuth};
//!
//! async fn handler() -> &'static str { "ok" }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let state = PropelState::load().unwrap();
//! let app: Router = Router::new()
//!     .route("/api/protected", get(handler))
//!     .layer(middleware::from_fn_with_state(state.clone(), PropelAuth::verify))
//!     .with_state(state);
//! # }
//! ```

pub mod auth;
pub mod error;
pub mod state;

pub use auth::{PropelAuth, SupabaseClaims};
pub use error::SdkError;
pub use state::PropelState;
