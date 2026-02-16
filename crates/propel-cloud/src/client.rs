use crate::executor::{GcloudExecutor, RealExecutor};
use crate::gcloud::GcloudError;
use propel_core::CloudRunConfig;
use std::path::Path;

/// GCP operations client, parameterized over the executor for testability.
pub struct GcloudClient<E: GcloudExecutor = RealExecutor> {
    executor: E,
}

impl GcloudClient<RealExecutor> {
    pub fn new() -> Self {
        Self {
            executor: RealExecutor,
        }
    }
}

impl Default for GcloudClient<RealExecutor> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: GcloudExecutor> GcloudClient<E> {
    pub fn with_executor(executor: E) -> Self {
        Self { executor }
    }

    // ── Preflight ──

    pub async fn check_prerequisites(
        &self,
        project_id: &str,
    ) -> Result<PreflightReport, PreflightError> {
        let mut report = PreflightReport::default();

        // 1. gcloud CLI available
        match self
            .executor
            .exec(&args(["version", "--format", "value(version)"]))
            .await
        {
            Ok(version) => report.gcloud_version = Some(version.trim().to_owned()),
            Err(_) => return Err(PreflightError::GcloudNotInstalled),
        }

        // 2. Authenticated
        match self
            .executor
            .exec(&args(["auth", "print-identity-token", "--quiet"]))
            .await
        {
            Ok(_) => report.authenticated = true,
            Err(_) => return Err(PreflightError::NotAuthenticated),
        }

        // 3. Project accessible
        match self
            .executor
            .exec(&args([
                "projects",
                "describe",
                project_id,
                "--format",
                "value(name)",
            ]))
            .await
        {
            Ok(name) => report.project_name = Some(name.trim().to_owned()),
            Err(_) => return Err(PreflightError::ProjectNotAccessible(project_id.to_owned())),
        }

        // 4. Required APIs enabled
        for api in &[
            "cloudbuild.googleapis.com",
            "run.googleapis.com",
            "secretmanager.googleapis.com",
        ] {
            let enabled = self
                .executor
                .exec(&args([
                    "services",
                    "list",
                    "--project",
                    project_id,
                    "--filter",
                    &format!("config.name={api}"),
                    "--format",
                    "value(config.name)",
                ]))
                .await
                .map(|out| !out.trim().is_empty())
                .unwrap_or(false);

            if !enabled {
                report.disabled_apis.push((*api).to_owned());
            }
        }

        Ok(report)
    }

    // ── Doctor ──

