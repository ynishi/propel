use propel_cloud::GcloudClient;
use propel_core::PropelConfig;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// IAM roles required for the CI deploy service account.
///
/// - `run.admin` is required (not `run.developer`) because `--allow-unauthenticated`
///   needs `run.services.setIamPolicy`.
/// - Secret access (secretAccessor) is granted per-secret at `propel secret set`
///   time, so CI only needs viewer to list secret names for --update-secrets.
const CI_SA_ROLES: &[&str] = &[
    "roles/artifactregistry.writer",
    "roles/cloudbuild.builds.editor",
    "roles/iam.serviceAccountUser",
    "roles/run.admin",
    "roles/secretmanager.viewer",
    "roles/serviceusage.serviceUsageViewer",
    "roles/storage.objectAdmin",
    "roles/viewer",
];

pub(super) const WIF_POOL_ID: &str = "propel-github";
const WIF_PROVIDER_ID: &str = "github";
pub(super) const CI_SA_ID: &str = "propel-deploy";
pub(super) const WORKFLOW_PATH: &str = ".github/workflows/propel-deploy.yml";

/// GitHub Actions secret names managed by `ci init` / `destroy --include-ci`.
pub(super) const GH_SECRET_NAMES: &[&str] =
    &["GCP_PROJECT_ID", "WIF_PROVIDER", "WIF_SERVICE_ACCOUNT"];

/// Set up GitHub Actions CI/CD pipeline.
pub async fn ci_init() -> anyhow::Result<()> {
    let project_dir = PathBuf::from(".");
    let client = GcloudClient::new();

    // ── Guard: workflow already exists ──
    let workflow_path = Path::new(WORKFLOW_PATH);
    if workflow_path.exists() {
        anyhow::bail!(
            "Workflow already exists at {WORKFLOW_PATH} — edit it directly, or delete it to re-run ci init"
        );
    }

    // ── Prerequisites ──

    println!("Checking prerequisites...");

    // gh CLI
    let gh_version = exec_gh(&["--version"])
        .await
        .map_err(|_| anyhow::anyhow!("gh CLI not found. Install: https://cli.github.com"))?;
    // lines().next() returns None only when output is completely empty
    let gh_ver_line = gh_version
        .lines()
        .next()
        .unwrap_or("unknown version")
        .trim();
    println!("  gh CLI: {gh_ver_line}");

    // gh auth
    exec_gh(&["auth", "status"])
        .await
        .map_err(|_| anyhow::anyhow!("Not authenticated with GitHub. Run: gh auth login"))?;
    println!("  gh auth: OK");

    // GitHub remote
    let github_repo = detect_github_repo().await?;
    println!("  Repository: {github_repo}");

    // propel.toml + gcp_project_id
    let config = PropelConfig::load(&project_dir)?;
    let gcp_project_id = super::require_gcp_project_id(&config)?;
    println!("  GCP Project: {gcp_project_id}");

    // Required GCP APIs check
    check_required_apis(&client, gcp_project_id).await?;
    println!("  Required APIs: OK");

    println!();

    // ── Workload Identity Federation ──

    println!("Setting up Workload Identity Federation...");

    let created = client.ensure_wif_pool(gcp_project_id, WIF_POOL_ID).await?;
    if created {
        println!("  Created Identity Pool: {WIF_POOL_ID}");
    } else {
        println!("  Identity Pool already exists: {WIF_POOL_ID}");
    }

    let created = client
        .ensure_oidc_provider(gcp_project_id, WIF_POOL_ID, WIF_PROVIDER_ID, &github_repo)
        .await?;
    if created {
        println!("  Created OIDC Provider: {WIF_PROVIDER_ID}");
    } else {
        println!("  OIDC Provider already exists: {WIF_PROVIDER_ID}");
    }

    println!();

    // ── Service Account ──

    println!("Setting up Service Account...");

    let sa_email = format!("{CI_SA_ID}@{gcp_project_id}.iam.gserviceaccount.com");

    let created = client
        .ensure_service_account(gcp_project_id, CI_SA_ID, "Propel CI Deploy")
        .await?;
    if created {
        println!("  Created SA: {sa_email}");
    } else {
        println!("  SA already exists: {sa_email}");
    }

    println!("  Binding IAM roles...");
    client
        .bind_iam_roles(gcp_project_id, &sa_email, CI_SA_ROLES)
        .await?;
    for role in CI_SA_ROLES {
        println!("    {role}");
    }

    // WIF → SA binding
    let project_number = client.get_project_number(gcp_project_id).await?;
    client
        .bind_wif_to_sa(
            gcp_project_id,
            &project_number,
            WIF_POOL_ID,
            &sa_email,
            &github_repo,
        )
        .await?;
    println!("  Bound WIF to SA (scoped to {github_repo})");

    println!();

    // ── GitHub Secrets ──

    println!("Configuring GitHub Secrets...");

    let wif_provider = format!(
        "projects/{project_number}/locations/global/workloadIdentityPools/{WIF_POOL_ID}/providers/{WIF_PROVIDER_ID}"
    );

    set_gh_secret("GCP_PROJECT_ID", gcp_project_id).await?;
    println!("  GCP_PROJECT_ID");

    set_gh_secret("WIF_PROVIDER", &wif_provider).await?;
    println!("  WIF_PROVIDER");

    set_gh_secret("WIF_SERVICE_ACCOUNT", &sa_email).await?;
    println!("  WIF_SERVICE_ACCOUNT");

    println!();

    // ── Generate workflow yaml ──

    if let Some(parent) = workflow_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(workflow_path, generate_workflow_yaml())?;
    println!("Generated: {WORKFLOW_PATH}");

    println!();
    println!("Push to main -> auto deploy to Cloud Run.");

    Ok(())
}

