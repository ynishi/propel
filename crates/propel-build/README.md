# propel-build

Dockerfile generation, source bundling, and eject for [Propel](https://github.com/ynishi/propel).

## Overview

- **`DockerfileGenerator`** — Generates optimized multi-stage Dockerfiles using Cargo Chef (planner → cacher → builder → distroless runtime)
- **`bundle`** — Creates deployment bundles from `git ls-files` output
- **`eject`** — Exports generated Dockerfile for manual customization

## Usage

```toml
[dependencies]
propel-build = "0.2"
```

```rust,ignore
use propel_build::DockerfileGenerator;
use propel_core::{PropelConfig, ProjectMeta};

let config = PropelConfig::load(".")?;
let meta = ProjectMeta::from_cargo_toml(".")?;
let generator = DockerfileGenerator::new(&config, &meta, 8080);
let dockerfile = generator.render();
```

## Part of the Propel workspace

| Crate | Description |
|-------|-------------|
| [propel-core](https://crates.io/crates/propel-core) | Configuration and shared types |
| **propel-build** | Dockerfile generation and source bundling (this crate) |
| [propel-cloud](https://crates.io/crates/propel-cloud) | GCP Cloud Run / Cloud Build operations |
| [propel-sdk](https://crates.io/crates/propel-sdk) | Axum middleware for Supabase Auth |
| [propel-cli](https://crates.io/crates/propel-cli) | CLI binary (`propel` command) |
| [propel](https://crates.io/crates/propel) | Unified facade crate |

## License

MIT OR Apache-2.0
