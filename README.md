# FerrousOwl

FerrousOwl visualizes ownership movement and lifetimes in Rust code. When you save a Rust file, FerrousOwl analyzes it and shows ownership/lifetime info when you hover over variables or function calls (or use a code action).

FerrousOwl uses colored underlines:

- 🟩 Green: variable's actual lifetime
- 🟦 Blue: immutable borrow
- 🟪 Purple: mutable borrow
- 🟧 Orange: value moved / function call
- 🟥 Red: lifetime error (invalid overlap or mismatch)

Move the cursor over a variable or function call and wait a few seconds to visualize info. FerrousOwl uses an extended LSP protocol, so it can be integrated with other editors.

## Installation

Install system packages:

- `rustup` ([install](https://rustup.rs/))
- C compiler (`gcc`, `clang`, or Visual Studio on Windows)

FerrousOwl requires a nightly Rust toolchain, which will be installed automatically by `rustup` based on [rust-toolchain.toml](rust-toolchain.toml) when you run `cargo install --path .` (don't run `cargo install --git` as it does not respect the toolchain file).

```bash
git clone git@github.com:wvhulle/ferrous-owl.git /tmp/ferrous-owl
cd /tmp/ferrous-owl
cargo install --path . --locked
```

Make sure the `~/.cargo/bin` directory is in your path. Then, complete the [editors/](./editors/) setup.
Run the server in LSP mode (done automatically when editor is configured properly):

```bash
ferrous-owl
```

Don't pass any arguments to the binary like `--stdio`, it listens to `stdin` by default.

## Usage

1. Open a Rust file in your editor (must be part of a Cargo workspace).
2. For VS Code, analysis starts automatically. For other editors, enable FerrousOwl manually or configure auto-loading.
3. Progress is shown in your editor. FerrousOwl works for analyzed portions, even if the whole workspace isn't finished.
4. Place the cursor on a variable or function call to inspect ownership/lifetime info.

## Notes

Thanks a lot to the original author Yuki Okamoto!

_This fork of [RustOwl](https://github.com/cordx56/rustowl) adds support for the Helix editor and other editors that are able to read code actions from an LSP-server and simplifies the codebase considerably._

`println!` macro may produce extra output (does not affect usability).