    /// Run all diagnostic checks without early return.
    /// Returns a report with pass/fail for each check item.
    pub async fn doctor(&self, project_id: Option<&str>) -> DoctorReport {
        let mut report = DoctorReport::default();

        // 1. gcloud CLI
        match self.executor.exec(&args(["version"])).await {
            Ok(v) => {
                // Parse "Google Cloud SDK X.Y.Z" from first line
                let version = v
                    .lines()
                    .next()
                    .and_then(|line| line.strip_prefix("Google Cloud SDK "))
                    .unwrap_or(v.trim());
                report.gcloud = CheckResult::ok(version.trim());
            }
            Err(e) => report.gcloud = CheckResult::fail(&e.to_string()),
        }

        // 2. Active account
        match self
            .executor
            .exec(&args(["config", "get-value", "account"]))
            .await
        {
            Ok(a) if !a.trim().is_empty() => report.account = CheckResult::ok(a.trim()),
            _ => report.account = CheckResult::fail("no active account"),
        }

        // 3. Project
        let Some(pid) = project_id else {
            report.project = CheckResult::fail("gcp_project_id not set in propel.toml");
            return report;
        };

        match self
            .executor
            .exec(&args([
                "projects",
                "describe",
                pid,
                "--format",
                "value(name)",
            ]))
            .await
        {
            Ok(name) => {
                report.project = CheckResult::ok(&format!("{pid} ({name})", name = name.trim()))
            }
            Err(_) => {
                report.project = CheckResult::fail(&format!("{pid} — not accessible"));
                return report;
            }
        }

        // 4. Billing
        match self
            .executor
            .exec(&args([
                "billing",
                "projects",
                "describe",
                pid,
                "--format",
                "value(billingEnabled)",
            ]))
            .await
        {
            Ok(v) if v.trim().eq_ignore_ascii_case("true") => {
                report.billing = CheckResult::ok("Enabled");
            }
            _ => report.billing = CheckResult::fail("Billing not enabled"),
        }

        // 5. Required APIs
        let required_apis = [
            ("Cloud Build", "cloudbuild.googleapis.com"),
            ("Cloud Run", "run.googleapis.com"),
            ("Secret Manager", "secretmanager.googleapis.com"),
            ("Artifact Registry", "artifactregistry.googleapis.com"),
        ];

        for (label, api) in &required_apis {
            let enabled = self
                .executor
                .exec(&args([
                    "services",
                    "list",
                    "--project",
                    pid,
                    "--filter",
                    &format!("config.name={api}"),
                    "--format",
                    "value(config.name)",
                ]))
                .await
                .map(|out| !out.trim().is_empty())
                .unwrap_or(false);

            report.apis.push(ApiCheck {
                name: label.to_string(),
                result: if enabled {
                    CheckResult::ok("Enabled")
                } else {
                    CheckResult::fail("Not enabled")
                },
            });
        }

        report
    }

    // ── Artifact Registry ──

    /// Ensure the Artifact Registry Docker repository exists, creating it if needed.
    pub async fn ensure_artifact_repo(
        &self,
        project_id: &str,
        region: &str,
        repo_name: &str,
    ) -> Result<(), DeployError> {
        let exists = self
            .executor
            .exec(&args([
                "artifacts",
                "repositories",
                "describe",
                repo_name,
                "--project",
                project_id,
                "--location",
                region,
            ]))
            .await
            .is_ok();

        if !exists {
            self.executor
                .exec(&args([
                    "artifacts",
                    "repositories",
                    "create",
                    repo_name,
                    "--project",
                    project_id,
                    "--location",
                    region,
                    "--repository-format",
                    "docker",
                    "--quiet",
                ]))
                .await
                .map_err(|e| DeployError::Deploy { source: e })?;
        }

        Ok(())
    }

    /// Delete a container image from Artifact Registry.
    pub async fn delete_image(&self, image_tag: &str, project_id: &str) -> Result<(), DeployError> {
        self.executor
            .exec(&args([
                "artifacts",
                "docker",
                "images",
                "delete",
                image_tag,
                "--project",
                project_id,
                "--delete-tags",
                "--quiet",
            ]))
            .await
            .map_err(|e| DeployError::Deploy { source: e })?;

        Ok(())
    }

    // ── Cloud Build ──

    pub async fn submit_build(
        &self,
        bundle_dir: &Path,
        project_id: &str,
        image_tag: &str,
    ) -> Result<(), CloudBuildError> {
        let bundle_str = bundle_dir
            .to_str()
            .ok_or_else(|| CloudBuildError::InvalidPath(bundle_dir.to_path_buf()))?;

        self.executor
            .exec_streaming(&args([
                "builds",
                "submit",
                bundle_str,
                "--project",
                project_id,
                "--tag",
                image_tag,
                "--quiet",
            ]))
            .await
            .map_err(|e| CloudBuildError::Submit { source: e })
    }

    // ── Cloud Run Deploy ──

