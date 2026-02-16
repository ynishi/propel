# Propel

Deploy Rust (Axum) apps to Google Cloud Run. Zero-config.

```
propel new my-app
cd my-app
propel deploy
```

## What it does

- Generates an optimized multi-stage Dockerfile using [Cargo Chef](https://github.com/LukeMathWalker/cargo-chef) (4 stages: planner, cacher, builder, distroless runtime)
- Submits remote builds via Cloud Build
- Deploys to Cloud Run with Artifact Registry
- Manages secrets through Secret Manager
- Provides Supabase Auth JWT middleware for Axum (via `propel` crate)

## Requirements

- **Rust ≥ 1.85** (edition 2024)
- [Google Cloud SDK](https://cloud.google.com/sdk/docs/install) (`gcloud` CLI)
- A GCP project with billing enabled
- Required APIs: Cloud Build, Cloud Run, Secret Manager, Artifact Registry

## Install

```bash
cargo install propel-cli
```

### As a library (Supabase Auth middleware for Axum)

```toml
[dependencies]
propel = "0.3"
```

See [docs/gcp-setup.md](docs/gcp-setup.md) for the full setup guide.

## Commands

| Command | Description |
|---------|-------------|
| `propel new <name>` | Scaffold a new project |
| `propel init` | Add Propel to an existing project |
| `propel deploy` | Build and deploy to Cloud Run |
| `propel deploy --allow-dirty` | Deploy with uncommitted changes |
| `propel destroy` | Delete service, image, and local bundle |
| `propel doctor` | Check GCP setup and readiness |
| `propel secret set KEY=VALUE` | Store a secret in Secret Manager |
| `propel secret list` | List stored secrets |
| `propel status` | Show Cloud Run service status |
| `propel logs` | Read Cloud Run logs |
| `propel eject` | Export Dockerfile for manual customization |

## Quick Start

### 1. Setup GCP

```bash
gcloud auth login
gcloud projects create your-project-id --name="your-project-id"
gcloud billing projects link your-project-id --billing-account=your-billing-account-id
gcloud services enable \
  cloudbuild.googleapis.com \
  run.googleapis.com \
  secretmanager.googleapis.com \
  artifactregistry.googleapis.com \
  --project your-project-id
```

### 2. Verify

```bash
propel doctor
```

```
Propel Doctor
------------------------------
gcloud CLI            OK  555.0.0
Authentication        OK  you@example.com
GCP Project           OK  your-project-id
Billing               OK  Enabled
Cloud Build API       OK  Enabled
Cloud Run API         OK  Enabled
Secret Manager API    OK  Enabled
Artifact Registry API OK  Enabled
propel.toml           OK  Found
------------------------------
All checks passed!
```

### 3. Create and deploy

```bash
propel new my-app
cd my-app

# Edit propel.toml — set gcp_project_id
propel deploy
```

### 4. Cleanup

```bash
propel destroy
```

## Configuration

`propel.toml`:

```toml
[project]
gcp_project_id = "your-project-id"
region = "asia-northeast1"

[build]
base_image = "rust:1.93-bookworm"            # Rust build image
runtime_image = "gcr.io/distroless/cc-debian12" # Minimal runtime
extra_packages = []                           # apt-get packages
cargo_chef_version = "0.1.73"

[cloud_run]
memory = "512Mi"
cpu = 1
min_instances = 0
max_instances = 10
concurrency = 80
port = 8080
```

### Bundle and runtime

By default, `propel deploy` bundles **all files in your git repository** (respecting `.gitignore`) and copies them into the runtime container. This means `migrations/`, `templates/`, `static/`, and any other committed files are available at runtime with zero configuration.

If you want a smaller runtime image, use `[build.include]` to select specific paths:

```toml
[build]
include = ["migrations/", "templates/"]

[build.env]
TEMPLATE_DIR = "/app/templates"
```

| Scenario | What to do |
|----------|-----------|
| Migrations, templates, config files | Nothing — they're included by default |
| Optimize runtime image size | Add `include = [...]` to select specific paths |
| Static env vars tied to image layout | Add `[build.env]` entries |
| Full Dockerfile control | Run `propel eject` |

### Dirty check

`propel deploy` verifies your git working tree is clean before deploying.
This prevents accidentally shipping uncommitted changes. Use `--allow-dirty` to override:

```bash
propel deploy                  # Requires clean working tree
propel deploy --allow-dirty    # Skips the check
```

## Crates

| Crate | crates.io | Description |
|-------|-----------|-------------|
| [`propel`](crates/propel) | [![crates.io](https://img.shields.io/crates/v/propel.svg)](https://crates.io/crates/propel) | Axum middleware for Supabase Auth on Cloud Run |
| [`propel-cli`](crates/propel-cli) | [![crates.io](https://img.shields.io/crates/v/propel-cli.svg)](https://crates.io/crates/propel-cli) | CLI binary (`propel` command) |
| [`propel-core`](crates/propel-core) | [![crates.io](https://img.shields.io/crates/v/propel-core.svg)](https://crates.io/crates/propel-core) | Configuration, project metadata, shared error types |
| [`propel-build`](crates/propel-build) | [![crates.io](https://img.shields.io/crates/v/propel-build.svg)](https://crates.io/crates/propel-build) | Dockerfile generation, source bundling, eject |
| [`propel-cloud`](crates/propel-cloud) | [![crates.io](https://img.shields.io/crates/v/propel-cloud.svg)](https://crates.io/crates/propel-cloud) | GCP operations (Cloud Build, Cloud Run, Secret Manager) |
| [`propel-sdk`](crates/propel-sdk) | [![crates.io](https://img.shields.io/crates/v/propel-sdk.svg)](https://crates.io/crates/propel-sdk) | **Deprecated** — use `propel` instead |

```text
propel (facade)
├── propel-core     ← PropelConfig, ProjectMeta, Error
├── propel-build    ← DockerfileGenerator, bundle, eject
├── propel-cloud    ← GcloudClient, GcloudExecutor
└── propel-sdk      ← DEPRECATED (re-exports propel)

propel-cli          ← CLI binary using core + build + cloud
```

### Architecture

```text
crates/
  propel/        Unified facade crate
  propel-cli/    CLI binary (clap)
  propel-core/   Config loading, project metadata
  propel-build/  Dockerfile generation, bundling, eject
  propel-cloud/  GCP operations (Cloud Build, Cloud Run, Secret Manager)
  propel-sdk/    Axum middleware (Supabase Auth JWT)
examples/
  hello-axum/    Minimal example project
docs/
  gcp-setup.md   Full GCP setup guide
```

## Examples

See [`examples/hello-axum/`](examples/hello-axum/) for a minimal Axum project that deploys with Propel.

## License

MIT OR Apache-2.0
