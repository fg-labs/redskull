#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use anyhow::{Result, ensure};
use clap::builder::styling;
use clap::{ColorChoice, Parser as ClapParser, ValueEnum};
use crates_io_api::SyncClient;
use env_logger::Env;
use redskull_lib::conda;
use redskull_lib::crate_inspector::{
    CargoMetadata, detect_license_files, parse_cargo_lock, resolve_workspace_members,
};
use redskull_lib::github_graphql;
use redskull_lib::recipe_builder::RecipeBuilder;
use redskull_lib::renderer::{MetaYamlRenderer, Renderer};
use redskull_lib::runtime_deps;
use redskull_lib::source::{self, GitHubRepo};
use redskull_lib::sys_deps;
use reqwest::blocking::ClientBuilder as ReqwestClientBuilder;

use std::path::PathBuf;
use std::process::ExitCode;

pub mod built_info {
    use std::sync::LazyLock;

    include!(concat!(env!("OUT_DIR"), "/built.rs"));

    /// Get a software version string including
    ///   - Git commit hash
    ///   - Git dirty info (whether the repo had uncommitted changes)
    ///   - Cargo package version if no git info found
    fn get_software_version() -> String {
        let prefix = if let Some(s) = GIT_COMMIT_HASH {
            format!("{}-{}", PKG_VERSION, s[0..8].to_owned())
        } else {
            // This shouldn't happen
            PKG_VERSION.to_string()
        };
        let suffix = match GIT_DIRTY {
            Some(true) => "-dirty",
            _ => "",
        };
        format!("{prefix}{suffix}")
    }

    pub static VERSION: LazyLock<String> = LazyLock::new(get_software_version);
}

/// The style for the usage
const STYLES: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::Yellow.on_default().bold())
    .usage(styling::AnsiColor::Yellow.on_default().bold())
    .literal(styling::AnsiColor::Blue.on_default().bold())
    .placeholder(styling::AnsiColor::Cyan.on_default());

/// # OVERVIEW
///
/// redskull: grayskull for rust.
///
/// Specify the crates.io packages name. Redskull can also accept a github url.
///
#[derive(ClapParser, Debug, Clone)]
#[clap(
    name = "redskull",
    color = ColorChoice::Auto,
    styles = STYLES,
    version = built_info::VERSION.as_str())
 ]
#[allow(clippy::struct_excessive_bools)]
struct Opts {
    /// List of maintainers which will be added to the recipe.
    #[clap(long)]
    maintainers: Vec<String>,

    /// Path to where the recipe will be created.
    #[clap(long)]
    output: Option<PathBuf>,

    /// Recursively run grayskull on missing dependencies.
    #[clap(long, short = 'r', default_value = "false")]
    recursive: bool,

    /// If tag is specified, Redskull will build from release tag
    #[clap(long = "tag")]
    github_release_tag: Option<String>,

    /// Override the crate version (defaults to the latest version on crates.io).
    #[clap(long)]
    crate_version: Option<String>,

    /// Add additional output for bioconda compatibility
    #[clap(long, default_value = "false")]
    bioconda: bool,

    /// Use cargo-bundle-licenses to generate THIRDPARTY.yml.
    /// Defaults to true when --bioconda is set, unless explicitly disabled.
    #[clap(long)]
    cargo_bundle_licenses: Option<bool>,

    /// Override: additional host dependencies (e.g., --host-dep zlib --host-dep openssl)
    #[clap(long)]
    host_dep: Vec<String>,

    /// Override: additional run dependencies (e.g., --run-dep samtools --run-dep minimap2)
    #[clap(long)]
    run_dep: Vec<String>,

    /// Override: test commands (e.g., --test-command "mytool --version")
    #[clap(long)]
    test_command: Vec<String>,

    /// Override: skip platforms (e.g., --skip-platform osx)
    #[clap(long)]
    skip_platform: Vec<String>,

    /// Override the recipe name (defaults to the crates.io package name).
    /// Useful when the bioconda recipe name differs from the crate name.
    #[clap(long)]
    recipe_name: Option<String>,

    /// Source type: "github" (default) or "crates-io"
    #[clap(long, default_value = "github")]
    source: SourceType,

    /// Override the max_pin value for run_exports (default: "x.x").
    /// Use "x" for pre-1.0 tools.
    #[clap(long)]
    max_pin: Option<String>,

    /// Use `refs/tags/` in GitHub archive URL template.
    /// Both forms resolve identically on GitHub; this controls the template text.
    #[clap(long, default_value = "false")]
    refs_tags: bool,

