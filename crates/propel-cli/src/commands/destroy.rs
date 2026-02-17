use super::ci;
use propel_cloud::GcloudClient;
use propel_core::{ProjectMeta, PropelConfig};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Mask a secret name, showing first 5 chars + "***".
fn mask_name(name: &str) -> String {
    let prefix: String = name.chars().take(5).collect();
    format!("{prefix}***")
}

/// Delete Cloud Run service, container image, and local bundle.
pub async fn destroy(
    skip_confirm: bool,
    include_secrets: bool,
    include_ci: bool,
) -> anyhow::Result<()> {
    let project_dir = PathBuf::from(".");
    let client = GcloudClient::new();

    let config = PropelConfig::load(&project_dir)?;
    let meta = ProjectMeta::from_cargo_toml(&project_dir)?;

    let gcp_project_id = super::require_gcp_project_id(&config)?;

    let service_name = config.project.name.as_deref().unwrap_or(&meta.name);
    let region = &config.project.region;

    // Discover secrets for display / deletion
    let secrets = match client.list_secrets(gcp_project_id).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Warning: could not list secrets: {e}");
            vec![]
        }
    };

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

        if include_ci {
            println!("  - Workload Identity Pool 'propel-github'");
            println!(
                "  - Service Account 'propel-deploy@{gcp_project_id}.iam.gserviceaccount.com'"
            );
            println!("  - GitHub Secrets (GCP_PROJECT_ID, WIF_PROVIDER, WIF_SERVICE_ACCOUNT)");
            println!("  - {}", ci::WORKFLOW_PATH);
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

    let repo_name = super::ARTIFACT_REPO_NAME;
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

    // 4. Delete CI/CD resources if requested
    if include_ci {
        println!("Deleting CI/CD resources...");

        // WIF Pool (providers are cascade-deleted)
        match client
            .delete_wif_pool(gcp_project_id, ci::WIF_POOL_ID)
            .await
        {
            Ok(()) => println!("  Deleted WIF Pool '{}'", ci::WIF_POOL_ID),
            Err(e) => println!("  Skipped WIF Pool ({})", e),
        }

        // Service Account
        let sa_email = format!("{}@{gcp_project_id}.iam.gserviceaccount.com", ci::CI_SA_ID);
        match client
            .delete_service_account(gcp_project_id, &sa_email)
            .await
        {
            Ok(()) => println!("  Deleted Service Account"),
            Err(e) => println!("  Skipped Service Account ({})", e),
        }

        // GitHub Secrets (best-effort)
        for secret_name in ci::GH_SECRET_NAMES {
            match ci::delete_gh_secret(secret_name).await {
                Ok(()) => println!("  Deleted GitHub Secret: {secret_name}"),
                Err(e) => println!("  Skipped GitHub Secret {secret_name} ({e})"),
            }
        }

        // Workflow file
        let workflow = Path::new(ci::WORKFLOW_PATH);
        if workflow.exists() {
            std::fs::remove_file(workflow)?;
            println!("  Deleted {}", ci::WORKFLOW_PATH);
        }
    }

    // 5. Clean local bundle
    let bundle_dir = project_dir.join(".propel-bundle");
    if bundle_dir.exists() {
        std::fs::remove_dir_all(&bundle_dir)?;
        println!("Removed local .propel-bundle/");
    }

    println!();
    println!("Destroy complete.");

    // Show remaining resource hints
    if !include_secrets && !secrets.is_empty() {
        println!();
        println!(
            "Note: {} secret(s) remain in Secret Manager.",
            secrets.len()
        );
        println!("  To delete them: propel destroy --include-secrets");
    }

    if !include_ci && Path::new(ci::WORKFLOW_PATH).exists() {
        println!();
        println!("Note: CI/CD resources remain (WIF, Service Account, GitHub Secrets, workflow).");
        println!("  To delete them: propel destroy --include-ci");
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
