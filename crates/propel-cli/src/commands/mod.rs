mod ci;
mod deploy;
mod destroy;
mod doctor;
mod eject;
mod init;
mod logs;
mod new;
mod secret;
mod status;

use propel_core::PropelConfig;

/// Artifact Registry repository name used for container images.
pub(crate) const ARTIFACT_REPO_NAME: &str = "propel";

/// Extract `gcp_project_id` from config, returning a clear error if not set.
fn require_gcp_project_id(config: &PropelConfig) -> anyhow::Result<&str> {
    config.project.gcp_project_id.as_deref().ok_or_else(|| {
        anyhow::anyhow!("gcp_project_id not set in propel.toml — set [project].gcp_project_id")
    })
}

pub use ci::ci_init;
pub use deploy::deploy;
pub use destroy::destroy;
pub use doctor::doctor;
pub use eject::eject;
pub use init::init_project;
pub use logs::logs;
pub use new::new_project;
pub use secret::{secret_delete, secret_list, secret_set};
pub use status::status;
