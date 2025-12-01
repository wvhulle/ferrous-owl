# FerrousOwl

FerrousOwl visualizes ownership movement and lifetimes in Rust code using colored underlines (may depend on editor / color theme):

- 🟩 Green: variable's actual lifetime
- 🟦 Blue: immutable borrow
- 🟪 Purple: mutable borrow
- 🟧 Orange: value moved / function call
- 🟥 Red: lifetime error (invalid overlap or mismatch)

## Installation

Install system packages:

- `rustup` ([install](https://rustup.rs/))
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

## Usage

Run the server in LSP mode (done automatically when editor is configured properly):

```bash
ferrous-owl
```

Don't pass any arguments to the binary like `--stdio`, it listens to `stdin` by default.

1. Open a Rust file in your editor (must be part of a Cargo workspace).
2. For VS Code, analysis starts automatically. For other editors, enable FerrousOwl manually or configure auto-loading.
3. Progress is shown in your editor. FerrousOwl works for analyzed portions, even if the whole workspace isn't finished.
4. Place the cursor on a variable or function call to inspect ownership/lifetime info.

## Notes

Thanks a lot to the original author Yuki Okamoto!

_This fork of [RustOwl](https://github.com/cordx56/rustowl) adds support for the Helix editor and other editors that are able to read code actions from an LSP-server and simplifies the codebase considerably._

`println!` macro may produce extra output (does not affect usability).
