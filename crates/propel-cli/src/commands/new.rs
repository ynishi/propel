use std::path::Path;

/// Scaffold a new Propel project.
pub async fn new_project(name: &str) -> anyhow::Result<()> {
    let project_dir = Path::new(name);
    if project_dir.exists() {
        anyhow::bail!("directory '{}' already exists", name);
    }

    std::fs::create_dir_all(project_dir.join("src"))?;

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = "0.8"
tokio = {{ version = "1", features = ["full"] }}
propel-sdk = "0.1"
tracing = "0.1"
tracing-subscriber = "0.3"
"#
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // main.rs
    let main_rs = r#"use axum::{routing::get, middleware, Router};
use propel_sdk::{PropelState, PropelAuth};

async fn health() -> &'static str {
    "ok"
}

async fn hello() -> &'static str {
    "Hello from Propel!"
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = PropelState::load().expect("failed to load config");

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(hello))
        .layer(middleware::from_fn_with_state(state.clone(), PropelAuth::verify))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind");

    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
"#;
    std::fs::write(project_dir.join("src/main.rs"), main_rs)?;

    // propel.toml
    let propel_toml = r#"[project]
# region = "us-central1"
# gcp_project_id = "your-project-id"

[build]
# extra_packages = []

[cloud_run]
# memory = "512Mi"
# cpu = 1
# max_instances = 10
"#;
    std::fs::write(project_dir.join("propel.toml"), propel_toml)?;

    // .env.example
    let env_example = r#"SUPABASE_URL=https://your-project.supabase.co
SUPABASE_ANON_KEY=your-anon-key
SUPABASE_JWT_SECRET=your-jwt-secret
"#;
    std::fs::write(project_dir.join(".env.example"), env_example)?;

    // .gitignore
    let gitignore = "/target\n.env\n.s2-bundle/\n";
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;

    eprintln!("Created project '{name}'");
    eprintln!();
    eprintln!("  cd {name}");
    eprintln!("  cp .env.example .env   # configure Supabase credentials");
    eprintln!("  cargo run              # local development");
    eprintln!("  propel deploy          # deploy to Cloud Run");

    Ok(())
}
