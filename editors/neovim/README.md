# RustOwl Neovim Plugin

Visualizes ownership and lifetimes in Rust code.

## Prerequisites

- Neovim 0.10+
- [lazy.nvim](https://github.com/folke/lazy.nvim) (recommended)
- Rust toolchain (for building RustOwl from source)

## Installation

RustOwl must be compiled from source. The plugin will build it automatically on installation:

```lua
{
  'cordx56/rustowl',
  version = '*',
  build = 'cargo install --path . --locked',
  lazy = false,
  opts = {},
}
```

## Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `auto_attach` | boolean | `true` | Automatically attach LSP client for Rust files |
| `auto_enable` | boolean | `false` | Enable highlighting immediately on attach |
| `idle_time` | number | `500` | Milliseconds before triggering analysis |
| `highlight_style` | string | `'undercurl'` | `'undercurl'` or `'underline'` |

### Colors

Customize highlighting colors via the `colors` table:

```lua
opts = {
  colors = {
    lifetime = '#00cc00',   -- Green: variable lifetime
    imm_borrow = '#0000cc', -- Blue: immutable borrow
    mut_borrow = '#cc00cc', -- Purple: mutable borrow
    move = '#cccc00',       -- Yellow: value moved
    call = '#cccc00',       -- Yellow: function call
    outlive = '#cc0000',    -- Red: lifetime errors
  },
}
```

### Custom Keybindings

```lua
opts = {
  client = {
    on_attach = function(_, buffer)
      vim.keymap.set('n', '<leader>ro', function()
        require('rustowl').toggle(buffer)
      end, { buffer = buffer, desc = 'Toggle RustOwl' })
    end
  },
}
```

## Commands

- `:Rustowl enable` - Enable highlighting
- `:Rustowl disable` - Disable highlighting
- `:Rustowl toggle` - Toggle highlighting
- `:Rustowl start_client` - Start LSP client
- `:Rustowl stop_client` - Stop LSP client
- `:Rustowl restart_client` - Restart LSP client

## Lua API

```lua
require('rustowl').enable()
require('rustowl').disable()
require('rustowl').toggle()
require('rustowl').is_enabled()
```

## Contributing

### Development Prerequisites

- Neovim 0.10+ installed and available in PATH

### Running Tests

Run the test suite:

```bash
./test.sh
```

This will execute all plugin tests using the Neovim test framework.
