use propel_build::dockerfile::DockerfileGenerator;
use propel_core::{ProjectMeta, PropelConfig};
use std::path::PathBuf;

pub async fn eject() -> anyhow::Result<()> {
    let project_dir = PathBuf::from(".");
    let config = PropelConfig::load(&project_dir)?;
    let meta = ProjectMeta::from_cargo_toml(&project_dir)?;

    let generator = DockerfileGenerator::new(&config.build, &meta, config.cloud_run.port);
    let dockerfile = generator.render();

    propel_build::eject::eject(&project_dir, &dockerfile)?;

    println!("Ejected build config to .propel/Dockerfile");
    println!("You can now edit it directly. propel deploy will use this file.");
    Ok(())
}
