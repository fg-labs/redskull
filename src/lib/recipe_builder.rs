//! Builder for assembling conda recipes from crate metadata.
//! Uses &mut self pattern for ergonomic conditional field setting.
//! build() consumes self and returns both Recipe and BuildScript.

use crate::build_script::BuildScript;
use crate::license_family::guess_license_family;
use crate::recipe::*;

pub struct RecipeBuilder {
    name: String,
    version: String,
    source_url: String,
    source_filename: String,
    source_sha256: String,
    license: Option<String>,
    license_family: Option<String>,
    license_files: Vec<String>,
    summary: Option<String>,
    homepage: Option<String>,
    repository: Option<String>,
    documentation: Option<String>,
    binaries: Vec<String>,
    maintainers: Vec<String>,
    use_cbl: bool,
    is_bioconda: bool,
    has_c_deps: bool,
    has_cxx_deps: bool,
    has_native_deps: bool,
    use_bindgen: bool,
    needs_pkg_config: bool,
    needs_make: bool,
    needs_cmake: bool,
    host_deps: Vec<(String, Option<String>)>,
    run_deps: Vec<(String, Option<String>)>,
    max_pin: String,
    additional_platforms: Vec<String>,
    skip_platforms: Vec<String>,
    workspace_path: Option<String>,
    test_command_overrides: Vec<String>,
    use_version_test: bool,
    emit_license_family: Option<bool>,
    cargo_net_git_fetch: bool,
    strip_binaries: bool,
    identifiers: Vec<String>,
}

