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

/// Artifact Registry repository name used for container images.
pub(crate) const ARTIFACT_REPO_NAME: &str = "propel";

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
