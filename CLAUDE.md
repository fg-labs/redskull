# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is redskull?

redskull is a conda recipe generator for Rust crates — "`grayskull` for Rust". Given a crates.io package name or GitHub URL, it queries the relevant APIs, detects system dependencies, and outputs a conda `meta.yaml` recipe and `build.sh` script.

## Build & Development Commands

```bash
# Build (debug)
cargo build

# Build (release, with LTO)
cargo build --release

# Run all checks before committing (format, clippy, tests)
./ci/check.sh

# Individual checks (cargo aliases defined in .cargo/config.toml)
cargo ci-fmt                       # format check
cargo ci-lint                      # clippy with -D warnings
cargo ci-test                      # tests with --locked

# Run a single test
cargo test <test_name>

# Run the tool
cargo run -- <crate_name> [options]
cargo run -- https://github.com/owner/repo [options]  # GitHub-only mode
```

## Architecture

The project is a single-crate Rust project with a library (`redskull_lib`) and binary (`redskull`).

### Library modules (`src/lib/`)
- **`mod.rs`** — Module exports
- **`recipe.rs`** — Data model: `Recipe`, `Source`, `Build`, `Requirements`, `Test`, `About`, `Extra`
- **`recipe_builder.rs`** — Builder pattern (`&mut self` setters, consuming `build()` returns `(Recipe, BuildScript)`)
- **`renderer.rs`** — `Renderer` trait + `MetaYamlRenderer` for conda meta.yaml output
- **`build_script.rs`** — `BuildScript` for inline script or `build.sh` generation
- **`source.rs`** — GitHub/crates.io source URL resolution, SHA256 computation, release tag detection
- **`crate_inspector.rs`** — Cargo.toml parsing: binary names, workspace detection, dependency extraction, license files
- **`sys_deps.rs`** — `-sys` crate → conda package mapping (openssl, zlib, htslib, etc.)
- **`runtime_deps.rs`** — R/Python runtime dependency detection from file trees
- **`license_family.rs`** — SPDX → conda license family mapping
- **`conda.rs`** — Conda channel package availability checking and name normalization

### Binary (`src/bin/main.rs`)
- CLI entry point using `clap` with `ValueEnum` for `--source`
- Two code paths: crates.io mode (default) and GitHub-only mode (when input is a URL)
- Supports `GITHUB_TOKEN` env var for authenticated GitHub API access
- HTTP request timeouts (30s request, 10s connect)

### Tests (`tests/`)
- **`tier1.rs`** — Unit tests (renderer, builder, parser, SHA256, workspace globs, etc.)
- **`tier2.rs`** — Integration smoke tests + `#[ignore]` network tests
- **`tier3.rs`** — System dependency mapping tests
- **`common/mod.rs`** — Test utilities

## Key Dependencies

- **`crates_io_api`** — Query crates.io API for package metadata
- **`clap`** — CLI argument parsing with derive macros
- **`reqwest`** (blocking) — HTTP client for GitHub API and conda channel checks
- **`toml`** — Parse Cargo.toml files
- **`sha2`** — SHA256 hash computation
- **`serde_json`** — GitHub API JSON parsing
- **`mimalloc`** — Global allocator override

## Rust Toolchain

- Pinned to Rust **1.85.0** via `rust-toolchain.toml` with `rustfmt` and `clippy` components.
- Edition **2024**.
- `rustfmt.toml`: `max_width = 100`, `use_small_heuristics = "max"`.

## CI

The GitHub Actions workflow (`.github/workflows/tests.yml`) runs on push/PR:
1. `cargo ci-test` on Ubuntu and macOS
2. `cargo ci-lint` (clippy)
3. `cargo ci-fmt` (rustfmt)
4. `.github/scripts/update-docs.sh` — updates CLI usage in README.md; CI verifies no unstaged changes
