use propel_cloud::GcloudClient;
use propel_core::{ProjectMeta, PropelConfig};
use std::io::Write;
use std::path::PathBuf;

/// Mask a secret name, showing first 5 chars + "***".
fn mask_name(name: &str) -> String {
    let prefix: String = name.chars().take(5).collect();
    format!("{prefix}***")
}

/// Delete Cloud Run service, container image, and local bundle.
pub async fn destroy(skip_confirm: bool, include_secrets: bool) -> anyhow::Result<()> {
    let project_dir = PathBuf::from(".");
    let client = GcloudClient::new();

    let config = PropelConfig::load(&project_dir)?;
    let meta = ProjectMeta::from_cargo_toml(&project_dir)?;

    let gcp_project_id = config.project.gcp_project_id.as_deref().ok_or_else(|| {
        anyhow::anyhow!("gcp_project_id not set in propel.toml — set [project].gcp_project_id")
    })?;

    let service_name = config.project.name.as_deref().unwrap_or(&meta.name);
    let region = &config.project.region;

    // Discover secrets for display / deletion
    let secrets = client
        .list_secrets(gcp_project_id)
        .await
        .unwrap_or_default();

    if !skip_confirm {
        println!("This will delete:");
        println!("  - Cloud Run service '{service_name}' in {region}");
        println!("  - Container images in Artifact Registry");
        println!("  - Local .propel-bundle/");

        if include_secrets && !secrets.is_empty() {
            println!("  - {} secret(s) from Secret Manager:", secrets.len());
            for s in &secrets {
                println!("      {}", mask_name(s));
            }
        }

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

    // 3. Delete secrets if requested
    if include_secrets && !secrets.is_empty() {
        println!("Deleting {} secret(s)...", secrets.len());
        for s in &secrets {
            match client.delete_secret(gcp_project_id, s).await {
                Ok(()) => println!("  Deleted {}", mask_name(s)),
                Err(e) => println!("  Skipped {} ({})", mask_name(s), e),
            }
        }
    }

    // 4. Clean local bundle
    let bundle_dir = project_dir.join(".propel-bundle");
    if bundle_dir.exists() {
        std::fs::remove_dir_all(&bundle_dir)?;
        println!("Removed local .propel-bundle/");
    }

    println!();
    println!("Destroy complete.");

    // Show remaining secrets hint
    if !include_secrets && !secrets.is_empty() {
        println!();
        println!(
            "Note: {} secret(s) remain in Secret Manager.",
            secrets.len()
        );
        println!("  To delete them: propel destroy --include-secrets");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_name_ascii_long() {
        assert_eq!(mask_name("MY_SECRET_KEY"), "MY_SE***");
    }

    #[test]
    fn mask_name_ascii_exact_five() {
        assert_eq!(mask_name("ABCDE"), "ABCDE***");
    }

    #[test]
    fn mask_name_ascii_short() {
        assert_eq!(mask_name("AB"), "AB***");
    }

    #[test]
    fn mask_name_empty() {
        assert_eq!(mask_name(""), "***");
    }

    #[test]
    fn mask_name_non_ascii() {
        assert_eq!(mask_name("秘密のキー"), "秘密のキー***");
    }

    #[test]
    fn mask_name_mixed_ascii_non_ascii() {
        assert_eq!(mask_name("KEY_秘密"), "KEY_秘***");
    }
}
