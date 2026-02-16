use propel_cloud::GcloudClient;
use propel_core::{ProjectMeta, PropelConfig};
use std::io::Write;
use std::path::PathBuf;

/// Delete Cloud Run service, container image, and local bundle.
pub async fn destroy(skip_confirm: bool) -> anyhow::Result<()> {
    let project_dir = PathBuf::from(".");
    let client = GcloudClient::new();

    let config = PropelConfig::load(&project_dir)?;
    let meta = ProjectMeta::from_cargo_toml(&project_dir)?;

    let gcp_project_id = config.project.gcp_project_id.as_deref().ok_or_else(|| {
        anyhow::anyhow!("gcp_project_id not set in propel.toml — set [project].gcp_project_id")
    })?;

    let service_name = config.project.name.as_deref().unwrap_or(&meta.name);
    let region = &config.project.region;

    if !skip_confirm {
        println!("This will delete:");
        println!("  - Cloud Run service '{service_name}' in {region}");
        println!("  - Container images in Artifact Registry");
        println!("  - Local .propel-bundle/");
        println!();
        print!("Are you sure? [y/N] ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !matches!(input.trim(), "y" | "Y" | "yes" | "YES") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let repo_name = "propel";
    let image_tag = format!(
        "{region}-docker.pkg.dev/{project}/{repo}/{service}",
        region = region,
        project = gcp_project_id,
        repo = repo_name,
        service = service_name,
    );

    // 1. Delete Cloud Run service
    println!("Deleting Cloud Run service '{service_name}'...");
    match client
        .delete_service(service_name, gcp_project_id, region)
        .await
    {
        Ok(()) => println!("  Deleted."),
        Err(e) => println!("  Skipped ({})", e),
    }

    // 2. Delete container image from Artifact Registry
    println!("Deleting container image...");
    match client.delete_image(&image_tag, gcp_project_id).await {
        Ok(()) => println!("  Deleted."),
        Err(e) => println!("  Skipped ({})", e),
    }

    // 3. Clean local bundle
    let bundle_dir = project_dir.join(".propel-bundle");
    if bundle_dir.exists() {
        std::fs::remove_dir_all(&bundle_dir)?;
        println!("Removed local .propel-bundle/");
    }

    println!();
    println!("Destroy complete.");

    Ok(())
}
