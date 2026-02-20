use propel_cloud::GcloudClient;
use propel_core::{CargoProject, PropelConfig};
use std::path::PathBuf;

pub async fn logs(follow: bool, tail: Option<u32>) -> anyhow::Result<()> {
    let project_dir = PathBuf::from(".");
    let config = PropelConfig::load(&project_dir)?;
    let project_id = config
        .project
        .gcp_project_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gcp_project_id not set in propel.toml"))?;

    let project = CargoProject::discover(&project_dir)?;
    let service_name = super::service_name(&config, &project);
    let region = &config.project.region;

    let client = GcloudClient::new();

    if follow {
        client.tail_logs(service_name, project_id, region).await?;
    } else {
        let limit = tail.unwrap_or(100);
        client
            .read_logs(service_name, project_id, region, limit)
            .await?;
    }

    Ok(())
}
