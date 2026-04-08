//! GitHub GraphQL API client.
//!
//! Consolidates multiple REST API calls into 1-2 GraphQL queries per crate:
//! - Releases + tags (for version detection)
//! - Repository tree (for license/runtime dep detection)
//! - File contents (Cargo.toml, workspace member Cargo.tomls)
//!
//! The archive download for SHA256 computation still uses REST (no GraphQL equivalent).

use anyhow::{Result, anyhow};
use reqwest::blocking::Client;

use crate::source::{GitHubRepo, is_prerelease_tag, looks_like_version_tag, tag_to_version};

const GRAPHQL_URL: &str = "https://api.github.com/graphql";

/// Result of the initial discovery query: releases, tags, tree, and root Cargo.toml.
pub struct RepoDiscovery {
    /// Version-like release tags, most recent first.
    pub release_tags: Vec<String>,
    /// Version-like ref tags, most recent first.
    pub ref_tags: Vec<String>,
    /// All file paths in the repository (recursive tree).
    pub tree: Vec<String>,
    /// Root Cargo.toml contents, if present.
    pub root_cargo_toml: Option<String>,
}

/// Execute the discovery query: fetch releases, tags, recursive tree, and root Cargo.toml
/// in a single GraphQL call.
pub fn discover_repo(client: &Client, repo: &GitHubRepo, tag: &str) -> Result<RepoDiscovery> {
    // Build the tree fragment 5 levels deep (covers most workspace layouts)
    let tree_fragment = tree_entries_fragment(5);

    let query = format!(
        r#"query($owner: String!, $repo: String!) {{
  repository(owner: $owner, name: $repo) {{
    releases(first: 10, orderBy: {{field: CREATED_AT, direction: DESC}}) {{
      nodes {{ tagName isPrerelease isDraft }}
    }}
    refs(refPrefix: "refs/tags/", first: 10, orderBy: {{field: TAG_COMMIT_DATE, direction: DESC}}) {{
      nodes {{ name }}
    }}
    rootCargo: object(expression: "{tag}:Cargo.toml") {{
      ... on Blob {{ text }}
    }}
    tree: object(expression: "{tag}:") {{
      ... on Tree {{
        {tree_fragment}
      }}
    }}
  }}
}}"#
    );

    let body = serde_json::json!({
        "query": query,
        "variables": {
            "owner": repo.owner,
            "repo": repo.name,
        }
    });

    let resp = client.post(GRAPHQL_URL).header("Accept", "application/json").json(&body).send()?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "GraphQL query failed for {}/{}: HTTP {}",
            repo.owner,
            repo.name,
            resp.status()
        ));
    }

    let json: serde_json::Value = resp.json()?;

    // Check for GraphQL errors
    if let Some(errors) = json.get("errors").and_then(|e| e.as_array()) {
        if !errors.is_empty() {
            let msg = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(anyhow!("GraphQL errors for {}/{}: {msg}", repo.owner, repo.name));
        }
    }

    let data = json
        .get("data")
        .and_then(|d| d.get("repository"))
        .ok_or_else(|| anyhow!("No repository data in GraphQL response"))?;

    // Parse releases
    let release_tags: Vec<String> = data
        .get("releases")
        .and_then(|r| r.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|nodes| {
            nodes
                .iter()
                .filter(|n| {
                    // Skip prereleases and drafts
                    let pre = n.get("isPrerelease").and_then(|v| v.as_bool()).unwrap_or(false);
                    let draft = n.get("isDraft").and_then(|v| v.as_bool()).unwrap_or(false);
                    !pre && !draft
                })
                .filter_map(|n| n.get("tagName").and_then(|t| t.as_str()).map(String::from))
                .filter(|t| looks_like_version_tag(t))
                .collect()
        })
        .unwrap_or_default();

    // Parse tags
    let ref_tags: Vec<String> = data
        .get("refs")
        .and_then(|r| r.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|n| n.get("name").and_then(|t| t.as_str()).map(String::from))
                .filter(|t| looks_like_version_tag(t) && !is_prerelease_tag(t))
                .collect()
        })
        .unwrap_or_default();

    // Parse tree recursively
    let tree = parse_tree_entries(data.get("tree"), "");

    // Parse root Cargo.toml
    let root_cargo_toml = data
        .get("rootCargo")
        .and_then(|o| o.get("text"))
        .and_then(|t| t.as_str())
        .map(String::from);

    Ok(RepoDiscovery { release_tags, ref_tags, tree, root_cargo_toml })
}

