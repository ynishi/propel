//! Core types and configuration for propel.
//!
//! This crate defines the `propel.toml` schema ([`PropelConfig`]),
//! Cargo project discovery ([`CargoProject`]), and shared error types.

pub mod cargo;
pub mod config;
pub mod error;

pub use cargo::{CargoBinary, CargoProject};
pub use config::{BuildConfig, CloudRunConfig, ProjectConfig, PropelConfig};
pub use error::{Error, Result};
