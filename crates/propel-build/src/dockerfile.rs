use propel_core::{BuildConfig, ProjectMeta};

/// Generates an optimized multi-stage Dockerfile using Cargo Chef.
pub struct DockerfileGenerator<'a> {
    config: &'a BuildConfig,
    meta: &'a ProjectMeta,
}

impl<'a> DockerfileGenerator<'a> {
    pub fn new(config: &'a BuildConfig, meta: &'a ProjectMeta) -> Self {
        Self { config, meta }
    }

    pub fn render(&self) -> String {
        let extra_packages = if self.config.extra_packages.is_empty() {
            String::new()
        } else {
            format!(
                "RUN apt-get update && apt-get install -y {} && rm -rf /var/lib/apt/lists/*\n",
                self.config.extra_packages.join(" ")
            )
        };

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
EXPOSE 8080
CMD ["app"]
"#,
            base = self.config.base_image,
            chef_version = self.config.cargo_chef_version,
            runtime = self.config.runtime_image,
            binary = self.meta.binary_name,
            extra_packages = extra_packages,
        )
    }
}
