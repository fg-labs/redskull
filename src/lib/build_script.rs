//! Build script generation for Rust conda recipes.
//! Supports both inline `script:` (simple recipes) and `build.sh` (complex recipes).

pub struct BuildScript {
    use_locked: bool,
    use_cbl: bool,
    use_bindgen: bool,
    native_deps: bool,
    workspace_path: Option<String>,
    force_build_sh: bool,
    cargo_net_git_fetch: bool,
    strip_binaries: bool,
    binary_names: Vec<String>,
}

impl Default for BuildScript {
    fn default() -> Self {
        Self {
            use_locked: true,
            use_cbl: false,
            use_bindgen: false,
            native_deps: false,
            workspace_path: None,
            force_build_sh: false,
            cargo_net_git_fetch: false,
            strip_binaries: false,
            binary_names: vec![],
        }
    }
}

impl BuildScript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn locked(mut self, v: bool) -> Self {
        self.use_locked = v;
        self
    }

    pub fn cargo_bundle_licenses(mut self, v: bool) -> Self {
        self.use_cbl = v;
        self
    }

    pub fn needs_bindgen(mut self, v: bool) -> Self {
        self.use_bindgen = v;
        self
    }

    pub fn has_native_deps(mut self, v: bool) -> Self {
        self.native_deps = v;
        self
    }

    pub fn workspace_path(mut self, path: &str) -> Self {
        self.workspace_path = Some(path.to_string());
        self
    }

    pub fn force_build_sh(mut self, v: bool) -> Self {
        self.force_build_sh = v;
        self
    }

    pub fn cargo_net_git_fetch(mut self, v: bool) -> Self {
        self.cargo_net_git_fetch = v;
        self
    }

    pub fn strip_binaries(mut self, v: bool) -> Self {
        self.strip_binaries = v;
        self
    }

    pub fn binaries(mut self, names: Vec<String>) -> Self {
        self.binary_names = names;
        self
    }

    /// Whether this recipe needs a separate build.sh (vs inline script:).
    pub fn needs_build_sh(&self) -> bool {
        self.force_build_sh
            || self.use_cbl
            || self.use_bindgen
            || self.native_deps
            || self.workspace_path.is_some()
    }

    /// Generate the cargo install command line.
    fn cargo_install_cmd(&self) -> String {
        let path = self.workspace_path.as_deref().unwrap_or(".");
        let locked = if self.use_locked { " --locked" } else { "" };
        format!("cargo install --no-track{locked} --verbose --root \"${{PREFIX}}\" --path {path}")
    }

    /// Generate inline script content (for simple recipes).
    pub fn inline_script(&self) -> String {
        self.cargo_install_cmd()
    }

    /// Generate build.sh content (for complex recipes).
    pub fn to_build_sh(&self) -> String {
        let mut out = String::new();
        out.push_str("#!/bin/bash\nset -euo pipefail\n\n");

        // Standard env vars
        out.push_str("export RUST_BACKTRACE=1\n");

        if self.cargo_net_git_fetch {
            out.push_str("export CARGO_NET_GIT_FETCH_WITH_CLI=true\n");
        }
        out.push('\n');

        if self.native_deps {
            out.push_str("export CPPFLAGS=\"${CPPFLAGS} -I${PREFIX}/include\"\n");
            out.push_str("export LDFLAGS=\"${LDFLAGS} -L${PREFIX}/lib\"\n");
            out.push_str("export CFLAGS=\"${CFLAGS} -O3\"\n\n");
        }

        if self.use_bindgen {
            out.push_str("export BINDGEN_EXTRA_CLANG_ARGS=\"${CFLAGS} ${CPPFLAGS}\"\n\n");
        }

        if self.use_cbl {
            out.push_str("cargo-bundle-licenses --format yaml --output THIRDPARTY.yml\n\n");
        }

        out.push_str(&self.cargo_install_cmd());
        out.push('\n');

        if self.strip_binaries && !self.binary_names.is_empty() {
            out.push('\n');
            for bin in &self.binary_names {
                out.push_str(&format!(
                    "${{STRIP}} \"${{PREFIX}}/bin/{bin}\" 2>/dev/null || true\n"
                ));
            }
        }

        out
    }
}
