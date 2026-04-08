//! Source URL generation and sha256 computation.

use anyhow::{Result, anyhow};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

/// Parsed GitHub repository info.
pub struct GitHubRepo {
    pub owner: String,
    pub name: String,
}

impl GitHubRepo {
    /// Parse owner/name from a GitHub URL.
    /// Handles trailing slashes, .git suffix, and various URL formats.
    pub fn from_url(url: &str) -> Result<Self> {
        if !url.contains("github.com") {
            return Err(anyhow!("URL is not a GitHub URL: {url}"));
        }
        let url = url.trim_end_matches('/').trim_end_matches(".git");
        let parts: Vec<&str> = url.rsplitn(3, '/').collect();
        if parts.len() < 2 {
            return Err(anyhow!("Cannot parse GitHub URL: {url}"));
        }
        Ok(Self { name: parts[0].to_string(), owner: parts[1].to_string() })
    }
}

/// Construct a GitHub archive URL for a given tag/version with a tag prefix.
pub fn github_archive_url(repo: &GitHubRepo, version: &str, tag_prefix: &str) -> String {
    format!("https://github.com/{}/{}/archive/{tag_prefix}{version}.tar.gz", repo.owner, repo.name)
}

/// Replace the version portion of a tag with the jinja `{{ version }}` placeholder.
/// If the tag does not contain the version, returns the literal tag.
pub fn tag_to_jinja_template(tag: &str, version: &str) -> String {
    if tag.contains(version) { tag.replace(version, "{{ version }}") } else { tag.to_string() }
}

/// Resolved GitHub source info.
pub struct ResolvedGitHubSource {
    /// URL template with `{{ version }}` jinja placeholder.
    pub url_template: String,
    /// SHA256 hash of the archive.
    pub sha256: String,
    /// The resolved tag (e.g., "v0.3.1" or "0.3.1").
    pub tag: String,
}

/// Try to resolve the correct GitHub archive URL and compute its SHA256.
/// When `tag_override` is provided, it is used directly without tag prefix detection.
/// Otherwise, tries `v`-prefixed tag first, then bare version.
/// When `use_refs_tags` is true, the URL template uses `/archive/refs/tags/` instead of
/// `/archive/`.
pub fn resolve_github_source(
    client: &Client,
    repo: &GitHubRepo,
    version: &str,
    tag_override: Option<&str>,
    use_refs_tags: bool,
) -> Result<ResolvedGitHubSource> {
    if let Some(tag) = tag_override {
        return resolve_with_tag(client, repo, version, tag, use_refs_tags);
    }

    // Try v-prefixed tag first, then bare version
    // Each attempts the public archive URL, then falls back to API tarball
    for tag in &[format!("v{version}"), version.to_string()] {
        let result = resolve_with_tag(client, repo, version, tag, use_refs_tags);
        if result.is_ok() {
            return result;
        }
    }

    Err(anyhow!("Could not download GitHub archive for {}/{} v{}", repo.owner, repo.name, version))
}

/// Resolve a GitHub source using a specific tag.
/// Tries the public archive URL first, then falls back to the API tarball endpoint
/// (which works for private repos when GITHUB_TOKEN is set).
fn resolve_with_tag(
    client: &Client,
    repo: &GitHubRepo,
    version: &str,
    tag: &str,
    use_refs_tags: bool,
) -> Result<ResolvedGitHubSource> {
    let url = format!("https://github.com/{}/{}/archive/{tag}.tar.gz", repo.owner, repo.name);
    let bytes = match client.get(&url).send() {
        Ok(resp) if resp.status().is_success() => resp.bytes()?,
        _ => {
            // Fall back to API tarball endpoint (works with auth for private repos)
            log::info!(
                "Public archive URL returned error; trying API tarball endpoint for {}/{}",
                repo.owner,
                repo.name
            );
            let api_url =
                format!("https://api.github.com/repos/{}/{}/tarball/{tag}", repo.owner, repo.name);
            let resp =
                client.get(&api_url).header("Accept", "application/vnd.github+json").send()?;
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "Could not download GitHub archive for {}/{} at tag {tag}: HTTP {}",
                    repo.owner,
                    repo.name,
                    resp.status()
                ));
            }
            resp.bytes()?
        }
    };
    let hash = sha256_hex(&bytes);

    let archive_base = if use_refs_tags { "archive/refs/tags" } else { "archive" };

    // Build URL template: replace the version portion of the tag with {{ version }}
    if !tag.contains(version) {
        log::warn!(
            "Tag '{tag}' does not contain version '{version}'; \
             URL template will use the literal tag and won't auto-update."
        );
    }
    let template_tag = tag_to_jinja_template(tag, version);
    let template = format!(
        "https://github.com/{}/{}/{archive_base}/{template_tag}.tar.gz",
        repo.owner, repo.name
    );

    Ok(ResolvedGitHubSource { url_template: template, sha256: hash, tag: tag.to_string() })
}

