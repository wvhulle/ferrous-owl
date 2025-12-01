# FerrousOwl

FerrousOwl visualizes ownership movement and lifetimes in Rust code. When you save a Rust file, FerrousOwl analyzes it and shows ownership/lifetime info when you hover over variables or function calls (or use a code action).

FerrousOwl uses colored underlines:

- 🟩 Green: variable's actual lifetime
- 🟦 Blue: immutable borrow
- 🟪 Purple: mutable borrow
- 🟧 Orange: value moved / function call
- 🟥 Red: lifetime error (invalid overlap or mismatch)

Move the cursor over a variable or function call and wait a few seconds to visualize info.

## Installation

This extension should activate upon opening a Rust file. The system binary `ferrous-owl` should normally be installed automatically when the extension is activated. If not, you can install it manually, see the [FerrousOwl Rust binary](https://github.com/wvhulle/ferrous-owl). If that fails as well, please create a bug report.

## Note

Underlines may not display perfectly for some characters (e.g., g, parentheses).