    pub async fn deploy_to_cloud_run(
        &self,
        service_name: &str,
        image_tag: &str,
        project_id: &str,
        region: &str,
        config: &CloudRunConfig,
        secrets: &[String],
    ) -> Result<String, DeployError> {
        let cpu = config.cpu.to_string();
        let min = config.min_instances.to_string();
        let max = config.max_instances.to_string();
        let concurrency = config.concurrency.to_string();
        let port = config.port.to_string();

        // Build --update-secrets value: ENV_VAR=SECRET_NAME:latest,...
        let secrets_flag = secrets
            .iter()
            .map(|s| format!("{s}={s}:latest"))
            .collect::<Vec<_>>()
            .join(",");

        let mut cmd = vec![
            "run",
            "deploy",
            service_name,
            "--image",
            image_tag,
            "--project",
            project_id,
            "--region",
            region,
            "--platform",
            "managed",
            "--memory",
            &config.memory,
            "--cpu",
            &cpu,
            "--min-instances",
            &min,
            "--max-instances",
            &max,
            "--concurrency",
            &concurrency,
            "--port",
            &port,
            "--allow-unauthenticated",
            "--quiet",
            "--format",
            "value(status.url)",
        ];

        if !secrets_flag.is_empty() {
            cmd.push("--update-secrets");
            cmd.push(&secrets_flag);
        }

        let cmd_owned: Vec<String> = cmd.iter().map(|s| (*s).to_owned()).collect();

        let output = self
            .executor
            .exec(&cmd_owned)
            .await
            .map_err(|e| DeployError::Deploy { source: e })?;

        Ok(output.trim().to_owned())
    }

    pub async fn describe_service(
        &self,
        service_name: &str,
        project_id: &str,
        region: &str,
    ) -> Result<String, DeployError> {
        self.executor
            .exec(&args([
                "run",
                "services",
                "describe",
                service_name,
                "--project",
                project_id,
                "--region",
                region,
                "--format",
                "yaml(status)",
            ]))
            .await
            .map_err(|e| DeployError::Deploy { source: e })
    }

    pub async fn delete_service(
        &self,
        service_name: &str,
        project_id: &str,
        region: &str,
    ) -> Result<(), DeployError> {
        self.executor
            .exec(&args([
                "run",
                "services",
                "delete",
                service_name,
                "--project",
                project_id,
                "--region",
                region,
                "--quiet",
            ]))
            .await
            .map_err(|e| DeployError::Deploy { source: e })?;

        Ok(())
    }

    pub async fn read_logs(
        &self,
        service_name: &str,
        project_id: &str,
        region: &str,
    ) -> Result<(), DeployError> {
        self.executor
            .exec_streaming(&args([
                "run",
                "services",
                "logs",
                "read",
                service_name,
                "--project",
                project_id,
                "--region",
                region,
                "--limit",
                "100",
            ]))
            .await
            .map_err(|e| DeployError::Deploy { source: e })
    }

    // ── Secret Manager ──

    pub async fn set_secret(
        &self,
        project_id: &str,
        secret_name: &str,
        secret_value: &str,
    ) -> Result<(), SecretError> {
        let secret_exists = self
            .executor
            .exec(&args([
                "secrets",
                "describe",
                secret_name,
                "--project",
                project_id,
            ]))
            .await
            .is_ok();

        if !secret_exists {
            self.executor
                .exec(&args([
                    "secrets",
                    "create",
                    secret_name,
                    "--project",
                    project_id,
                    "--replication-policy",
                    "automatic",
                ]))
                .await
                .map_err(|e| SecretError::Create { source: e })?;
        }

        self.executor
            .exec_with_stdin(
                &args([
                    "secrets",
                    "versions",
                    "add",
                    secret_name,
                    "--project",
                    project_id,
                    "--data-file",
                    "-",
                ]),
                secret_value.as_bytes(),
            )
            .await
            .map_err(|e| SecretError::AddVersion { source: e })?;

        Ok(())
    }

    pub async fn get_project_number(&self, project_id: &str) -> Result<String, DeployError> {
        let output = self
            .executor
            .exec(&args([
                "projects",
                "describe",
                project_id,
                "--format",
                "value(projectNumber)",
            ]))
            .await
            .map_err(|e| DeployError::Deploy { source: e })?;

        Ok(output.trim().to_owned())
    }

