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
tracing = "0.1"
tracing-subscriber = "0.3"
"#
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // main.rs
    let main_rs = r#"use axum::{routing::get, Router};

async fn health() -> &'static str {
    "ok"
}

async fn hello() -> &'static str {
    "Hello from Propel!"
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(hello));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind");

    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
"#;
    std::fs::write(project_dir.join("src/main.rs"), main_rs)?;

    // propel.toml
    std::fs::write(project_dir.join("propel.toml"), super::PROPEL_TOML_TEMPLATE)?;

    // .gitignore
    let gitignore = "/target\n.env\n.propel-bundle/\n";
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;

    println!("Created project '{name}'");
    println!();
    println!("  cd {name}");
    println!("  cargo run              # local development");
    println!("  propel deploy          # deploy to Cloud Run");
    println!();
    println!("To add Supabase Auth, run `propel init` and follow the instructions.");

    Ok(())
}
