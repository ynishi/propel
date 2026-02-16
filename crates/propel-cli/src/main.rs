mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "propel", about = "Deploy Rust apps to Cloud Run with Supabase")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Propel project
    New {
        /// Project name
        name: String,
    },
    /// Deploy to Google Cloud Run
    Deploy {
        /// Allow deploying with uncommitted changes
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Manage secrets
    Secret {
        #[command(subcommand)]
        action: SecretAction,
    },
    /// Eject Dockerfile for manual customization
    Eject,
    /// Delete Cloud Run service, images, and local bundle
    Destroy,
    /// Check GCP setup and readiness
    Doctor,
    /// Show Cloud Run service status
    Status,
    /// Stream Cloud Run logs
    Logs,
}

#[derive(Subcommand)]
enum SecretAction {
    /// Set a secret (KEY=VALUE)
    Set {
        /// Secret in KEY=VALUE format
        key_value: String,
    },
    /// List all secrets
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::New { name } => commands::new_project(&name).await?,
        Commands::Deploy { allow_dirty } => commands::deploy(allow_dirty).await?,
        Commands::Secret { action } => match action {
            SecretAction::Set { key_value } => commands::secret_set(&key_value).await?,
            SecretAction::List => commands::secret_list().await?,
        },
        Commands::Eject => commands::eject().await?,
        Commands::Destroy => commands::destroy().await?,
        Commands::Doctor => commands::doctor().await?,
        Commands::Status => commands::status().await?,
        Commands::Logs => commands::logs().await?,
    }

    Ok(())
}
