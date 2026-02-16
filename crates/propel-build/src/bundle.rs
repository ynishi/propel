use std::path::{Path, PathBuf};
use std::process::Command;

/// Files/directories that propel always excludes from bundles,
/// regardless of .gitignore content.
const PROPEL_EXCLUDES: &[&str] = &[".propel-bundle", ".propel", ".git"];

/// Bundles project files for Cloud Build submission.
///
/// Uses `git ls-files` to respect `.gitignore`, then copies all tracked
/// and untracked-but-not-ignored files into `.propel-bundle/`.
/// The generated Dockerfile is written into the bundle.
pub fn create_bundle(project_dir: &Path, dockerfile_content: &str) -> Result<PathBuf, BundleError> {
    let bundle_dir = project_dir.join(".propel-bundle");

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

    // Get file list from git (respects .gitignore)
    let files = git_ls_files(project_dir)?;

    // Copy each file into the bundle
    for relative_path in &files {
        // Skip propel-specific directories
        if PROPEL_EXCLUDES
            .iter()
            .any(|ex| relative_path.starts_with(ex))
        {
            continue;
        }

        let src = project_dir.join(relative_path);
        let dst = bundle_dir.join(relative_path);

        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| BundleError::Create {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        std::fs::copy(&src, &dst).map_err(|e| BundleError::CopyFile {
            path: src,
            source: e,
        })?;
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

/// Returns the list of files git considers part of the project:
/// tracked files + untracked files that are not .gitignored.
fn git_ls_files(project_dir: &Path) -> Result<Vec<PathBuf>, BundleError> {
    let output = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| BundleError::GitCommand {
            detail: "failed to execute git ls-files".to_owned(),
            source: e,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BundleError::GitFailed {
            detail: format!(
                "git ls-files exited with {}: {}",
                output.status,
                stderr.trim()
            ),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<PathBuf> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect();

    Ok(files)
}

/// Checks whether the git working tree has uncommitted changes.
pub fn is_dirty(project_dir: &Path) -> Result<bool, BundleError> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| BundleError::GitCommand {
            detail: "failed to execute git status".to_owned(),
            source: e,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BundleError::GitFailed {
            detail: format!(
                "git status exited with {}: {}",
                output.status,
                stderr.trim()
            ),
        });
    }

    Ok(!output.stdout.is_empty())
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
    #[error("git command failed: {detail}")]
    GitCommand {
        detail: String,
        source: std::io::Error,
    },
    #[error("git failed: {detail}")]
    GitFailed { detail: String },
}
