//! Cargo project discovery via `cargo metadata`.
//!
//! Replaces manual `Cargo.toml` TOML parsing with the official metadata
//! protocol, correctly handling:
//!
//! - Workspace `version.workspace = true` inheritance
//! - Multiple binary targets with `default-run` selection
//! - Workspace member identification
//! - Accurate manifest and directory paths

use cargo_metadata::{MetadataCommand, TargetKind};
use std::path::{Path, PathBuf};

/// A binary target in a Cargo package.
///
/// # Examples
///
/// ```
/// use propel_core::CargoBinary;
/// use std::path::PathBuf;
///
/// let bin = CargoBinary {
///     name: "my-server".to_owned(),
///     src_path: PathBuf::from("src/main.rs"),
/// };
/// assert_eq!(bin.name, "my-server");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoBinary {
    /// Binary name (used with `cargo build --bin <name>`)
    pub name: String,
    /// Absolute path to the source file
    pub src_path: PathBuf,
}

/// Cargo project metadata, discovered via `cargo metadata --no-deps`.
///
/// This is the primary domain entity for Rust project management.
/// All fields are resolved by Cargo itself, ensuring accuracy even
/// for workspace members with inherited fields.
///
/// # Construction
///
/// Use [`CargoProject::discover()`] to create instances from real Cargo
/// projects. Direct struct construction is available for testing but
/// callers must ensure `default_binary` exists in `binaries`.
///
/// # Examples
///
/// ```no_run
/// use propel_core::CargoProject;
/// use std::path::Path;
///
/// let project = CargoProject::discover(Path::new(".")).unwrap();
/// println!("Deploying {} v{}", project.name, project.version);
/// println!("Binary: {}", project.default_binary);
/// ```
#[derive(Debug, Clone)]
pub struct CargoProject {
    /// Package name from `[package].name`
    pub name: String,
    /// Resolved version (handles `version.workspace = true`)
    pub version: String,
    /// Absolute path to the package's `Cargo.toml`
    pub manifest_path: PathBuf,
    /// Absolute path to the package directory (parent of `manifest_path`)
    pub package_dir: PathBuf,
    /// Absolute path to the workspace root directory
    pub workspace_root: PathBuf,
    /// All binary targets in this package
    pub binaries: Vec<CargoBinary>,
    /// The binary selected for deployment.
    ///
    /// **Invariant:** must match a name in [`binaries`](Self::binaries).
    pub default_binary: String,
}

