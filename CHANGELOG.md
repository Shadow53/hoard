# Changelog

All notable changes to this project will be documented in this file.

## 0.6.0 (2023-10-05)

### Breaking Changes

#### allow setting default environment variable values (#185)

### Fixes

#### update rust crate sha2 to 0.10.7 (#176)

#### update rust crate tempfile to 3.6 (#170)

#### update rust crate uuid to 1.3 (#167)

#### update rust crate regex to 1.8 (#154)

#### update rust crate digest to 0.10.7 (#155)

#### update rust crate toml to 0.7.4 (#159)

#### update rust crate thiserror to 1.0.40 (#164)

#### update rust crate clap to 4.3 (#165)

## 0.5.2

### Features

- add init subcommand (#168)

## 0.5.1

### Fixes

- get Windows string length in a different way
- update rust crate similar to 2.2 (#150)
- update rust crate md-5 to 0.10.5 (#143)
- update rust crate digest to 0.10.5 (#142)
- update rust crate regex to 1.6 (#141)
- update rust crate uuid to 1.2 (#138)
- update rust crate once_cell to 1.15 (#137)
- update rust crate sha2 to 0.10.6 (#148)
- update rust crate thiserror to 1.0.37 (#149)

## 0.5.0

### Bug Fixes

- Prevent "invalid directive" warning ([#78](https://github.com/Shadow53/hoard/issues/78))
- Fix operation version upgrades ([#129](https://github.com/Shadow53/hoard/issues/129))

### Dependency Upgrades

- Update rust crate directories to 3.0.2 ([#81](https://github.com/Shadow53/hoard/issues/81))
- Update rust crate structopt to 0.3.26 ([#82](https://github.com/Shadow53/hoard/issues/82))
- Update actions/checkout action to v2 ([#90](https://github.com/Shadow53/hoard/issues/90))
- Update rust crate thiserror to 1.0.30 ([#84](https://github.com/Shadow53/hoard/issues/84))
- Update rust crate directories to v4 ([#91](https://github.com/Shadow53/hoard/issues/91))
- Update rust crate which to 4.2 ([#89](https://github.com/Shadow53/hoard/issues/89))
- Update rust crate petgraph to 0.6 ([#88](https://github.com/Shadow53/hoard/issues/88))
- Update rust crate once_cell to 1.9 ([#87](https://github.com/Shadow53/hoard/issues/87))
- Update rust crate tempfile to 3.3 ([#85](https://github.com/Shadow53/hoard/issues/85))
- Update rust crate windows to 0.34 ([#108](https://github.com/Shadow53/hoard/issues/108))
- Update rust crate once_cell to 1.10 ([#105](https://github.com/Shadow53/hoard/issues/105))
- Update actions/checkout action to v3 ([#104](https://github.com/Shadow53/hoard/issues/104))
- Update rust crate md-5 to 0.10 ([#86](https://github.com/Shadow53/hoard/issues/86))
- Update codecov/codecov-action action to v3 ([#118](https://github.com/Shadow53/hoard/issues/118))
- Update rust crate windows to 0.35 ([#117](https://github.com/Shadow53/hoard/issues/117))
- Update rust crate toml to 0.5.9 ([#119](https://github.com/Shadow53/hoard/issues/119))
- Update rust crate uuid to v1 ([#125](https://github.com/Shadow53/hoard/issues/125))
- Update rust crate nix to 0.24 ([#126](https://github.com/Shadow53/hoard/issues/126))
- Update rust crate tokio to 1.18 ([#131](https://github.com/Shadow53/hoard/issues/131))
- Update rust crate windows to 0.36 ([#130](https://github.com/Shadow53/hoard/issues/130))
- Update rust crate thiserror to 1.0.31 ([#132](https://github.com/Shadow53/hoard/issues/132))

### Features

- Add edit command with tests ([#71](https://github.com/Shadow53/hoard/issues/71))
- Add status and diff commands ([#76](https://github.com/Shadow53/hoard/issues/76))
- Allow periods in names ([#115](https://github.com/Shadow53/hoard/issues/115))
- Handle file permissions based on config ([#122](https://github.com/Shadow53/hoard/issues/122))

### Miscellaneous Tasks

- Update deps
- Expand Makefile.toml
- Bump version to 0.5.0-beta
- Generate changelog with git-cliff

### Other

- Add license scan report and status ([#79](https://github.com/Shadow53/hoard/issues/79))
- Configure Renovate ([#80](https://github.com/Shadow53/hoard/issues/80))

### Refactor

- Make backup/restore use the files iterator ([#94](https://github.com/Shadow53/hoard/issues/94))
- Refactor! introduce v2 operation logs ([#98](https://github.com/Shadow53/hoard/issues/98))
- [**breaking**] Port Python tests to Rust ([#106](https://github.com/Shadow53/hoard/issues/106))
- Introduce strong types enforcing invariants ([#109](https://github.com/Shadow53/hoard/issues/109))
- Reuse operation logs for backup/restore ([#111](https://github.com/Shadow53/hoard/issues/111))
- Replace structopt with clap v3 ([#112](https://github.com/Shadow53/hoard/issues/112))
- Replace hoards_root with config_dir and data_dir ([#113](https://github.com/Shadow53/hoard/issues/113))
- Clean up file diff iterator logic ([#120](https://github.com/Shadow53/hoard/issues/120))
- [**breaking**] Optimize operations with multithreaded tokio ([#127](https://github.com/Shadow53/hoard/issues/127))
- Log errors at creation site ([#133](https://github.com/Shadow53/hoard/issues/133))

### Testing

- Fix running tests locally

## [0.4.0] - 2021-12-27

### Bug Fixes

- Expand env variables in path_exists for consistency ([#54](https://github.com/Shadow53/hoard/issues/54))
- Create config dir for uuid if not exists + tests ([#64](https://github.com/Shadow53/hoard/issues/64))

### Documentation

- Add Getting Started section ([#56](https://github.com/Shadow53/hoard/issues/56)) ([#61](https://github.com/Shadow53/hoard/issues/61))

### Features

- Add YAML support ([#66](https://github.com/Shadow53/hoard/issues/66))
- Impl list command, custom log output ([#68](https://github.com/Shadow53/hoard/issues/68))

### Miscellaneous Tasks

- Update hoard version and deps

### Other

- Fix ignore file patterns ([#55](https://github.com/Shadow53/hoard/issues/55))
- Missing parent error ([#57](https://github.com/Shadow53/hoard/issues/57))
- Error on invalid "config" item ([#59](https://github.com/Shadow53/hoard/issues/59))

### Refactor

- Parse glob patterns when reading config, not after ([#60](https://github.com/Shadow53/hoard/issues/60))

## [0.3.0] - 2021-10-08

### Documentation

- Remove outdated note from README
- Add env var docs and make command for viewing ([#45](https://github.com/Shadow53/hoard/issues/45))

### Features

- Add `game` subcommand
- Enable env vars in pile paths #22 ([#23](https://github.com/Shadow53/hoard/issues/23))
- Add cleanup command with tests ([#37](https://github.com/Shadow53/hoard/issues/37)) ([#39](https://github.com/Shadow53/hoard/issues/39))

### Miscellaneous Tasks

- Add metadata to Cargo.toml

### Other

- First commit
- Rename to save_hoarder
- Add license
- Update Cargo.lock, add license to Cargo.toml
- Add Config Subcommand ([#10](https://github.com/Shadow53/hoard/issues/10))
- Add Builder type ([#12](https://github.com/Shadow53/hoard/issues/12))
- Add GitHub Actions
- Flesh out fmt job
- Fix tarpaulin args
- Use grcov instead of tarpaulin
- Use manual grcov because action is old
- Switch to tarpaulin for coverage
- Revert "ci: switch to tarpaulin for coverage"
- Use grcov action w/out source coverage
- Remove unnecessary path-mapping grcov config
- Cache stable and nightly separately
- Merge pull request #7 from Shadow53/4-integration-tests
- Use custom envs, refactor ([#10](https://github.com/Shadow53/hoard/issues/10))
- Implement Commands, minor fixes ([#11](https://github.com/Shadow53/hoard/issues/11))
- Implement better logging ([#24](https://github.com/Shadow53/hoard/issues/24))
- Prevent file footguns ([#29](https://github.com/Shadow53/hoard/issues/29))
- Ignore paths ([#30](https://github.com/Shadow53/hoard/issues/30))
- Merge pile config ([#32](https://github.com/Shadow53/hoard/issues/32))
- Add mdBook Documentation ([#36](https://github.com/Shadow53/hoard/issues/36))
- Attach release builds to GitHub release ([#38](https://github.com/Shadow53/hoard/issues/38))
- Get code coverage from python test scripts too ([#41](https://github.com/Shadow53/hoard/issues/41))
- Implement better test coverage ([#42](https://github.com/Shadow53/hoard/issues/42)) ([#43](https://github.com/Shadow53/hoard/issues/43))
- Build/release action requires items must be archived

### Refactor

- Move subcommands to their own modules
- Use thiserror to impl all Error types
- More intuitively model program structure

### Styling

- Run cargo fmt
- Run cargo fmt
- Make clippy happy
- Run cargo fmt

### Testing

- Add unit tests where applicable
- Add remaining unit tests
- Add integration tests for config subcmd
- Add integration tests for game subcmd
- Add remaining integration tests

<!-- generated by git-cliff -->
