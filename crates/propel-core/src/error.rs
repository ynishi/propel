use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to load config from {path}")]
    ConfigLoad {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config at {path}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("invalid include path {path:?}: {reason}")]
    InvalidIncludePath { path: String, reason: &'static str },

    // ── Cargo project discovery ──
    #[error("cargo metadata failed for {manifest_path}: {detail}")]
    CargoMetadata {
        manifest_path: PathBuf,
        detail: String,
    },

    #[error("failed to resolve project directory {path}")]
    ProjectDirResolve {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(
        "no package found in {dir}; workspace members: {}",
        format_members(workspace_members)
    )]
    NoPackageInDir {
        dir: PathBuf,
        workspace_members: Vec<String>,
    },

    #[error("no binary target in package '{package}' — propel requires a binary to deploy")]
    NoBinaryTarget { package: String },

    #[error(
        "multiple binary targets found: {}; set `default-run` in Cargo.toml to select one",
        names.join(", ")
    )]
    MultipleBinaries { names: Vec<String> },
}

fn format_members(members: &[String]) -> String {
    if members.is_empty() {
        "(none)".to_owned()
    } else {
        members.join(", ")
    }
}
