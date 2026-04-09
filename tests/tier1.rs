mod common;
use common::*;
use redskull_lib::build_script::BuildScript;
use redskull_lib::crate_inspector::{
    CargoMetadata, parse_cargo_lock_str, resolve_workspace_members,
};
use redskull_lib::recipe::*;
use redskull_lib::recipe_builder::RecipeBuilder;
use redskull_lib::renderer::{MetaYamlRenderer, Renderer};
use redskull_lib::runtime_deps::detect_runtime_hints;
use redskull_lib::source::{self, GitHubRepo, github_archive_url};
use redskull_lib::sys_deps;

#[test]
fn test_renderer_preamble() {
    let recipe = minimal_recipe("ska2", "0.3.12");
    let output = render(&recipe);
    assert_contains(&output, "{% set name = \"ska2\" %}", "preamble name");
    assert_contains(&output, "{% set version = \"0.3.12\" %}", "preamble version");
}

#[test]
fn test_renderer_package_uses_name_lower() {
    let recipe = minimal_recipe("MyPackage", "1.0.0");
    let output = render(&recipe);
    assert_contains(&output, "{{ name }}", "package name should use name variable");
}

#[test]
fn test_renderer_max_pin_default_xx() {
    let mut recipe = minimal_recipe("test", "1.0.0");
    recipe.build.with_run_exports = true;
    recipe.build.max_pin = "x.x".to_string();
    let output = render(&recipe);
    assert_contains(&output, "max_pin=\"x.x\"", "default max_pin should be x.x");
}

#[test]
fn test_renderer_license_file_single() {
    let mut recipe = minimal_recipe("test", "1.0.0");
    recipe.about.license_file = vec!["LICENSE".to_string()];
    let output = render(&recipe);
    assert_contains(&output, "  license_file: LICENSE", "single license file");
    // Should NOT be in list format
    assert!(!output.contains("    - LICENSE"), "single file should not be a YAML list item");
}

#[test]
fn test_renderer_license_file_multiple() {
    let mut recipe = minimal_recipe("test", "1.0.0");
    recipe.about.license_file = vec!["LICENSE".to_string(), "THIRDPARTY.yml".to_string()];
    let output = render(&recipe);
    assert_contains(&output, "  license_file:", "license_file header");
    assert_contains(&output, "    - LICENSE", "LICENSE in list");
    assert_contains(&output, "    - THIRDPARTY.yml", "THIRDPARTY.yml in list");
}

#[test]
fn test_renderer_requirement_with_selector() {
    let mut recipe = minimal_recipe("test", "1.0.0");
    recipe.requirements.host = vec![Requirement {
        name: "openssl".to_string(),
        version: None,
        selector: Some("not osx".to_string()),
    }];
    let output = render(&recipe);
    assert_contains(&output, "- openssl  # [not osx]", "openssl with osx selector");
}

#[test]
fn test_renderer_inline_script_single_line() {
    let mut recipe = minimal_recipe("test", "1.0.0");
    recipe.build.script = Some(
        "cargo install --no-track --locked --verbose --root \"${PREFIX}\" --path .".to_string(),
    );
    let output = render(&recipe);
    assert_contains(&output, "  script: cargo install", "inline single-line script");
}

#[test]
fn test_renderer_inline_script_multiline() {
    let mut recipe = minimal_recipe("test", "1.0.0");
    recipe.build.script = Some(
        "cargo-bundle-licenses --format yaml --output THIRDPARTY.yml\ncargo install --no-track"
            .to_string(),
    );
    let output = render(&recipe);
    assert_contains(&output, "  script: |", "multiline script uses | block");
}

#[test]
fn test_renderer_no_empty_sections() {
    let recipe = minimal_recipe("test", "1.0.0");
    let output = render(&recipe);
    // Empty test section should not appear
    assert!(!output.contains("test:"), "empty test section should be omitted");
    // Empty extra section should not appear
    assert!(!output.contains("extra:"), "empty extra section should be omitted");
}

#[test]
fn test_github_url_from_repository() {
    let repo = GitHubRepo::from_url("https://github.com/bacpop/ska.rust").unwrap();
    assert_eq!(repo.owner, "bacpop");
    assert_eq!(repo.name, "ska.rust");
    let url = github_archive_url(&repo, "0.3.12", "v");
    assert_eq!(url, "https://github.com/bacpop/ska.rust/archive/v0.3.12.tar.gz");
    let url_bare = github_archive_url(&repo, "0.3.12", "");
    assert_eq!(url_bare, "https://github.com/bacpop/ska.rust/archive/0.3.12.tar.gz");
}

