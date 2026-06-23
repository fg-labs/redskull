# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4](https://github.com/fg-labs/redskull/compare/v0.1.3...v0.1.4) - 2026-06-23

### Other

- chore(ci)(deps): bump the github-actions group with 2 updates ([#37](https://github.com/fg-labs/redskull/pull/37))
- chore(ci)(deps): bump the github-actions group across 1 directory with 3 updates ([#28](https://github.com/fg-labs/redskull/pull/28))

## [0.1.3](https://github.com/fg-labs/redskull/compare/v0.1.2...v0.1.3) - 2026-06-16

### Fixed

- *(sys-deps)* treat vendored compression -sys crates as C-compiler-only ([#33](https://github.com/fg-labs/redskull/pull/33))

## [0.1.2](https://github.com/fg-labs/redskull/compare/v0.1.1...v0.1.2) - 2026-05-10

### Fixed

- *(renderer)* emit fn: directive so conda-build can extract source tarballs ([#23](https://github.com/fg-labs/redskull/pull/23))
- *(recipe)* substitute {{ name }} for the recipe-name segment in source.url ([#21](https://github.com/fg-labs/redskull/pull/21))
- *(recipe)* emit compiler('c') alongside clangdev when bindgen is detected ([#20](https://github.com/fg-labs/redskull/pull/20))

### Other

- *(crate-inspector)* cover [[bin]] name auto-detection end-to-end ([#22](https://github.com/fg-labs/redskull/pull/22))
- *(readme)* use absolute URLs for logo images ([#15](https://github.com/fg-labs/redskull/pull/15))

## [0.1.1](https://github.com/fg-labs/redskull/compare/v0.1.0...v0.1.1) - 2026-04-24

### Added

- *(cli)* print help on no args and remove unimplemented --recursive ([#12](https://github.com/fg-labs/redskull/pull/12))

### Fixed

- *(recipe)* emit stdlib('c') unconditionally and template crates.io URL ([#13](https://github.com/fg-labs/redskull/pull/13))
- *(recipe)* polish generated bioconda recipe output ([#2](https://github.com/fg-labs/redskull/pull/2))

### Other

- chore(ci)(deps): bump actions/checkout from 4.2.2 to 6.0.2 ([#3](https://github.com/fg-labs/redskull/pull/3))
- *(ci)* include major updates in dependabot groups ([#10](https://github.com/fg-labs/redskull/pull/10))
- add dependabot config for cargo and github-actions ([#1](https://github.com/fg-labs/redskull/pull/1))
