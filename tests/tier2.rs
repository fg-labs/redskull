mod common;
use common::*;
use redskull_lib::conda;
use redskull_lib::crate_inspector::{CargoMetadata, detect_license_files};
use redskull_lib::recipe_builder::RecipeBuilder;
use redskull_lib::renderer::{MetaYamlRenderer, Renderer};
use redskull_lib::runtime_deps;
use redskull_lib::source::{self, GitHubRepo};

#[test]
fn test_tier2_recipe_structure() {
    let mut builder = RecipeBuilder::new("fgumi", "1.0.0");
    builder
        .github_source("fulcrumgenomics", "fgumi", "abc123")
        .license("MIT")
        .add_binary("fgumi")
        .bioconda(true)
        .cargo_bundle_licenses(true)
        .add_maintainer("nh13")
        .add_maintainer("tfenne");

    let (recipe, script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);

    // CBL in build deps
    assert_contains(&output, "cargo-bundle-licenses", "should have CBL");
    // THIRDPARTY.yml in license_file list
    assert_contains(&output, "    - THIRDPARTY.yml", "THIRDPARTY.yml as list item");
    assert_contains(&output, "    - LICENSE", "LICENSE as list item");
    // Platforms
    assert_contains(&output, "linux-aarch64", "linux-aarch64 platform");
    assert_contains(&output, "osx-arm64", "osx-arm64 platform");
    // run_exports with x.x
    assert_contains(&output, "max_pin=\"x.x\"", "max_pin x.x");
    // build.sh needed (has CBL)
    assert!(script.needs_build_sh());
    let build_sh = script.to_build_sh();
    assert_contains(&build_sh, "cargo-bundle-licenses", "CBL in build.sh");
    // No compiler('c') for pure Rust with CBL
    assert_not_contains(&output, "compiler('c')", "no C compiler for pure Rust");
}

/// Smoke test: build recipes for a sample of Tier 2 crate names.
/// Network-free — just verifies builder doesn't panic with synthetic data.
#[test]
fn test_tier2_batch_builder_smoke() {
    let crates = [
        "rasusa",
        "fgumi",
        "fg-stitch",
        "fqtk",
        "fasten",
        "bed2gtf",
        "bigtools",
        "galah",
        "ontime",
        "simpleaf",
    ];
    for name in crates {
        let mut builder = RecipeBuilder::new(name, "1.0.0");
        builder
            .github_source("test", name, "0000")
            .license("MIT")
            .add_binary(name)
            .bioconda(true)
            .cargo_bundle_licenses(true);
        let (recipe, _script) = builder.build();
        let output = MetaYamlRenderer.render(&recipe);
        assert!(!output.is_empty(), "empty output for {name}");
    }
}

// --- Integration tests requiring network access ---
// Run with: cargo test -- --ignored

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::ClientBuilder::new().user_agent("redskull-test/0.0.1").build().unwrap()
}

#[test]
#[ignore]
fn test_resolve_github_source_v_prefix() {
    let client = http_client();
    let repo = GitHubRepo::from_url("https://github.com/fulcrumgenomics/fqtk").unwrap();
    let resolved = source::resolve_github_source(&client, &repo, "0.3.1", None, false).unwrap();
    assert!(resolved.tag.starts_with('v'), "fqtk should use v-prefixed tags");
    assert!(resolved.url_template.contains("/archive/v{{ version }}"));
    assert!(!resolved.sha256.is_empty());
}

#[test]
#[ignore]
fn test_resolve_github_source_bare_version() {
    let client = http_client();
    let repo = GitHubRepo::from_url("https://github.com/esteinig/nanoq").unwrap();
    let resolved = source::resolve_github_source(&client, &repo, "0.10.0", None, false).unwrap();
    assert!(!resolved.tag.starts_with('v'), "nanoq should use bare version tags");
    assert!(resolved.url_template.contains("/archive/{{ version }}"));
    assert!(!resolved.url_template.contains("/archive/v{{ version }}"));
}

#[test]
#[ignore]
fn test_fetch_github_raw_cargo_toml() {
    let client = http_client();
    let repo = GitHubRepo::from_url("https://github.com/fulcrumgenomics/fqtk").unwrap();
    let content = source::fetch_github_raw(&client, &repo, "v0.3.1", "Cargo.toml").unwrap();
    let meta = CargoMetadata::from_toml_str(&content).unwrap();
    assert_eq!(meta.package_name(), Some("fqtk".to_string()));
    assert_eq!(meta.binary_names(), vec!["fqtk"]);
}

