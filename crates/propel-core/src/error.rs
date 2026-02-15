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

    #[error("failed to read Cargo.toml at {path}")]
    CargoTomlRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse Cargo.toml at {path}")]
    CargoTomlParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("missing [package] section in Cargo.toml at {0}")]
    MissingPackageSection(PathBuf),

    #[error("missing package.name in Cargo.toml at {0}")]
    MissingPackageName(PathBuf),
}
