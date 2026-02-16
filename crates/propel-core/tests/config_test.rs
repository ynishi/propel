use propel_core::PropelConfig;
use tempfile::TempDir;

#[test]
fn load_returns_defaults_when_no_config_file() {
    let tmp = TempDir::new().unwrap();
    let config = PropelConfig::load(tmp.path()).unwrap();

    assert_eq!(config.project.region, "us-central1");
    assert!(config.project.name.is_none());
    assert!(config.project.gcp_project_id.is_none());
    assert_eq!(config.build.base_image, "rust:1.84-bookworm");
    assert_eq!(config.build.runtime_image, "gcr.io/distroless/cc-debian12");
    assert!(config.build.extra_packages.is_empty());
    assert_eq!(config.cloud_run.memory, "512Mi");
    assert_eq!(config.cloud_run.cpu, 1);
    assert_eq!(config.cloud_run.min_instances, 0);
    assert_eq!(config.cloud_run.max_instances, 10);
    assert_eq!(config.cloud_run.concurrency, 80);
    assert_eq!(config.cloud_run.port, 8080);
}

#[test]
fn load_parses_full_config() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"
[project]
name = "my-api"
region = "asia-northeast1"
gcp_project_id = "my-gcp-project"

[build]
base_image = "rust:1.82-slim"
runtime_image = "debian:bookworm-slim"
extra_packages = ["libssl-dev", "pkg-config"]
cargo_chef_version = "0.1.70"

[cloud_run]
memory = "1Gi"
cpu = 2
min_instances = 1
max_instances = 50
concurrency = 200
port = 3000
"#;
    std::fs::write(tmp.path().join("propel.toml"), toml).unwrap();

    let config = PropelConfig::load(tmp.path()).unwrap();

    assert_eq!(config.project.name.as_deref(), Some("my-api"));
    assert_eq!(config.project.region, "asia-northeast1");
    assert_eq!(
        config.project.gcp_project_id.as_deref(),
        Some("my-gcp-project")
    );
    assert_eq!(config.build.base_image, "rust:1.82-slim");
    assert_eq!(config.build.runtime_image, "debian:bookworm-slim");
    assert_eq!(
        config.build.extra_packages,
        vec!["libssl-dev", "pkg-config"]
    );
    assert_eq!(config.build.cargo_chef_version, "0.1.70");
    assert_eq!(config.cloud_run.memory, "1Gi");
    assert_eq!(config.cloud_run.cpu, 2);
    assert_eq!(config.cloud_run.min_instances, 1);
    assert_eq!(config.cloud_run.max_instances, 50);
    assert_eq!(config.cloud_run.concurrency, 200);
    assert_eq!(config.cloud_run.port, 3000);
}

#[test]
fn load_partial_config_fills_defaults() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"
[project]
gcp_project_id = "partial-project"
"#;
    std::fs::write(tmp.path().join("propel.toml"), toml).unwrap();

    let config = PropelConfig::load(tmp.path()).unwrap();

    assert_eq!(
        config.project.gcp_project_id.as_deref(),
        Some("partial-project")
    );
    // Defaults preserved
    assert_eq!(config.project.region, "us-central1");
    assert_eq!(config.cloud_run.memory, "512Mi");
    assert_eq!(config.build.base_image, "rust:1.84-bookworm");
}

#[test]
fn load_invalid_toml_returns_parse_error() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("propel.toml"), "not valid {{{{ toml").unwrap();

    let result = PropelConfig::load(tmp.path());
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    assert!(err.contains("parse"));
}

#[test]
fn load_empty_config_returns_defaults() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("propel.toml"), "").unwrap();

    let config = PropelConfig::load(tmp.path()).unwrap();
    assert_eq!(config.project.region, "us-central1");
}

// ── include / env Tests ──

#[test]
fn load_defaults_include_is_none() {
    let tmp = TempDir::new().unwrap();
    let config = PropelConfig::load(tmp.path()).unwrap();

    assert!(config.build.include.is_none());
    assert!(config.build.env.is_empty());
}

#[test]
fn load_include_paths() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"
[build]
include = ["migrations/", "templates/"]
"#;
    std::fs::write(tmp.path().join("propel.toml"), toml).unwrap();

    let config = PropelConfig::load(tmp.path()).unwrap();

    let include = config.build.include.unwrap();
    assert_eq!(include, vec!["migrations/", "templates/"]);
}

#[test]
fn load_build_env() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"
[build.env]
TEMPLATE_DIR = "/app/templates"
LUA_DIR = "/app/lua"
"#;
    std::fs::write(tmp.path().join("propel.toml"), toml).unwrap();

    let config = PropelConfig::load(tmp.path()).unwrap();

    assert_eq!(config.build.env.len(), 2);
    assert_eq!(config.build.env["TEMPLATE_DIR"], "/app/templates");
    assert_eq!(config.build.env["LUA_DIR"], "/app/lua");
}

#[test]
fn load_include_empty_vec() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"
[build]
include = []
"#;
    std::fs::write(tmp.path().join("propel.toml"), toml).unwrap();

    let config = PropelConfig::load(tmp.path()).unwrap();

    let include = config.build.include.unwrap();
    assert!(include.is_empty());
}
