use serde::Deserialize;
use std::path::Path;

/// Metadata extracted from the user's Cargo.toml
#[derive(Debug, Clone)]
pub struct ProjectMeta {
    pub name: String,
    pub version: String,
    pub binary_name: String,
}

#[derive(Deserialize)]
struct CargoToml {
    package: Option<PackageSection>,
    bin: Option<Vec<BinSection>>,
}

#[derive(Deserialize)]
struct PackageSection {
    name: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct BinSection {
    name: Option<String>,
}

impl ProjectMeta {
    /// Extract project metadata from a Cargo.toml file.
    pub fn from_cargo_toml(project_dir: &Path) -> crate::Result<Self> {
        let cargo_path = project_dir.join("Cargo.toml");
        let content =
            std::fs::read_to_string(&cargo_path).map_err(|e| crate::Error::CargoTomlRead {
                path: cargo_path.clone(),
                source: e,
            })?;

        let parsed: CargoToml =
            toml::from_str(&content).map_err(|e| crate::Error::CargoTomlParse {
                path: cargo_path.clone(),
                source: e,
            })?;

        let package = parsed
            .package
            .ok_or_else(|| crate::Error::MissingPackageSection(cargo_path.clone()))?;

        let name = package
            .name
            .ok_or_else(|| crate::Error::MissingPackageName(cargo_path.clone()))?;

        let version = package.version.unwrap_or_else(|| "0.1.0".to_owned());

        // Binary name: first [[bin]] entry, or package name
        let binary_name = parsed
            .bin
            .and_then(|bins| bins.into_iter().next())
            .and_then(|b| b.name)
            .unwrap_or_else(|| name.clone());

        Ok(Self {
            name,
            version,
            binary_name,
        })
    }
}