#[test]
fn test_github_url_strips_trailing_slash() {
    let repo = GitHubRepo::from_url("https://github.com/fulcrumgenomics/fgumi/").unwrap();
    assert_eq!(repo.owner, "fulcrumgenomics");
    assert_eq!(repo.name, "fgumi");
}

#[test]
fn test_github_url_strips_dotgit() {
    let repo = GitHubRepo::from_url("https://github.com/owner/repo.git").unwrap();
    assert_eq!(repo.name, "repo");
}

#[test]
fn test_parse_cargo_toml_binaries() {
    let toml_str = r#"
[package]
name = "ska2"

[[bin]]
name = "ska"
path = "src/main.rs"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert_eq!(meta.binary_names(), vec!["ska"]);
}

#[test]
fn test_parse_cargo_toml_no_explicit_bin() {
    let toml_str = r#"
[package]
name = "rasusa"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert_eq!(meta.binary_names(), vec!["rasusa"]);
}

#[test]
fn test_parse_cargo_toml_workspace_members() {
    let toml_str = r#"
[workspace]
members = ["fg-stitch-lib", "fg-stitch-cli"]
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert_eq!(meta.workspace_members(), vec!["fg-stitch-lib", "fg-stitch-cli"]);
    assert!(meta.is_workspace());
    assert!(!meta.has_package());
}

#[test]
fn test_workspace_with_root_package() {
    let toml_str = r#"
[workspace]
members = [".", "crates/mylib"]

[package]
name = "mytool"
version = "1.0.0"

[[bin]]
name = "mytool"
path = "src/main.rs"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert!(meta.is_workspace());
    assert!(meta.has_package());
    assert_eq!(meta.binary_names(), vec!["mytool"]);
    assert_eq!(meta.package_name(), Some("mytool".to_string()));
}

#[test]
fn test_non_workspace_crate() {
    let toml_str = r#"
[package]
name = "simple"
version = "0.1.0"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert!(!meta.is_workspace());
    assert!(meta.has_package());
}

#[test]
fn test_detect_license_file() {
    use redskull_lib::crate_inspector::detect_license_files;

    let files = vec!["Cargo.toml".to_string(), "LICENSE".to_string(), "src/main.rs".to_string()];
    assert_eq!(detect_license_files(&files), vec!["LICENSE"]);
}

#[test]
fn test_detect_license_file_md() {
    use redskull_lib::crate_inspector::detect_license_files;

    let files = vec!["LICENSE.md".to_string(), "README.md".to_string()];
    assert_eq!(detect_license_files(&files), vec!["LICENSE.md"]);
}

#[test]
fn test_detect_dual_license_files() {
    use redskull_lib::crate_inspector::detect_license_files;

    let files = vec!["LICENSE-MIT".to_string(), "LICENSE-APACHE".to_string()];
    assert_eq!(detect_license_files(&files), vec!["LICENSE-APACHE", "LICENSE-MIT"]);
}

#[test]
fn test_detect_license_files_ignores_nested() {
    use redskull_lib::crate_inspector::detect_license_files;

    let files = vec![
        "LICENSE".to_string(),
        "crates/foo/LICENSE".to_string(),
        "vendor/bar/COPYING".to_string(),
    ];
    assert_eq!(detect_license_files(&files), vec!["LICENSE"]);
}

#[test]
fn test_simple_inline_script() {
    let script = BuildScript::new().locked(true);
    // Simple recipe: should produce a single-line inline script
    assert!(!script.needs_build_sh());
    let inline = script.inline_script();
    assert!(inline.contains("cargo install"));
    assert!(inline.contains("--locked"));
    assert!(inline.contains("--no-track"));
    assert!(!inline.contains("cargo-bundle-licenses"));
}

#[test]
fn test_cbl_build_script() {
    let script = BuildScript::new().locked(true).cargo_bundle_licenses(true);
    // CBL requires multi-line script
    let output = script.to_build_sh();
    assert!(output.contains("cargo-bundle-licenses --format yaml --output THIRDPARTY.yml"));
    assert!(output.contains("cargo install"));
}

#[test]
fn test_native_deps_build_script() {
    let script = BuildScript::new()
        .locked(true)
        .cargo_bundle_licenses(true)
        .needs_bindgen(true)
        .has_native_deps(true);
    assert!(script.needs_build_sh());
    let output = script.to_build_sh();
    assert!(output.contains("BINDGEN_EXTRA_CLANG_ARGS"));
    assert!(output.contains("CPPFLAGS"));
    assert!(output.contains("LDFLAGS"));
}

#[test]
fn test_workspace_path() {
    let script = BuildScript::new().locked(true).workspace_path("fg-stitch-cli");
    let output = script.to_build_sh();
    assert!(output.contains("--path fg-stitch-cli"));
}

