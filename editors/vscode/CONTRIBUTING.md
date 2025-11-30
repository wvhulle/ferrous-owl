# VS Code Extension Contributing Guide

## Prerequisites

- [VS Code](https://code.visualstudio.com/)
- [Node.js](https://nodejs.org/) v20+
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)

## Development Workflow

Install dependencies:

```bash
pnpm install --frozen-lockfile
```

Format code:

```bash
pnpm fmt
```

Lint code:

```bash
pnpm lint
```

Type check:

```bash
pnpm check-types
```

Run tests:

```bash
pnpm test
```

## Testing

Open this directory in VS Code and press `F5` to launch a development instance with the extension loaded.
