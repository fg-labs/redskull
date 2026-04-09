# redskull

<p align="center">
  <img src="images/logo.svg" alt="redskull logo" width="400">
</p>

<p align="center">
  <a href="https://github.com/fg-labs/redskull/actions/workflows/tests.yml"><img src="https://github.com/fg-labs/redskull/actions/workflows/tests.yml/badge.svg?branch=main" alt="Build Status"></a>
  <img src="https://img.shields.io/crates/l/redskull.svg" alt="license">
  <a href="https://crates.io/crates/redskull"><img src="https://img.shields.io/crates/v/redskull.svg?colorB=319e8c" alt="Version info"></a>
  <a href="http://bioconda.github.io/recipes/redskull/README.html"><img src="https://img.shields.io/badge/install%20with-bioconda-brightgreen.svg?style=flat" alt="Install with bioconda"></a>
  <br>
</p>

A conda recipe generator for Rust crates, written in Rust.

<p>
<a href="https://fulcrumgenomics.com">
<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/logos/fulcrumgenomics-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset=".github/logos/fulcrumgenomics-light.svg">
  <img alt="Fulcrum Genomics" src=".github/logos/fulcrumgenomics-light.svg" height="100">
</picture>
</a>
</p>

[Visit us at Fulcrum Genomics](https://www.fulcrumgenomics.com) to learn more about how we can power your Bioinformatics with redskull and beyond.

<a href="mailto:contact@fulcrumgenomics.com?subject=[GitHub inquiry]"><img src="https://img.shields.io/badge/Email_us-%2338b44a.svg?&style=for-the-badge&logo=gmail&logoColor=white"/></a>
<a href="https://www.fulcrumgenomics.com"><img src="https://img.shields.io/badge/Visit_Us-%2326a8e0.svg?&style=for-the-badge&logo=wordpress&logoColor=white"/></a>

`redskull`: "`grayskull` for Rust".

`redskull` is a conda recipe generator for rust recipes.

Usage for `redskull` follows:

<!-- start usage -->
```console

OVERVIEW

redskull: grayskull for rust.

Specify the crates.io packages name. Redskull can also accept a github url.

Usage: redskull [OPTIONS] [ARGS]...

Arguments:
  [ARGS]...
          Crates.io package names or GitHub URLs

Options:
      --maintainers <MAINTAINERS>
          List of maintainers which will be added to the recipe

      --output <OUTPUT>
          Path to where the recipe will be created

  -r, --recursive
          Recursively run grayskull on missing dependencies

      --tag <GITHUB_RELEASE_TAG>
          If tag is specified, Redskull will build from release tag

      --crate-version <CRATE_VERSION>
          Override the crate version (defaults to the latest version on crates.io)

      --bioconda
          Add additional output for bioconda compatibility

      --cargo-bundle-licenses <CARGO_BUNDLE_LICENSES>
          Use cargo-bundle-licenses to generate THIRDPARTY.yml. Defaults to true when --bioconda is set, unless explicitly disabled

          [possible values: true, false]

      --host-dep <HOST_DEP>
          Override: additional host dependencies (e.g., --host-dep zlib --host-dep openssl)

      --run-dep <RUN_DEP>
          Override: additional run dependencies (e.g., --run-dep samtools --run-dep minimap2)

      --test-command <TEST_COMMAND>
          Override: test commands (e.g., --test-command "mytool --version")

      --skip-platform <SKIP_PLATFORM>
          Override: skip platforms (e.g., --skip-platform osx)

      --recipe-name <RECIPE_NAME>
          Override the recipe name (defaults to the crates.io package name). Useful when the bioconda recipe name differs from the crate name

      --source <SOURCE>
          Source type: "github" (default) or "crates-io"

          [default: github]

          Possible values:
          - github:    Use GitHub release archives (default)
          - crates-io: Use crates.io tarballs

      --max-pin <MAX_PIN>
          Override the max_pin value for run_exports (default: "x.x"). Use "x" for pre-1.0 tools

      --refs-tags
          Use refs/tags/ in GitHub archive URL template. Both forms resolve identically on GitHub; this controls the template text

      --cargo-net-git-fetch
          Set CARGO_NET_GIT_FETCH_WITH_CLI=true in build.sh. Needed when the build environment requires SSH for git fetches

      --test-version
          Use --version instead of --help for auto-generated test commands

      --strip
          Strip debug symbols from binaries after build

      --license-family <LICENSE_FAMILY>
          Emit license_family in the recipe. Defaults to true

          [possible values: true, false]

      --identifier <IDENTIFIER>
          Add identifiers to the extra section (e.g., --identifier "doi:10.1234/foo")

      --workspace-path <WORKSPACE_PATH>
          Override workspace member path (e.g., --workspace-path crates/cmdline)

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
<!-- end usage -->

## Installing

### Installing with `conda`
To install with conda you must first [install conda](https://conda.io/projects/conda/en/latest/user-guide/install/index.html#installation).
Then, in your command line (and with the environment you wish to install redskull into active) run:

```console
conda install -c bioconda redskull
```

### Installing with `cargo`
To install with cargo you must first [install rust](https://doc.rust-lang.org/cargo/getting-started/installation.html).
Which (On Mac OS and Linux) can be done with the command:

```console
curl https://sh.rustup.rs -sSf | sh
```

Then, to install `redskull` run:

```console
cargo install redskull
```

### Building From Source

First, clone the git repo:

```console
git clone https://github.com/fg-labs/redskull.git
```

Secondly, if you do not already have rust development tools installed, install via [rustup](https://rustup.rs/):

```console
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then build the toolkit in release mode:

```console
cd redskull
cargo build --release
./target/release/redskull --help
```

## Developing

redskull is developed in Rust and follows the conventions of using `rustfmt` and `clippy` to ensure both code quality and standardized formatting.
When working on redskull, before pushing any commits, please first run `./ci/check.sh` and resolve any issues that are reported.

## Releasing a New Version

Releases are automated with [`release-plz`][release-plz-link], driven by [Conventional Commits][conventional-commits-link] on `main`.

### How it works

On every push to `main`, the `release-plz` workflow (`.github/workflows/release-plz.yml`) runs and does two things:

1. **Opens or updates a release PR** that bumps the version in `Cargo.toml` and `Cargo.lock` and updates `CHANGELOG.md` based on commit messages since the last tag.
2. **Publishes to crates.io and creates a GitHub release** when a release PR is merged, tagging the commit and pushing the crate to crates.io.

### Conventional Commits

The version bump is derived from commit messages since the last release:

* `feat:` — minor bump (new functionality)
* `fix:` — patch bump (bug fix)
* `feat!:` or `BREAKING CHANGE:` — major bump (incompatible change)
* `chore:`, `docs:`, `refactor:`, `test:`, `ci:` — no bump on their own

This tool follows [Semantic Versioning](https://semver.org/).

### Cutting a release

1. Merge your changes to `main` using Conventional Commit messages.
2. Review the release PR opened by `release-plz` (version bump + changelog).
3. Merge the release PR. `release-plz` publishes to crates.io and creates the GitHub release automatically.

### Publishing credentials: Trusted Publishing

This repository uses [crates.io Trusted Publishing][trusted-publishing-link] (OIDC) instead of a long-lived
`CARGO_REGISTRY_TOKEN` secret. The `release-plz` workflow requests a short-lived token from crates.io via
GitHub's OIDC provider at publish time — nothing needs to be stored in the repository.

The workflow already sets `id-token: write` on the release job, which is required for OIDC.

#### First release

Trusted Publishing [cannot be used for the first version of a new crate][trusted-publishing-limit-link] —
the `0.1.0` release must be published manually:

```console
cargo login                 # with a short-lived API token from https://crates.io/settings/tokens
cargo publish
cargo logout
```

Then tag the release on GitHub and delete the API token.

#### Subsequent releases

After `0.1.0` is on crates.io, configure a Trusted Publisher for this repository under
[crates.io → Settings → Trusted Publishing][trusted-publishing-settings-link]:

* Repository owner: `fg-labs`
* Repository name: `redskull`
* Workflow filename: `release-plz.yml`
* Environment: *(leave blank)*

Once configured, merging a release PR on `main` will publish automatically.

[release-plz-link]:             https://release-plz.dev
[conventional-commits-link]:    https://www.conventionalcommits.org
[trusted-publishing-link]:      https://crates.io/docs/trusted-publishing
[trusted-publishing-limit-link]: https://release-plz.dev/docs/github/quickstart
[trusted-publishing-settings-link]: https://crates.io/settings/trusted-publishers