#[test]
fn test_pure_rust_requirements() {
    let no_tools = BuildToolNeeds { pkg_config: false, make: false, cmake: false };
    let reqs = Requirements::for_rust_crate(false, false, false, false, &no_tools);
    let names: Vec<&str> = reqs.build.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"{{ compiler('rust') }}"));
    assert!(!names.contains(&"{{ compiler('c') }}"), "pure Rust should not include compiler('c')");
    assert!(!names.contains(&"{{ compiler('cxx') }}"));
    assert!(!names.contains(&"{{ stdlib('c') }}"), "pure Rust should not include stdlib('c')");
    assert!(!names.contains(&"cargo-bundle-licenses"));
    assert!(!names.contains(&"clangdev"));
    assert!(!names.contains(&"pkg-config"));
    assert!(!names.contains(&"make"));
    assert!(!names.contains(&"cmake"));
}

#[test]
fn test_requirements_with_c_deps() {
    let no_tools = BuildToolNeeds { pkg_config: false, make: false, cmake: false };
    let reqs = Requirements::for_rust_crate(false, true, false, false, &no_tools);
    let names: Vec<&str> = reqs.build.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"{{ compiler('c') }}"));
    assert!(names.contains(&"{{ stdlib('c') }}"), "C deps should pull in stdlib('c')");
    assert!(names.contains(&"{{ compiler('rust') }}"));
    assert!(!names.contains(&"{{ compiler('cxx') }}"), "cxx only when explicitly needed");
}

#[test]
fn test_requirements_with_cxx_deps() {
    let no_tools = BuildToolNeeds { pkg_config: false, make: false, cmake: false };
    let reqs = Requirements::for_rust_crate(false, true, true, false, &no_tools);
    let names: Vec<&str> = reqs.build.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"{{ compiler('c') }}"));
    assert!(names.contains(&"{{ compiler('cxx') }}"));
    assert!(names.contains(&"{{ stdlib('c') }}"), "C++ deps should pull in stdlib('c')");
    assert!(names.contains(&"{{ compiler('rust') }}"));
}

/// Exercise the C++-only branch (no C deps). The stdlib('c') requirement must
/// still appear — bioconda's `compiler_needs_stdlib_c` lint requires it whenever
/// any C/C++ compiler is present — and `compiler('c')` must NOT be added when
/// only C++ is requested.
#[test]
fn test_requirements_with_cxx_only_deps() {
    let no_tools = BuildToolNeeds { pkg_config: false, make: false, cmake: false };
    let reqs = Requirements::for_rust_crate(false, false, true, false, &no_tools);
    let names: Vec<&str> = reqs.build.iter().map(|r| r.name.as_str()).collect();
    assert!(!names.contains(&"{{ compiler('c') }}"), "cxx-only must not add compiler('c')");
    assert!(names.contains(&"{{ compiler('cxx') }}"));
    assert!(names.contains(&"{{ stdlib('c') }}"), "C++ still needs stdlib('c') for bioconda lint");
    assert!(names.contains(&"{{ compiler('rust') }}"));
}

#[test]
fn test_requirements_with_bindgen_adds_clangdev() {
    let no_tools = BuildToolNeeds { pkg_config: false, make: false, cmake: false };
    let reqs = Requirements::for_rust_crate(false, false, false, true, &no_tools);
    let names: Vec<&str> = reqs.build.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"clangdev"));
}

#[test]
fn test_requirements_with_native_deps_adds_build_tools() {
    let tools = BuildToolNeeds { pkg_config: true, make: true, cmake: true };
    let reqs = Requirements::for_rust_crate(true, true, false, false, &tools);
    let names: Vec<&str> = reqs.build.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"cargo-bundle-licenses"));
    assert!(names.contains(&"pkg-config"));
    assert!(names.contains(&"make"));
    assert!(names.contains(&"cmake"));
}

#[test]
fn test_requirements_pkg_config_only() {
    let tools = BuildToolNeeds { pkg_config: true, make: false, cmake: false };
    let reqs = Requirements::for_rust_crate(false, false, false, false, &tools);
    let names: Vec<&str> = reqs.build.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"pkg-config"));
    assert!(!names.contains(&"make"));
    assert!(!names.contains(&"cmake"));
}

#[test]
fn test_from_binaries() {
    let test = Test::from_binaries(&["ska"]);
    assert_eq!(test.commands, vec!["ska --help"]);
}

#[test]
fn test_from_multiple_binaries() {
    let test = Test::from_binaries(&["tool1", "tool2"]);
    assert_eq!(test.commands, vec!["tool1 --help", "tool2 --help"]);
}

