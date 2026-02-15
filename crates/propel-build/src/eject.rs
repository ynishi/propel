use std::path::Path;

/// Ejects build configuration files into the project directory.
///
/// After ejecting, `propel deploy` will use `.propel/Dockerfile`
/// instead of generating one.
pub fn eject(project_dir: &Path, dockerfile_content: &str) -> Result<(), EjectError> {
    let propel_dir = project_dir.join(".propel");
    std::fs::create_dir_all(&propel_dir).map_err(|e| EjectError::CreateDir {
        path: propel_dir.clone(),
        source: e,
    })?;

    let dockerfile_path = propel_dir.join("Dockerfile");
    if dockerfile_path.exists() {
        return Err(EjectError::AlreadyEjected(dockerfile_path));
    }

    std::fs::write(&dockerfile_path, dockerfile_content).map_err(|e| EjectError::Write {
        path: dockerfile_path,
        source: e,
    })?;

    Ok(())
}

/// Check if the project has ejected build config.
pub fn is_ejected(project_dir: &Path) -> bool {
    project_dir.join(".propel").join("Dockerfile").exists()
}

/// Load ejected Dockerfile content.
pub fn load_ejected_dockerfile(project_dir: &Path) -> Result<String, EjectError> {
    let path = project_dir.join(".propel").join("Dockerfile");
    std::fs::read_to_string(&path).map_err(|e| EjectError::Read { path, source: e })
}

#[derive(Debug, thiserror::Error)]
pub enum EjectError {
    #[error("failed to create .propel directory at {path}")]
    CreateDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("build config already ejected at {0} — edit directly or delete to re-eject")]
    AlreadyEjected(std::path::PathBuf),
    #[error("failed to write {path}")]
    Write {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read ejected Dockerfile at {path}")]
    Read {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}
