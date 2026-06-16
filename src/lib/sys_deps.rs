//! Mapping from Rust crate dependencies to conda host packages.
//!
//! Includes:
//! - Direct `-sys` crate -> conda package mappings
//! - "Bundle" mappings for wrapper crates that pull multiple system deps
//! - Platform selector annotations (e.g., openssl excluded on macOS)
//! - Build tool detection (C/C++ compilers, pkg-config, make, cmake, clangdev)

/// Returns (conda_package_name, optional_platform_selector) pairs
/// for a given Rust crate name. Returns empty vec for unknown crates.
///
/// Only crates that dynamically link a system shared library belong here. The
/// compression `-sys` crates (libdeflate-sys, zstd-sys, ...) deliberately do
/// *not* appear: they vendor their C sources and statically link by default,
/// so they need a C compiler but no conda host/run dependency. See
/// [`is_vendored_static_sys_crate`].
pub fn map_sys_crate(crate_name: &str) -> Vec<(&'static str, Option<&'static str>)> {
    match crate_name {
        // TLS/crypto — openssl excluded on macOS (uses system SSL)
        "openssl-sys" => vec![("openssl", Some("not osx"))],

        // Networking
        "curl-sys" | "libcurl-sys" => vec![("libcurl", None)],

        // Databases
        "libsqlite3-sys" => vec![("sqlite", None)],

        // Math/science
        "gsl-sys" => vec![("gsl", None)],
        "blas-sys" | "openblas-sys" => vec![("openblas", None)],
        "cblas-sys" => vec![("libcblas", None)],
        "lapack-sys" => vec![("liblapack", None)],

        // Version control
        "libgit2-sys" => vec![("libgit2", None)],

        // Protobuf
        "protobuf-src" | "protoc-grpcio" => vec![("protobuf", None)],

        // Bundle: rust-htslib pulls in htslib + its transitive deps
        "rust-htslib" | "hts-sys" => vec![
            ("htslib", None),
            ("zlib", None),
            ("bzip2", None),
            ("xz", None),
            ("libdeflate", None),
            ("libcurl", None),
        ],

        _ => vec![],
    }
}

/// Given a list of dependency crate names, returns all conda
/// host packages needed, with their platform selectors.
/// Deduplicates by package name.
pub fn detect_host_deps(dependency_names: &[&str]) -> Vec<(&'static str, Option<&'static str>)> {
    let mut deps: Vec<(&str, Option<&str>)> =
        dependency_names.iter().flat_map(|name| map_sys_crate(name)).collect();
    deps.sort_by_key(|(name, _)| *name);
    deps.dedup_by_key(|(name, _)| *name);
    deps
}

/// Crates that require a C compiler but no external host dependency
/// (they bundle and statically link their own C sources).
const C_ONLY_CRATES: &[&str] = &["mimalloc", "libmimalloc-sys"];

/// `-sys` crates that vendor their C sources and statically link them by
/// default. They need a C compiler at build time but *not* a conda host/run
/// dependency, `pkg-config`, or `make`/`cmake` — the bundled sources are
/// compiled directly via the `cc` crate.
///
/// These are kept separate from [`map_sys_crate`] because emitting a host/run
/// dep for them would add a runtime dependency the binary never dynamically
/// loads, plus over-broad `run_exports` pinning, to the generated recipe.
const VENDORED_STATIC_SYS_CRATES: &[&str] =
    &["libdeflate-sys", "libz-sys", "libz-ng-sys", "bzip2-sys", "lzma-sys", "lz4-sys", "zstd-sys"];

/// Returns true if `crate_name` vendors its C sources and statically links by
/// default (so it needs only a C compiler, no system host library).
pub fn is_vendored_static_sys_crate(crate_name: &str) -> bool {
    VENDORED_STATIC_SYS_CRATES.contains(&crate_name)
}

/// Returns true if any dependency links C code.
/// Detects `-sys` crates, the `cc` build crate, and crates that bundle their own C sources.
pub fn needs_c_compiler(dependency_names: &[&str]) -> bool {
    dependency_names.iter().any(|name| {
        *name == "cc"
            || !map_sys_crate(name).is_empty()
            || C_ONLY_CRATES.contains(name)
            || is_vendored_static_sys_crate(name)
    })
}

/// Returns true if any dependency requires a C++ compiler.
/// Detects the `cxx` bridge crate, `cxx-build`, `cpp`, and known C++-requiring `-sys` crates.
pub fn needs_cxx_compiler(dependency_names: &[&str]) -> bool {
    const CXX_CRATES: &[&str] = &[
        "cxx",
        "cxx-build",
        "cpp",
        "cmake",
        "cmake-build",
        "rocksdb",
        "rocksdb-sys",
        "snappy-sys",
        "leveldb-sys",
        "grpcio-sys",
        "protobuf-src",
        "rust-htslib",
        "hts-sys",
    ];
    dependency_names.iter().any(|name| CXX_CRATES.contains(name))
}

/// Returns true if any dependency uses bindgen for FFI.
pub fn needs_bindgen(dependency_names: &[&str]) -> bool {
    dependency_names.iter().any(|name| *name == "bindgen")
}

/// Returns true if any dependency typically needs pkg-config to locate system libraries.
pub fn needs_pkg_config(dependency_names: &[&str]) -> bool {
    // Vendored, static-by-default `-sys` crates (see VENDORED_STATIC_SYS_CRATES)
    // are intentionally absent: they compile their own C sources and never need
    // pkg-config to locate a system library.
    const PKG_CONFIG_CRATES: &[&str] = &[
        "openssl-sys",
        "curl-sys",
        "libcurl-sys",
        "libsqlite3-sys",
        "gsl-sys",
        "blas-sys",
        "openblas-sys",
        "cblas-sys",
        "lapack-sys",
        "libgit2-sys",
        "hts-sys",
        "rust-htslib",
    ];
    dependency_names.iter().any(|name| PKG_CONFIG_CRATES.contains(name))
}

/// Returns true if any dependency typically needs cmake to build.
pub fn needs_cmake(dependency_names: &[&str]) -> bool {
    // zstd-sys is intentionally excluded: it vendors its C sources and compiles
    // them via the `cc` crate by default, so it needs neither cmake nor a system
    // zstd (see VENDORED_STATIC_SYS_CRATES).
    const CMAKE_CRATES: &[&str] =
        &["grpcio-sys", "rocksdb-sys", "snappy-sys", "leveldb-sys", "libssh2-sys"];
    dependency_names
        .iter()
        .any(|name| CMAKE_CRATES.contains(name) || *name == "cmake" || *name == "cmake-build")
}

/// Returns true if any dependency typically needs make to build.
pub fn needs_make(dependency_names: &[&str]) -> bool {
    // Most -sys crates use make; this is the broadest heuristic
    dependency_names.iter().any(|name| !map_sys_crate(name).is_empty())
}
