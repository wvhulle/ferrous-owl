# FerrousOwl

This binary aims to help new users of Rust struggling with the ownership system.

The editor extensions that use this binary visualize ownership movement and lifetimes with colored underlines. The color mapping is roughly as follows:

- 🟩 Green: variable's actual lifetime
- 🟦 Blue: immutable borrow
- 🟪 Purple: mutable borrow
- 🟧 Orange: value moved / function call
- 🟥 Red: lifetime error (invalid overlap or mismatch)

Exact colors may vary upon editor or chosen color theme. In Helix, for example, less colors are available.

## Usage

Run this binary (done automatically when editor if an editor extension is configured):

```bash
ferrous-owl
```

Don't pass any arguments to the binary like `--stdio`, it listens to `stdin` by default.

1. Open a Rust file in your editor (must be part of a Cargo workspace).
2. Place the cursor on a variable definition or reference.
3. Analysis should start automatically (check the extension status) and complete in a few seconds.
4. Hover over the highlighted lines to check ownership status changes

In some editors, you might need to manually enable ownership diagnostics with a code action.

## Installation

Install system packages:

- Rust compiler toolchain: `rustup` ([install](https://rustup.rs/))
- C compiler (`gcc`, `clang`, or Visual Studio on Windows)

Install required Rust compiler components:

```bash
rustup update nightly
rustup toolchain install nightly --component rustc-dev rust-src llvm-tools
```

Then install ferrous-owl:

```bash
cargo +nightly install ferrous-owl --locked
```

Or from git:

```bash
cargo +nightly install --git https://github.com/wvhulle/ferrous-owl --locked
```

Make sure the `~/.cargo/bin` directory is in your path. Then, configure one of the editor extensions that are supported out of the box (see [editors/](./editors/)):

- Helix
- VS Code: [VS Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=WillemVanhulle.ferrous-owl)

FerrousOwl uses an extended LSP protocol, so it can be integrated with other editors.

## Notes

`println!` macro may produce extra output (does not affect usability).