/// Compute SHA256 hex digest from bytes.
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Fetch a raw file from a GitHub repo at a given tag.
pub fn fetch_github_raw(
    client: &Client,
    repo: &GitHubRepo,
    tag: &str,
    path: &str,
) -> Result<String> {
    let url =
        format!("https://raw.githubusercontent.com/{}/{}/{tag}/{path}", repo.owner, repo.name);
    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("Failed to fetch {path} from {}/{} at {tag}", repo.owner, repo.name));
    }
    Ok(resp.text()?)
}

/// Fetch the file listing of a GitHub repo at a given tag using the Trees API.
/// Returns a list of file paths relative to the repo root.
pub fn fetch_github_tree(client: &Client, repo: &GitHubRepo, tag: &str) -> Result<Vec<String>> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{tag}?recursive=1",
        repo.owner, repo.name
    );
    let resp = client.get(&url).header("Accept", "application/vnd.github+json").send()?;
    if !resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch tree for {}/{} at {tag}: HTTP {}",
            repo.owner,
            repo.name,
            resp.status()
        ));
    }
    let body: serde_json::Value = resp.json()?;
    let paths = body
        .get("tree")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry.get("path").and_then(|p| p.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(paths)
}

/// Returns true if the given string looks like a valid SHA256 hex digest.
pub fn is_valid_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

/// Fetch the latest release tag from a GitHub repo.
/// Tries both the releases API and tags API, then picks the best version-like tag.
/// Skips non-version tags like "latest", "nightly", etc.
/// Returns the tag name (e.g., "v1.2.3" or "1.2.3").
pub fn latest_github_release(client: &Client, repo: &GitHubRepo) -> Result<String> {
    let mut candidates: Vec<String> = Vec::new();

    // Try releases API — check up to 10 recent releases for a stable version-like tag
    let releases_url =
        format!("https://api.github.com/repos/{}/{}/releases?per_page=10", repo.owner, repo.name);
    if let Ok(resp) =
        client.get(&releases_url).header("Accept", "application/vnd.github+json").send()
    {
        if resp.status().is_success() {
            if let Ok(body) = resp.json::<serde_json::Value>() {
                if let Some(releases) = body.as_array() {
                    for release in releases {
                        // Skip drafts and prereleases
                        let is_pre =
                            release.get("prerelease").and_then(|v| v.as_bool()).unwrap_or(false);
                        let is_draft =
                            release.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);
                        if is_pre || is_draft {
                            continue;
                        }
                        if let Some(tag) = release.get("tag_name").and_then(|t| t.as_str()) {
                            if looks_like_version_tag(tag) && !is_prerelease_tag(tag) {
                                candidates.push(tag.to_string());
                                break;
                            }
                            log::debug!(
                                "Skipping non-version release tag '{tag}' for {}/{}",
                                repo.owner,
                                repo.name
                            );
                        }
                    }
                }
            }
        }
    }

    // Try tags API — get the most recent stable version-like tag
    let tags_url =
        format!("https://api.github.com/repos/{}/{}/tags?per_page=10", repo.owner, repo.name);
    if let Ok(resp) = client.get(&tags_url).header("Accept", "application/vnd.github+json").send() {
        if resp.status().is_success() {
            if let Ok(body) = resp.json::<serde_json::Value>() {
                if let Some(tags) = body.as_array() {
                    for tag_obj in tags {
                        if let Some(tag) = tag_obj.get("name").and_then(|n| n.as_str()) {
                            if looks_like_version_tag(tag) && !is_prerelease_tag(tag) {
                                candidates.push(tag.to_string());
                                break;
                            }
                            log::debug!(
                                "Skipping non-stable tag '{tag}' for {}/{}",
                                repo.owner,
                                repo.name
                            );
                        }
                    }
                }
            }
        }
    }

    if candidates.is_empty() {
        return Err(anyhow!(
            "No version-like tags found for {}/{}. \
             Use --tag to specify the release tag manually.",
            repo.owner,
            repo.name
        ));
    }

    // If we have candidates from both sources, pick the higher version
    if candidates.len() > 1 {
        let v0 = tag_to_version(&candidates[0]);
        let v1 = tag_to_version(&candidates[1]);
        if v0 != v1 {
            log::debug!(
                "Release tag '{}' vs tags API '{}' — comparing versions",
                candidates[0],
                candidates[1]
            );
            // Compare as semver-ish: split on dots, compare numerically
            if compare_version_strings(&v1, &v0) {
                log::info!(
                    "Tags API has newer version '{}' than latest release '{}'; using tags",
                    candidates[1],
                    candidates[0]
                );
                return Ok(candidates.swap_remove(1));
            }
        }
    }

    Ok(candidates.swap_remove(0))
}

