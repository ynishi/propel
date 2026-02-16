//! Core types and configuration for propel.
//!
//! This crate defines the `propel.toml` schema ([`PropelConfig`]),
//! project metadata ([`ProjectMeta`]), and shared error types.

pub mod config;
pub mod error;
pub mod project;

pub use config::{BuildConfig, CloudRunConfig, ProjectConfig, PropelConfig};
pub use error::{Error, Result};
pub use project::ProjectMeta;