/// Fetch multiple file contents in a single GraphQL query using aliases.
/// Returns a Vec of (path, contents) pairs for files that exist.
pub fn fetch_files(
    client: &Client,
    repo: &GitHubRepo,
    tag: &str,
    paths: &[String],
) -> Result<Vec<(String, String)>> {
    if paths.is_empty() {
        return Ok(vec![]);
    }

    // Build aliased object queries for each path
    let file_queries: Vec<String> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            format!(
                r#"  file_{i}: object(expression: "{tag}:{path}") {{
    ... on Blob {{ text }}
  }}"#
            )
        })
        .collect();

    let query = format!(
        "query($owner: String!, $repo: String!) {{\n  repository(owner: $owner, name: $repo) {{\n{}\n  }}\n}}",
        file_queries.join("\n")
    );

    let body = serde_json::json!({
        "query": query,
        "variables": {
            "owner": repo.owner,
            "repo": repo.name,
        }
    });

    let resp = client.post(GRAPHQL_URL).header("Accept", "application/json").json(&body).send()?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "GraphQL file fetch failed for {}/{}: HTTP {}",
            repo.owner,
            repo.name,
            resp.status()
        ));
    }

    let json: serde_json::Value = resp.json()?;
    let data = json
        .get("data")
        .and_then(|d| d.get("repository"))
        .ok_or_else(|| anyhow!("No repository data in GraphQL response"))?;

    let mut results = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        let alias = format!("file_{i}");
        if let Some(text) = data.get(&alias).and_then(|o| o.get("text")).and_then(|t| t.as_str()) {
            results.push((path.clone(), text.to_string()));
        }
    }

    Ok(results)
}

/// Pick the best version tag from discovery results.
/// Compares release tags and ref tags, preferring the newer version.
pub fn best_version_tag(discovery: &RepoDiscovery) -> Option<String> {
    let release = discovery.release_tags.first();
    let tag = discovery.ref_tags.first();

    match (release, tag) {
        (Some(r), Some(t)) => {
            if r == t {
                Some(r.clone())
            } else {
                let rv = tag_to_version(r);
                let tv = tag_to_version(t);
                if compare_version_strings(&tv, &rv) {
                    log::info!(
                        "Tags API has newer version '{t}' than latest release '{r}'; using tags"
                    );
                    Some(t.clone())
                } else {
                    Some(r.clone())
                }
            }
        }
        (Some(r), None) => Some(r.clone()),
        (None, Some(t)) => Some(t.clone()),
        (None, None) => None,
    }
}

/// Build a GraphQL tree entries fragment to a given depth.
fn tree_entries_fragment(depth: usize) -> String {
    if depth == 0 {
        return "entries { name type }".to_string();
    }
    let inner = tree_entries_fragment(depth - 1);
    format!("entries {{ name type object {{ ... on Tree {{ {inner} }} }} }}")
}

/// Recursively parse tree entries from GraphQL response into flat file paths.
fn parse_tree_entries(node: Option<&serde_json::Value>, prefix: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let Some(entries) = node.and_then(|n| n.get("entries")).and_then(|e| e.as_array()) else {
        return paths;
    };

    for entry in entries {
        let Some(name) = entry.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let entry_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let full_path =
            if prefix.is_empty() { name.to_string() } else { format!("{prefix}/{name}") };

        match entry_type {
            "blob" => paths.push(full_path),
            "tree" => {
                // Add the directory path itself
                paths.push(full_path.clone());
                // Recurse into subdirectory
                let subtree = entry.get("object");
                paths.extend(parse_tree_entries(subtree, &full_path));
            }
            _ => paths.push(full_path),
        }
    }
    paths
}

/// Compare two version strings, returning true if `a` is greater than `b`.
fn compare_version_strings(a: &str, b: &str) -> bool {
    let parse_segments = |s: &str| -> Vec<u64> {
        s.split(['.', '-']).filter_map(|seg| seg.parse::<u64>().ok()).collect()
    };
    parse_segments(a) > parse_segments(b)
}