/// Compare two version strings, returning true if `a` is greater than `b`.
/// Splits on `.` and `-`, compares segments numerically where possible.
fn compare_version_strings(a: &str, b: &str) -> bool {
    let parse_segments = |s: &str| -> Vec<u64> {
        s.split(['.', '-']).filter_map(|seg| seg.parse::<u64>().ok()).collect()
    };
    let sa = parse_segments(a);
    let sb = parse_segments(b);
    sa > sb
}

/// Strip a version prefix (`v` or `v.`) from a tag to get a bare version string.
/// Handles `v1.2.3`, `v.1.2.3`, and bare `1.2.3` patterns.
pub fn tag_to_version(tag: &str) -> String {
    // Strip `v.` before `v` to handle `v.1.2.3` (e.g., alejandrogzi tools)
    if let Some(rest) = tag.strip_prefix("v.") {
        return rest.to_string();
    }
    tag.strip_prefix('v').unwrap_or(tag).to_string()
}

/// Returns true if the given string looks like a version tag.
/// Matches patterns like `v1.2.3`, `1.2.3`, `v.1.2.3`, `tool-v1.2.3`, etc.
/// Returns false for tags like `latest`, `nightly`, `stable`.
pub fn looks_like_version_tag(tag: &str) -> bool {
    // Strip known prefixes to find the version portion
    let version_part = tag.strip_prefix("v.").or_else(|| tag.strip_prefix('v')).unwrap_or(tag);
    // A version-like string starts with a digit
    version_part.starts_with(|c: char| c.is_ascii_digit())
}

/// Returns true if the tag looks like a pre-release version.
/// Detects common pre-release suffixes: -alpha, -beta, -rc, -dev, -pre.
pub fn is_prerelease_tag(tag: &str) -> bool {
    let lower = tag.to_lowercase();
    ["-alpha", "-beta", "-rc", "-dev", "-pre", ".alpha", ".beta", ".rc"]
        .iter()
        .any(|suffix| lower.contains(suffix))
}

/// Construct a crates.io download URL.
pub fn crates_io_url(base_url: &str, dl_path: &str) -> String {
    format!("{base_url}{dl_path}")
}

/// Download a URL and compute its sha256 hex digest.
pub fn compute_sha256(client: &Client, url: &str) -> Result<(Vec<u8>, String)> {
    let response = client.get(url).send()?;
    let bytes = response.bytes()?;
    let hash = sha256_hex(&bytes);
    Ok((bytes.to_vec(), hash))
}
