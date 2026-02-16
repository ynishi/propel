use std::fmt::Write;

use propel_core::{BuildConfig, ProjectMeta};

/// Generates an optimized multi-stage Dockerfile using Cargo Chef.
///
/// The Dockerfile has four stages:
///
/// 1. **Planner** — `cargo chef prepare` extracts the dependency recipe
/// 2. **Cacher** — `cargo chef cook` pre-builds dependencies (cached layer)
/// 3. **Builder** — `cargo build --release` compiles the application
/// 4. **Runtime** — minimal distroless image with binary + runtime assets
///
/// # Runtime stage behavior
///
/// The runtime stage content depends on [`BuildConfig::include`]:
///
/// - **`None`** (default): `COPY . .` — entire bundle is available at runtime.
///   Migrations, templates, and config files work without any configuration.
///
/// - **`Some(paths)`**: only the specified paths are copied via individual
///   `COPY` directives. The binary is always copied regardless.
///
/// [`BuildConfig::env`] entries become `ENV` directives in the runtime stage.
pub struct DockerfileGenerator<'a> {
    config: &'a BuildConfig,
    meta: &'a ProjectMeta,
    port: u16,
}

impl<'a> DockerfileGenerator<'a> {
    pub fn new(config: &'a BuildConfig, meta: &'a ProjectMeta, port: u16) -> Self {
        Self { config, meta, port }
    }

    pub fn render(&self) -> String {
        tracing::debug!(
            base = %self.config.base_image,
            runtime = %self.config.runtime_image,
            binary = %self.meta.binary_name,
            port = self.port,
            "generating Dockerfile"
        );
        let extra_packages = if self.config.extra_packages.is_empty() {
            String::new()
        } else {
            format!(
                "RUN apt-get update && apt-get install -y {} && rm -rf /var/lib/apt/lists/*\n",
                self.config.extra_packages.join(" ")
            )
        };

        let runtime_copies = self.render_runtime_copies();
        let env_directives = self.render_env_directives();

        format!(
            r#"# === Base: cargo-chef installed once ===
FROM {base} AS chef
RUN cargo install cargo-chef --version {chef_version} --locked
WORKDIR /app

# === Stage 1: Planner ===
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# === Stage 2: Cacher (dependency build) ===
FROM chef AS cacher
{extra_packages}COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# === Stage 3: Builder ===
FROM chef AS builder
{extra_packages}COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
COPY . .
RUN cargo build --release --bin {binary}

# === Stage 4: Runtime ===
FROM {runtime}
COPY --from=builder /app/target/release/{binary} /usr/local/bin/app
WORKDIR /app
{runtime_copies}{env_directives}EXPOSE {port}
CMD ["app"]
"#,
            base = self.config.base_image,
            chef_version = self.config.cargo_chef_version,
            runtime = self.config.runtime_image,
            binary = self.meta.binary_name,
            extra_packages = extra_packages,
            runtime_copies = runtime_copies,
            env_directives = env_directives,
            port = self.port,
        )
    }

    /// Generates COPY directives for the runtime stage.
    ///
    /// - `include = None`: copies entire build context (`COPY . .`)
    /// - `include = Some(paths)`: copies only specified paths
    fn render_runtime_copies(&self) -> String {
        match &self.config.include {
            None => "COPY . .\n".to_owned(),
            Some(paths) if paths.is_empty() => String::new(),
            Some(paths) => {
                let mut out = String::new();
                for path in paths {
                    let trimmed = path.trim_end_matches('/');
                    let _ = writeln!(out, "COPY {trimmed}/ ./{trimmed}/");
                }
                out
            }
        }
    }

    /// Generates ENV directives from `[build.env]`.
    fn render_env_directives(&self) -> String {
        if self.config.env.is_empty() {
            return String::new();
        }

        let mut keys: Vec<&String> = self.config.env.keys().collect();
        keys.sort();

        let mut out = String::new();
        for key in keys {
            let value = &self.config.env[key];
            let _ = writeln!(out, "ENV {key}={value}");
        }
        out
    }
}
