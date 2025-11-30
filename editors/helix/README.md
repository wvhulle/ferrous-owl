# RustOwl for Helix

This directory contains the Helix editor configuration for RustOwl LSP.

## Prerequisites

1. Install the `rustowl` binary (see [installation instructions](../../installation/README.md))
2. Ensure `rustowl` is in your `$PATH`

## Installation

Copy the `languages.toml` file to your Helix config directory:

```bash
mkdir -p ~/.config/helix
cp languages.toml ~/.config/helix/languages.toml
```

If you already have a `languages.toml`, merge the RustOwl configuration:

```toml
[language-server.rustowl]
command = "rustowl"

[[language]]
name = "rust"
language-servers = ["rust-analyzer", "rustowl"]
```

## Usage

1. Open a Rust file in Helix
2. Save the file to trigger analysis
3. RustOwl will analyze the code and provide ownership/lifetime information

## Limitations

Helix does not support the custom `rustowl/cursor` LSP method that enables hover-triggered decorations. Basic LSP functionality (diagnostics, etc.) will work, but the colored underlines for ownership visualization require editor-specific support.

For full RustOwl functionality, consider using VS Code or Neovim with the dedicated plugins.
