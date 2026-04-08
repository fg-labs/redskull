mod common;

use redskull_lib::recipe_builder::RecipeBuilder;
use redskull_lib::renderer::{MetaYamlRenderer, Renderer};
use redskull_lib::sys_deps::{detect_host_deps, map_sys_crate, needs_bindgen, needs_c_compiler};

use common::*;

#[test]
fn test_simple_sys_mappings() {
    assert_eq!(map_sys_crate("openssl-sys"), vec![("openssl", Some("not osx"))]);
    assert_eq!(map_sys_crate("libz-sys"), vec![("zlib", None)]);
    assert_eq!(map_sys_crate("bzip2-sys"), vec![("bzip2", None)]);
    assert_eq!(map_sys_crate("lzma-sys"), vec![("xz", None)]);
    assert_eq!(map_sys_crate("libcurl-sys"), vec![("libcurl", None)]);
    assert_eq!(map_sys_crate("libsqlite3-sys"), vec![("sqlite", None)]);
}

#[test]
fn test_bundle_mapping_rust_htslib() {
    let deps = map_sys_crate("rust-htslib");
    let names: Vec<&str> = deps.iter().map(|(n, _)| *n).collect();
    assert!(names.contains(&"htslib"));
    assert!(names.contains(&"zlib"));
    assert!(names.contains(&"bzip2"));
    assert!(names.contains(&"xz"));
    assert!(names.contains(&"libdeflate"));
    assert!(names.contains(&"libcurl"));
}

#[test]
fn test_unknown_crate() {
    assert!(map_sys_crate("serde").is_empty());
    assert!(map_sys_crate("clap").is_empty());
}

#[test]
fn test_detect_host_deps() {
    let deps = detect_host_deps(&["serde", "openssl-sys", "clap", "libz-sys"]);
    assert_eq!(deps.len(), 2);
    assert!(deps.iter().any(|(n, _)| *n == "openssl"));
    assert!(deps.iter().any(|(n, _)| *n == "zlib"));
}

#[test]
fn test_needs_c_compiler() {
    assert!(needs_c_compiler(&["libz-sys", "serde"]));
    assert!(needs_c_compiler(&["cc", "serde"]));
    assert!(!needs_c_compiler(&["serde", "clap", "tokio"]));
}

#[test]
fn test_needs_bindgen() {
    assert!(needs_bindgen(&["bindgen", "serde"]));
    assert!(!needs_bindgen(&["serde", "clap"]));
}

#[test]
fn test_openssl_has_osx_selector() {
    let deps = map_sys_crate("openssl-sys");
    let openssl = deps.iter().find(|(n, _)| *n == "openssl").unwrap();
    assert_eq!(openssl.1, Some("not osx"));
}

#[test]
fn test_tier3_recipe_with_openssl() {
    let mut builder = RecipeBuilder::new("echtvar", "1.0.0");
    builder
        .github_source("brentp", "echtvar", "abc123")
        .license("MIT")
        .add_binary("echtvar")
        .bioconda(true)
        .cargo_bundle_licenses(true)
        .has_c_deps(true)
        .has_native_deps(true)
        .needs_pkg_config(true)
        .needs_make(true)
        .add_host_dep("openssl", Some("not osx"));

    let (recipe, script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);

    assert_contains(&output, "compiler('c')", "should have C compiler for native deps");
    assert_contains(&output, "- openssl  # [not osx]", "openssl with platform selector");
    assert_contains(&output, "pkg-config", "pkg-config for native deps");
    assert_contains(&output, "make", "make for native deps");
    // Host deps should be mirrored to run deps
    assert!(
        output.matches("openssl").count() >= 2,
        "openssl should appear in both host and run sections"
    );
    assert!(script.needs_build_sh(), "native deps need build.sh");
    let build_sh = script.to_build_sh();
    assert_contains(&build_sh, "CPPFLAGS", "should set CPPFLAGS");
    assert_contains(&build_sh, "LDFLAGS", "should set LDFLAGS");
}

#[test]
fn test_tier3_recipe_with_htslib_bundle() {
    let mut builder = RecipeBuilder::new("bamslice", "1.0.0");
    builder
        .github_source("test", "bamslice", "abc123")
        .license("MIT")
        .add_binary("bamslice")
        .bioconda(true)
        .cargo_bundle_licenses(true)
        .has_c_deps(true)
        .has_native_deps(true)
        .needs_bindgen(true);

    // Simulate rust-htslib detected in deps
    let host_deps = redskull_lib::sys_deps::map_sys_crate("rust-htslib");
    for (dep, selector) in &host_deps {
        builder.add_host_dep(dep, *selector);
    }

    let (recipe, script) = builder.build();
    let output = MetaYamlRenderer.render(&recipe);

    assert_contains(&output, "htslib", "should have htslib");
    assert_contains(&output, "zlib", "should have zlib from bundle");
    assert_contains(&output, "bzip2", "should have bzip2 from bundle");
    let build_sh = script.to_build_sh();
    assert_contains(&build_sh, "BINDGEN_EXTRA_CLANG_ARGS", "bindgen env var");
    assert_contains(&build_sh, "CPPFLAGS", "compiler flags");
}
