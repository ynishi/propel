use std::path::Path;

/// Bundles source files for Cloud Build submission.
///
/// Creates a `.s2-bundle/` directory containing:
/// - src/
/// - Cargo.toml
/// - Cargo.lock
/// - Dockerfile (generated)
pub fn create_bundle(
    project_dir: &Path,
    dockerfile_content: &str,
) -> Result<std::path::PathBuf, BundleError> {
    let bundle_dir = project_dir.join(".s2-bundle");

    // Clean previous bundle
    if bundle_dir.exists() {
        std::fs::remove_dir_all(&bundle_dir).map_err(|e| BundleError::Cleanup {
            path: bundle_dir.clone(),
            source: e,
        })?;
    }
    std::fs::create_dir_all(&bundle_dir).map_err(|e| BundleError::Create {
        path: bundle_dir.clone(),
        source: e,
    })?;

    // Copy source files
    copy_dir_recursive(&project_dir.join("src"), &bundle_dir.join("src"))?;

    // Copy Cargo.toml and Cargo.lock
    for filename in &["Cargo.toml", "Cargo.lock"] {
        let src = project_dir.join(filename);
        if src.exists() {
            std::fs::copy(&src, bundle_dir.join(filename)).map_err(|e| BundleError::CopyFile {
                path: src,
                source: e,
            })?;
        }
    }

    // Write generated Dockerfile
    std::fs::write(bundle_dir.join("Dockerfile"), dockerfile_content).map_err(|e| {
        BundleError::WriteDockerfile {
            path: bundle_dir.join("Dockerfile"),
            source: e,
        }
    })?;

    Ok(bundle_dir)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), BundleError> {
    std::fs::create_dir_all(dst).map_err(|e| BundleError::Create {
        path: dst.to_path_buf(),
        source: e,
    })?;

    for entry in std::fs::read_dir(src).map_err(|e| BundleError::ReadDir {
        path: src.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| BundleError::ReadDir {
            path: src.to_path_buf(),
            source: e,
        })?;
        let file_type = entry.file_type().map_err(|e| BundleError::ReadDir {
            path: entry.path(),
            source: e,
        })?;
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path).map_err(|e| BundleError::CopyFile {
                path: entry.path(),
                source: e,
            })?;
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("failed to clean up bundle directory {path}")]
    Cleanup {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to create directory {path}")]
    Create {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read directory {path}")]
    ReadDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to copy file {path}")]
    CopyFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write Dockerfile at {path}")]
    WriteDockerfile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}