impl RecipeBuilder {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            source_url: String::new(),
            source_filename: String::new(),
            source_sha256: String::new(),
            license: None,
            license_family: None,
            license_files: vec![],
            summary: None,
            homepage: None,
            repository: None,
            documentation: None,
            binaries: vec![],
            maintainers: vec![],
            use_cbl: false,
            is_bioconda: false,
            has_c_deps: false,
            has_cxx_deps: false,
            has_native_deps: false,
            use_bindgen: false,
            needs_pkg_config: false,
            needs_make: false,
            needs_cmake: false,
            host_deps: vec![],
            run_deps: vec![],
            max_pin: "x.x".to_string(),
            additional_platforms: vec![],
            skip_platforms: vec![],
            workspace_path: None,
            test_command_overrides: vec![],
            use_version_test: false,
            emit_license_family: None,
            cargo_net_git_fetch: false,
            strip_binaries: false,
            identifiers: vec![],
        }
    }

    pub fn github_source(&mut self, owner: &str, repo: &str, sha256: &str) -> &mut Self {
        self.source_url =
            format!("https://github.com/{owner}/{repo}/archive/v{{{{ version }}}}.tar.gz");
        self.source_filename = format!("{}-{{{{ version }}}}.tar.gz", self.name);
        self.source_sha256 = sha256.to_string();
        self
    }

    /// Set source from a pre-resolved GitHub URL template and SHA256.
    /// Use this when the URL template and SHA256 have been computed from the actual archive.
    pub fn github_source_resolved(&mut self, url_template: &str, sha256: &str) -> &mut Self {
        self.source_url = url_template.to_string();
        self.source_filename = format!("{}-{{{{ version }}}}.tar.gz", self.name);
        self.source_sha256 = sha256.to_string();
        self
    }

    pub fn crates_io_source(&mut self, dl_path: &str, sha256: &str) -> &mut Self {
        self.source_url = format!("https://crates.io{dl_path}");
        self.source_filename = format!("{}.{{{{ version }}}}.tar.gz", self.name);
        self.source_sha256 = sha256.to_string();
        self
    }

    pub fn license(&mut self, license: &str) -> &mut Self {
        self.license_family = Some(guess_license_family(license));
        self.license = Some(license.to_string());
        self
    }

    pub fn license_files(&mut self, files: Vec<String>) -> &mut Self {
        self.license_files = files;
        self
    }

    pub fn summary(&mut self, s: &str) -> &mut Self {
        self.summary = Some(s.to_string());
        self
    }

    pub fn homepage(&mut self, u: &str) -> &mut Self {
        self.homepage = Some(u.to_string());
        self
    }

    pub fn repository(&mut self, u: &str) -> &mut Self {
        self.repository = Some(u.to_string());
        self
    }

    pub fn documentation(&mut self, u: &str) -> &mut Self {
        self.documentation = Some(u.to_string());
        self
    }

    pub fn add_binary(&mut self, name: &str) -> &mut Self {
        self.binaries.push(name.to_string());
        self
    }

    pub fn add_maintainer(&mut self, name: &str) -> &mut Self {
        self.maintainers.push(name.to_string());
        self
    }

    pub fn cargo_bundle_licenses(&mut self, v: bool) -> &mut Self {
        self.use_cbl = v;
        self
    }

    pub fn bioconda(&mut self, v: bool) -> &mut Self {
        self.is_bioconda = v;
        self
    }

    pub fn has_c_deps(&mut self, v: bool) -> &mut Self {
        self.has_c_deps = v;
        self
    }

    pub fn has_cxx_deps(&mut self, v: bool) -> &mut Self {
        self.has_cxx_deps = v;
        self
    }

    pub fn has_native_deps(&mut self, v: bool) -> &mut Self {
        self.has_native_deps = v;
        self
    }

    pub fn needs_bindgen(&mut self, v: bool) -> &mut Self {
        self.use_bindgen = v;
        self
    }

    pub fn needs_pkg_config(&mut self, v: bool) -> &mut Self {
        self.needs_pkg_config = v;
        self
    }

    pub fn needs_make(&mut self, v: bool) -> &mut Self {
        self.needs_make = v;
        self
    }

    pub fn needs_cmake(&mut self, v: bool) -> &mut Self {
        self.needs_cmake = v;
        self
    }

    pub fn max_pin(&mut self, pin: &str) -> &mut Self {
        self.max_pin = pin.to_string();
        self
    }

    pub fn workspace_path(&mut self, path: &str) -> &mut Self {
        self.workspace_path = Some(path.to_string());
        self
    }

    pub fn add_host_dep(&mut self, name: &str, selector: Option<&str>) -> &mut Self {
        self.host_deps.push((name.to_string(), selector.map(String::from)));
        self
    }

    pub fn add_run_dep(&mut self, name: &str, selector: Option<&str>) -> &mut Self {
        self.run_deps.push((name.to_string(), selector.map(String::from)));
        self
    }

    pub fn add_platform(&mut self, platform: &str) -> &mut Self {
        self.additional_platforms.push(platform.to_string());
        self
    }

    pub fn skip_platform(&mut self, platform: &str) -> &mut Self {
        self.skip_platforms.push(platform.to_string());
        self
    }

    /// Override the auto-generated test commands.
    /// When overrides are provided, they replace the default `binary --help` commands.
    pub fn add_test_command(&mut self, cmd: &str) -> &mut Self {
        self.test_command_overrides.push(cmd.to_string());
        self
    }

    /// Use `--version` instead of `--help` for auto-generated test commands.
    pub fn use_version_test(&mut self, v: bool) -> &mut Self {
        self.use_version_test = v;
        self
    }

    /// Control whether `license_family` is emitted.
    /// Defaults to `true` unless explicitly disabled.
    pub fn emit_license_family(&mut self, v: bool) -> &mut Self {
        self.emit_license_family = Some(v);
        self
    }

    pub fn cargo_net_git_fetch(&mut self, v: bool) -> &mut Self {
        self.cargo_net_git_fetch = v;
        self
    }

    pub fn strip_binaries(&mut self, v: bool) -> &mut Self {
        self.strip_binaries = v;
        self
    }

    pub fn add_identifier(&mut self, id: &str) -> &mut Self {
        self.identifiers.push(id.to_string());
        self
    }

    /// Consume the builder and produce both a Recipe and a BuildScript.
    pub fn build(self) -> (Recipe, BuildScript) {
        // Determine license files
        let license_file = if self.license_files.is_empty() {
            let mut files = vec!["LICENSE".to_string()];
            if self.use_cbl {
                files.push("THIRDPARTY.yml".to_string());
            }
            files
        } else {
            let mut files = self.license_files;
            if self.use_cbl && !files.iter().any(|f| f == "THIRDPARTY.yml") {
                files.push("THIRDPARTY.yml".to_string());
            }
            files
        };

        // Build script (consuming builder pattern)
        let script = {
            let mut s = BuildScript::new()
                .locked(true)
                .cargo_bundle_licenses(self.use_cbl)
                .needs_bindgen(self.use_bindgen)
                .has_native_deps(self.has_native_deps)
                .force_build_sh(self.is_bioconda)
                .cargo_net_git_fetch(self.cargo_net_git_fetch)
                .strip_binaries(self.strip_binaries)
                .binaries(self.binaries.clone());
            if let Some(ref path) = self.workspace_path {
                s = s.workspace_path(path);
            }
            s
        };

        // Inline script for simple recipes
        let build_script_inline =
            if script.needs_build_sh() { None } else { Some(script.inline_script()) };

        // Determine whether to emit license_family
        let emit_family = self.emit_license_family.unwrap_or(true);
        let license_family = if emit_family { self.license_family } else { None };

        // Requirements
        let build_tools = BuildToolNeeds {
            pkg_config: self.needs_pkg_config,
            make: self.needs_make,
            cmake: self.needs_cmake,
        };
        let mut requirements = Requirements::for_rust_crate(
            self.use_cbl,
            self.has_c_deps,
            self.has_cxx_deps,
            self.use_bindgen,
            &build_tools,
        );
        for (name, selector) in &self.host_deps {
            requirements.add_host(name, selector.as_deref());
            // Mirror host deps as run deps (shared libraries needed at runtime)
            requirements.add_run(name, selector.as_deref());
        }
        for (name, selector) in &self.run_deps {
            requirements.add_run(name, selector.as_deref());
        }

        // Test commands: use overrides if provided, otherwise generate from binaries
        let test = if !self.test_command_overrides.is_empty() {
            Test { commands: self.test_command_overrides }
        } else if self.binaries.is_empty() {
            Test { commands: vec![] }
        } else {
            let refs: Vec<&str> = self.binaries.iter().map(|s| s.as_str()).collect();
            if self.use_version_test {
                Test::from_binaries_version(&refs)
            } else {
                Test::from_binaries(&refs)
            }
        };

        // Platforms
        let additional_platforms = if self.is_bioconda && self.additional_platforms.is_empty() {
            vec!["linux-aarch64".to_string(), "osx-arm64".to_string()]
        } else {
            self.additional_platforms
        };

        let recipe = Recipe {
            preamble: Preamble { name: self.name.clone(), version: self.version },
            package: Package {},
            source: Source {
                url: self.source_url,
                filename: self.source_filename,
                sha256: self.source_sha256,
            },
            build: Build {
                name: self.name,
                with_run_exports: self.is_bioconda,
                max_pin: self.max_pin,
                script: build_script_inline,
            },
            requirements,
            test,
            about: About {
                home: self.homepage,
                license: self.license,
                license_family,
                license_file,
                summary: self.summary,
                dev_url: self.repository,
                doc_url: self.documentation,
            },
            extra: Extra {
                additional_platforms,
                recipe_maintainers: self.maintainers,
                identifiers: self.identifiers,
                skip_platforms: self.skip_platforms,
            },
        };

        (recipe, script)
    }
}
