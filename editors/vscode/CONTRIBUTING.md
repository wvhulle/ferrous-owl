# Contributing

## Prerequisites

- [FerrousOwl](https://github.com/wvhulle/ferrous-owl) Rust binary installed
- [VS Code](https://code.visualstudio.com/)
- [Node.js](https://nodejs.org/) v20+
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)

## Development

Install dependencies:

```bash
pnpm install --frozen-lockfile
```

Launch a development instance with the extension loaded using VS Code and the `.vscode/launch.json` file.

## Testing

```bash
pnpm test
```

## Building

```bash
pnpm run package
```