impl CargoProject {
    /// Discover the Cargo project at the given directory.
    ///
    /// Runs `cargo metadata --no-deps` and locates the package whose
    /// manifest lives in `project_dir`. For single-package projects this
    /// is the only package; for workspaces the matching member is selected.
    ///
    /// # Errors
    ///
    /// - [`Error::CargoMetadata`] if `cargo metadata` fails (e.g. cargo not installed)
    /// - [`Error::NoPackageInDir`] if `project_dir` is a workspace root without `[package]`
    /// - [`Error::NoBinaryTarget`] if the package has no binary targets
    /// - [`Error::MultipleBinaries`] if multiple binaries exist and none is selected
    pub fn discover(project_dir: &Path) -> crate::Result<Self> {
        let manifest_path = project_dir.join("Cargo.toml");
        tracing::debug!(path = %manifest_path.display(), "running cargo metadata");

        let metadata = MetadataCommand::new()
            .manifest_path(&manifest_path)
            .no_deps()
            .exec()
            .map_err(|e| crate::Error::CargoMetadata {
                manifest_path: manifest_path.clone(),
                detail: e.to_string(),
            })?;

        let workspace_root = PathBuf::from(metadata.workspace_root.as_std_path());

        // Canonicalize project_dir for reliable path comparison
        let canonical_dir =
            project_dir
                .canonicalize()
                .map_err(|e| crate::Error::ProjectDirResolve {
                    path: project_dir.to_path_buf(),
                    source: e,
                })?;

        // Find the package whose Cargo.toml is in project_dir
        let package = metadata
            .packages
            .iter()
            .find(|p| {
                p.manifest_path
                    .as_std_path()
                    .parent()
                    .and_then(|d| match d.canonicalize() {
                        Ok(c) => Some(c),
                        Err(e) => {
                            tracing::warn!(
                                path = %d.display(),
                                error = %e,
                                "failed to canonicalize manifest parent; skipping package"
                            );
                            None
                        }
                    })
                    .is_some_and(|d| d == canonical_dir)
            })
            .ok_or_else(|| crate::Error::NoPackageInDir {
                dir: canonical_dir.clone(),
                workspace_members: metadata
                    .packages
                    .iter()
                    .filter(|p| metadata.workspace_members.contains(&p.id))
                    .map(|p| p.name.clone())
                    .collect(),
            })?;

        // Extract binary targets
        let binaries: Vec<CargoBinary> = package
            .targets
            .iter()
            .filter(|t| t.kind.contains(&TargetKind::Bin))
            .map(|t| CargoBinary {
                name: t.name.clone(),
                src_path: PathBuf::from(t.src_path.as_std_path()),
            })
            .collect();

        // Determine default binary
        let default_binary =
            Self::resolve_default_binary(&binaries, package.default_run.as_deref(), &package.name)?;

        let pkg_manifest = PathBuf::from(package.manifest_path.as_std_path());
        let pkg_dir = pkg_manifest
            .parent()
            .expect("manifest_path from cargo metadata is always absolute")
            .to_path_buf();

        tracing::debug!(
            name = %package.name,
            version = %package.version,
            binary = %default_binary,
            binaries = binaries.len(),
            workspace_root = %workspace_root.display(),
            "cargo project discovered"
        );

        Ok(Self {
            name: package.name.clone(),
            version: package.version.to_string(),
            manifest_path: pkg_manifest,
            package_dir: pkg_dir,
            workspace_root,
            binaries,
            default_binary,
        })
    }