/// Detect the GitHub owner/repo from the git remote origin URL.
async fn detect_github_repo() -> anyhow::Result<String> {
    let output = tokio::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("No git remote 'origin' found");
    }

    let url = String::from_utf8(output.stdout)?.trim().to_owned();
    parse_github_repo(&url)
        .ok_or_else(|| anyhow::anyhow!("Remote '{url}' is not a GitHub repository"))
}

/// Parse "owner/repo" from various GitHub URL formats.
fn parse_github_repo(url: &str) -> Option<String> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let repo = rest.strip_suffix(".git").unwrap_or(rest);
        return Some(repo.to_owned());
    }

    // HTTPS: https://github.com/owner/repo.git
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        let repo = rest.strip_suffix(".git").unwrap_or(rest);
        // Strip trailing slash if present
        let repo = repo.strip_suffix('/').unwrap_or(repo);
        return Some(repo.to_owned());
    }

    None
}

/// Check that the required GCP APIs (Cloud Build, Cloud Run, Secret Manager) are enabled.
async fn check_required_apis<E: propel_cloud::GcloudExecutor>(
    client: &GcloudClient<E>,
    project_id: &str,
) -> anyhow::Result<()> {
    let output = client
        .check_prerequisites(project_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if output.has_warnings() {
        anyhow::bail!(
            "Required APIs not enabled: {}",
            output.disabled_apis.join(", ")
        );
    }

    Ok(())
}

/// Execute a gh CLI command and capture stdout.
async fn exec_gh(gh_args: &[&str]) -> anyhow::Result<String> {
    let output = tokio::process::Command::new("gh")
        .args(gh_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh {}: {stderr}", gh_args.join(" "))
    }
}

/// Set a GitHub Actions secret via stdin to avoid exposing the value in process args.
async fn set_gh_secret(name: &str, value: &str) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new("gh")
        .args(["secret", "set", name])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(value.as_bytes()).await?;
        stdin.shutdown().await?;
    }

    let output = child.wait_with_output().await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh secret set {name}: {stderr}");
    }

    Ok(())
}

/// Delete a GitHub Actions secret (best-effort).
pub(super) async fn delete_gh_secret(name: &str) -> anyhow::Result<()> {
    exec_gh(&["secret", "delete", name, "--yes"]).await?;
    Ok(())
}

