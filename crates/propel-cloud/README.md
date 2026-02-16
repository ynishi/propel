# propel-cloud

GCP Cloud Run and Cloud Build operations for [Propel](https://github.com/ynishi/propel).

## Overview

- **`GcloudClient`** — Wrapper around the `gcloud` CLI for Cloud Build, Cloud Run, Secret Manager, and Artifact Registry
- **`GcloudExecutor` trait** — Abstraction over command execution (enables testing with `mockall`)
- **`RealExecutor`** — Production implementation that runs actual `gcloud` commands
- Preflight checks, doctor reports, and structured error types

## Usage

```toml
[dependencies]
propel-cloud = "0.2"
```

```rust,ignore
use propel_cloud::{GcloudClient, RealExecutor};

let client = GcloudClient::new();
let report = client.doctor("my-gcp-project").await?;
```

## Part of the Propel workspace

| Crate | Description |
|-------|-------------|
| [propel-core](https://crates.io/crates/propel-core) | Configuration and shared types |
| [propel-build](https://crates.io/crates/propel-build) | Dockerfile generation and source bundling |
| **propel-cloud** | GCP Cloud Run / Cloud Build operations (this crate) |
| [propel-sdk](https://crates.io/crates/propel-sdk) | Axum middleware for Supabase Auth |
| [propel-cli](https://crates.io/crates/propel-cli) | CLI binary (`propel` command) |
| [propel](https://crates.io/crates/propel) | Unified facade crate |

## License

MIT OR Apache-2.0