#[test]
fn test_builder_minimal_pure_rust() {
    let mut builder = RecipeBuilder::new("ska2", "0.3.12");
    builder
        .github_source("bacpop", "ska.rust", "deadbeef1234")
        .license("Apache-2.0")
        .summary("Split k-mer analysis")
        .add_binary("ska")
        .add_maintainer("johndoe");

    let (recipe, script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);

    assert!(output.contains("ska2"));
    assert!(output.contains("0.3.12"));
    assert!(output.contains("github.com/bacpop/ska.rust"));
    assert!(output.contains("Apache-2.0"));
    assert!(output.contains("ska --help"));
    assert!(output.contains("johndoe"));
    // Pure Rust: no compiler('c')
    assert!(!output.contains("compiler('c')"));
    // Non-bioconda: no run_exports
    assert!(!output.contains("run_exports"));
    // No CBL, so license_file should be just LICENSE
    assert!(output.contains("license_file: LICENSE"));
    // Simple recipe: should have inline script
    assert!(recipe.build.script.is_some());
    assert!(!script.needs_build_sh());
}

#[test]
fn test_builder_crates_io_source() {
    let mut builder = RecipeBuilder::new("fqtk", "0.3.1");
    builder
        .crates_io_source("/api/v1/crates/fqtk/0.3.1/download", "abc123sha")
        .license("MIT")
        .add_binary("fqtk")
        .bioconda(true)
        .cargo_bundle_licenses(true);

    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);

    assert!(output.contains("crates.io"), "should use crates.io URL");
    assert!(output.contains("abc123sha"), "should use provided SHA");
    assert!(!output.contains("github.com"), "should not reference GitHub");
    assert!(output.contains("fqtk --help"), "binary test command");
}

#[test]
fn test_builder_bioconda_with_cbl() {
    let mut builder = RecipeBuilder::new("rasusa", "2.1.0");
    builder
        .github_source("mbhall88", "rasusa", "abcdef5678")
        .license("MIT")
        .add_binary("rasusa")
        .bioconda(true)
        .cargo_bundle_licenses(true);

    let (recipe, script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);

    assert!(output.contains("cargo-bundle-licenses"));
    assert!(output.contains("THIRDPARTY.yml"));
    assert!(output.contains("linux-aarch64"));
    assert!(output.contains("osx-arm64"));
    assert!(output.contains("run_exports"));
    // CBL needs build.sh
    assert!(script.needs_build_sh());
}

#[test]
fn test_detect_r_scripts() {
    let files = vec![
        "src/main.rs".to_string(),
        "scripts/plot.R".to_string(),
        "scripts/analyze.Rmd".to_string(),
    ];
    let hints = detect_runtime_hints(&files);
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].package, "r-base");
}

#[test]
fn test_detect_python_scripts() {
    let files = vec![
        "src/main.rs".to_string(),
        "scripts/helper.py".to_string(),
        "requirements.txt".to_string(),
    ];
    let hints = detect_runtime_hints(&files);
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].package, "python");
}

#[test]
fn test_detect_no_runtime_deps() {
    let files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string(), "Cargo.toml".to_string()];
    let hints = detect_runtime_hints(&files);
    assert!(hints.is_empty());
}

#[test]
fn test_detect_both_r_and_python() {
    let files = vec!["scripts/plot.R".to_string(), "scripts/preprocess.py".to_string()];
    let hints = detect_runtime_hints(&files);
    assert_eq!(hints.len(), 2);
    let packages: Vec<&str> = hints.iter().map(|h| h.package).collect();
    assert!(packages.contains(&"r-base"));
    assert!(packages.contains(&"python"));
}

#[test]
fn test_detect_ignores_setup_py() {
    let files = vec!["setup.py".to_string(), "conf.py".to_string()];
    let hints = detect_runtime_hints(&files);
    assert!(hints.is_empty());
}

// --- SHA256 validation tests ---

#[test]
fn test_valid_sha256() {
    let hash = "a".repeat(64);
    assert!(source::is_valid_sha256(&hash));
}

#[test]
fn test_invalid_sha256_too_short() {
    assert!(!source::is_valid_sha256("abc123"));
}

#[test]
fn test_invalid_sha256_empty() {
    assert!(!source::is_valid_sha256(""));
}

#[test]
fn test_invalid_sha256_non_hex() {
    let hash = "z".repeat(64);
    assert!(!source::is_valid_sha256(&hash));
}

// --- Test command override tests ---

