# FerrousOwl LSP Specification

`ferrous-owl` is an LSP server that provides Rust ownership and lifetime visualization.

## Standard LSP Capabilities

The server advertises these standard LSP capabilities:

### Text Document Sync

- **Open/Close**: Notified when documents are opened or closed
- **Save**: Notified when documents are saved
- **Change**: Incremental text synchronization

When a Rust file is opened, the server automatically adds it to the analysis target and triggers analysis.

### Workspace Folders

- Supports multiple workspace folders
- Notified when workspace folders are added or removed

### Code Actions

The server provides code actions at the cursor position:

| Action | Description |
|--------|-------------|
| Show ownership | Publishes ownership decorations as diagnostics for the variable at cursor |
| Re-analyze | Triggers re-analysis of the workspace |

The "Show ownership" action title reflects the current analysis state (analyzing, waiting for analysis, or ready).

### Execute Command

The server supports these commands via `workspace/executeCommand`:

| Command | Arguments | Description |
|---------|-----------|-------------|
| `ferrous-owl.showOwnership` | `[uri, line, character]` | Show ownership diagnostics for the variable at the given position |
| `ferrous-owl.analyze` | none | Trigger re-analysis |

### Did Save

When a Rust file is saved, the server automatically triggers re-analysis.

## Types

### `OprType`

```typescript
"lifetime" | "imm_borrow" | "mut_borrow" | "move" | "call" | "outlive" | "shared_mut"
```

### `AnalysisStatus`

```typescript
"analyzing" | "finished" | "error"
```

### `Decoration`

<pre><code>{
    "type": <a href="#oprtype">OprType</a>,
    "range": <a href="https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#range">Range</a>,
    "hover_text": string | null,
    "overlapped": bool
}
</code></pre>

The `overlapped` field indicates that the decoration overlaps with another and should be hidden.

## Custom Methods

### `ferrous-owl/cursor`

Returns ownership decorations for the variable at the given cursor position.

**Request:**

<pre><code>{
    "position": <a href="https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#position">Position</a>,
    "document": {
        "uri": <a href="https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#documentUri">DocumentUri</a>
    }
}
</code></pre>

**Response:**

<pre><code>{
    "is_analyzed": bool,
    "status": <a href="#analysisstatus">AnalysisStatus</a>,
    "path": string | null,
    "decorations": [<a href="#decoration">Decoration</a>]
}
</code></pre>

### `ferrous-owl/analyze`

Triggers analysis of the workspace. Analysis runs automatically on initialization and when files are opened/changed.

**Request:** `{}`

**Response:** `{}`

## Diagnostics

When ownership visualization is requested via code action or command, the server publishes decorations as LSP diagnostics with these severity mappings:

| Decoration Type | Severity |
|-----------------|----------|
| `outlive` | Error |
| `shared_mut`, `move` | Warning |
| `mut_borrow`, `call` | Information |
| `imm_borrow` | Hint |

Note: `lifetime` decorations are filtered from diagnostics as they are too verbose.
