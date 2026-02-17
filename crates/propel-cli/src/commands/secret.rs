use propel_cloud::GcloudClient;
use propel_core::PropelConfig;
use std::io::Write;
use std::path::PathBuf;

pub async fn secret_set(key_value: &str) -> anyhow::Result<()> {
    let (key, value) = key_value
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("expected KEY=VALUE format"))?;

    let config = PropelConfig::load(&PathBuf::from("."))?;
    let project_id = super::require_gcp_project_id(&config)?;

    let client = GcloudClient::new();
    client.set_secret(project_id, key, value).await?;

    // Grant Cloud Run default SA access to read this secret.
    // This runs locally where the user has admin permissions,
    // so deploy (CI) only needs secretmanager.viewer.
    let project_number = client.get_project_number(project_id).await?;
    let sa = format!("{project_number}-compute@developer.gserviceaccount.com");
    client.grant_secret_access(project_id, key, &sa).await?;

    println!("Secret '{key}' set successfully (Cloud Run SA granted access)");
    Ok(())
}

pub async fn secret_delete(key: &str, skip_confirm: bool) -> anyhow::Result<()> {
    let config = PropelConfig::load(&PathBuf::from("."))?;
    let project_id = super::require_gcp_project_id(&config)?;

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

    // Revoke Cloud Run SA's access before deleting the secret itself.
    let project_number = client.get_project_number(project_id).await?;
    let sa = format!("{project_number}-compute@developer.gserviceaccount.com");
    if let Err(e) = client.revoke_secret_access(project_id, key, &sa).await {
        eprintln!("Warning: could not revoke SA binding for '{key}': {e}");
    }

    client.delete_secret(project_id, key).await?;

    println!("Secret '{key}' deleted");
    Ok(())
}

pub async fn secret_list() -> anyhow::Result<()> {
    let config = PropelConfig::load(&PathBuf::from("."))?;
    let project_id = super::require_gcp_project_id(&config)?;

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
