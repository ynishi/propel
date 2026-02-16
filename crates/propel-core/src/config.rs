use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// propel.toml configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropelConfig {
    #[serde(default)]
    pub project: ProjectConfig,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub cloud_run: CloudRunConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name (defaults to Cargo.toml package name)
    pub name: Option<String>,
    /// GCP region (defaults to us-central1)
    #[serde(default = "default_region")]
    pub region: String,
    /// GCP project ID
    pub gcp_project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Rust builder image
    #[serde(default = "default_builder_image")]
    pub base_image: String,
    /// Runtime base image
    #[serde(default = "default_runtime_image")]
    pub runtime_image: String,
    /// Additional system packages to install via apt-get
    #[serde(default)]
    pub extra_packages: Vec<String>,
    /// Cargo Chef version
    #[serde(default = "default_cargo_chef_version")]
    pub cargo_chef_version: String,
    /// Files/directories to include in the runtime image.
    /// When None, the entire bundle is copied (COPY . .).
    /// When Some, only the specified paths are copied (overrides all-in default).
    #[serde(default)]
    pub include: Option<Vec<String>>,
    /// Static environment variables baked into the container image.
    /// These become ENV directives in the Dockerfile.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunConfig {
    /// Memory allocation
    #[serde(default = "default_memory")]
    pub memory: String,
    /// CPU count
    #[serde(default = "default_cpu")]
    pub cpu: u32,
    /// Minimum instances
    #[serde(default)]
    pub min_instances: u32,
    /// Maximum instances
    #[serde(default = "default_max_instances")]
    pub max_instances: u32,
    /// Max concurrent requests per instance
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    /// Port the application listens on
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: None,
            region: default_region(),
            gcp_project_id: None,
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            base_image: default_builder_image(),
            runtime_image: default_runtime_image(),
            extra_packages: Vec::new(),
            cargo_chef_version: default_cargo_chef_version(),
            include: None,
            env: HashMap::new(),
        }
    }
}

impl Default for CloudRunConfig {
    fn default() -> Self {
        Self {
            memory: default_memory(),
            cpu: default_cpu(),
            min_instances: 0,
            max_instances: default_max_instances(),
            concurrency: default_concurrency(),
            port: default_port(),
        }
    }
}

impl PropelConfig {
    /// Load from propel.toml at the given path, or return defaults if not found.
    pub fn load(project_dir: &std::path::Path) -> crate::Result<Self> {
        let config_path = project_dir.join("propel.toml");
        if config_path.exists() {
            let content =
                std::fs::read_to_string(&config_path).map_err(|e| crate::Error::ConfigLoad {
                    path: config_path.clone(),
                    source: e,
                })?;
            toml::from_str(&content).map_err(|e| crate::Error::ConfigParse {
                path: config_path,
                source: e,
            })
        } else {
            Ok(Self::default())
        }
    }
}

fn default_region() -> String {
    "us-central1".to_owned()
}

fn default_builder_image() -> String {
    "rust:1.84-bookworm".to_owned()
}

fn default_runtime_image() -> String {
    "gcr.io/distroless/cc-debian12".to_owned()
}

fn default_cargo_chef_version() -> String {
    "0.1.68".to_owned()
}

fn default_memory() -> String {
    "512Mi".to_owned()
}

fn default_cpu() -> u32 {
    1
}

fn default_max_instances() -> u32 {
    10
}

fn default_concurrency() -> u32 {
    80
}

fn default_port() -> u16 {
    8080
}
