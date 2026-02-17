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
    /// Add Propel to an existing Rust project
    Init,
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
    Destroy {
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        /// Also delete secrets from Secret Manager
        #[arg(long)]
        include_secrets: bool,
        /// Also delete CI/CD resources (WIF, service account, GitHub Secrets, workflow)
        #[arg(long)]
        include_ci: bool,
    },
    /// Check GCP setup and readiness
    Doctor,
    /// Show Cloud Run service status
    Status,
    /// Stream Cloud Run logs
    Logs {
        /// Tail logs in real-time
        #[arg(long, short = 'f')]
        follow: bool,
        /// Number of log entries to show (default: 100)
        #[arg(long, short = 'n')]
        tail: Option<u32>,
    },
    /// Manage CI/CD pipeline
    Ci {
        #[command(subcommand)]
        action: CiAction,
    },
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
    /// Delete a secret
    Delete {
        /// Secret name
        key: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum CiAction {
    /// Set up GitHub Actions CI/CD pipeline (WIF + Service Account + GitHub Secrets + workflow)
    Init,
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
        Commands::Init => commands::init_project().await?,
        Commands::Deploy { allow_dirty } => commands::deploy(allow_dirty).await?,
        Commands::Secret { action } => match action {
            SecretAction::Set { key_value } => commands::secret_set(&key_value).await?,
            SecretAction::List => commands::secret_list().await?,
            SecretAction::Delete { key, yes } => commands::secret_delete(&key, yes).await?,
        },
        Commands::Eject => commands::eject().await?,
        Commands::Destroy {
            yes,
            include_secrets,
            include_ci,
        } => commands::destroy(yes, include_secrets, include_ci).await?,
        Commands::Doctor => commands::doctor().await?,
        Commands::Status => commands::status().await?,
        Commands::Logs { follow, tail } => commands::logs(follow, tail).await?,
        Commands::Ci { action } => match action {
            CiAction::Init => commands::ci_init().await?,
        },
    }

    Ok(())
}