    /// Select the binary to use for deployment.
    ///
    /// Priority:
    /// 1. `default-run` from Cargo.toml (explicit user choice)
    /// 2. Single binary (unambiguous)
    /// 3. Binary matching the package name (Cargo convention)
    /// 4. Error with guidance
    fn resolve_default_binary(
        binaries: &[CargoBinary],
        default_run: Option<&str>,
        package_name: &str,
    ) -> crate::Result<String> {
        // 1. Explicit default-run
        if let Some(name) = default_run
            && binaries.iter().any(|b| b.name == name)
        {
            return Ok(name.to_owned());
        }

        match binaries.len() {
            0 => Err(crate::Error::NoBinaryTarget {
                package: package_name.to_owned(),
            }),
            1 => Ok(binaries[0].name.clone()),
            _ => {
                // Multiple binaries: prefer the one matching the package name
                if binaries.iter().any(|b| b.name == package_name) {
                    return Ok(package_name.to_owned());
                }
                Err(crate::Error::MultipleBinaries {
                    names: binaries.iter().map(|b| b.name.clone()).collect(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_default_binary unit tests ──

    fn bin(name: &str) -> CargoBinary {
        CargoBinary {
            name: name.to_owned(),
            src_path: PathBuf::from(format!("src/bin/{name}.rs")),
        }
    }

    #[test]
    fn resolve_single_binary() {
        let bins = vec![bin("my-server")];
        let result = CargoProject::resolve_default_binary(&bins, None, "my-pkg");
        assert_eq!(result.unwrap(), "my-server");
    }

    #[test]
    fn resolve_default_run_takes_priority() {
        let bins = vec![bin("server"), bin("worker")];
        let result = CargoProject::resolve_default_binary(&bins, Some("worker"), "my-pkg");
        assert_eq!(result.unwrap(), "worker");
    }

    #[test]
    fn resolve_multiple_prefers_package_name() {
        let bins = vec![bin("my-pkg"), bin("worker")];
        let result = CargoProject::resolve_default_binary(&bins, None, "my-pkg");
        assert_eq!(result.unwrap(), "my-pkg");
    }

    #[test]
    fn resolve_no_binaries_errors() {
        let result = CargoProject::resolve_default_binary(&[], None, "lib-only");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("no binary target"), "got: {err}");
    }

    #[test]
    fn resolve_ambiguous_multiple_errors() {
        let bins = vec![bin("server"), bin("worker")];
        let result = CargoProject::resolve_default_binary(&bins, None, "my-pkg");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("server"), "got: {err}");
        assert!(err.contains("worker"), "got: {err}");
    }

    #[test]
    fn resolve_default_run_ignored_if_not_in_binaries() {
        let bins = vec![bin("server")];
        // default_run points to a non-existent binary: fall back to single-binary rule
        let result = CargoProject::resolve_default_binary(&bins, Some("ghost"), "my-pkg");
        assert_eq!(result.unwrap(), "server");
    }

    // ── Property-based tests ──

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// Strategy: valid crate name (lowercase ascii + hyphens, 1-20 chars)
        fn crate_name() -> impl Strategy<Value = String> {
            "[a-z][a-z0-9-]{0,19}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
        }

        /// Strategy: vec of 0-5 unique binary names
        fn bin_names(max: usize) -> impl Strategy<Value = Vec<String>> {
            proptest::collection::hash_set(crate_name(), 0..=max)
                .prop_map(|s| s.into_iter().collect::<Vec<_>>())
        }

        fn bins_from_names(names: &[String]) -> Vec<CargoBinary> {
            names.iter().map(|n| bin(n)).collect()
        }

        proptest! {
            #[test]
            fn never_panics(
                names in bin_names(5),
                default_run in proptest::option::of(crate_name()),
                pkg_name in crate_name(),
            ) {
                let bins = bins_from_names(&names);
                let _ = CargoProject::resolve_default_binary(
                    &bins,
                    default_run.as_deref(),
                    &pkg_name,
                );
            }

            #[test]
            fn default_run_in_binaries_always_selected(
                extra_names in bin_names(4),
                chosen in crate_name(),
            ) {
                let mut names: Vec<String> = extra_names
                    .into_iter()
                    .filter(|n| *n != chosen)
                    .collect();
                names.push(chosen.clone());
                let bins = bins_from_names(&names);

                let result = CargoProject::resolve_default_binary(
                    &bins,
                    Some(&chosen),
                    "unrelated-pkg",
                );
                prop_assert_eq!(result.unwrap(), chosen);
            }

            #[test]
            fn empty_binaries_always_errors(
                default_run in proptest::option::of(crate_name()),
                pkg_name in crate_name(),
            ) {
                let result = CargoProject::resolve_default_binary(
                    &[],
                    default_run.as_deref(),
                    &pkg_name,
                );
                prop_assert!(result.is_err());
            }

            #[test]
            fn single_binary_always_succeeds(
                name in crate_name(),
                default_run in proptest::option::of(crate_name()),
                pkg_name in crate_name(),
            ) {
                let bins = vec![bin(&name)];
                let result = CargoProject::resolve_default_binary(
                    &bins,
                    default_run.as_deref(),
                    &pkg_name,
                );
                // Single binary: either default_run matches it, or fallback picks it
                prop_assert!(result.is_ok());
            }

            #[test]
            fn result_is_always_from_binaries(
                names in bin_names(5).prop_filter("non-empty", |v| !v.is_empty()),
                default_run in proptest::option::of(crate_name()),
                pkg_name in crate_name(),
            ) {
                let bins = bins_from_names(&names);
                let result = CargoProject::resolve_default_binary(
                    &bins,
                    default_run.as_deref(),
                    &pkg_name,
                );
                if let Ok(selected) = result {
                    prop_assert!(
                        names.contains(&selected),
                        "selected '{}' not in binaries {:?}",
                        selected,
                        names,
                    );
                }
            }
        }
    }
}
