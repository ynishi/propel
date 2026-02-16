//! **DEPRECATED:** This crate has been merged into [`propel`](https://crates.io/crates/propel).
//!
//! Replace in your `Cargo.toml`:
//!
//! ```toml
//! # Before
//! propel-sdk = "0.2"
//!
//! # After
//! propel = "0.3"
//! ```
//!
//! Then update imports:
//!
//! ```text
//! // Before
//! use propel_sdk::{PropelAuth, PropelState};
//!
//! // After
//! use propel::{PropelAuth, PropelState};
//! ```

#[deprecated(since = "0.3.0", note = "use `propel` crate instead")]
pub use propel::PropelAuth;

#[deprecated(since = "0.3.0", note = "use `propel` crate instead")]
pub use propel::PropelState;

#[deprecated(since = "0.3.0", note = "use `propel` crate instead")]
pub use propel::SupabaseClaims;

#[deprecated(since = "0.3.0", note = "use `propel` crate instead")]
pub use propel::SdkError;

/// Re-exported for migration. Use `propel::auth` instead.
#[deprecated(since = "0.3.0", note = "use `propel::auth` instead")]
pub mod middleware {
    pub use propel::auth::*;
}

/// Re-exported for migration. Use `propel::state` instead.
#[deprecated(since = "0.3.0", note = "use `propel::state` instead")]
pub mod state {
    pub use propel::state::*;
}

/// Re-exported for migration. Use `propel::error` instead.
#[deprecated(since = "0.3.0", note = "use `propel::error` instead")]
pub mod error {
    pub use propel::error::*;
}