#[test]
fn test_builder_test_command_overrides() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .add_test_command("mytool --version")
        .add_test_command("mytool subcommand --help");

    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);

    // Should use overrides, not the default --help
    assert_contains(&output, "mytool --version", "custom test command");
    assert_contains(&output, "mytool subcommand --help", "custom test command 2");
    assert!(!output.contains("mytool --help"), "should not have default --help");
}

#[test]
fn test_builder_no_test_command_overrides_uses_defaults() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder.github_source("test", "mytool", "abc123").license("MIT").add_binary("mytool");

    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);

    assert_contains(&output, "mytool --help", "default --help test command");
}

// --- Workspace glob resolution tests ---

#[test]
fn test_resolve_workspace_members_literal() {
    let members = vec!["crate-a".to_string(), "crate-b".to_string()];
    let tree = vec![];
    let resolved = resolve_workspace_members(&members, &tree);
    assert_eq!(resolved, vec!["crate-a", "crate-b"]);
}

#[test]
fn test_resolve_workspace_members_glob() {
    let members = vec!["crates/*".to_string()];
    let tree = vec![
        "crates/foo/Cargo.toml".to_string(),
        "crates/foo/src/main.rs".to_string(),
        "crates/bar/Cargo.toml".to_string(),
        "Cargo.toml".to_string(),
    ];
    let resolved = resolve_workspace_members(&members, &tree);
    assert_eq!(resolved, vec!["crates/foo", "crates/bar"]);
}

#[test]
fn test_resolve_workspace_members_mixed() {
    let members = vec!["cli".to_string(), "libs/*".to_string()];
    let tree = vec![
        "cli/Cargo.toml".to_string(),
        "libs/core/Cargo.toml".to_string(),
        "libs/utils/Cargo.toml".to_string(),
    ];
    let resolved = resolve_workspace_members(&members, &tree);
    assert_eq!(resolved, vec!["cli", "libs/core", "libs/utils"]);
}

#[test]
fn test_resolve_workspace_members_no_match() {
    let members = vec!["crates/*".to_string()];
    let tree = vec!["src/main.rs".to_string()];
    let resolved = resolve_workspace_members(&members, &tree);
    assert!(resolved.is_empty());
}

// --- CargoMetadata accessor tests ---

#[test]
fn test_cargo_metadata_version() {
    let toml_str = r#"
[package]
name = "mytool"
version = "1.2.3"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert_eq!(meta.version(None), Some("1.2.3".to_string()));
}

#[test]
fn test_cargo_metadata_license() {
    let toml_str = r#"
[package]
name = "mytool"
version = "1.0.0"
license = "MIT OR Apache-2.0"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert_eq!(meta.license(None), Some("MIT OR Apache-2.0".to_string()));
}

#[test]
fn test_cargo_metadata_description() {
    let toml_str = r#"
[package]
name = "mytool"
version = "1.0.0"
description = "A great tool"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert_eq!(meta.description(None), Some("A great tool".to_string()));
}

#[test]
fn test_cargo_metadata_workspace_inheritance() {
    let workspace_toml = r#"
[workspace]
members = ["crates/*"]

[workspace.package]
version = "2.0.0"
license = "GPL-3.0"
description = "Workspace description"
"#;
    let crate_toml = r#"
[package]
name = "member-crate"
version.workspace = true
license.workspace = true
description.workspace = true
"#;
    let ws_meta = CargoMetadata::from_toml_str(workspace_toml).unwrap();
    let crate_meta = CargoMetadata::from_toml_str(crate_toml).unwrap();

    assert_eq!(crate_meta.version(Some(&ws_meta)), Some("2.0.0".to_string()));
    assert_eq!(crate_meta.license(Some(&ws_meta)), Some("GPL-3.0".to_string()));
    assert_eq!(crate_meta.description(Some(&ws_meta)), Some("Workspace description".to_string()));
}

#[test]
fn test_cargo_metadata_workspace_inheritance_without_workspace() {
    let crate_toml = r#"
[package]
name = "member-crate"
version.workspace = true
"#;
    let crate_meta = CargoMetadata::from_toml_str(crate_toml).unwrap();
    // Without workspace meta, should return None
    assert_eq!(crate_meta.version(None), None);
}

#[test]
fn test_cargo_metadata_dependencies() {
    let toml_str = r#"
[package]
name = "mytool"
version = "1.0.0"

[dependencies]
serde = "1.0"
clap = { version = "4.0", features = ["derive"] }
tokio = { version = "1", optional = true }

[build-dependencies]
cc = "1.0"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    let deps = meta.dependencies();
    assert_eq!(deps.len(), 3);

    // Check names and optional flags
    let serde = deps.iter().find(|(n, _)| n == "serde").unwrap();
    assert!(!serde.1, "serde should not be optional");

    let tokio = deps.iter().find(|(n, _)| n == "tokio").unwrap();
    assert!(tokio.1, "tokio should be optional");

    let build_deps = meta.build_dependencies();
    assert_eq!(build_deps, vec!["cc"]);
}

