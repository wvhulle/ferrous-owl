<div align="center">
    <h1>
      <picture>
        <source media="(prefers-color-scheme: dark)" srcset="docs/assets/rustowl-logo-dark.svg">
        <img alt="RustOwl" src="docs/assets/rustowl-logo.svg" width="400">
      </picture>
    </h1>
    <p>
        Visualize ownership and lifetimes in Rust for debugging and optimization
    </p>
    <h4>
        <a href="https://crates.io/crates/rustowl">
            <img alt="Crates.io Version" src="https://img.shields.io/crates/v/rustowl?style=for-the-badge">
        </a>
        <a href="https://aur.archlinux.org/packages/rustowl-bin">
            <img alt="AUR Version" src="https://img.shields.io/aur/version/rustowl-bin?style=for-the-badge">
        </a>
        <img alt="WinGet Package Version" src="https://img.shields.io/winget/v/Cordx56.Rustowl?style=for-the-badge">
    </h4>
    <h4>
        <a href="https://marketplace.visualstudio.com/items?itemName=cordx56.rustowl-vscode">
            <img alt="Visual Studio Marketplace Version" src="https://img.shields.io/visual-studio-marketplace/v/cordx56.rustowl-vscode?style=for-the-badge&label=VS%20Code">
        </a>
        <a href="https://open-vsx.org/extension/cordx56/rustowl-vscode">
            <img alt="Open VSX Version" src="https://img.shields.io/open-vsx/v/cordx56/rustowl-vscode?style=for-the-badge">
        </a>
        <a href="https://github.com/siketyan/intellij-rustowl">
            <img alt="JetBrains Plugin Version" src="https://img.shields.io/jetbrains/plugin/v/26504-rustowl?style=for-the-badge">
        </a>
    </h4>
    <p>
        <img src="docs/assets/readme-screenshot-3.png" />
    </p>
</div>

RustOwl visualizes ownership movement and lifetimes of variables.
When you save Rust source code, it is analyzed, and the ownership and lifetimes of variables are visualized when you hover over a variable or function call.

RustOwl visualizes those by using underlines:

- 🟩 green: variable's actual lifetime
- 🟦 blue: immutable borrowing
- 🟪 purple: mutable borrowing
- 🟧 orange: value moved / function call
- 🟥 red: lifetime error
  - diff of lifetime between actual and expected, or
  - invalid overlapped lifetime of mutable and shared (immutable) references

Detailed usage is described [here](docs/usage.md).

Currently, we offer VSCode extension, Neovim plugin and Emacs package.
For these editors, move the text cursor over the variable or function call you want to inspect and wait for 2 seconds to visualize the information.
We implemented LSP server with an extended protocol.
So, RustOwl can be used easily from other editor.

In this tool, due to the limitations of VS Code's decoration specifications, characters with descenders, such as g or parentheses, may occasionally not display underlines properly.
Additionally, we observed that the `println!` macro sometimes produces extra output, though this does not affect usability in any significant way.

## Installation

For package manager installations, see [installation/README.md](installation/README.md).

For building from source, see [installation/source/README.md](installation/source/README.md).

Install the extension for your editor, see [editors](./editors/).

## Getting started

Then, please open a Rust source code file (`.rs`) in the editor.
RustOwl only works with a Cargo workspace, so you need to open the source code that is part of a Cargo workspace.
I recommend you try RustOwl with a small, simple workspace first.
RustOwl's analysis may take a long time for a large workspace.

VS Code extension will automatically start analyzing the workspace.
For other editors, you may need to enable RustOwl manually, but you can enable automatic loading in your configuration file.
The progress of analysis will be shown in your editor.
![Progress](assets/vs-code-progress.png)

After the analysis started, RustOwl waits for your request.
Please place the text cursor on a variable or function call you would like to inspect.
![Cursor on unwrap](assets/vs-code-cursor-on-unwrap.png)
RustOwl works for the analyzed portion, even if the entire analysis has not finished.
If your program has some fatal errors (e.g., syntax errors or unrecoverable type errors), RustOwl cannot work for the part where the analysis failed.

Wait for a few seconds, and then the ownership-related operations and lifetimes of the variable to which the `unwrap()` method call assigns a value will appear.
![unwrap visualized](assets/vs-code-cursor-on-unwrap-visualized.png)

## Basic usage

Basically, RustOwl can be used to resolve ownership and lifetime errors.
What RustOwl visualizes is:

- Actual lifetime of variables
- Shared (immutable) borrowing of variables
- Mutable borrowing of variables
- Value movement
- Function call
- Ownership and lifetime errors

You can see which color is assigned to them on the top page of this repository.
RustOwl can be used to see where a variable lives, where it dies, and where it is borrowed or moved.

For VS Code, you can see the message that explains the meaning of the underline by hovering your mouse cursor over it.
![Hover message on VS Code](assets/readme-screenshot-3.png)

This is the basic usage of RustOwl!
Now you have a master's degree in RustOwl.

## Advanced usage

The lifetime that RustOwl visualizes is the range of these variables:

- Where the variable _lives_
  - For the meaning of _NLL_
- Until the variable is _dropped_ or _moved_
  - For the meaning of _RAII_

Based on this, we can use RustOwl as listed below:

- To resolve _dead lock_ that is caused by some data structures like `Mutex`
  - Because these _locks_ are freed where the lock object is dropped
- To manage resources
  - Like memories, files, and anything which is managed by _RAII_ respective

Did you get a Ph.D. in lifetimes?
So let's try managing resources with RustOwl.
You will get a Ph.D. in RustOwl and computer resource management.
