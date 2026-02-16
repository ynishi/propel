# propel-sdk

Axum middleware and helpers for [Propel](https://github.com/ynishi/propel) — Supabase Auth JWT verification on Cloud Run.

## Overview

- **`PropelAuth`** — Axum middleware that verifies Supabase JWT tokens
- **`PropelState`** — Application state loaded from environment variables (`.env` or Cloud Run secrets)

## Usage

```toml
[dependencies]
propel-sdk = "0.2"
```

```rust,ignore
use axum::{Router, middleware};
use propel_sdk::{PropelAuth, PropelState};

let state = PropelState::load()?;
let app = Router::new()
    .route("/api/protected", get(handler))
    .layer(middleware::from_fn_with_state(
        state.clone(),
        PropelAuth::verify,
    ))
    .with_state(state);
```

## Stability

**This crate is pre-1.0.** Breaking changes may occur in minor version updates.

The SDK will expand around Axum + Supabase integration (auth, database, storage). The API will stabilize at 1.0.

## Part of the Propel workspace

| Crate | Description |
|-------|-------------|
| [propel-core](https://crates.io/crates/propel-core) | Configuration and shared types |
| [propel-build](https://crates.io/crates/propel-build) | Dockerfile generation and source bundling |
| [propel-cloud](https://crates.io/crates/propel-cloud) | GCP Cloud Run / Cloud Build operations |
| **propel-sdk** | Axum middleware for Supabase Auth (this crate) |
| [propel-cli](https://crates.io/crates/propel-cli) | CLI binary (`propel` command) |
| [propel](https://crates.io/crates/propel) | Unified facade crate |

## License

MIT OR Apache-2.0