    pub async fn grant_secret_access(
        &self,
        project_id: &str,
        secret_name: &str,
        service_account: &str,
    ) -> Result<(), SecretError> {
        let member = format!("serviceAccount:{service_account}");
        self.executor
            .exec(&args([
                "secrets",
                "add-iam-policy-binding",
                secret_name,
                "--project",
                project_id,
                "--member",
                &member,
                "--role",
                "roles/secretmanager.secretAccessor",
            ]))
            .await
            .map_err(|e| SecretError::GrantAccess { source: e })?;

        Ok(())
    }

    pub async fn list_secrets(&self, project_id: &str) -> Result<Vec<String>, SecretError> {
        let output = self
            .executor
            .exec(&args([
                "secrets",
                "list",
                "--project",
                project_id,
                "--format",
                "value(name)",
            ]))
            .await
            .map_err(|e| SecretError::List { source: e })?;

        Ok(output.lines().map(|s| s.to_owned()).collect())
    }
}

// ── Helper ──

fn args<const N: usize>(a: [&str; N]) -> Vec<String> {
    a.iter().map(|s| (*s).to_owned()).collect()
}

// ── Error types ──

#[derive(Debug, Default)]
pub struct PreflightReport {
    pub gcloud_version: Option<String>,
    pub authenticated: bool,
    pub project_name: Option<String>,
    pub disabled_apis: Vec<String>,
}

impl PreflightReport {
    pub fn has_warnings(&self) -> bool {
        !self.disabled_apis.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PreflightError {
    #[error("gcloud CLI not installed — https://cloud.google.com/sdk/docs/install")]
    GcloudNotInstalled,

    #[error("not authenticated — run: gcloud auth login")]
    NotAuthenticated,

    #[error("GCP project '{0}' is not accessible — check project ID and permissions")]
    ProjectNotAccessible(String),
}

// ── Doctor types ──

#[derive(Debug, Default)]
pub struct DoctorReport {
    pub gcloud: CheckResult,
    pub account: CheckResult,
    pub project: CheckResult,
    pub billing: CheckResult,
    pub apis: Vec<ApiCheck>,
    pub config_file: CheckResult,
}

impl DoctorReport {
    pub fn all_passed(&self) -> bool {
        self.gcloud.passed
            && self.account.passed
            && self.project.passed
            && self.billing.passed
            && self.config_file.passed
            && self.apis.iter().all(|a| a.result.passed)
    }
}

#[derive(Debug, Default, Clone)]
pub struct CheckResult {
    pub passed: bool,
    pub detail: String,
}

impl CheckResult {
    pub fn ok(detail: &str) -> Self {
        Self {
            passed: true,
            detail: detail.to_owned(),
        }
    }

    pub fn fail(detail: &str) -> Self {
        Self {
            passed: false,
            detail: detail.to_owned(),
        }
    }

    pub fn icon(&self) -> &'static str {
        if self.passed { "OK" } else { "NG" }
    }
}

#[derive(Debug, Clone)]
pub struct ApiCheck {
    pub name: String,
    pub result: CheckResult,
}

#[derive(Debug, thiserror::Error)]
pub enum CloudBuildError {
    #[error("bundle path is not valid UTF-8: {0}")]
    InvalidPath(std::path::PathBuf),

    #[error("cloud build submission failed")]
    Submit { source: GcloudError },
}

#[derive(Debug, thiserror::Error)]
pub enum DeployError {
    #[error("cloud run deployment failed")]
    Deploy { source: GcloudError },
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("failed to create secret")]
    Create { source: GcloudError },

    #[error("failed to add secret version")]
    AddVersion { source: GcloudError },

    #[error("failed to list secrets")]
    List { source: GcloudError },

    #[error("failed to grant secret access")]
    GrantAccess { source: GcloudError },
}
