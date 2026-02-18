use propel_cloud::GcloudClient;
use propel_core::PropelConfig;
use std::path::Path;

pub async fn doctor() -> anyhow::Result<()> {
    let config = PropelConfig::load(Path::new("."));
    let project_id = config
        .as_ref()
        .ok()
        .and_then(|c| c.project.gcp_project_id.as_deref());

    let client = GcloudClient::new();
    let mut report = client.doctor(project_id).await;

    // Config file check
    let config_exists = Path::new("propel.toml").exists();
    if config_exists {
        report.config_file = propel_cloud::CheckResult::ok("Found");
    } else {
        report.config_file = propel_cloud::CheckResult::fail("Not found");
    }

    println!();
    println!("{report}");

    if !report.all_passed() {
        anyhow::bail!("some checks failed — see above for details");
    }

    Ok(())
}
