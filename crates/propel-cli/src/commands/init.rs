use std::path::Path;

/// Initialize Propel in an existing Rust project.
pub async fn init_project() -> anyhow::Result<()> {
    // Must be inside a Cargo project
    if !Path::new("Cargo.toml").exists() {
        anyhow::bail!("Cargo.toml not found. Run this command from a Rust project root.");
    }

    let mut created = Vec::new();

    // propel.toml
    let propel_toml_path = Path::new("propel.toml");
    if propel_toml_path.exists() {
        eprintln!("propel.toml already exists, skipping");
    } else {
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
        std::fs::write(propel_toml_path, propel_toml)?;
        created.push("propel.toml");
    }

    // .env.example
    let env_example_path = Path::new(".env.example");
    if env_example_path.exists() {
        eprintln!(".env.example already exists, skipping");
    } else {
        let env_example = r#"SUPABASE_URL=https://your-project.supabase.co
SUPABASE_ANON_KEY=your-anon-key
SUPABASE_JWT_SECRET=your-jwt-secret
"#;
        std::fs::write(env_example_path, env_example)?;
        created.push(".env.example");
    }

    if created.is_empty() {
        println!("Nothing to create — already initialized.");
    } else {
        for f in &created {
            println!("Created {f}");
        }
    }

    println!();
    println!("Next steps:");
    println!();
    println!("  1. Add propel dependency:");
    println!("     cargo add propel");
    println!();
    println!("  2. Configure credentials:");
    println!("     cp .env.example .env");
    println!();
    println!("  3. Add middleware to your app:");
    println!();
    println!("     use axum::{{routing::get, middleware, Router}};");
    println!("     use propel::{{PropelState, PropelAuth}};");
    println!();
    println!("     let state = PropelState::load().expect(\"failed to load config\");");
    println!("     let app = Router::new()");
    println!("         .route(\"/health\", get(health))");
    println!("         .route(\"/api/protected\", get(handler))");
    println!("         .layer(middleware::from_fn_with_state(");
    println!("             state.clone(), PropelAuth::verify))");
    println!("         .with_state(state);");
    println!();
    println!("  4. Deploy:");
    println!("     propel deploy");

    Ok(())
}
