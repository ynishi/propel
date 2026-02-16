//! Verify that the facade crate correctly re-exports sub-crate types
//! under each feature flag combination.

// ── Default features (core + build + cloud) ──

#[test]
fn core_types_available_at_root() {
    // PropelConfig and ProjectMeta should be flattened into propel::*
    let _config = propel::PropelConfig::default();
    let _meta = propel::ProjectMeta {
        name: "test".to_owned(),
        version: "0.1.0".to_owned(),
        binary_name: "test".to_owned(),
    };
}

#[test]
fn build_module_reexports_types() {
    // DockerfileGenerator and bundle functions should be in propel::build::*
    let config = propel::BuildConfig::default();
    let meta = propel::ProjectMeta {
        name: "test".to_owned(),
        version: "0.1.0".to_owned(),
        binary_name: "test".to_owned(),
    };
    let generator = propel::build::DockerfileGenerator::new(&config, &meta, 8080);
    let output = generator.render();
    assert!(output.contains("FROM"));
}

#[test]
fn cloud_module_reexports_types() {
    // GcloudClient should be accessible via propel::cloud::*
    let _client = propel::cloud::GcloudClient::new();
}
