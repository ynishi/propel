use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use propel_build::bundle::{create_bundle, is_dirty};
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

/// Initialize a git repo with a minimal Rust project and an initial commit.
fn init_git_project(dir: &Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .unwrap();
}

// ── Dockerfile Generation Tests ──

#[test]
fn dockerfile_contains_cargo_chef_stages() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
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
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
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
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();

    assert!(output.contains("apt-get install -y libssl-dev pkg-config"));
}

#[test]
fn dockerfile_no_extra_packages_when_empty() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
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
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();

    assert!(output.contains("--bin custom-bin"));
    assert!(output.contains("/app/target/release/custom-bin"));
}

#[test]
fn dockerfile_exposes_port_8080() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();

    assert!(output.contains("EXPOSE 8080"));
}

#[test]
fn dockerfile_exposes_custom_port() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 3000);
    let output = generator.render();

    assert!(output.contains("EXPOSE 3000"));
    assert!(!output.contains("EXPOSE 8080"));
}

// ── Dockerfile: include / env Tests ──

#[test]
fn dockerfile_default_include_none_copies_all() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();

    // include=None → runtime gets COPY . .
    assert!(output.contains("COPY . ."));
    assert!(output.contains("WORKDIR /app"));
}

#[test]
fn dockerfile_include_some_copies_only_specified() {
    let config = BuildConfig {
        include: Some(vec!["migrations/".to_owned(), "templates/".to_owned()]),
        ..Default::default()
    };
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();

    // Should have individual COPY directives, not COPY . .
    // The runtime stage should contain these:
    let runtime_section = output.split("Stage 4: Runtime").nth(1).unwrap();
    assert!(runtime_section.contains("COPY migrations/ ./migrations/"));
    assert!(runtime_section.contains("COPY templates/ ./templates/"));
    // Should NOT have the all-in COPY . . in the runtime section
    // Note: builder stage still has COPY . . — that's expected
    assert!(!runtime_section.contains("COPY . ."));
}

#[test]
fn dockerfile_include_empty_vec_no_runtime_copy() {
    let config = BuildConfig {
        include: Some(vec![]),
        ..Default::default()
    };
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();

    let runtime_section = output.split("Stage 4: Runtime").nth(1).unwrap();
    // Binary is still copied
    assert!(runtime_section.contains("COPY --from=builder"));
    // No bundle files copied
    assert!(!runtime_section.contains("COPY . ."));
}

#[test]
fn dockerfile_build_env_generates_env_directives() {
    let mut env = HashMap::new();
    env.insert("TEMPLATE_DIR".to_owned(), "/app/templates".to_owned());
    env.insert("LUA_DIR".to_owned(), "/app/lua".to_owned());

    let config = BuildConfig {
        env,
        ..Default::default()
    };
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();

    assert!(output.contains("ENV LUA_DIR=/app/lua"));
    assert!(output.contains("ENV TEMPLATE_DIR=/app/templates"));
}

#[test]
fn dockerfile_no_env_when_empty() {
    let config = BuildConfig::default();
    let meta = default_meta();
    let generator = DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();

    assert!(!output.contains("ENV "));
}

// ── Bundle Tests ──

#[test]
fn bundle_creates_expected_structure() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();
    init_git_project(project);

    let bundle_dir = create_bundle(project, "FROM rust\n").unwrap();

    assert!(bundle_dir.join("Dockerfile").exists());
    assert!(bundle_dir.join("Cargo.toml").exists());
    assert!(bundle_dir.join("src/main.rs").exists());

    let dockerfile = std::fs::read_to_string(bundle_dir.join("Dockerfile")).unwrap();
    assert_eq!(dockerfile, "FROM rust\n");
}

#[test]
fn bundle_includes_additional_dirs() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    // Create project with extra dirs
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(project.join("migrations")).unwrap();
    std::fs::create_dir_all(project.join("templates")).unwrap();
    std::fs::write(project.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::write(project.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(project.join("migrations/001.sql"), "CREATE TABLE t;").unwrap();
    std::fs::write(project.join("templates/index.html"), "<h1>hello</h1>").unwrap();

    // Git init and commit all
    Command::new("git")
        .args(["init"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(project)
        .output()
        .unwrap();

    let bundle_dir = create_bundle(project, "FROM rust\n").unwrap();

    // Additional dirs should be in the bundle
    assert!(bundle_dir.join("migrations/001.sql").exists());
    assert!(bundle_dir.join("templates/index.html").exists());
}

#[test]
fn bundle_respects_gitignore() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(project.join("target")).unwrap();
    std::fs::write(project.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::write(project.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(project.join("target/debug"), "binary").unwrap();
    std::fs::write(project.join(".gitignore"), "target/\n").unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(project)
        .output()
        .unwrap();

    let bundle_dir = create_bundle(project, "FROM rust\n").unwrap();

    // .gitignored files should NOT be in the bundle
    assert!(!bundle_dir.join("target").exists());
    // Tracked files should be
    assert!(bundle_dir.join("src/main.rs").exists());
    assert!(bundle_dir.join(".gitignore").exists());
}

#[test]
fn bundle_excludes_propel_dirs() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(project.join(".propel")).unwrap();
    std::fs::write(project.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::write(project.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(project.join(".propel/Dockerfile"), "custom").unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(project)
        .output()
        .unwrap();

    let bundle_dir = create_bundle(project, "FROM rust\n").unwrap();

    // .propel/ should be excluded by PROPEL_EXCLUDES
    assert!(!bundle_dir.join(".propel").exists());
    assert!(bundle_dir.join("src/main.rs").exists());
}

#[test]
fn bundle_cleans_previous_bundle() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();
    init_git_project(project);

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

    Command::new("git")
        .args(["init"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(project)
        .output()
        .unwrap();

    let bundle_dir = create_bundle(project, "FROM rust\n").unwrap();

    assert!(bundle_dir.join("src/handlers/mod.rs").exists());
}

// ── Dirty Check Tests ──

#[test]
fn is_dirty_clean_repo() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();
    init_git_project(project);

    assert!(!is_dirty(project).unwrap());
}

#[test]
fn is_dirty_with_uncommitted_changes() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();
    init_git_project(project);

    // Modify a tracked file without committing
    std::fs::write(
        project.join("src/main.rs"),
        "fn main() { println!(\"dirty\"); }",
    )
    .unwrap();

    assert!(is_dirty(project).unwrap());
}

#[test]
fn is_dirty_with_untracked_file() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();
    init_git_project(project);

    // Add an untracked file
    std::fs::write(project.join("new_file.txt"), "hello").unwrap();

    assert!(is_dirty(project).unwrap());
}

// ── Eject Tests ──

#[test]
fn eject_creates_propel_dir_with_dockerfile() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    assert!(!is_ejected(project));

    eject(project, "FROM rust:1.85\nRUN cargo build\n").unwrap();

    assert!(is_ejected(project));
    assert!(project.join(".propel/Dockerfile").exists());
}

#[test]
fn eject_preserves_dockerfile_content() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();
    let content = "FROM rust:1.85\nWORKDIR /app\nCOPY . .\nRUN cargo build --release\n";

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
