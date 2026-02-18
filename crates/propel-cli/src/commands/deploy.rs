use std::path::PathBuf;

/// Execute the full deploy pipeline (CLI entry point).
pub async fn deploy(allow_dirty: bool) -> anyhow::Result<()> {
    let outcome = super::deploy_pipeline::run(&PathBuf::from("."), allow_dirty, false).await?;

    for step in &outcome.steps {
        println!("{step}");
    }

    Ok(())
}
