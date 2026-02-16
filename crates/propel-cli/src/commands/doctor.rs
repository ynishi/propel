use propel_cloud::GcloudClient;
use propel_core::PropelConfig;
use std::path::Path;

pub async fn doctor() -> anyhow::Result<()> {
    println!();
    println!("Propel Doctor");
    println!("------------------------------");

    let config = PropelConfig::load(Path::new("."));
    let project_id = config
        .as_ref()
        .ok()
        .and_then(|c| c.project.gcp_project_id.as_deref());

    let client = GcloudClient::new();
    let mut report = client.doctor(project_id).await;

    // Config file check
    let config_exists = std::path::Path::new("propel.toml").exists();
    if config_exists {
        report.config_file = propel_cloud::CheckResult::ok("Found");
    } else {
        report.config_file = propel_cloud::CheckResult::fail("Not found");
    }

    // Render table
    let rows: Vec<(&str, &propel_cloud::CheckResult)> = vec![
        ("gcloud CLI", &report.gcloud),
        ("Authentication", &report.account),
        ("GCP Project", &report.project),
        ("Billing", &report.billing),
    ];

    for (label, result) in &rows {
        println!("{:<22}{:<4}{}", label, result.icon(), result.detail);
    }

    for api in &report.apis {
        println!(
            "{:<22}{:<4}{}",
            format!("{} API", api.name),
            api.result.icon(),
            api.result.detail
        );
    }

    println!(
        "{:<22}{:<4}{}",
        "propel.toml",
        report.config_file.icon(),
        report.config_file.detail
    );

    println!("------------------------------");

    if report.all_passed() {
        println!("All checks passed!");
    } else {
        anyhow::bail!("some checks failed — see above for details");
    }

    Ok(())
}
