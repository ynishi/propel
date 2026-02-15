use propel_build::bundle::create_bundle;
use propel_build::dockerfile::DockerfileGenerator;
use propel_build::eject::{eject, is_ejected, load_ejected_dockerfile};
use propel_core::{BuildConfig, ProjectMeta};
use tempfile::TempDir;

fn default_meta() -> ProjectMeta {
    ProjectMeta {
        name: "my-service".to_owned(),
        version: "0.1.0".to_owned(),
        binary_name: "my-service".to_owned(),
    }
}

// ── Dockerfile Generation Tests ──

#[test]
fn dockerfile_contains_cargo_chef_stages() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta);
    let output = generator.render();

    assert!(output.contains("Stage 1: Planner"));
    assert!(output.contains("Stage 2: Cacher"));
    assert!(output.contains("Stage 3: Builder"));
    assert!(output.contains("Stage 4: Runtime"));
    assert!(output.contains("cargo chef prepare"));
    assert!(output.contains("cargo chef cook --release"));
    assert!(output.contains("cargo build --release --bin my-service"));
}

#[test]
fn dockerfile_uses_configured_images() {
    let config = BuildConfig {
        base_image: "rust:1.82-slim".to_owned(),
        runtime_image: "debian:bookworm-slim".to_owned(),
        ..Default::default()
    };
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta);
    let output = generator.render();

    assert!(output.contains("FROM rust:1.82-slim AS chef"));
    assert!(output.contains("FROM debian:bookworm-slim"));
}

#[test]
fn dockerfile_includes_extra_packages() {
    let config = BuildConfig {
        extra_packages: vec!["libssl-dev".to_owned(), "pkg-config".to_owned()],
        ..Default::default()
    };
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta);
    let output = generator.render();

    assert!(output.contains("apt-get install -y libssl-dev pkg-config"));
}

#[test]
fn dockerfile_no_extra_packages_when_empty() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta);
    let output = generator.render();

    assert!(!output.contains("apt-get install"));
}

#[test]
fn dockerfile_uses_custom_binary_name() {
    let config = BuildConfig::default();
    let meta = ProjectMeta {
        name: "my-service".to_owned(),
        version: "0.1.0".to_owned(),
        binary_name: "custom-bin".to_owned(),
    };
    let generator = DockerfileGenerator::new(&config, &meta);
    let output = generator.render();

    assert!(output.contains("--bin custom-bin"));
    assert!(output.contains("/app/target/release/custom-bin"));
}

#[test]
fn dockerfile_exposes_port_8080() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta);
    let output = generator.render();

    assert!(output.contains("EXPOSE 8080"));
}

// ── Bundle Tests ──

#[test]
fn bundle_creates_expected_structure() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    // Setup minimal project
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::write(project.join("Cargo.lock"), "# lock file").unwrap();
    std::fs::write(project.join("src/main.rs"), "fn main() {}").unwrap();

    let bundle_dir = create_bundle(project, "FROM rust\n").unwrap();

    assert!(bundle_dir.join("Dockerfile").exists());
    assert!(bundle_dir.join("Cargo.toml").exists());
    assert!(bundle_dir.join("Cargo.lock").exists());
    assert!(bundle_dir.join("src/main.rs").exists());

    let dockerfile = std::fs::read_to_string(bundle_dir.join("Dockerfile")).unwrap();
    assert_eq!(dockerfile, "FROM rust\n");
}

#[test]
fn bundle_works_without_cargo_lock() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::write(project.join("src/main.rs"), "fn main() {}").unwrap();

    let bundle_dir = create_bundle(project, "FROM rust\n").unwrap();

    assert!(bundle_dir.join("Dockerfile").exists());
    assert!(bundle_dir.join("Cargo.toml").exists());
    assert!(!bundle_dir.join("Cargo.lock").exists());
}

#[test]
fn bundle_cleans_previous_bundle() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::write(project.join("src/main.rs"), "fn main() {}").unwrap();

    // Create first bundle
    let bundle1 = create_bundle(project, "FROM rust:1\n").unwrap();
    assert!(bundle1.join("Dockerfile").exists());

    // Create second bundle — should overwrite
    let bundle2 = create_bundle(project, "FROM rust:2\n").unwrap();
    let content = std::fs::read_to_string(bundle2.join("Dockerfile")).unwrap();
    assert_eq!(content, "FROM rust:2\n");
}

#[test]
fn bundle_copies_nested_src_dirs() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    std::fs::create_dir_all(project.join("src/handlers")).unwrap();
    std::fs::write(project.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::write(project.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(project.join("src/handlers/mod.rs"), "pub fn handle() {}").unwrap();

    let bundle_dir = create_bundle(project, "FROM rust\n").unwrap();

    assert!(bundle_dir.join("src/handlers/mod.rs").exists());
}

// ── Eject Tests ──

#[test]
fn eject_creates_propel_dir_with_dockerfile() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    assert!(!is_ejected(project));

    eject(project, "FROM rust:1.84\nRUN cargo build\n").unwrap();

    assert!(is_ejected(project));
    assert!(project.join(".propel/Dockerfile").exists());
}

#[test]
fn eject_preserves_dockerfile_content() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();
    let content = "FROM rust:1.84\nWORKDIR /app\nCOPY . .\nRUN cargo build --release\n";

    eject(project, content).unwrap();

    let loaded = load_ejected_dockerfile(project).unwrap();
    assert_eq!(loaded, content);
}

#[test]
fn eject_fails_if_already_ejected() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    eject(project, "first").unwrap();
    let result = eject(project, "second");

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("already ejected"));
}

#[test]
fn is_ejected_false_without_propel_dir() {
    let tmp = TempDir::new().unwrap();
    assert!(!is_ejected(tmp.path()));
}