#[test]
fn test_cargo_metadata_no_dependencies() {
    let toml_str = r#"
[package]
name = "mytool"
version = "1.0.0"
"#;
    let meta = CargoMetadata::from_toml_str(toml_str).unwrap();
    assert!(meta.dependencies().is_empty());
    assert!(meta.build_dependencies().is_empty());
}

// --- Gap 2: bioconda always generates build.sh ---

#[test]
fn test_bioconda_always_needs_build_sh() {
    let script = BuildScript::new().force_build_sh(true);
    assert!(script.needs_build_sh());
}

#[test]
fn test_bioconda_builder_forces_build_sh() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .bioconda(true);
    // No CBL, no bindgen, no native deps — but bioconda should force build.sh
    let (recipe, script) = builder.build();
    assert!(script.needs_build_sh(), "bioconda should force build.sh");
    assert!(recipe.build.script.is_none(), "bioconda should not have inline script");
}

#[test]
fn test_non_bioconda_simple_uses_inline() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder.github_source("test", "mytool", "abc123").license("MIT").add_binary("mytool");
    let (recipe, script) = builder.build();
    assert!(!script.needs_build_sh());
    assert!(recipe.build.script.is_some(), "non-bioconda simple should use inline");
}

// --- Gap 3: RUST_BACKTRACE in build.sh ---

#[test]
fn test_build_sh_contains_rust_backtrace() {
    let script = BuildScript::new().force_build_sh(true);
    let output = script.to_build_sh();
    assert_contains(&output, "RUST_BACKTRACE=1", "should set RUST_BACKTRACE");
}

// --- Gap 5: C++ compiler detection ---

#[test]
fn test_needs_cxx_compiler_with_cxx_crate() {
    assert!(sys_deps::needs_cxx_compiler(&["cxx", "serde"]));
    assert!(sys_deps::needs_cxx_compiler(&["cxx-build", "clap"]));
    assert!(sys_deps::needs_cxx_compiler(&["rocksdb", "serde"]));
}

#[test]
fn test_needs_cxx_compiler_without() {
    assert!(!sys_deps::needs_cxx_compiler(&["serde", "clap", "tokio"]));
    assert!(!sys_deps::needs_cxx_compiler(&["cc", "openssl-sys"]));
}

#[test]
fn test_cxx_in_recipe_output() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .has_cxx_deps(true);
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "compiler('cxx')", "should have C++ compiler");
}

// --- Gap 6: clangdev for bindgen ---

#[test]
fn test_bindgen_adds_clangdev_in_recipe() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .needs_bindgen(true);
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "clangdev", "should have clangdev for bindgen");
}

// --- Gap 7: CARGO_NET_GIT_FETCH_WITH_CLI ---

#[test]
fn test_build_sh_cargo_net_git_fetch() {
    let script = BuildScript::new().force_build_sh(true).cargo_net_git_fetch(true);
    let output = script.to_build_sh();
    assert_contains(&output, "CARGO_NET_GIT_FETCH_WITH_CLI=true", "should set git fetch env");
}

#[test]
fn test_build_sh_no_cargo_net_git_fetch_by_default() {
    let script = BuildScript::new().force_build_sh(true);
    let output = script.to_build_sh();
    assert!(!output.contains("CARGO_NET_GIT_FETCH_WITH_CLI"));
}

// --- Gap 8: individual build tool detection ---

#[test]
fn test_needs_pkg_config() {
    assert!(sys_deps::needs_pkg_config(&["openssl-sys", "serde"]));
    assert!(sys_deps::needs_pkg_config(&["libz-sys"]));
    assert!(!sys_deps::needs_pkg_config(&["serde", "clap"]));
}

#[test]
fn test_needs_cmake() {
    assert!(sys_deps::needs_cmake(&["cmake", "serde"]));
    assert!(sys_deps::needs_cmake(&["rocksdb-sys"]));
    assert!(!sys_deps::needs_cmake(&["openssl-sys"]));
}

#[test]
fn test_needs_make() {
    assert!(sys_deps::needs_make(&["openssl-sys", "serde"]));
    assert!(!sys_deps::needs_make(&["serde", "clap"]));
}

// --- Gap 10: max_pin option ---

#[test]
fn test_max_pin_x() {
    let mut builder = RecipeBuilder::new("mytool", "0.5.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .bioconda(true)
        .max_pin("x");
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "max_pin=\"x\"", "should use max_pin x");
    assert!(!output.contains("max_pin=\"x.x\""));
}

