use propel_cloud::GcloudClient;
use propel_core::PropelConfig;
use std::path::PathBuf;

pub async fn logs() -> anyhow::Result<()> {
    let config = PropelConfig::load(&PathBuf::from("."))?;
    let project_id = config
        .project
        .gcp_project_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gcp_project_id not set in propel.toml"))?;

    let meta = propel_core::ProjectMeta::from_cargo_toml(&PathBuf::from("."))?;
    let service_name = config.project.name.as_deref().unwrap_or(&meta.name);
    let region = &config.project.region;

    let client = GcloudClient::new();
    client.read_logs(service_name, project_id, region).await?;

    Ok(())
}
