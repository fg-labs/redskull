//! Mapping from Rust crate dependencies to conda host packages.
//!
//! Includes:
//! - Direct `-sys` crate -> conda package mappings
//! - "Bundle" mappings for wrapper crates that pull multiple system deps
//! - Platform selector annotations (e.g., openssl excluded on macOS)
//! - Build tool detection (C/C++ compilers, pkg-config, make, cmake, clangdev)

/// Returns (conda_package_name, optional_platform_selector) pairs
/// for a given Rust crate name. Returns empty vec for unknown crates.
pub fn map_sys_crate(crate_name: &str) -> Vec<(&'static str, Option<&'static str>)> {
    match crate_name {
        // Compression
        "libz-sys" | "libz-ng-sys" => vec![("zlib", None)],
        "bzip2-sys" => vec![("bzip2", None)],
        "lzma-sys" => vec![("xz", None)],
        "libdeflate-sys" => vec![("libdeflate", None)],
        "lz4-sys" => vec![("lz4", None)],
        "zstd-sys" => vec![("zstd", None)],

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

/// Returns true if any dependency links C code.
/// Detects both `-sys` crates and the `cc` build crate.
pub fn needs_c_compiler(dependency_names: &[&str]) -> bool {
    dependency_names.iter().any(|name| *name == "cc" || !map_sys_crate(name).is_empty())
}

/// Returns true if any dependency requires a C++ compiler.
/// Detects the `cxx` bridge crate, `cxx-build`, `cpp`, and known C++-requiring `-sys` crates.
pub fn needs_cxx_compiler(dependency_names: &[&str]) -> bool {
    const CXX_CRATES: &[&str] = &[
        "cxx",
        "cxx-build",
        "cpp",
        "mimalloc",
        "libmimalloc-sys",
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
        "libgit2-sys",
    ];
    dependency_names.iter().any(|name| CXX_CRATES.contains(name))
}

/// Returns true if any dependency uses bindgen for FFI.
pub fn needs_bindgen(dependency_names: &[&str]) -> bool {
    dependency_names.iter().any(|name| *name == "bindgen")
}

/// Returns true if any dependency typically needs pkg-config to locate system libraries.
pub fn needs_pkg_config(dependency_names: &[&str]) -> bool {
    const PKG_CONFIG_CRATES: &[&str] = &[
        "openssl-sys",
        "libz-sys",
        "libz-ng-sys",
        "bzip2-sys",
        "lzma-sys",
        "libdeflate-sys",
        "lz4-sys",
        "zstd-sys",
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
    const CMAKE_CRATES: &[&str] = &[
        "grpcio-sys",
        "rocksdb-sys",
        "snappy-sys",
        "leveldb-sys",
        "libgit2-sys",
        "libssh2-sys",
        "zstd-sys",
    ];
    dependency_names
        .iter()
        .any(|name| CMAKE_CRATES.contains(name) || *name == "cmake" || *name == "cmake-build")
}

/// Returns true if any dependency typically needs make to build.
pub fn needs_make(dependency_names: &[&str]) -> bool {
    // Most -sys crates use make; this is the broadest heuristic
    dependency_names.iter().any(|name| !map_sys_crate(name).is_empty())
}