/// Generate the GitHub Actions workflow yaml content.
fn generate_workflow_yaml() -> String {
    r#"# Generated by: propel ci init
name: Deploy

on:
  push:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      id-token: write

    steps:
      - uses: actions/checkout@v4

      - uses: google-github-actions/auth@v2
        with:
          workload_identity_provider: ${{ secrets.WIF_PROVIDER }}
          service_account: ${{ secrets.WIF_SERVICE_ACCOUNT }}

      - uses: google-github-actions/setup-gcloud@v2

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache propel binary
        uses: actions/cache@v4
        with:
          path: ~/.cargo/bin/propel
          key: propel-cli-${{ hashFiles('Cargo.lock') }}

      - name: Install propel
        run: |
          if ! command -v propel &> /dev/null; then
            cargo install propel-cli
          fi

      - name: Deploy
        run: propel deploy --allow-dirty
"#
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_repo_ssh() {
        assert_eq!(
            parse_github_repo("git@github.com:ynishi/propel.git"),
            Some("ynishi/propel".to_owned())
        );
    }

    #[test]
    fn parse_github_repo_ssh_no_suffix() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo"),
            Some("owner/repo".to_owned())
        );
    }

    #[test]
    fn parse_github_repo_https() {
        assert_eq!(
            parse_github_repo("https://github.com/ynishi/propel.git"),
            Some("ynishi/propel".to_owned())
        );
    }

    #[test]
    fn parse_github_repo_https_no_suffix() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo"),
            Some("owner/repo".to_owned())
        );
    }

    #[test]
    fn parse_github_repo_https_trailing_slash() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo/"),
            Some("owner/repo".to_owned())
        );
    }

    #[test]
    fn parse_github_repo_non_github() {
        assert_eq!(parse_github_repo("git@gitlab.com:owner/repo.git"), None);
    }

    #[test]
    fn parse_github_repo_empty() {
        assert_eq!(parse_github_repo(""), None);
    }

    #[test]
    fn workflow_yaml_contains_required_sections() {
        let yaml = generate_workflow_yaml();
        assert!(yaml.contains("workload_identity_provider"));
        assert!(yaml.contains("service_account"));
        assert!(yaml.contains("propel deploy --allow-dirty"));
        assert!(yaml.contains("id-token: write"));
        assert!(yaml.contains("cargo install propel-cli"));
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn parse_github_repo_never_panics(s in "\\PC*") {
                let _ = parse_github_repo(&s);
            }

            #[test]
            fn parse_github_repo_ssh_roundtrip(
                owner in "[a-zA-Z0-9_-]{1,39}",
                repo in "[a-zA-Z0-9._-]{1,100}",
            ) {
                let url = format!("git@github.com:{owner}/{repo}.git");
                let result = parse_github_repo(&url);
                prop_assert_eq!(result, Some(format!("{owner}/{repo}")));
            }

            #[test]
            fn parse_github_repo_https_roundtrip(
                owner in "[a-zA-Z0-9_-]{1,39}",
                repo in "[a-zA-Z0-9._-]{1,100}",
            ) {
                let url = format!("https://github.com/{owner}/{repo}.git");
                let result = parse_github_repo(&url);
                prop_assert_eq!(result, Some(format!("{owner}/{repo}")));
            }

            #[test]
            fn parse_github_repo_http_roundtrip(
                owner in "[a-zA-Z0-9_-]{1,39}",
                repo in "[a-zA-Z0-9._-]{1,100}",
            ) {
                let url = format!("http://github.com/{owner}/{repo}.git");
                let result = parse_github_repo(&url);
                prop_assert_eq!(result, Some(format!("{owner}/{repo}")));
            }

            #[test]
            fn parse_github_repo_non_github_returns_none(
                host in "[a-z]{3,10}\\.[a-z]{2,5}",
                path in "[a-zA-Z0-9/_-]{1,50}",
            ) {
                prop_assume!(host != "github.com");
                let ssh = format!("git@{host}:{path}.git");
                prop_assert_eq!(parse_github_repo(&ssh), None);

                let https = format!("https://{host}/{path}.git");
                prop_assert_eq!(parse_github_repo(&https), None);
            }
        }
    }
}
