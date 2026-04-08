//! Inspect crate source to extract binary names, license files, and workspace info.

use anyhow::{Result, anyhow};

/// Parsed metadata from a crate's Cargo.toml.
pub struct CargoMetadata {
    parsed: toml::Value,
}

impl CargoMetadata {
    /// Parse a Cargo.toml string.
    pub fn from_toml_str(s: &str) -> Result<Self> {
        let parsed: toml::Value =
            s.parse().map_err(|e| anyhow!("Failed to parse Cargo.toml: {e}"))?;
        Ok(Self { parsed })
    }

    /// Returns the binary names defined in [[bin]] sections.
    /// Falls back to the package name if no [[bin]] sections exist.
    pub fn binary_names(&self) -> Vec<String> {
        if let Some(bins) = self.parsed.get("bin").and_then(|b| b.as_array()) {
            let names: Vec<String> = bins
                .iter()
                .filter_map(|b| b.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect();
            if !names.is_empty() {
                return names;
            }
        }
        // Default: package name is the binary name
        if let Some(name) =
            self.parsed.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str())
        {
            vec![name.to_string()]
        } else {
            vec![]
        }
    }

    /// Returns workspace member paths, if this is a workspace root.
    /// Note: may contain glob patterns like `crates/*`.
    pub fn workspace_members(&self) -> Vec<String> {
        self.parsed
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    /// Returns the package name, if present.
    pub fn package_name(&self) -> Option<String> {
        self.parsed
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .map(String::from)
    }

    /// Returns true if this is a workspace root (has [workspace] section).
    pub fn is_workspace(&self) -> bool {
        self.parsed.get("workspace").is_some()
    }

    /// Returns true if this has a [package] section (is a buildable crate).
    pub fn has_package(&self) -> bool {
        self.parsed.get("package").is_some()
    }

    /// Returns the package version, if present.
    /// Handles `workspace = true` inheritance when `workspace_meta` is provided.
    pub fn version(&self, workspace_meta: Option<&Self>) -> Option<String> {
        self.package_field_or_workspace("version", workspace_meta)
    }

    /// Returns the package license, if present.
    pub fn license(&self, workspace_meta: Option<&Self>) -> Option<String> {
        self.package_field_or_workspace("license", workspace_meta)
    }

    /// Returns the package description, if present.
    pub fn description(&self, workspace_meta: Option<&Self>) -> Option<String> {
        self.package_field_or_workspace("description", workspace_meta)
    }

    /// Returns the package homepage, if present.
    pub fn homepage(&self, workspace_meta: Option<&Self>) -> Option<String> {
        self.package_field_or_workspace("homepage", workspace_meta)
    }

    /// Returns the package repository, if present.
    pub fn repository(&self, workspace_meta: Option<&Self>) -> Option<String> {
        self.package_field_or_workspace("repository", workspace_meta)
    }

    /// Returns the package documentation URL, if present.
    pub fn documentation(&self, workspace_meta: Option<&Self>) -> Option<String> {
        self.package_field_or_workspace("documentation", workspace_meta)
    }

    /// Get a [package] string field, falling back to [workspace.package] if `workspace = true`.
    fn package_field_or_workspace(
        &self,
        field: &str,
        workspace_meta: Option<&Self>,
    ) -> Option<String> {
        let pkg = self.parsed.get("package")?;
        let val = pkg.get(field)?;

        // If the value is a table with `workspace = true`, resolve from workspace
        if let Some(table) = val.as_table() {
            if table.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
                return workspace_meta.and_then(|ws| {
                    ws.parsed
                        .get("workspace")
                        .and_then(|w| w.get("package"))
                        .and_then(|p| p.get(field))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });
            }
        }

        val.as_str().map(String::from)
    }

    /// Returns dependency crate names from [dependencies].
    /// Returns pairs of (crate_name, is_optional).
    pub fn dependencies(&self) -> Vec<(String, bool)> {
        Self::parse_dep_table(self.parsed.get("dependencies"))
    }

    /// Returns build-dependency crate names from [build-dependencies].
    pub fn build_dependencies(&self) -> Vec<String> {
        Self::parse_dep_table(self.parsed.get("build-dependencies"))
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    /// Parse a dependency table (either [dependencies] or [build-dependencies]).
    fn parse_dep_table(table: Option<&toml::Value>) -> Vec<(String, bool)> {
        let Some(table) = table.and_then(|t| t.as_table()) else {
            return vec![];
        };
        table
            .iter()
            .map(|(name, val)| {
                let optional = val
                    .as_table()
                    .and_then(|t| t.get("optional"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                (name.clone(), optional)
            })
            .collect()
    }
}

/// Resolve workspace member patterns against a file tree.
/// Expands glob patterns like `crates/*` by finding directories that contain `Cargo.toml`.
/// Literal members are returned as-is.
pub fn resolve_workspace_members(members: &[String], tree: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for member in members {
        if member.contains('*') {
            // Expand glob: find directories matching the pattern that contain Cargo.toml
            let prefix = member.split('*').next().unwrap_or("");
            for path in tree {
                if let Some(rest) = path.strip_prefix(prefix) {
                    // Match: prefix/something/Cargo.toml (one level deep)
                    if rest.ends_with("/Cargo.toml") && !rest[..rest.len() - 11].contains('/') {
                        // Extract the directory: prefix + dir_name
                        let dir = &path[..path.len() - 11]; // strip /Cargo.toml
                        if !result.contains(&dir.to_string()) {
                            result.push(dir.to_string());
                        }
                    }
                }
            }
        } else {
            result.push(member.clone());
        }
    }
    result
}

/// Given a list of file paths in the source archive, detect license files.
/// Only considers root-level files (no directory separators).
/// Matches common patterns: LICENSE, LICENSE.md, LICENSE.txt, LICENCE, COPYING,
/// LICENSE-MIT, LICENSE-APACHE, etc.
pub fn detect_license_files(file_names: &[String]) -> Vec<String> {
    let patterns = ["LICENSE", "LICENCE", "COPYING"];
    let mut found: Vec<String> = file_names
        .iter()
        .filter(|f| {
            // Only root-level files
            if f.contains('/') {
                return false;
            }
            let upper = f.to_uppercase();
            patterns.iter().any(|p| upper.starts_with(p))
        })
        .cloned()
        .collect();
    found.sort();
    found
}
