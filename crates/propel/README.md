# propel

Unified facade crate for [Propel](https://github.com/ynishi/propel) — deploy Rust (Axum) apps to Google Cloud Run.

## Usage

```toml
[dependencies]
propel = "0.2"
```

```rust,ignore
use propel::{PropelConfig, ProjectMeta};
use propel::build::DockerfileGenerator;

let config = PropelConfig::load(".")?;
let meta = ProjectMeta::from_cargo_toml(".")?;
let generator = DockerfileGenerator::new(&config, &meta, 8080);
let dockerfile = generator.render();
```

## Feature flags

| Feature | Default | Crate | Description |
|---------|---------|-------|-------------|
| `core` | yes | [propel-core](https://crates.io/crates/propel-core) | Configuration and shared types |
| `build` | yes | [propel-build](https://crates.io/crates/propel-build) | Dockerfile generation and bundling |
| `cloud` | yes | [propel-cloud](https://crates.io/crates/propel-cloud) | GCP Cloud Run / Cloud Build operations |
| `sdk` | no | [propel-sdk](https://crates.io/crates/propel-sdk) | Axum middleware for Supabase Auth |

### SDK feature

```toml
[dependencies]
propel = { version = "0.2", features = ["sdk"] }
```

```rust,ignore
use propel::sdk::{PropelAuth, PropelState};
```

**Stability:** The `sdk` module is pre-1.0. Breaking changes may occur in minor version updates as the Axum + Supabase integration expands toward auth, database, and storage features.

## Crate structure

```text
propel (this crate)
  re-exports:
  ├── propel-core   — PropelConfig, ProjectMeta, Error
  ├── propel-build  — DockerfileGenerator, bundle, eject
  ├── propel-cloud  — GcloudClient, GcloudExecutor
  └── propel-sdk    — PropelAuth, PropelState (feature = "sdk")
```

## License

MIT OR Apache-2.0
