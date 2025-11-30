# Contributing to RustOwl

## Setup

- Rust toolchain (managed via `rust-toolchain.toml`)
- Basic build tools

See [installation/](installation/) for install steps.
Editor setup: [editors/](editors/)

## Building

```bash
cargo build
cargo test
```

## Pre-PR Checklist

Format, lint, test, and build:

```bash
cargo fmt
cargo clippy --all-targets --all-features
cargo test
cargo build --release
```

## Security Audit

```bash
cargo install cargo-deny
cargo deny check
```

- Checks vulnerabilities, licenses, duplicates
- Config: [deny.toml](deny.toml)

## Memory Safety (optional)

```bash
cargo miri test
valgrind --leak-check=full ./target/release/rustowl
```
