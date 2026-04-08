//! Recipe data model for conda meta.yaml / recipe.yaml.

/// Jinja2 preamble with name and version variables.
pub struct Preamble {
    pub name: String,
    pub version: String,
}

pub struct Package {}

pub struct Source {
    pub url: String,
    pub filename: String,
    pub sha256: String,
}

pub struct Build {
    pub name: String,
    pub with_run_exports: bool,
    pub max_pin: String,
    /// If Some, use inline script instead of build.sh.
    pub script: Option<String>,
}

pub struct Requirement {
    pub name: String,
    pub version: Option<String>,
    /// Platform selector, e.g. "not osx", "not win", "not arm64".
    pub selector: Option<String>,
}

pub struct Requirements {
    pub build: Vec<Requirement>,
    pub host: Vec<Requirement>,
    pub run: Vec<Requirement>,
}

pub struct Test {
    pub commands: Vec<String>,
}

pub struct About {
    pub home: Option<String>,
    pub license: Option<String>,
    pub license_family: Option<String>,
    /// List of license files (rendered as YAML list when multiple).
    pub license_file: Vec<String>,
    pub summary: Option<String>,
    pub dev_url: Option<String>,
    pub doc_url: Option<String>,
}

pub struct Extra {
    pub additional_platforms: Vec<String>,
    pub recipe_maintainers: Vec<String>,
    pub identifiers: Vec<String>,
    pub skip_platforms: Vec<String>,
}

pub struct Recipe {
    pub preamble: Preamble,
    pub package: Package,
    pub source: Source,
    pub build: Build,
    pub requirements: Requirements,
    pub test: Test,
    pub about: About,
    pub extra: Extra,
}

impl Test {
    pub fn from_binaries(binaries: &[&str]) -> Self {
        Self { commands: binaries.iter().map(|b| format!("{b} --help")).collect() }
    }

    pub fn from_binaries_version(binaries: &[&str]) -> Self {
        Self { commands: binaries.iter().map(|b| format!("{b} --version")).collect() }
    }
}

impl Requirement {
    pub fn simple(name: &str) -> Self {
        Self { name: name.to_string(), version: None, selector: None }
    }
}

/// Configuration for which build tools to include in requirements.
pub struct BuildToolNeeds {
    pub pkg_config: bool,
    pub make: bool,
    pub cmake: bool,
}

impl Requirements {
    /// Create requirements for a Rust crate.
    ///
    /// * `cargo_bundle_licenses` - include CBL in build deps
    /// * `has_c_deps` - crate links C code (adds compiler('c'))
    /// * `has_cxx_deps` - crate links C++ code (adds compiler('cxx'))
    /// * `has_bindgen` - crate uses bindgen (adds clangdev)
    /// * `build_tools` - which build tools to include (pkg-config, make, cmake)
    pub fn for_rust_crate(
        cargo_bundle_licenses: bool,
        has_c_deps: bool,
        has_cxx_deps: bool,
        has_bindgen: bool,
        build_tools: &BuildToolNeeds,
    ) -> Self {
        let mut build = vec![];

        if has_c_deps {
            build.push(Requirement::simple("{{ compiler('c') }}"));
        }
        if has_cxx_deps {
            build.push(Requirement::simple("{{ compiler('cxx') }}"));
        }
        build.push(Requirement::simple("{{ compiler('rust') }}"));

        if cargo_bundle_licenses {
            build.push(Requirement::simple("cargo-bundle-licenses"));
        }

        if has_bindgen {
            build.push(Requirement::simple("clangdev"));
        }

        if build_tools.pkg_config {
            build.push(Requirement::simple("pkg-config"));
        }
        if build_tools.make {
            build.push(Requirement::simple("make"));
        }
        if build_tools.cmake {
            build.push(Requirement::simple("cmake"));
        }

        Self { build, host: vec![], run: vec![] }
    }

    pub fn add_host(&mut self, name: &str, selector: Option<&str>) {
        self.host.push(Requirement {
            name: name.to_string(),
            version: None,
            selector: selector.map(String::from),
        });
    }

    pub fn add_run(&mut self, name: &str, selector: Option<&str>) {
        self.run.push(Requirement {
            name: name.to_string(),
            version: None,
            selector: selector.map(String::from),
        });
    }
}
