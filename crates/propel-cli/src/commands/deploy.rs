use propel_build::dockerfile::DockerfileGenerator;
use propel_build::{bundle, eject as eject_mod};
use propel_cloud::GcloudClient;
use propel_core::{ProjectMeta, PropelConfig};
use std::path::PathBuf;

/// Execute the full deploy pipeline.
pub async fn deploy() -> anyhow::Result<()> {
    let project_dir = PathBuf::from(".");
    let client = GcloudClient::new();

    // Load configuration
    let config = PropelConfig::load(&project_dir)?;
    let meta = ProjectMeta::from_cargo_toml(&project_dir)?;

    let gcp_project_id = config.project.gcp_project_id.as_deref().ok_or_else(|| {
        anyhow::anyhow!("gcp_project_id not set in propel.toml — set [project].gcp_project_id")
    })?;

    let service_name = config.project.name.as_deref().unwrap_or(&meta.name);

    let region = &config.project.region;
    let repo_name = "propel";
    let image_tag = format!(
        "{region}-docker.pkg.dev/{project}/{repo}/{service}:latest",
        region = region,
        project = gcp_project_id,
        repo = repo_name,
        service = service_name,
    );

    // Pre-flight checks
    eprintln!("Running pre-flight checks...");
    let report = client.check_prerequisites(gcp_project_id).await?;

    if report.has_warnings() {
        eprintln!("Warning: the following APIs are not enabled:");
        for api in &report.disabled_apis {
            eprintln!("  - {api}");
        }
        eprintln!("Enable them with: gcloud services enable <api> --project {gcp_project_id}");
        anyhow::bail!("required APIs not enabled");
    }

    // Ensure Artifact Registry repository
    eprintln!("Ensuring Artifact Registry repository...");
    client
        .ensure_artifact_repo(gcp_project_id, region, repo_name)
        .await?;

    // Determine Dockerfile content
    let dockerfile_content = if eject_mod::is_ejected(&project_dir) {
        eprintln!("Using ejected Dockerfile from .propel/Dockerfile");
        eject_mod::load_ejected_dockerfile(&project_dir)?
    } else {
        let generator = DockerfileGenerator::new(&config.build, &meta);
        generator.render()
    };

    // Bundle source
    eprintln!("Bundling source...");
    let bundle_dir = bundle::create_bundle(&project_dir, &dockerfile_content)?;

    // Submit build
    eprintln!("Submitting build to Cloud Build...");
    client
        .submit_build(&bundle_dir, gcp_project_id, &image_tag)
        .await?;

    // Deploy to Cloud Run
    eprintln!("Deploying to Cloud Run ({region})...");
    let url = client
        .deploy_to_cloud_run(
            service_name,
            &image_tag,
            gcp_project_id,
            region,
            &config.cloud_run,
        )
        .await?;

    eprintln!();
    eprintln!("Deployed: {url}");

    Ok(())
}
