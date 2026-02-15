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
- Provides Supabase Auth JWT middleware for Axum (via `propel-sdk`)

## Install

```bash
cargo install --path crates/propel-cli
```

## Prerequisites

- [Google Cloud SDK](https://cloud.google.com/sdk/docs/install) (`gcloud` CLI)
- A GCP project with billing enabled
- Required APIs: Cloud Build, Cloud Run, Secret Manager, Artifact Registry

See [docs/gcp-setup.md](docs/gcp-setup.md) for the full setup guide.

## Commands

| Command | Description |
|---------|-------------|
| `propel new <name>` | Scaffold a new project |
| `propel deploy` | Build and deploy to Cloud Run |
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
base_image = "rust:1.84-bookworm"            # Rust build image
runtime_image = "gcr.io/distroless/cc-debian12" # Minimal runtime
extra_packages = []                           # apt-get packages
cargo_chef_version = "0.1.68"

[cloud_run]
memory = "512Mi"
cpu = 1
min_instances = 0
max_instances = 10
concurrency = 80
port = 8080
```

Run `propel eject` to export and customize the generated Dockerfile.

## Architecture

```
crates/
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