    /// Set CARGO_NET_GIT_FETCH_WITH_CLI=true in build.sh.
    /// Needed when the build environment requires SSH for git fetches.
    #[clap(long, default_value = "false")]
    cargo_net_git_fetch: bool,

    /// Use `--version` instead of `--help` for auto-generated test commands.
    #[clap(long, default_value = "false")]
    test_version: bool,

    /// Strip debug symbols from binaries after build.
    #[clap(long, default_value = "false")]
    strip: bool,

    /// Emit license_family in the recipe. Defaults to true.
    #[clap(long)]
    license_family: Option<bool>,

    /// Add identifiers to the extra section (e.g., --identifier "doi:10.1234/foo").
    #[clap(long)]
    identifier: Vec<String>,

    /// Override workspace member path (e.g., --workspace-path crates/cmdline).
    #[clap(long)]
    workspace_path: Option<String>,

    /// Crates.io package names or GitHub URLs.
    args: Vec<String>,
}

/// Source type for archive downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SourceType {
    /// Use GitHub release archives (default).
    Github,
    /// Use crates.io tarballs.
    #[value(name = "crates-io")]
    CratesIo,
}

/// Runs redskull and converts Option<u8> to ``ExitCode``
///
/// Set the exit code:
/// - exit code SUCCESS if there were matches
/// - 1 if there was an error during building the recipe
/// - 101 if it panicked
fn main() -> ExitCode {
    // Receives u8 from
    if let Some(redskull_output) = redskull(&setup()) {
        ExitCode::from(redskull_output)
    } else {
        ExitCode::SUCCESS
    }
}

/// Runs ``redskull_from_opts`` and returns None upon success and an error number if error or zero matches
///
/// - None if there were matches
/// - Some(1) if there was an error during building the recipe
/// - Some(101) if it panicked
fn redskull(opts: &Opts) -> Option<u8> {
    let outer = std::panic::catch_unwind(|| redskull_from_opts(opts));
    match outer {
        Err(_) => {
            eprintln!("Error: redskull panicked.  Please report this as a bug!");
            Some(101)
        }
        Ok(inner) => match inner {
            Ok(()) => None,
            Err(e) => {
                eprintln!("Error: {e}");
                Some(2)
            }
        },
    }
}

/// Verify that detected host deps and CLI-provided host deps exist on conda-forge.
/// Logs warnings for any that are not found.
fn verify_host_deps(
    conda_client: &reqwest::blocking::Client,
    channel: &str,
    detected_deps: &[(&str, Option<&str>)],
    cli_deps: &[String],
) {
    for (dep, _selector) in detected_deps {
        match conda::is_pkg_available(conda_client, dep, channel) {
            Ok(true) => log::debug!("Host dep '{dep}' found on {channel}"),
            Ok(false) => log::warn!("Host dep '{dep}' not found on {channel}"),
            Err(e) => log::debug!("Could not check '{dep}' on {channel}: {e}"),
        }
    }
    for dep in cli_deps {
        match conda::is_pkg_available(conda_client, dep, channel) {
            Ok(true) => log::debug!("Host dep '{dep}' found on {channel}"),
            Ok(false) => log::warn!("Host dep '{dep}' (from --host-dep) not found on {channel}"),
            Err(e) => log::debug!("Could not check '{dep}' on {channel}: {e}"),
        }
    }
}

/// Build a README doc_url on GitHub for the given repo at the given tag,
/// with the version portion of the tag replaced by the jinja `{{ version }}`
/// placeholder so the URL auto-updates across releases.
fn build_readme_doc_url(repo: &GitHubRepo, tag: &str, version: &str) -> String {
    let template_tag = source::tag_to_jinja_template(tag, version);
    format!("https://github.com/{}/{}/blob/{template_tag}/README.md", repo.owner, repo.name)
}

