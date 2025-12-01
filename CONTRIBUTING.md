# Contributing to FerrousOwl

## Required checks

Format, lint, test, and build:

```bash
cargo fmt
cargo clippy --all-targets --all-features
cargo test
cargo build --release
```

## Security audit

```bash
cargo install cargo-deny
cargo deny check
```

- Checks vulnerabilities, licenses, duplicates
- Config: [deny.toml](deny.toml)
