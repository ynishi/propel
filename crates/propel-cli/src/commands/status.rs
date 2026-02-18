use propel_cloud::GcloudClient;
use propel_core::PropelConfig;
use std::path::PathBuf;

pub async fn status() -> anyhow::Result<()> {
    let config = PropelConfig::load(&PathBuf::from("."))?;
    let project_id = config
        .project
        .gcp_project_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gcp_project_id not set in propel.toml"))?;

    let meta = propel_core::ProjectMeta::from_cargo_toml(&PathBuf::from("."))?;
    let service_name = super::service_name(&config, &meta);
    let region = &config.project.region;

    let client = GcloudClient::new();
    let output = client
        .describe_service(service_name, project_id, region)
        .await?;

    println!("{output}");
    Ok(())
}