// --- Gap 13: license_family optional ---

#[test]
fn test_bioconda_has_license_family_by_default() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .bioconda(true);
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "license_family: MIT", "bioconda should emit license_family");
}

#[test]
fn test_non_bioconda_has_license_family() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder.github_source("test", "mytool", "abc123").license("MIT").add_binary("mytool");
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "license_family: MIT", "non-bioconda should have license_family");
}

#[test]
fn test_bioconda_explicit_disable_license_family() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .bioconda(true)
        .emit_license_family(false);
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert!(!output.contains("license_family"), "explicit disable should suppress license_family");
}

// --- Gap 14: binary stripping ---

#[test]
fn test_build_sh_strip_binaries() {
    let script = BuildScript::new()
        .force_build_sh(true)
        .strip_binaries(true)
        .binaries(vec!["mytool".to_string()]);
    let output = script.to_build_sh();
    assert_contains(&output, "${STRIP}", "should strip binaries");
    assert_contains(&output, "mytool", "should reference binary name");
}

#[test]
fn test_build_sh_no_strip_by_default() {
    let script = BuildScript::new().force_build_sh(true);
    let output = script.to_build_sh();
    assert!(!output.contains("${STRIP}"));
}

// --- Gap 15: identifiers ---

#[test]
fn test_identifier_in_output() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .bioconda(true)
        .add_identifier("doi:10.1234/foo");
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "identifiers:", "should have identifiers section");
    assert_contains(&output, "doi:10.1234/foo", "should have the DOI");
}

#[test]
fn test_multiple_identifiers() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .bioconda(true)
        .add_identifier("doi:10.1234/foo")
        .add_identifier("biotools:mytool");
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "doi:10.1234/foo", "first identifier");
    assert_contains(&output, "biotools:mytool", "second identifier");
}

// --- Gap 16: --version test commands ---

#[test]
fn test_version_test_command() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .use_version_test(true);
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "mytool --version", "should use --version");
    assert!(!output.contains("mytool --help"), "should not use --help");
}

#[test]
fn test_version_test_does_not_override_explicit() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .use_version_test(true)
        .add_test_command("mytool subcommand --help");
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "mytool subcommand --help", "explicit override should win");
    assert!(!output.contains("mytool --version"), "should not have auto-generated --version");
}

// --- Gap 9: run deps ---

#[test]
fn test_run_dep_in_output() {
    let mut builder = RecipeBuilder::new("mytool", "1.0.0");
    builder
        .github_source("test", "mytool", "abc123")
        .license("MIT")
        .add_binary("mytool")
        .add_run_dep("samtools", None)
        .add_run_dep("minimap2", None);
    let (recipe, _script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);
    assert_contains(&output, "samtools", "should have samtools run dep");
    assert_contains(&output, "minimap2", "should have minimap2 run dep");
    assert_contains(&output, "  run:", "should have run section");
}

#[test]
fn test_tag_to_version() {
    assert_eq!(source::tag_to_version("v1.2.3"), "1.2.3");
    assert_eq!(source::tag_to_version("1.2.3"), "1.2.3");
    assert_eq!(source::tag_to_version("v0.10.0"), "0.10.0");
    // v. prefix (e.g., alejandrogzi tools like bed2gtf)
    assert_eq!(source::tag_to_version("v.1.9.4"), "1.9.4");
    assert_eq!(source::tag_to_version("v.0.1.0"), "0.1.0");
}

// --- looks_like_version_tag ---

#[test]
fn test_looks_like_version_tag() {
    // Valid version tags
    assert!(source::looks_like_version_tag("v1.2.3"));
    assert!(source::looks_like_version_tag("1.2.3"));
    assert!(source::looks_like_version_tag("v0.10.0"));
    assert!(source::looks_like_version_tag("v.1.9.4"));
    assert!(source::looks_like_version_tag("0.1.0-beta"));
    // Non-version tags
    assert!(!source::looks_like_version_tag("latest"));
    assert!(!source::looks_like_version_tag("nightly"));
    assert!(!source::looks_like_version_tag("stable"));
    assert!(!source::looks_like_version_tag("release"));
}

// --- Pre-release tag detection ---

#[test]
fn test_is_prerelease_tag() {
    assert!(source::is_prerelease_tag("v2.0.0-rc.3"));
    assert!(source::is_prerelease_tag("v0.15.0-beta.2"));
    assert!(source::is_prerelease_tag("v1.0.0-alpha.1"));
    assert!(source::is_prerelease_tag("1.0.0-dev.1"));
    assert!(source::is_prerelease_tag("v3.0.0-pre.1"));
    // Stable versions
    assert!(!source::is_prerelease_tag("v1.2.3"));
    assert!(!source::is_prerelease_tag("1.2.3"));
    assert!(!source::is_prerelease_tag("v0.10.0"));
    assert!(!source::is_prerelease_tag("v.1.9.4"));
}