#[test]
#[ignore]
fn test_fetch_github_tree_license_detection() {
    let client = http_client();
    let repo = GitHubRepo::from_url("https://github.com/fulcrumgenomics/fqtk").unwrap();
    let files = source::fetch_github_tree(&client, &repo, "v0.3.1").unwrap();
    let license_files = detect_license_files(&files);
    assert!(license_files.contains(&"LICENSE".to_string()));
}

#[test]
#[ignore]
fn test_fetch_github_tree_runtime_deps_fgumi() {
    let client = http_client();
    let repo = GitHubRepo::from_url("https://github.com/fulcrumgenomics/fgumi").unwrap();
    let files = source::fetch_github_tree(&client, &repo, "v0.1.2").unwrap();
    let hints = runtime_deps::detect_runtime_hints(&files);
    assert!(!hints.is_empty(), "fgumi should detect R runtime deps");
    assert_eq!(hints[0].package, "r-base");
}

#[test]
#[ignore]
fn test_fetch_github_tree_dual_license() {
    let client = http_client();
    let repo = GitHubRepo::from_url("https://github.com/stjude-rust-labs/sprocket").unwrap();
    let files = source::fetch_github_tree(&client, &repo, "v0.22.0").unwrap();
    let license_files = detect_license_files(&files);
    assert!(license_files.contains(&"LICENSE-MIT".to_string()));
    assert!(license_files.contains(&"LICENSE-APACHE".to_string()));
}

#[test]
#[ignore]
fn test_latest_github_release() {
    let client = http_client();
    let repo = GitHubRepo::from_url("https://github.com/fulcrumgenomics/fqtk").unwrap();
    let tag = source::latest_github_release(&client, &repo).unwrap();
    assert!(!tag.is_empty(), "should find a release tag");
}

#[test]
#[ignore]
fn test_conda_pkg_available_real_package() {
    let client = http_client();
    assert!(
        conda::is_pkg_available(&client, "zlib", "conda-forge").unwrap(),
        "zlib should be available on conda-forge"
    );
}

#[test]
#[ignore]
fn test_conda_pkg_not_available_fake_package() {
    let client = http_client();
    assert!(
        !conda::is_pkg_available(&client, "totally-fake-nonexistent-pkg-12345", "conda-forge")
            .unwrap(),
        "fake package should not be available"
    );
}

#[test]
#[ignore]
fn test_conda_normalize_pkg_name_snake_case() {
    let client = http_client();
    // rust-htslib maps to rust-htslib on conda-forge (kebab form)
    let normalized = conda::normalize_pkg_name(&client, "rust-htslib", "bioconda");
    assert_eq!(normalized, "rust-htslib");
}

#[test]
#[ignore]
fn test_conda_check_dependency_found() {
    let client = http_client();
    let result = conda::check_dependency(&client, "openssl", "conda-forge").unwrap();
    assert!(result.is_some(), "openssl should be found on conda-forge");
    assert_eq!(result.unwrap(), "openssl");
}

#[test]
#[ignore]
fn test_conda_check_dependency_not_found() {
    let client = http_client();
    let result =
        conda::check_dependency(&client, "totally-fake-pkg-xyz-999", "conda-forge").unwrap();
    assert!(result.is_none(), "fake package should not be found");
}

#[test]
#[ignore]
fn test_github_only_mode_cargo_metadata() {
    let client = http_client();
    let repo = GitHubRepo::from_url("https://github.com/fulcrumgenomics/fqtk").unwrap();
    let toml = source::fetch_github_raw(&client, &repo, "v0.3.1", "Cargo.toml").unwrap();
    let meta = CargoMetadata::from_toml_str(&toml).unwrap();

    assert_eq!(meta.package_name(), Some("fqtk".to_string()));
    assert_eq!(meta.version(None), Some("0.3.1".to_string()));
    assert!(meta.license(None).is_some(), "should have a license");
    assert!(meta.description(None).is_some(), "should have a description");

    let deps = meta.dependencies();
    assert!(!deps.is_empty(), "should have dependencies");
}