/// Resolve the effective dependency list to feed into sys-dep detection.
///
/// Prefers the full resolved graph from `Cargo.lock` if one exists in `source_root`.
/// This catches transitive `-sys` crates (e.g., `openssl-sys` pulled in through `reqwest`),
/// which direct-dependency inspection misses.
///
/// Falls back to `direct_deps` with a warning when no lockfile is present (for example,
/// when crates.io `.crate` tarballs for library-only crates don't ship a `Cargo.lock`).
/// The returned vec owns the strings; call sites should take `&[&str]` views into it.
fn resolve_effective_deps(
    source_root: Option<&std::path::Path>,
    direct_deps: &[String],
    crate_label: &str,
) -> Vec<String> {
    let Some(root) = source_root else {
        log::warn!(
            "No extracted source tree for {crate_label}; \
             using direct dependencies only (may miss transitive -sys crates)"
        );
        return direct_deps.to_vec();
    };
    let lock_path = root.join("Cargo.lock");
    if !lock_path.exists() {
        log::warn!(
            "No Cargo.lock in {} for {crate_label}; \
             using direct dependencies only (may miss transitive -sys crates)",
            root.display()
        );
        return direct_deps.to_vec();
    }
    match parse_cargo_lock(&lock_path) {
        Ok(mut names) => {
            log::info!(
                "Resolved {} packages from Cargo.lock for {crate_label} (transitive graph)",
                names.len()
            );
            names.sort();
            names.dedup();
            names
        }
        Err(e) => {
            log::warn!(
                "Failed to parse {} for {crate_label}: {e}. Falling back to direct deps.",
                lock_path.display()
            );
            direct_deps.to_vec()
        }
    }
}

/// Set the crates.io source on the builder, downloading and extracting the archive
/// so the caller can inspect `Cargo.lock` for transitive dependency detection.
///
/// The crates.io-provided checksum is used when valid; otherwise the hash is
/// recomputed from the downloaded bytes. Returns the extracted source tree when
/// download + extraction succeeds, or `None` on any failure (the recipe is still
/// populated with the best checksum we have).
fn set_crates_io_source(
    builder: &mut RecipeBuilder,
    http_client: &reqwest::blocking::Client,
    dl_path: &str,
    checksum: &str,
) -> Option<source::ExtractedSource> {
    let url = format!("https://crates.io{dl_path}");
    match source::fetch_and_extract(http_client, &url) {
        Ok((computed, extracted)) => {
            let final_checksum = if source::is_valid_sha256(checksum) {
                if checksum != computed {
                    log::warn!(
                        "crates.io checksum '{checksum}' disagrees with computed '{computed}' \
                         for {dl_path}; using computed value."
                    );
                    &computed
                } else {
                    checksum
                }
            } else {
                log::warn!(
                    "Invalid SHA256 from crates.io for {dl_path} (got '{checksum}'); \
                     using computed value."
                );
                &computed
            };
            builder.crates_io_source(dl_path, final_checksum);
            Some(extracted)
        }
        Err(e) => {
            log::warn!(
                "Failed to download/extract crates.io archive for {dl_path}: {e}. \
                 Falling back to the crates.io-provided checksum without transitive dep detection."
            );
            if source::is_valid_sha256(checksum) {
                builder.crates_io_source(dl_path, checksum);
            } else {
                log::warn!(
                    "Invalid SHA256 from crates.io for {dl_path} (got '{checksum}') \
                     and download failed; recipe will have an invalid sha256."
                );
                builder.crates_io_source(dl_path, checksum);
            }
            None
        }
    }
}

