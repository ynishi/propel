# propel-core

Core types, configuration, and error handling for [Propel](https://github.com/ynishi/propel).

## Overview

This crate provides the shared foundation used by all other Propel crates:

- **`PropelConfig`** — `propel.toml` schema (project settings, build options, Cloud Run config)
- **`ProjectMeta`** — Project name, version, and binary name extracted from `Cargo.toml`
- **`Error` / `Result`** — Shared error types via `thiserror`

## Usage

```toml
[dependencies]
propel-core = "0.2"
```

```rust,ignore
use propel_core::{PropelConfig, ProjectMeta};

let config = PropelConfig::load(".")?;
let meta = ProjectMeta::from_cargo_toml(".")?;
```

## Part of the Propel workspace

| Crate | Description |
|-------|-------------|
| **propel-core** | Configuration and shared types (this crate) |
| [propel-build](https://crates.io/crates/propel-build) | Dockerfile generation and source bundling |
| [propel-cloud](https://crates.io/crates/propel-cloud) | GCP Cloud Run / Cloud Build operations |
| [propel-sdk](https://crates.io/crates/propel-sdk) | Axum middleware for Supabase Auth |
| [propel-cli](https://crates.io/crates/propel-cli) | CLI binary (`propel` command) |
| [propel](https://crates.io/crates/propel) | Unified facade crate |

## License

MIT OR Apache-2.0
