# RustOwl Emacs Plugin

Visualizes ownership and lifetimes in Rust code.

## Prerequisites

- [RustOwl](https://github.com/wvhulle/rustowl) installed manually
- Emacs 24.1+
- lsp-mode 9.0.0+

## Installation

### Elpaca

```elisp
(elpaca
  (rustowl
    :host github
    :repo "cordx56/rustowl"))
```

Then use-package:

```elisp
(use-package rustowl
  :after lsp-mode)
```
