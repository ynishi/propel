use propel_cloud::GcloudClient;
use propel_core::PropelConfig;
use std::io::Write;
use std::path::PathBuf;

pub async fn secret_set(key_value: &str) -> anyhow::Result<()> {
    let (key, value) = key_value
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("expected KEY=VALUE format"))?;

    let config = PropelConfig::load(&PathBuf::from("."))?;
    let project_id = config
        .project
        .gcp_project_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gcp_project_id not set in propel.toml"))?;

    let client = GcloudClient::new();
    client.set_secret(project_id, key, value).await?;

    println!("Secret '{key}' set successfully");
    Ok(())
}

pub async fn secret_delete(key: &str, skip_confirm: bool) -> anyhow::Result<()> {
    let config = PropelConfig::load(&PathBuf::from("."))?;
    let project_id = config
        .project
        .gcp_project_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gcp_project_id not set in propel.toml"))?;

    if !skip_confirm {
        print!("Delete secret '{key}'? [y/N] ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !matches!(input.trim(), "y" | "Y" | "yes" | "YES") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let client = GcloudClient::new();
    client.delete_secret(project_id, key).await?;

    println!("Secret '{key}' deleted");
    Ok(())
}

pub async fn secret_list() -> anyhow::Result<()> {
    let config = PropelConfig::load(&PathBuf::from("."))?;
    let project_id = config
        .project
        .gcp_project_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("gcp_project_id not set in propel.toml"))?;

    let client = GcloudClient::new();
    let secrets = client.list_secrets(project_id).await?;

    if secrets.is_empty() {
        println!("No secrets found");
    } else {
        for name in &secrets {
            println!("{name}");
        }
    }
    Ok(())
}
