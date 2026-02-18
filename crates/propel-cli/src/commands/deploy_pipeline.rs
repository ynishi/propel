use propel_build::dockerfile::DockerfileGenerator;
use propel_build::{bundle, eject as eject_mod};
use propel_cloud::GcloudClient;
use propel_core::{ProjectMeta, PropelConfig};
use std::path::Path;

/// Result of a successful deploy pipeline run.
pub(crate) struct DeployOutcome {
    pub steps: Vec<String>,
    /// Captured Cloud Build output (only when `capture_build` is `true`).
    pub build_output: Option<String>,
}

/// Run the full deploy pipeline: dirty check → bundle → build → deploy.
///
/// `capture_build`: when `true`, Cloud Build output is captured (MCP / non-TTY).
///                  when `false`, output is streamed to stdout (CLI).
pub(crate) async fn run(
    project_dir: &Path,
    allow_dirty: bool,
    capture_build: bool,
) -> anyhow::Result<DeployOutcome> {
    let client = GcloudClient::new();
    let mut steps = Vec::new();

    // Dirty check
    if !allow_dirty && bundle::is_dirty(project_dir)? {
        anyhow::bail!(
            "uncommitted changes detected.\n\
             Commit your changes, or pass --allow-dirty / allow_dirty=true to deploy anyway."
        );
    }

    // Load configuration
    let config = PropelConfig::load(project_dir)?;
    let meta = ProjectMeta::from_cargo_toml(project_dir)?;

    let gcp_project_id = super::require_gcp_project_id(&config)?;

    let service_name = config.project.name.as_deref().unwrap_or(&meta.name);
    let region = &config.project.region;
    let repo_name = super::ARTIFACT_REPO_NAME;
    let image_tag = format!(
        "{region}-docker.pkg.dev/{project}/{repo}/{service}:latest",
        region = region,
        project = gcp_project_id,
        repo = repo_name,
        service = service_name,
    );

    // Pre-flight checks
    let report = client.check_prerequisites(gcp_project_id).await?;
    if report.has_warnings() {
        let disabled = report.disabled_apis.join(", ");
        anyhow::bail!(
            "Required APIs not enabled: {disabled}. \
             Enable them with: gcloud services enable <api> --project {gcp_project_id}"
        );
    }
    steps.push("Pre-flight checks passed".to_string());

    // Ensure Artifact Registry repository
    client
        .ensure_artifact_repo(gcp_project_id, region, repo_name)
        .await?;
    steps.push("Artifact Registry repository ensured".to_string());

    // Determine Dockerfile content
    let dockerfile_content = if eject_mod::is_ejected(project_dir) {
        steps.push("Using ejected Dockerfile".to_string());
        eject_mod::load_ejected_dockerfile(project_dir)?
    } else {
        let generator = DockerfileGenerator::new(&config.build, &meta, config.cloud_run.port);
        generator.render()
    };

    // Bundle source
    let bundle_dir = bundle::create_bundle(project_dir, &dockerfile_content)?;
    steps.push("Source bundled".to_string());

    // Submit build
    let build_output = client
        .submit_build(&bundle_dir, gcp_project_id, &image_tag, capture_build)
        .await?;
    steps.push("Cloud Build completed".to_string());

    // Discover secrets
    let secrets = match client.list_secrets(gcp_project_id).await {
        Ok(s) => s,
        Err(e) => {
            steps.push(format!("Warning: could not list secrets: {e}"));
            vec![]
        }
    };
    if secrets.is_empty() {
        steps.push("No secrets found in Secret Manager".to_string());
    } else {
        steps.push(format!("{} secret(s) will be injected", secrets.len()));
    }

    // Deploy to Cloud Run
    let url = client
        .deploy_to_cloud_run(
            service_name,
            &image_tag,
            gcp_project_id,
            region,
            &config.cloud_run,
            &secrets,
        )
        .await?;
    steps.push(format!("Deployed: {url}"));

    Ok(DeployOutcome {
        steps,
        build_output,
    })
}