#[allow(clippy::too_many_lines)]
fn redskull_from_opts(opts: &Opts) -> Result<()> {
    ensure!(!opts.args.is_empty(), "No packages given. Please specify at least one crate.");
    ensure!(!opts.recursive, "Recursive dependency resolution is not yet implemented.");

    let user_agent =
        format!("redskull/{} (https://github.com/fg-labs/redskull)", built_info::VERSION.as_str());
    let timeout = std::time::Duration::from_secs(30);
    let connect_timeout = std::time::Duration::from_secs(10);

    let crates_client = SyncClient::new(&user_agent, std::time::Duration::from_millis(1000))?;

    // Build HTTP client with timeouts and optional GitHub auth
    let mut http_headers = reqwest::header::HeaderMap::new();
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        http_headers.insert(
            "Authorization",
            format!("Bearer {token}").parse().expect("invalid GITHUB_TOKEN"),
        );
        log::info!("Using GitHub authentication (GITHUB_TOKEN set)");
    } else {
        log::debug!("No GITHUB_TOKEN set; using unauthenticated GitHub API (60 req/hr limit)");
    }
    let http_client = ReqwestClientBuilder::new()
        .user_agent(&user_agent)
        .default_headers(http_headers)
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .build()?;
    let conda_client = ReqwestClientBuilder::new()
        .user_agent(&user_agent)
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .build()?;
    let conda_channel = "conda-forge";

    for crate_name in &opts.args {
        log::info!("Processing crate: {crate_name}");

        // GitHub-only mode: if input is a GitHub URL, skip crates.io entirely
        if let Ok(repo) = GitHubRepo::from_url(crate_name) {
            process_github_only(
                &http_client,
                &conda_client,
                conda_channel,
                &repo,
                opts.github_release_tag.as_deref(),
                opts.crate_version.as_deref(),
                opts.recipe_name.as_deref(),
                opts,
            )?;
            continue;
        }

        let crate_data = crates_client.full_crate(crate_name, true)?;

        // Find a valid version (skip broken ones like 0.0.0)
        // Use --crate-version if specified, otherwise try max_version first
        let mut version_str =
            opts.crate_version.clone().unwrap_or_else(|| crate_data.max_version.clone());
        let mut version_idx: Option<usize> = None;
        let dep_list = loop {
            match crates_client.crate_dependencies(&crate_data.id, &version_str) {
                Ok(deps) => break deps,
                Err(e) => {
                    log::warn!(
                        "Could not fetch deps for {} v{}: {e}. Trying next version.",
                        crate_data.id,
                        version_str
                    );
                    let next_idx = version_idx.map_or(0, |i| i + 1);
                    if next_idx >= crate_data.versions.len() {
                        return Err(anyhow::anyhow!(
                            "No valid version found for {}",
                            crate_data.id
                        ));
                    }
                    version_idx = Some(next_idx);
                    version_str = crate_data.versions[next_idx].num.clone();
                }
            }
        };
        let version = &crate_data.versions[version_idx.unwrap_or(0)];
        log::info!("Resolved {} v{}", crate_data.id, version_str);
        let direct_dep_names: Vec<String> = dep_list.iter().map(|d| d.crate_id.clone()).collect();

        // Build recipe
        let recipe_name = opts.recipe_name.as_deref().unwrap_or(&crate_data.id);
        let mut builder = RecipeBuilder::new(recipe_name, &version_str);

        // Source URL + resolve GitHub repo info for Cargo.toml fetching.
        // We also capture an extracted source tree (from crates.io or GitHub) to
        // parse Cargo.lock for transitive dependency detection.
        let mut github_info: Option<(GitHubRepo, String)> = None;
        let extracted_source: Option<source::ExtractedSource> = if opts.source
            == SourceType::CratesIo
        {
            set_crates_io_source(&mut builder, &http_client, &version.dl_path, &version.checksum)
        } else if let Some(ref repo_url) = crate_data.repository {
            if let Ok(repo) = GitHubRepo::from_url(repo_url) {
                let tag_override = opts.github_release_tag.as_deref();
                match source::resolve_github_source(
                    &http_client,
                    &repo,
                    &version_str,
                    tag_override,
                    opts.refs_tags,
                ) {
                    Ok(mut resolved) => {
                        builder.github_source_resolved(&resolved.url_template, &resolved.sha256);
                        github_info = Some((repo, resolved.tag));
                        resolved.extracted.take()
                    }
                    Err(e) => {
                        log::warn!(
                            "Could not resolve GitHub archive for {}: {e}. \
                                 Falling back to crates.io.",
                            crate_data.id
                        );
                        set_crates_io_source(
                            &mut builder,
                            &http_client,
                            &version.dl_path,
                            &version.checksum,
                        )
                    }
                }
            } else {
                // Not a GitHub URL, fall back to crates.io
                set_crates_io_source(
                    &mut builder,
                    &http_client,
                    &version.dl_path,
                    &version.checksum,
                )
            }
        } else {
            set_crates_io_source(&mut builder, &http_client, &version.dl_path, &version.checksum)
        };

        // Resolve effective dep list (prefers Cargo.lock, falls back to direct deps).
        let source_root = extracted_source.as_ref().map(|e| e.root.as_path());
        let effective_deps = resolve_effective_deps(source_root, &direct_dep_names, &crate_data.id);
        let dep_names: Vec<&str> = effective_deps.iter().map(String::as_str).collect();

        // Detect host deps and compiler needs
        let host_deps = sys_deps::detect_host_deps(&dep_names);
        let has_c = sys_deps::needs_c_compiler(&dep_names);
        let has_cxx = sys_deps::needs_cxx_compiler(&dep_names);
        let has_bindgen = sys_deps::needs_bindgen(&dep_names);
        let pkg_config = sys_deps::needs_pkg_config(&dep_names);
        let make = sys_deps::needs_make(&dep_names);
        let cmake = sys_deps::needs_cmake(&dep_names);

        // Verify detected host deps exist on conda-forge
        verify_host_deps(&conda_client, conda_channel, &host_deps, &opts.host_dep);

        // Metadata
        if let Some(ref license) = crate_data.license {
            builder.license(license);
        }
        if let Some(ref summary) = crate_data.description {
            builder.summary(summary);
        }
        if let Some(ref homepage) = crate_data.homepage {
            builder.homepage(homepage);
        }
        if let Some(ref repo) = crate_data.repository {
            builder.repository(repo);
        }
        if let Some(ref docs) = crate_data.documentation {
            builder.documentation(docs);
        } else if let Some((ref repo, ref tag)) = github_info {
            // Fall back to repo README for doc_url, with `{{ version }}` jinja placeholder
            builder.documentation(&build_readme_doc_url(repo, tag, &version_str));
        }

        // Binary names + workspace detection from GitHub Cargo.toml
        if let Some((ref repo, ref tag)) = github_info {
            match source::fetch_github_raw(&http_client, repo, tag, "Cargo.toml") {
                Ok(cargo_toml) => match CargoMetadata::from_toml_str(&cargo_toml) {
                    Ok(root_meta) => {
                        if root_meta.is_workspace() && !root_meta.has_package() {
                            // Workspace-only root: resolve glob patterns and find matching member
                            let raw_members = root_meta.workspace_members();
                            let has_globs = raw_members.iter().any(|m| m.contains('*'));
                            let members = if has_globs {
                                let tree = source::fetch_github_tree(&http_client, repo, tag)
                                    .unwrap_or_default();
                                resolve_workspace_members(&raw_members, &tree)
                            } else {
                                raw_members
                            };
                            // First pass: match by package name
                            let mut all_members: Vec<(String, Vec<String>)> = Vec::new();
                            let mut found: Option<(String, Vec<String>)> = None;
                            for member in &members {
                                let path = format!("{member}/Cargo.toml");
                                let Some(toml) =
                                    source::fetch_github_raw(&http_client, repo, tag, &path).ok()
                                else {
                                    continue;
                                };
                                let Some(meta) = CargoMetadata::from_toml_str(&toml).ok() else {
                                    continue;
                                };
                                if meta.package_name().as_deref() == Some(&*crate_data.id) {
                                    found = Some((member.clone(), meta.binary_names()));
                                    break;
                                }
                                all_members.push((member.clone(), meta.binary_names()));
                            }
                            // Second pass: match by binary name
                            if found.is_none() {
                                found = all_members
                                    .into_iter()
                                    .find(|(_, bins)| bins.iter().any(|b| b == &crate_data.id));
                            }
                            if let Some((member_path, bins)) = found {
                                builder.workspace_path(&member_path);
                                for bin in &bins {
                                    builder.add_binary(bin);
                                }
                            } else {
                                log::warn!(
                                    "Workspace has no member matching '{}'. \
                                     You may need to set workspace_path manually.",
                                    crate_data.id
                                );
                                builder.add_binary(&crate_data.id);
                            }
                        } else {
                            // Standard crate or workspace with root package
                            for bin in &root_meta.binary_names() {
                                builder.add_binary(bin);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse Cargo.toml: {e}");
                        builder.add_binary(&crate_data.id);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to fetch Cargo.toml from GitHub: {e}");
                    builder.add_binary(&crate_data.id);
                }
            }
        } else {
            builder.add_binary(&crate_data.id);
        }

        // Scan source tree for runtime deps and license files
        if let Some((ref repo, ref tag)) = github_info {
            match source::fetch_github_tree(&http_client, repo, tag) {
                Ok(files) => {
                    // Runtime dep warnings
                    let hints = runtime_deps::detect_runtime_hints(&files);
                    for hint in &hints {
                        log::warn!(
                            "Potential run dependency: {} ({}). Consider adding: --host-dep {}",
                            hint.package,
                            hint.reason,
                            hint.package,
                        );
                    }

                    // License file detection
                    let license_files = detect_license_files(&files);
                    if !license_files.is_empty() {
                        builder.license_files(license_files);
                    }
                }
                Err(e) => {
                    log::debug!("Could not fetch repo tree for runtime dep detection: {e}");
                }
            }
        }

        // Maintainers
        let authors: crates_io_api::Authors =
            crates_client.crate_authors(crate_name, &version_str)?;
        let names = if authors.names.is_empty() {
            let owners = crates_client.crate_owners(crate_name)?;
            owners.into_iter().map(|user| user.login).collect()
        } else {
            authors.names
        };
        // CLI --maintainers overrides crates.io-derived authors when non-empty.
        if opts.maintainers.is_empty() {
            for name in names.into_iter().filter(|n| !n.starts_with("github:")) {
                builder.add_maintainer(&name);
            }
        } else {
            for name in &opts.maintainers {
                builder.add_maintainer(name);
            }
        }

        // Build flags
        let has_host_deps = !host_deps.is_empty() || !opts.host_dep.is_empty();
        let use_cbl = opts.cargo_bundle_licenses.unwrap_or(opts.bioconda);
        builder
            .bioconda(opts.bioconda)
            .cargo_bundle_licenses(use_cbl)
            .has_c_deps(has_c)
            .has_cxx_deps(has_cxx)
            .has_native_deps(has_host_deps)
            .needs_bindgen(has_bindgen)
            .needs_pkg_config(pkg_config)
            .needs_make(make)
            .needs_cmake(cmake)
            .cargo_net_git_fetch(opts.cargo_net_git_fetch)
            .strip_binaries(opts.strip)
            .use_version_test(opts.test_version);

        if let Some(ref pin) = opts.max_pin {
            builder.max_pin(pin);
        }
        if let Some(emit) = opts.license_family {
            builder.emit_license_family(emit);
        }
        if let Some(ref ws_path) = opts.workspace_path {
            builder.workspace_path(ws_path);
        }

        // Detected host deps
        for (dep, selector) in &host_deps {
            builder.add_host_dep(dep, *selector);
        }

        // CLI overrides
        for dep in &opts.host_dep {
            builder.add_host_dep(dep, None);
        }
        for dep in &opts.run_dep {
            builder.add_run_dep(dep, None);
        }
        for platform in &opts.skip_platform {
            builder.skip_platform(platform);
        }
        for cmd in &opts.test_command {
            builder.add_test_command(cmd);
        }
        for id in &opts.identifier {
            builder.add_identifier(id);
        }

        // Build and render
        let (recipe, script) = builder.build();
        output_recipe(&recipe, &script, opts.output.as_deref())?;
    }

    Ok(())
}

/// Render and output a recipe, writing to files or stdout.
fn output_recipe(
    recipe: &redskull_lib::recipe::Recipe,
    script: &redskull_lib::build_script::BuildScript,
    output_dir: Option<&std::path::Path>,
) -> Result<()> {
    let renderer = MetaYamlRenderer;
    let meta_yaml = renderer.render(recipe);

    if let Some(output_dir) = output_dir {
        std::fs::create_dir_all(output_dir)?;
        let meta_path = output_dir.join("meta.yaml");
        std::fs::write(&meta_path, &meta_yaml)?;
        log::info!("Wrote {}", meta_path.display());
        if script.needs_build_sh() {
            let build_path = output_dir.join("build.sh");
            std::fs::write(&build_path, script.to_build_sh())?;
            log::info!("Wrote {}", build_path.display());
        }
    } else {
        print!("{meta_yaml}");
        if script.needs_build_sh() {
            println!("---");
            println!("# build.sh");
            print!("{}", script.to_build_sh());
        }
    }
    Ok(())
}

/// Process a GitHub-only crate (not on crates.io).
/// Uses GraphQL to fetch releases, tags, tree, and Cargo.toml in 1-2 API calls.
/// Falls back to REST API if GraphQL is unavailable (e.g., no auth token).
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn process_github_only(
    http_client: &reqwest::blocking::Client,
    conda_client: &reqwest::blocking::Client,
    conda_channel: &str,
    repo: &GitHubRepo,
    tag_override: Option<&str>,
    version_override: Option<&str>,
    recipe_name_override: Option<&str>,
    opts: &Opts,
) -> Result<()> {
    // Determine tag — need it before the main GraphQL query
    let tag = match tag_override {
        Some(t) => t.to_string(),
        None => {
            log::info!("Detecting latest version via GraphQL...");
            // Use a discovery query with a placeholder tag for releases/tags only,
            // then re-query with the resolved tag for tree + files
            let pre_discovery = github_graphql::discover_repo(http_client, repo, "HEAD").ok();
            if let Some(ref disc) = pre_discovery {
                if let Some(best) = github_graphql::best_version_tag(disc) {
                    best
                } else {
                    log::debug!("GraphQL found no version-like tags; falling back to REST");
                    source::latest_github_release(http_client, repo)?
                }
            } else {
                log::debug!("GraphQL unavailable; falling back to REST");
                source::latest_github_release(http_client, repo)?
            }
        }
    };
    let version_str =
        version_override.map(String::from).unwrap_or_else(|| source::tag_to_version(&tag));
    log::info!("Using tag '{tag}' (version {version_str})");

    // Main GraphQL query: tree + root Cargo.toml at the resolved tag
    log::info!("Fetching repo metadata via GraphQL...");
    let discovery = github_graphql::discover_repo(http_client, repo, &tag);

    // Resolve source archive and SHA256 (must stay REST — downloads the tarball).
    // This also extracts the archive so we can inspect `Cargo.lock` for transitive deps.
    let mut resolved =
        source::resolve_github_source(http_client, repo, &version_str, Some(&tag), opts.refs_tags)?;
    let extracted_source = resolved.extracted.take();

    // Extract root Cargo.toml from GraphQL or fall back to REST
    let root_toml_str = if let Ok(ref disc) = discovery {
        if let Some(ref toml) = disc.root_cargo_toml {
            toml.clone()
        } else {
            source::fetch_github_raw(http_client, repo, &tag, "Cargo.toml")?
        }
    } else {
        log::debug!("GraphQL discovery failed; fetching Cargo.toml via REST");
        source::fetch_github_raw(http_client, repo, &tag, "Cargo.toml")?
    };
    let root_meta = CargoMetadata::from_toml_str(&root_toml_str)?;

    // Determine which Cargo.toml has the package metadata
    let (pkg_meta, workspace_path) = if root_meta.is_workspace() && !root_meta.has_package() {
        // Need to find the right member crate
        let target_name = recipe_name_override.unwrap_or(&repo.name);
        let raw_members = root_meta.workspace_members();
        let has_globs = raw_members.iter().any(|m| m.contains('*'));
        let members = if has_globs {
            let tree = if let Ok(ref disc) = discovery {
                disc.tree.clone()
            } else {
                source::fetch_github_tree(http_client, repo, &tag).unwrap_or_default()
            };
            resolve_workspace_members(&raw_members, &tree)
        } else {
            raw_members
        };

        // Batch-fetch all member Cargo.tomls via GraphQL
        let member_paths: Vec<String> = members.iter().map(|m| format!("{m}/Cargo.toml")).collect();
        let fetched =
            github_graphql::fetch_files(http_client, repo, &tag, &member_paths).unwrap_or_default();

        // First pass: match by package name
        let mut all_members: Vec<(String, CargoMetadata)> = Vec::new();
        let mut found: Option<(CargoMetadata, Option<String>)> = None;
        for (i, member) in members.iter().enumerate() {
            let toml_str = fetched.iter().find(|(p, _)| *p == member_paths[i]).map(|(_, c)| c);
            let Some(toml_str) = toml_str else {
                // GraphQL didn't return this file; try REST as fallback
                let path = &member_paths[i];
                if let Ok(toml) = source::fetch_github_raw(http_client, repo, &tag, path) {
                    if let Ok(meta) = CargoMetadata::from_toml_str(&toml) {
                        if meta.package_name().as_deref() == Some(target_name) {
                            found = Some((meta, Some(member.clone())));
                            break;
                        }
                        all_members.push((member.clone(), meta));
                    }
                }
                continue;
            };
            let Some(meta) = CargoMetadata::from_toml_str(toml_str).ok() else {
                continue;
            };
            if meta.package_name().as_deref() == Some(target_name) {
                found = Some((meta, Some(member.clone())));
                break;
            }
            all_members.push((member.clone(), meta));
        }
        // Second pass: match by binary name if package name didn't match
        if found.is_none() {
            found = all_members.into_iter().find_map(|(member, meta)| {
                let bins = meta.binary_names();
                if bins.iter().any(|b| b == target_name) {
                    log::info!(
                        "Matched workspace member '{member}' by binary name '{target_name}' \
                         (package: {:?})",
                        meta.package_name()
                    );
                    Some((meta, Some(member)))
                } else {
                    None
                }
            });
        }
        match found {
            Some((meta, ws_path)) => (meta, ws_path),
            None => {
                return Err(anyhow::anyhow!(
                    "Workspace has no member matching '{target_name}'. \
                     Use --recipe-name to specify which crate to build."
                ));
            }
        }
    } else {
        (root_meta, None)
    };

    let ws_root_meta_str = if workspace_path.is_some() {
        Some(CargoMetadata::from_toml_str(&root_toml_str)?)
    } else {
        None
    };
    let ws_ref = ws_root_meta_str.as_ref();

    // Build recipe
    let recipe_name = recipe_name_override
        .map(String::from)
        .or_else(|| pkg_meta.package_name())
        .unwrap_or_else(|| repo.name.clone());
    let mut builder = RecipeBuilder::new(&recipe_name, &version_str);
    builder.github_source_resolved(&resolved.url_template, &resolved.sha256);

    // Metadata from Cargo.toml
    if let Some(license) = pkg_meta.license(ws_ref) {
        builder.license(&license);
    }
    if let Some(desc) = pkg_meta.description(ws_ref) {
        builder.summary(&desc);
    }
    if let Some(homepage) = pkg_meta.homepage(ws_ref) {
        builder.homepage(&homepage);
    }
    if let Some(repo_url) = pkg_meta.repository(ws_ref) {
        builder.repository(&repo_url);
    }
    if let Some(doc_url) = pkg_meta.documentation(ws_ref) {
        builder.documentation(&doc_url);
    } else {
        // Fall back to repo README for doc_url, with `{{ version }}` jinja placeholder
        builder.documentation(&build_readme_doc_url(repo, &tag, &version_str));
    }

    // Workspace path
    if let Some(ref ws_path) = workspace_path {
        builder.workspace_path(ws_path);
    }

    // Binary names
    let bins = pkg_meta.binary_names();
    for bin in &bins {
        builder.add_binary(bin);
    }

    // Dependencies -> detect host deps and compiler needs.
    // Prefer Cargo.lock (the full resolved graph) to catch transitive `-sys` crates;
    // fall back to direct Cargo.toml deps when no lockfile is available.
    let direct_deps: Vec<String> = pkg_meta
        .dependencies()
        .into_iter()
        .map(|(name, _)| name)
        .chain(pkg_meta.build_dependencies())
        .collect();
    let source_root = extracted_source.as_ref().map(|e| e.root.as_path());
    let effective_deps = resolve_effective_deps(source_root, &direct_deps, &recipe_name);
    let dep_names: Vec<&str> = effective_deps.iter().map(String::as_str).collect();
    let host_deps = sys_deps::detect_host_deps(&dep_names);
    let has_c = sys_deps::needs_c_compiler(&dep_names);
    let has_cxx = sys_deps::needs_cxx_compiler(&dep_names);
    let has_bindgen = sys_deps::needs_bindgen(&dep_names);
    let pkg_config = sys_deps::needs_pkg_config(&dep_names);
    let make = sys_deps::needs_make(&dep_names);
    let cmake = sys_deps::needs_cmake(&dep_names);

    // Verify detected host deps exist on conda-forge
    verify_host_deps(conda_client, conda_channel, &host_deps, &opts.host_dep);

    // Scan source tree for runtime deps and license files (from GraphQL discovery or REST)
    let files = if let Ok(ref disc) = discovery {
        disc.tree.clone()
    } else {
        source::fetch_github_tree(http_client, repo, &tag).unwrap_or_default()
    };
    if !files.is_empty() {
        let hints = runtime_deps::detect_runtime_hints(&files);
        for hint in &hints {
            log::warn!(
                "Potential run dependency: {} ({}). Consider adding: --host-dep {}",
                hint.package,
                hint.reason,
                hint.package,
            );
        }
        let license_files = detect_license_files(&files);
        if !license_files.is_empty() {
            builder.license_files(license_files);
        }
    }

    // Build flags
    let has_host_deps = !host_deps.is_empty() || !opts.host_dep.is_empty();
    let use_cbl = opts.cargo_bundle_licenses.unwrap_or(opts.bioconda);
    builder
        .bioconda(opts.bioconda)
        .cargo_bundle_licenses(use_cbl)
        .has_c_deps(has_c)
        .has_cxx_deps(has_cxx)
        .has_native_deps(has_host_deps)
        .needs_bindgen(has_bindgen)
        .needs_pkg_config(pkg_config)
        .needs_make(make)
        .needs_cmake(cmake)
        .cargo_net_git_fetch(opts.cargo_net_git_fetch)
        .strip_binaries(opts.strip)
        .use_version_test(opts.test_version);

    if let Some(ref pin) = opts.max_pin {
        builder.max_pin(pin);
    }
    if let Some(emit) = opts.license_family {
        builder.emit_license_family(emit);
    }

    // CLI workspace path override takes precedence over auto-detected
    if let Some(ref ws_path) = opts.workspace_path {
        builder.workspace_path(ws_path);
    }

    for (dep, selector) in &host_deps {
        builder.add_host_dep(dep, *selector);
    }
    for dep in &opts.host_dep {
        builder.add_host_dep(dep, None);
    }
    for dep in &opts.run_dep {
        builder.add_run_dep(dep, None);
    }
    for platform in &opts.skip_platform {
        builder.skip_platform(platform);
    }
    for cmd in &opts.test_command {
        builder.add_test_command(cmd);
    }
    for id in &opts.identifier {
        builder.add_identifier(id);
    }
    for name in &opts.maintainers {
        builder.add_maintainer(name);
    }

    let (recipe, script) = builder.build();
    output_recipe(&recipe, &script, opts.output.as_deref())
}

/// Parse args and set up logging / tracing
fn setup() -> Opts {
    if std::env::var("RUST_LOG").is_err() {
        // SAFETY: Called from main() before any threads are spawned.
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
    }
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    Opts::parse()
}
