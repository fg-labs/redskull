//! Shared test utilities for recipe validation.
//! All test data is generated programmatically — no committed fixture files.

use redskull_lib::recipe::*;
use redskull_lib::renderer::{MetaYamlRenderer, Renderer};

/// Helper to build a minimal valid recipe for testing.
#[allow(dead_code)]
pub fn minimal_recipe(name: &str, version: &str) -> Recipe {
    Recipe {
        preamble: Preamble { name: name.to_string(), version: version.to_string() },
        package: Package {},
        source: Source {
            url: format!("https://github.com/test/{name}/archive/v{version}.tar.gz"),
            filename: format!("{name}-{version}.tar.gz"),
            sha256: "deadbeef".repeat(8),
        },
        build: Build {
            name: name.to_string(),
            with_run_exports: true,
            max_pin: "x.x".to_string(),
            script: None,
        },
        requirements: Requirements { build: vec![], host: vec![], run: vec![] },
        test: Test { commands: vec![] },
        about: About {
            home: None,
            license: None,
            license_family: None,
            license_file: vec![],
            summary: None,
            dev_url: None,
            doc_url: None,
        },
        extra: Extra {
            additional_platforms: vec![],
            recipe_maintainers: vec![],
            identifiers: vec![],
            skip_platforms: vec![],
        },
    }
}

/// Render a recipe to meta.yaml string.
#[allow(dead_code)]
pub fn render(recipe: &Recipe) -> String {
    MetaYamlRenderer.render(recipe)
}

/// Assert that rendered output contains a substring.
#[allow(dead_code)]
pub fn assert_contains(output: &str, needle: &str, msg: &str) {
    assert!(output.contains(needle), "{msg}\nExpected to find: {needle}\nIn:\n{output}");
}

/// Assert that rendered output does NOT contain a substring.
#[allow(dead_code)]
pub fn assert_not_contains(output: &str, needle: &str, msg: &str) {
    assert!(!output.contains(needle), "{msg}\nExpected NOT to find: {needle}\nIn:\n{output}");
}
