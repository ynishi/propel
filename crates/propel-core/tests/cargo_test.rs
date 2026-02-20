use propel_core::CargoProject;
use std::process::Command;
use tempfile::TempDir;

/// Create a minimal Cargo project in a temp directory.
fn init_cargo_project(dir: &std::path::Path, name: &str) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "1.2.3"
edition = "2021"
"#
        ),
    )
    .unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
}

/// Create a Cargo project with an explicit [[bin]] section.
fn init_cargo_project_with_bin(dir: &std::path::Path, pkg_name: &str, bin_name: &str) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{pkg_name}"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "{bin_name}"
path = "src/main.rs"
"#
        ),
    )
    .unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
}

// ── Single package tests ──

#[test]
fn discover_single_package() {
    let tmp = TempDir::new().unwrap();
    init_cargo_project(tmp.path(), "my-api");

    let project = CargoProject::discover(tmp.path()).unwrap();

    assert_eq!(project.name, "my-api");
    assert_eq!(project.version, "1.2.3");
    assert_eq!(project.default_binary, "my-api");
    assert_eq!(project.binaries.len(), 1);
    assert_eq!(project.binaries[0].name, "my-api");
    assert!(project.manifest_path.ends_with("Cargo.toml"));
    assert_eq!(project.package_dir, project.workspace_root);
}

#[test]
fn discover_explicit_bin_name() {
    let tmp = TempDir::new().unwrap();
    init_cargo_project_with_bin(tmp.path(), "my-lib", "my-server");

    let project = CargoProject::discover(tmp.path()).unwrap();

    assert_eq!(project.name, "my-lib");
    assert_eq!(project.default_binary, "my-server");
}

#[test]
fn discover_version_workspace_inheritance() {
    let tmp = TempDir::new().unwrap();

    // Workspace root
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["api"]

[workspace.package]
version = "2.0.0"
edition = "2021"
"#,
    )
    .unwrap();

    // Member
    let api_dir = tmp.path().join("api");
    std::fs::create_dir_all(api_dir.join("src")).unwrap();
    std::fs::write(
        api_dir.join("Cargo.toml"),
        r#"[package]
name = "api"
version.workspace = true
edition.workspace = true
"#,
    )
    .unwrap();
    std::fs::write(api_dir.join("src/main.rs"), "fn main() {}\n").unwrap();

    let project = CargoProject::discover(&api_dir).unwrap();

    // Version should be resolved from workspace, not "0.1.0" fallback
    assert_eq!(project.version, "2.0.0");
    assert_eq!(project.name, "api");
}

#[test]
fn discover_multiple_binaries_with_default_run() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src/bin")).unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[package]
name = "multi"
version = "0.1.0"
edition = "2021"
default-run = "worker"

[[bin]]
name = "server"
path = "src/bin/server.rs"

[[bin]]
name = "worker"
path = "src/bin/worker.rs"
"#,
    )
    .unwrap();
    std::fs::write(tmp.path().join("src/bin/server.rs"), "fn main() {}\n").unwrap();
    std::fs::write(tmp.path().join("src/bin/worker.rs"), "fn main() {}\n").unwrap();

    let project = CargoProject::discover(tmp.path()).unwrap();

    assert_eq!(project.default_binary, "worker");
    assert_eq!(project.binaries.len(), 2);
}

#[test]
fn discover_multiple_binaries_prefers_package_name() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src/bin")).unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[package]
name = "myapp"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "myapp"
path = "src/bin/myapp.rs"

[[bin]]
name = "helper"
path = "src/bin/helper.rs"
"#,
    )
    .unwrap();
    std::fs::write(tmp.path().join("src/bin/myapp.rs"), "fn main() {}\n").unwrap();
    std::fs::write(tmp.path().join("src/bin/helper.rs"), "fn main() {}\n").unwrap();

    let project = CargoProject::discover(tmp.path()).unwrap();

    assert_eq!(project.default_binary, "myapp");
}

// ── Workspace tests ──

#[test]
fn discover_workspace_root_errors() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["api"]
"#,
    )
    .unwrap();

    let api_dir = tmp.path().join("api");
    std::fs::create_dir_all(api_dir.join("src")).unwrap();
    std::fs::write(
        api_dir.join("Cargo.toml"),
        r#"[package]
name = "api"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    std::fs::write(api_dir.join("src/main.rs"), "fn main() {}\n").unwrap();

    let result = CargoProject::discover(tmp.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("no package found"), "got: {err}");
    assert!(
        err.contains("api"),
        "should list workspace members, got: {err}"
    );
}

#[test]
fn discover_workspace_member() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["api", "worker"]
"#,
    )
    .unwrap();

    for member in &["api", "worker"] {
        let dir = tmp.path().join(member);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                r#"[package]
name = "{member}"
version = "0.1.0"
edition = "2021"
"#
            ),
        )
        .unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
    }

    // Discover specific member
    let project = CargoProject::discover(&tmp.path().join("api")).unwrap();
    assert_eq!(project.name, "api");

    // workspace_root should be the parent
    assert_eq!(
        project.workspace_root.canonicalize().unwrap(),
        tmp.path().canonicalize().unwrap()
    );
}

// ── Error cases ──

#[test]
fn discover_no_cargo_toml() {
    let tmp = TempDir::new().unwrap();

    let result = CargoProject::discover(tmp.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cargo metadata"), "got: {err}");
}

#[test]
fn discover_lib_only_errors() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[package]
name = "lib-only"
version = "0.1.0"
edition = "2021"

[lib]
name = "lib_only"
"#,
    )
    .unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

    let result = CargoProject::discover(tmp.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("no binary target"), "got: {err}");
}

#[test]
fn discover_multiple_binaries_ambiguous_errors() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src/bin")).unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[package]
name = "ambig"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "server"
path = "src/bin/server.rs"

[[bin]]
name = "worker"
path = "src/bin/worker.rs"
"#,
    )
    .unwrap();
    std::fs::write(tmp.path().join("src/bin/server.rs"), "fn main() {}\n").unwrap();
    std::fs::write(tmp.path().join("src/bin/worker.rs"), "fn main() {}\n").unwrap();

    let result = CargoProject::discover(tmp.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("multiple binary"), "got: {err}");
    assert!(
        err.contains("default-run"),
        "should suggest fix, got: {err}"
    );
}

// ── Auto-detect binary from src/bin/ ──

#[test]
fn discover_auto_detected_src_bin() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src/bin")).unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[package]
name = "auto-detect"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    // Cargo auto-discovers src/bin/server.rs as a binary named "server"
    std::fs::write(tmp.path().join("src/bin/server.rs"), "fn main() {}\n").unwrap();

    let project = CargoProject::discover(tmp.path()).unwrap();
    assert_eq!(project.default_binary, "server");
    assert_eq!(project.binaries.len(), 1);
}

// ── Git + cargo project combined ──

#[test]
fn discover_in_git_repo() {
    let tmp = TempDir::new().unwrap();
    init_cargo_project(tmp.path(), "git-project");

    Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let project = CargoProject::discover(tmp.path()).unwrap();
    assert_eq!(project.name, "git-project");
}
