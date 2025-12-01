# Changelog

## Forked as FerrousOwl - 2025-11-30

This repo was forked by @wvhulle from  <https://github.com/cordx56/rustowl>.

## Unreleased

### Changed

- Move to mimalloc allocator
- Move to aws_lc instead of ring
- Refactor runtime stack size and core usage

### Added

- Add performance test to repo
- Add Docker image
- Update to rustc 1.88.0
- Enhanced CLI with all-targets and features options
- Security and memory safety testing workflow
- Cache mechanism
- Winget package

### Fixed

- Skip installing RustOwl toolchain option
- Wrong visualization range from byte check
- Visualization inside async function
- Windows zip top-level directory

## v0.3.4 - 2025-05-20

### Fixed

- Actual lifetime range visualization for Drop variable

## v0.3.3 - 2025-05-17

### Added

- Update rustc to 1.87.0

### Fixed

- CRLF support
- Native CA certs via reqwest

## v0.3.2 - 2025-05-09

### Added

- RUSTOWL_SYSROOT_DIRS support
- Single .rs file analysis
- VS Code download progress

### Fixed

- macOS gsed support
- cargo-binstall pkg-fmt

## v0.3.1 - 2025-05-07

### Added

- RustOwl version check for VS Code
- AUR packages
- Dependabot automation
- Windows zip support

### Fixed

- VS Code version check null return
- Windows arm build
- Sysroot detection

## v0.3.0 - 2025-04-30

### Added

- Shell completions and man pages

## v0.2.2 - 2025-04-18

### Added

- RUSTOWL_TOOLCHAIN_DIR to bypass rustup

## v0.2.1 - 2025-04-15

## v0.2.0 - 2025-04-09

### Changed

- Migrate to Rust 2024
- Add prefix to functions with common names

## v0.1.4 - 2025-02-22

### Changed

- Simplify HashMap insertion using entry API

## v0.1.3 - 2025-02-20

### Fixed

- Install newest version

## v0.1.2 - 2025-02-19

### Added

- Issue templates

### Fixed

- Client/server process cleanup

## v0.1.1 - 2025-02-07

## v0.1.0 - 2025-02-05

### Added

- Windows support

## v0.0.5 - 2025-02-02

## v0.0.4 - 2025-01-31

## v0.0.3 - 2025-01-30

### Changed

- Enable LTO codegen

## v0.0.2 - 2025-01-23

## v0.0.1 - 2024-11-13

Initial release