// --- CXX compiler detection expanded ---

#[test]
fn test_mimalloc_needs_c_not_cxx() {
    // mimalloc is pure C — it should require a C compiler but not C++.
    assert!(sys_deps::needs_c_compiler(&["mimalloc", "serde"]));
    assert!(sys_deps::needs_c_compiler(&["libmimalloc-sys", "clap"]));
    assert!(!sys_deps::needs_cxx_compiler(&["mimalloc", "serde"]));
    assert!(!sys_deps::needs_cxx_compiler(&["libmimalloc-sys", "clap"]));
}

#[test]
fn test_needs_cxx_compiler_cmake_crate() {
    assert!(sys_deps::needs_cxx_compiler(&["cmake", "serde"]));
    assert!(sys_deps::needs_cxx_compiler(&["cmake-build", "clap"]));
}

#[test]
fn test_needs_cxx_compiler_htslib() {
    assert!(sys_deps::needs_cxx_compiler(&["rust-htslib", "serde"]));
    assert!(sys_deps::needs_cxx_compiler(&["hts-sys", "clap"]));
}

#[test]
fn test_libgit2_does_not_require_cxx_or_cmake() {
    // libgit2 is pure C; libgit2-sys uses pkg-config to find the system lib.
    assert!(!sys_deps::needs_cxx_compiler(&["libgit2-sys", "serde"]));
    assert!(!sys_deps::needs_cmake(&["libgit2-sys", "serde"]));
    assert!(sys_deps::needs_pkg_config(&["libgit2-sys"]));
}

// --- Workspace binary name fallback ---

#[test]
fn test_workspace_member_binary_name_fallback() {
    // Simulate: workspace member has package name "sage-core-cli" but binary name "sage"
    let cli_toml = r#"
[package]
name = "sage-core-cli"
version = "1.0.0"

[[bin]]
name = "sage"
path = "src/main.rs"
"#;
    let meta = CargoMetadata::from_toml_str(cli_toml).unwrap();
    // Package name doesn't match "sage"
    assert_ne!(meta.package_name().as_deref(), Some("sage"));
    // But binary name does
    let bins = meta.binary_names();
    assert!(bins.contains(&"sage".to_string()));
}

/// A lockfile containing a transitive `openssl-sys` entry should surface openssl
/// through the sys-dep detection pipeline. This is the central guarantee of
/// transitive dep detection: the root crate has no direct `-sys` dep, but the
/// resolved graph does.
#[test]
fn test_parse_cargo_lock_picks_up_transitive_openssl() {
    let lockfile = r#"
# This file is automatically @generated by Cargo.
version = 3

[[package]]
name = "my-crate"
version = "0.1.0"
dependencies = [
 "reqwest",
]

[[package]]
name = "reqwest"
version = "0.12.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "hyper-tls",
]

[[package]]
name = "hyper-tls"
version = "0.6.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "native-tls",
]

[[package]]
name = "native-tls"
version = "0.2.11"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "openssl-sys",
]

[[package]]
name = "openssl-sys"
version = "0.9.106"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
    let names = parse_cargo_lock_str(lockfile).unwrap();
    assert!(names.contains(&"openssl-sys".to_string()), "expected openssl-sys in: {names:?}");
    assert!(names.contains(&"reqwest".to_string()));

    // Feeding the full list into sys-dep detection should surface openssl as a host dep
    // and demand a C compiler — neither of which would happen if we only inspected the
    // root crate's direct deps.
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let host = sys_deps::detect_host_deps(&refs);
    assert!(
        host.iter().any(|(pkg, _)| *pkg == "openssl"),
        "expected openssl in host deps: {host:?}"
    );
    assert!(sys_deps::needs_c_compiler(&refs));
    assert!(sys_deps::needs_pkg_config(&refs));
}

/// An empty Cargo.lock (no [[package]] entries) should return an empty vec, not error.
#[test]
fn test_parse_cargo_lock_empty() {
    let lockfile = "version = 3\n";
    let names = parse_cargo_lock_str(lockfile).unwrap();
    assert!(names.is_empty());
}

/// Malformed TOML should return an error, not panic.
#[test]
fn test_parse_cargo_lock_invalid_toml() {
    let lockfile = "this is not valid toml [[";
    assert!(parse_cargo_lock_str(lockfile).is_err());
}
