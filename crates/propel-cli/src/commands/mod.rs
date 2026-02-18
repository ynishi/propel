mod ci;
mod deploy;
mod destroy;
mod doctor;
mod eject;
mod init;
mod logs;
pub(crate) mod mcp;
mod new;
mod secret;
mod status;

use propel_core::{ProjectMeta, PropelConfig};

/// Artifact Registry repository name used for container images.
pub(crate) const ARTIFACT_REPO_NAME: &str = "propel";

/// Resolve the Cloud Run service name: config override or Cargo package name.
pub(crate) fn service_name<'a>(config: &'a PropelConfig, meta: &'a ProjectMeta) -> &'a str {
    config.project.name.as_deref().unwrap_or(&meta.name)
}

/// Build the Artifact Registry image path (without tag).
pub(crate) fn image_path(region: &str, project_id: &str, repo: &str, service: &str) -> String {
    format!("{region}-docker.pkg.dev/{project_id}/{repo}/{service}")
}

/// Initial `propel.toml` template with comprehensive documentation.
///
/// This is the only configuration file users need to write after `propel new`
/// or `propel init`, so every field is documented inline with defaults,
/// valid values, and usage notes.
pub(crate) const PROPEL_TOML_TEMPLATE: &str = r##"# ============================================================================
# propel.toml — Propel configuration
# ============================================================================
#
# This file controls how `propel deploy` builds and deploys your Rust
# application to Google Cloud Run.
#
# All sections and fields are optional. Sensible defaults are provided
# so a bare `propel.toml` (or even an empty file) is perfectly valid.
#
# Workflow:
#   propel new <name>   — scaffold a new project (generates this file)
#   propel init         — add Propel to an existing Rust project
#   propel deploy       — build & deploy to Cloud Run
#   propel eject        — take full control of the Dockerfile
#
# Escalation path for build customization:
#   Zero config  →  [build] include/env  →  propel eject
#   (all-in)        (selective)              (full Dockerfile control)

# ── Project ─────────────────────────────────────────────────────────────────
#
# Identifies your project and its target GCP environment.

[project]

# GCP project ID (REQUIRED for deploy).
#
# Find yours with:  gcloud config get-value project
# gcp_project_id = "your-project-id"

# Service name override. Defaults to Cargo.toml [package].name.
# Useful when the crate name differs from the desired Cloud Run service name.
# name = "my-service"

# GCP region for Cloud Run deployment.
# See available regions: gcloud run regions list
# Default: "us-central1"
# region = "us-central1"

# ── Build ───────────────────────────────────────────────────────────────────
#
# Controls Docker image generation.
#
# Propel generates a multi-stage Dockerfile using Cargo Chef for optimal
# layer caching:
#   Stage 1 (Planner)  — extract dependency recipe
#   Stage 2 (Cacher)   — pre-build dependencies (cached between deploys)
#   Stage 3 (Builder)  — compile the application
#   Stage 4 (Runtime)  — minimal image with binary + runtime assets

[build]

# Rust builder image. Must be >= 1.85 for edition = "2024".
# Default: "rust:1.93-bookworm"
# base_image = "rust:1.93-bookworm"

# Runtime base image. Distroless is recommended for minimal attack surface.
# Default: "gcr.io/distroless/cc-debian12"
#
# Common alternatives:
#   "debian:bookworm-slim"       — when you need a shell for debugging
#   "gcr.io/distroless/cc-debian12"  — minimal, no shell (default)
# runtime_image = "gcr.io/distroless/cc-debian12"

# System packages to install via apt-get during the build stage.
# Needed when your crate depends on C libraries (e.g. OpenSSL, libpq).
#
# Example:
#   extra_packages = ["libssl-dev", "pkg-config"]
# extra_packages = []

# Cargo Chef version for dependency caching.
# Default: "0.1.73"
# cargo_chef_version = "0.1.73"

# Paths to copy into the runtime image.
#
# By default (when omitted), the entire build context is copied into the
# runtime container via `COPY . .`. This means migrations, templates,
# Lua scripts, and any other files in your repo are available at runtime
# with zero configuration.
#
# When specified, ONLY the listed paths (plus the compiled binary) are
# copied into the runtime image. This produces a smaller, cleaner image.
#
# Convention:
#   - Directories MUST end with `/`   →  COPY dir/ ./dir/
#   - Files MUST NOT end with `/`     →  COPY file ./file
#
# Examples:
#   include = ["migrations/"]
#   include = ["lua/", "seeds.txt", "templates/"]
#   include = []                # binary only, no extra files
# include = ["migrations/", "templates/"]

# Static environment variables baked into the container image.
#
# These become `ENV` directives in the generated Dockerfile and are
# available to the application at runtime.
#
# For sensitive values (API keys, tokens), use Cloud Run environment
# variables or Secret Manager via `propel secret set KEY=VALUE` instead.
#
# [build.env]
# TEMPLATE_DIR = "/app/templates"
# RUST_LOG = "info"

# ── Cloud Run ───────────────────────────────────────────────────────────────
#
# Cloud Run service configuration. These map directly to `gcloud run deploy`
# flags. See: https://cloud.google.com/run/docs/configuring/memory-limits

[cloud_run]

# Port the application listens on. Must match your code's bind address.
# Default: 8080
# port = 8080

# Memory allocation per instance.
# Valid: "128Mi", "256Mi", "512Mi", "1Gi", "2Gi", "4Gi", ...up to "32Gi"
# Default: "512Mi"
# memory = "512Mi"

# CPU count per instance.
# Valid: 1, 2, 4, 8
# Default: 1
# cpu = 1

# Minimum number of instances to keep warm (avoids cold starts).
# Default: 0 (scale to zero)
# Billing note: min_instances > 0 incurs idle charges.
# min_instances = 0

# Maximum number of instances to scale up to.
# Default: 10
# max_instances = 10

# Maximum concurrent requests per instance.
# Default: 80
# Higher values improve throughput; lower values improve per-request latency.
# concurrency = 80
"##;

/// Extract `gcp_project_id` from config, returning a clear error if not set.
fn require_gcp_project_id(config: &PropelConfig) -> anyhow::Result<&str> {
    config.project.gcp_project_id.as_deref().ok_or_else(|| {
        anyhow::anyhow!("gcp_project_id not set in propel.toml — set [project].gcp_project_id")
    })
}

pub use ci::ci_init;
pub use deploy::deploy;
pub use destroy::destroy;
pub use doctor::doctor;
pub use eject::eject;
pub use init::init_project;
pub use logs::logs;
pub use new::new_project;
pub use secret::{secret_delete, secret_list, secret_set};
pub use status::status;
