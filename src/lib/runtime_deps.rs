//! Detect non-Rust runtime dependencies from source tree file patterns.
//! Scans file paths for indicators of R, Python, or other scripting languages
//! that would need corresponding conda run dependencies.

/// A detected runtime dependency hint.
pub struct RuntimeHint {
    /// Suggested conda package (e.g., "r-base", "python").
    pub package: &'static str,
    /// Human-readable reason for the suggestion.
    pub reason: String,
}

/// Scan a list of file paths for R/Python runtime dependency indicators.
/// Returns hints about potential run deps the user should consider adding.
pub fn detect_runtime_hints(file_paths: &[String]) -> Vec<RuntimeHint> {
    let mut hints = Vec::new();

    let r_scripts: Vec<&str> = file_paths
        .iter()
        .filter(|p| {
            let lower = p.to_lowercase();
            lower.ends_with(".r") || lower.ends_with(".rmd") || lower.ends_with(".rscript")
        })
        .map(|p| p.as_str())
        .collect();

    let py_scripts: Vec<&str> = file_paths
        .iter()
        .filter(|p| {
            let lower = p.to_lowercase();
            lower.ends_with(".py") && !lower.contains("setup.py") && !lower.contains("conf.py")
        })
        .map(|p| p.as_str())
        .collect();

    let has_r_description =
        file_paths.iter().any(|p| p == "DESCRIPTION" || p.ends_with("/DESCRIPTION"));
    let has_renv = file_paths.iter().any(|p| p.ends_with("renv.lock"));
    let has_requirements_txt = file_paths.iter().any(|p| {
        let name = p.rsplit('/').next().unwrap_or(p);
        name == "requirements.txt"
    });

    if !r_scripts.is_empty() || has_r_description || has_renv {
        let count = r_scripts.len();
        let examples: Vec<&str> = r_scripts.iter().take(3).copied().collect();
        let detail = if count > 0 {
            format!("found {count} R script(s): {}", examples.join(", "))
        } else if has_r_description {
            "found DESCRIPTION file (R package)".to_string()
        } else {
            "found renv.lock file".to_string()
        };
        hints.push(RuntimeHint { package: "r-base", reason: detail });
    }

    if !py_scripts.is_empty() || has_requirements_txt {
        let count = py_scripts.len();
        let examples: Vec<&str> = py_scripts.iter().take(3).copied().collect();
        let detail = if count > 0 {
            format!("found {count} Python script(s): {}", examples.join(", "))
        } else {
            "found requirements.txt file".to_string()
        };
        hints.push(RuntimeHint { package: "python", reason: detail });
    }

    hints
}
