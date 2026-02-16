use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

fn propel() -> assert_cmd::Command {
    cargo_bin_cmd!("propel")
}

// ── Help / Version ──

#[test]
fn shows_help() {
    propel()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Deploy Rust apps to Cloud Run"));
}

#[test]
fn shows_version() {
    propel()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("propel"));
}

// ── New Command ──

#[test]
fn new_creates_project_structure() {
    let tmp = TempDir::new().unwrap();
    let project_name = "test-project";

    propel()
        .current_dir(tmp.path())
        .args(["new", project_name])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created project"));

    let project_dir = tmp.path().join(project_name);
    assert!(project_dir.join("Cargo.toml").exists());
    assert!(project_dir.join("src/main.rs").exists());
    assert!(project_dir.join("propel.toml").exists());
    assert!(project_dir.join(".env.example").exists());
    assert!(project_dir.join(".gitignore").exists());
}

#[test]
fn new_cargo_toml_contains_dependencies() {
    let tmp = TempDir::new().unwrap();

    propel()
        .current_dir(tmp.path())
        .args(["new", "dep-check"])
        .assert()
        .success();

    let content = std::fs::read_to_string(tmp.path().join("dep-check/Cargo.toml")).unwrap();
    assert!(content.contains("axum"));
    assert!(content.contains("tokio"));
    assert!(content.contains("propel-sdk"));
}

#[test]
fn new_main_rs_uses_propel_sdk() {
    let tmp = TempDir::new().unwrap();

    propel()
        .current_dir(tmp.path())
        .args(["new", "sdk-check"])
        .assert()
        .success();

    let content = std::fs::read_to_string(tmp.path().join("sdk-check/src/main.rs")).unwrap();
    assert!(content.contains("PropelState"));
    assert!(content.contains("PropelAuth"));
    assert!(content.contains("0.0.0.0:8080"));
}

#[test]
fn new_fails_if_directory_exists() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("existing")).unwrap();

    propel()
        .current_dir(tmp.path())
        .args(["new", "existing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn new_env_example_has_supabase_vars() {
    let tmp = TempDir::new().unwrap();

    propel()
        .current_dir(tmp.path())
        .args(["new", "env-check"])
        .assert()
        .success();

    let content = std::fs::read_to_string(tmp.path().join("env-check/.env.example")).unwrap();
    assert!(content.contains("SUPABASE_URL"));
    assert!(content.contains("SUPABASE_ANON_KEY"));
    assert!(content.contains("SUPABASE_JWT_SECRET"));
}

// ── Eject Command ──

#[test]
fn eject_creates_dockerfile_in_propel_dir() {
    let tmp = TempDir::new().unwrap();

    // Create minimal project structure
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"eject-test\"\nversion = \"0.1.0\"\nedition = \"2024\"",
    )
    .unwrap();
    std::fs::create_dir(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

    propel()
        .current_dir(tmp.path())
        .arg("eject")
        .assert()
        .success()
        .stdout(predicate::str::contains("Ejected"));

    assert!(tmp.path().join(".propel/Dockerfile").exists());

    let dockerfile = std::fs::read_to_string(tmp.path().join(".propel/Dockerfile")).unwrap();
    assert!(dockerfile.contains("cargo chef"));
    assert!(dockerfile.contains("--bin eject-test"));
}

#[test]
fn eject_fails_on_second_run() {
    let tmp = TempDir::new().unwrap();

    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"double-eject\"\nversion = \"0.1.0\"\nedition = \"2024\"",
    )
    .unwrap();
    std::fs::create_dir(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

    // First eject
    propel()
        .current_dir(tmp.path())
        .arg("eject")
        .assert()
        .success();

    // Second eject
    propel()
        .current_dir(tmp.path())
        .arg("eject")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already ejected"));
}

// ── Deploy Command (no GCP) ──

#[test]
fn deploy_fails_without_gcp_project_id() {
    let tmp = TempDir::new().unwrap();

    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"no-gcp\"\nversion = \"0.1.0\"\nedition = \"2024\"",
    )
    .unwrap();
    std::fs::write(tmp.path().join("propel.toml"), "").unwrap();
    std::fs::create_dir(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

    // --allow-dirty skips git check so we can test config validation
    propel()
        .current_dir(tmp.path())
        .args(["deploy", "--allow-dirty"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("gcp_project_id"));
}

// ── Deploy: Dirty Check ──

#[test]
fn deploy_fails_on_non_git_directory() {
    let tmp = TempDir::new().unwrap();

    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"no-git\"\nversion = \"0.1.0\"\nedition = \"2024\"",
    )
    .unwrap();
    std::fs::create_dir(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

    propel()
        .current_dir(tmp.path())
        .arg("deploy")
        .assert()
        .failure()
        .stderr(predicate::str::contains("git"));
}

#[test]
fn deploy_dirty_repo_blocked_without_flag() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"dirty\"\nversion = \"0.1.0\"\nedition = \"2024\"",
    )
    .unwrap();
    std::fs::create_dir(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();

    // git init + commit
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "t@t.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "T"])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .unwrap();

    // Make dirty
    std::fs::write(dir.join("src/main.rs"), "fn main() { /* dirty */ }").unwrap();

    propel()
        .current_dir(dir)
        .arg("deploy")
        .assert()
        .failure()
        .stderr(predicate::str::contains("uncommitted changes"));
}

// ── Secret Command ──

#[test]
fn secret_set_rejects_invalid_format() {
    let tmp = TempDir::new().unwrap();

    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"sec\"\nversion = \"0.1.0\"\nedition = \"2024\"",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("propel.toml"),
        "[project]\ngcp_project_id = \"proj\"",
    )
    .unwrap();

    propel()
        .current_dir(tmp.path())
        .args(["secret", "set", "NO_EQUALS_SIGN"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("KEY=VALUE"));
}
