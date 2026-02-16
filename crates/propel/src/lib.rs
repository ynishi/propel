//! Deploy Rust (Axum) apps to Google Cloud Run.
//!
//! This is the unified facade crate that re-exports all Propel sub-crates.
//! Use feature flags to control which components are included.
//!
//! # Feature flags
//!
//! | Feature | Default | Crate | Description |
//! |---------|---------|-------|-------------|
//! | `core` | yes | [`propel-core`](https://crates.io/crates/propel-core) | Configuration and shared types |
//! | `build` | yes | [`propel-build`](https://crates.io/crates/propel-build) | Dockerfile generation and bundling |
//! | `cloud` | yes | [`propel-cloud`](https://crates.io/crates/propel-cloud) | GCP Cloud Run / Cloud Build operations |
//! | `sdk` | no | [`propel-sdk`](https://crates.io/crates/propel-sdk) | Axum middleware for Supabase Auth |
//!
//! # Quick start
//!
//! ```toml
//! [dependencies]
//! propel = "0.2"
//! ```
//!
//! ```rust,no_run
//! use std::path::Path;
//! use propel::{PropelConfig, ProjectMeta};
//! use propel::build::DockerfileGenerator;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = PropelConfig::load(Path::new("."))?;
//! let meta = ProjectMeta::from_cargo_toml(Path::new("."))?;
//! let generator = DockerfileGenerator::new(&config.build, &meta, config.cloud_run.port);
//! let dockerfile = generator.render();
//! # Ok(())
//! # }
//! ```

// Core types flattened into root namespace for convenience.
#[cfg(feature = "core")]
pub use propel_core::*;

/// Dockerfile generation, source bundling, and eject.
///
/// See [`propel-build`](https://crates.io/crates/propel-build) for details.
#[cfg(feature = "build")]
pub mod build {
    pub use propel_build::*;
}

/// GCP Cloud Run and Cloud Build operations.
///
/// See [`propel-cloud`](https://crates.io/crates/propel-cloud) for details.
#[cfg(feature = "cloud")]
pub mod cloud {
    pub use propel_cloud::*;
}

/// Axum middleware for Supabase Auth JWT verification.
///
/// **Requires** the `sdk` feature flag (not enabled by default).
///
/// **Stability:** This module is pre-1.0. Breaking changes may occur in minor
/// version updates as the Axum + Supabase integration expands.
///
/// See [`propel-sdk`](https://crates.io/crates/propel-sdk) for details.
#[cfg(feature = "sdk")]
pub mod sdk {
    pub use propel_sdk::*;
}
