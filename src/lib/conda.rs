//! Conda channel package availability checking and name normalization.

use anyhow::Result;
use reqwest::blocking::Client;

/// Verify if the package is available on Anaconda for a specific channel.
pub fn is_pkg_available(client: &Client, pkg_name: &str, channel: &str) -> Result<bool> {
    let url = format!("https://api.anaconda.org/package/{channel}/{pkg_name}");
    match client.get(url).send() {
        Ok(response) => Ok(response.status() == 200),
        Err(e) => Err(e.into()),
    }
}

/// Attempts to find the package name in a conda channel and returns the normalized
/// name if it exists, otherwise returns the name as-is.
///
/// Tries the original name, then snake_case variant, then kebab-case variant.
pub fn normalize_pkg_name(client: &Client, pkg_name: &str, channel: &str) -> String {
    if let Ok(true) = is_pkg_available(client, pkg_name, channel) {
        return pkg_name.to_string();
    }
    if pkg_name.contains('-') {
        let snake_case = pkg_name.replace('-', "_");
        if let Ok(true) = is_pkg_available(client, &snake_case, channel) {
            return snake_case;
        }
    }
    if pkg_name.contains('_') {
        let kebab_case = pkg_name.replace('_', "-");
        if let Ok(true) = is_pkg_available(client, &kebab_case, channel) {
            return kebab_case;
        }
    }
    pkg_name.to_string()
}

/// Checks if a dependency is available on a conda channel, returning its normalized
/// name if found.
pub fn check_dependency(client: &Client, dep_name: &str, channel: &str) -> Result<Option<String>> {
    let normalized = normalize_pkg_name(client, dep_name, channel);
    let avail = is_pkg_available(client, &normalized, channel)?;
    log::debug!("dependency: {dep_name} normalized: {normalized} available: {avail}");
    if avail { Ok(Some(normalized)) } else { Ok(None) }
}
