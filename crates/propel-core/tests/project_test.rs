use propel_core::ProjectMeta;
use tempfile::TempDir;

#[test]
fn extracts_name_and_version() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"
[package]
name = "my-api"
version = "1.2.3"
"#;
    std::fs::write(tmp.path().join("Cargo.toml"), toml).unwrap();

    let meta = ProjectMeta::from_cargo_toml(tmp.path()).unwrap();
    assert_eq!(meta.name, "my-api");
    assert_eq!(meta.version, "1.2.3");
    assert_eq!(meta.binary_name, "my-api");
}

#[test]
fn uses_bin_name_when_present() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"
[package]
name = "my-lib"
version = "0.1.0"

[[bin]]
name = "my-server"
path = "src/main.rs"
"#;
    std::fs::write(tmp.path().join("Cargo.toml"), toml).unwrap();

    let meta = ProjectMeta::from_cargo_toml(tmp.path()).unwrap();
    assert_eq!(meta.name, "my-lib");
    assert_eq!(meta.binary_name, "my-server");
}

#[test]
fn defaults_version_when_missing() {
    let tmp = TempDir::new().unwrap();
    let toml = r#"
[package]
name = "no-version"
"#;
    std::fs::write(tmp.path().join("Cargo.toml"), toml).unwrap();

    let meta = ProjectMeta::from_cargo_toml(tmp.path()).unwrap();
    assert_eq!(meta.version, "0.1.0");
}

#[test]
fn error_when_no_cargo_toml() {
    let tmp = TempDir::new().unwrap();

    let result = ProjectMeta::from_cargo_toml(tmp.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Cargo.toml"));
}

#[test]
fn error_when_missing_package_section() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[dependencies]\nserde = \"1\"",
    )
    .unwrap();

    let result = ProjectMeta::from_cargo_toml(tmp.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("[package]"));
}

#[test]
fn error_when_missing_package_name() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nversion = \"0.1.0\"",
    )
    .unwrap();

    let result = ProjectMeta::from_cargo_toml(tmp.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("name"));
}
