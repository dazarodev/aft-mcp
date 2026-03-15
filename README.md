<p align="center">
  <img src="assets/banner.jpeg" alt="AFT — Agent File Toolkit" width="100%" />
</p>

# AFT — Agent File Toolkit

**Tree-sitter powered code analysis tools for AI coding agents.**

AFT replaces generic read/write file operations with semantic, symbol-level tools that give agents
precise control over code — without the token waste of reading full files or the fragility of
line-number edits.

---

## What is AFT?

AI coding agents are fast, but their interaction with code is often blunt. The typical pattern:
read an entire file to find one function, construct a diff from memory, apply it by line number,
and hope nothing shifted. Tokens burned on context noise. Edits that break when the file changes.
Navigation that requires reading three files to answer "what calls this?"

AFT is a set of eight tools built on top of tree-sitter's concrete syntax trees. Every operation
addresses code by what it *is* — a function, a class, a call site, a symbol — not by where it
happens to sit in a file right now. Agents can outline a file's structure in one call, zoom into
a single function, edit it by name, then follow its callers across the workspace. All without
reading a single line they don't need.

The toolkit is a two-component system: a Rust binary that does the heavy lifting (parsing,
analysis, edits, formatting) and a TypeScript plugin that integrates with OpenCode. The binary
ships pre-built for all major platforms and downloads automatically on first use — no install
ceremony required.

---

## How it Helps Agents

**The token problem.** A 500-line file costs ~375 tokens to read. Most of the time, the agent
needs one function. `aft_zoom` returns that function plus a few lines of context: ~40 tokens.
Over a multi-step task, the savings compound fast.

**The fragile-edit problem.** Line-number edits break the moment anything above the target moves.
`aft_edit` in `symbol` mode addresses the function by name. The agent writes the new body; AFT
finds the symbol, replaces it, validates syntax, and runs the formatter. Nothing to count.

**The navigation problem.** "Where is this function called?" means grep or reading every importer.
`aft_navigate` with `callers` mode returns every call site across the workspace in one round trip.
`impact` mode goes further: it tells the agent what else breaks if that function's signature changes.

---

## Features

- **Semantic outline** — list all symbols in a file (or several files at once) with kind, name, line range, visibility
- **Symbol zoom** — read a named symbol with call-graph annotations (`calls_out`, `called_by`), or batch multiple symbols in one call
- **Symbol editing** — replace, delete, insert before/after a named symbol with auto-format and syntax validation
- **Match editing** — find-and-replace by content when there's no named symbol to target
- **Batch & transaction edits** — atomic multi-edit within a file, or atomic multi-file edits with rollback
- **Call tree & callers** — forward call graph and reverse lookup across the workspace
- **Trace-to & impact analysis** — how does execution reach this function? what breaks if it changes?
- **Data flow tracing** — follow a value through assignments and parameters across files
- **Auto-format & auto-backup** — every edit formats the file and saves a snapshot for undo
- **Import management** — add, remove, organize imports language-aware (TS/JS/TSX/Python/Rust/Go)
- **Structural transforms** — add class members, Rust derive macros, Python decorators, Go struct tags, wrap try/catch
- **Workspace-wide refactoring** — move symbols between files (updates all imports), extract functions, inline functions
- **Safety & recovery** — undo last edit, named checkpoints, restore to any checkpoint
- **AST pattern search & replace** — structural code search using meta-variables (`$VAR`, `$$$`), powered by ast-grep

---

## How to Use

### OpenCode

Add AFT to your OpenCode config:

```json
// ~/.config/opencode/config.json
{
  "plugins": ["@cortexkit/aft-opencode"]
}
```

That's it. On the next session start, the binary downloads if needed and all eight tools become
available. Here's a typical agent workflow:

**1. Get the file structure:**

```json
// aft_outline
{ "file": "src/auth/session.ts" }
```

```json
// response
{
  "symbols": [
    { "kind": "function", "name": "createSession", "line_start": 12, "line_end": 34, "visibility": "export" },
    { "kind": "function", "name": "validateToken", "line_start": 36, "line_end": 58, "visibility": "export" },
    { "kind": "interface", "name": "SessionOptions", "line_start": 1, "line_end": 10, "visibility": "export" }
  ]
}
```

**2. Read the specific function:**

```json
// aft_zoom
{ "file": "src/auth/session.ts", "symbol": "validateToken" }
```

**3. Edit it by name:**

```json
// aft_edit
{
  "mode": "symbol",
  "file": "src/auth/session.ts",
  "symbol": "validateToken",
  "operation": "replace",
  "content": "export function validateToken(token: string): boolean {\n  if (!token) return false;\n  return verifyJwt(token);\n}"
}
```

**4. Check who calls it before changing its signature:**

```json
// aft_navigate
{ "mode": "callers", "file": "src/auth/session.ts", "symbol": "validateToken" }
```

---

## Tool Reference

> **All line numbers are 1-based** (matching editor, git, and compiler conventions).
> Line 1 is the first line of the file.

| Tool | Description | Key Params |
|------|-------------|------------|
| `aft_outline` | Structural outline of a file (including Markdown headings) | `file`, `files[]` |
| `aft_zoom` | Deep-inspect a symbol with call-graph info | `file`, `symbol`, `symbols[]`, `start_line`, `end_line` |
| `aft_edit` | Precision file edits (symbol, match, write, batch, transaction) | `mode`, `file`, `symbol`, `match`, `content`, `edits[]` |
| `aft_navigate` | Call graph and data-flow navigation | `mode`, `file`, `symbol`, `depth` |
| `aft_import` | Language-aware import add/remove/organize | `op`, `file`, `module`, `names[]` |
| `aft_transform` | Structural code transforms (members, derives, decorators) | `op`, `file`, `scope`, `target` |
| `aft_refactor` | Workspace-wide move, extract, inline | `op`, `file`, `symbol`, `destination` |
| `aft_safety` | Undo, history, checkpoints, restore | `op`, `file`, `name` |
| `aft_ast_search` | AST-aware pattern search with meta-variables | `pattern`, `lang`, `paths[]`, `globs[]` |
| `aft_ast_replace` | AST-aware pattern replace (dry-run by default) | `pattern`, `rewrite`, `lang`, `dryRun` |

---

### aft_outline

Returns all top-level symbols in a file with their kind, name, line range, visibility, and nested
`members` (methods in classes, sub-headings in Markdown). Accepts either a single `file` or a
`files` array to outline multiple files in one call.

For **Markdown** files (`.md`, `.mdx`): returns heading hierarchy with section ranges — each
heading becomes a symbol you can zoom into by name.

```json
// Outline two files at once
{ "files": ["src/server.ts", "src/router.ts"] }
```

---

### aft_zoom

Deep-inspect a symbol — returns its full source, surrounding context lines, and call-graph
annotations (`calls_out`, `called_by`). Three access patterns:

- **Named symbol**: `{ "file": "...", "symbol": "myFunction" }`
- **Multiple symbols**: `{ "file": "...", "symbols": ["funcA", "funcB"] }`
- **Line range**: `{ "file": "...", "start_line": 10, "end_line": 25 }`

For **Markdown**: use the heading text as the symbol name (e.g. `"symbol": "Architecture"`) to
read the entire section.

Use `scope` to disambiguate symbols with the same name (e.g. `"scope": "MyClass.method"`).
Use `context_lines` to control how many surrounding lines appear (default: 3).

---

### aft_edit

The main editing tool. Four modes:

**`symbol`** — preferred for code changes. Edit a named symbol directly.

```json
{
  "mode": "symbol",
  "file": "src/utils.ts",
  "symbol": "formatDate",
  "operation": "replace",
  "content": "export function formatDate(d: Date): string {\n  return d.toISOString().split('T')[0];\n}"
}
```

Operations: `replace`, `delete`, `insert_before`, `insert_after`.

**`match`** — find-and-replace by content. Good for config values, strings, unnamed code.

```json
{
  "mode": "match",
  "file": "src/config.ts",
  "match": "const TIMEOUT = 5000",
  "replacement": "const TIMEOUT = 10000"
}
```

Set `replace_all: true` to replace every occurrence. If multiple matches exist without `occurrence`
or `replace_all`, the response returns `ambiguous_match` with all candidates.

**`write`** — write the full file content. For new files or complete rewrites.
**`write`** — write the full file content. For new files or complete rewrites.

**Glob patterns** — use a glob in `file` to replace across multiple files in one call:

```json
{ "mode": "match", "file": "**/*.ts", "match": "oldName", "replacement": "newName" }
```

Returns `{ files: [...], total_replacements, total_files }`.

**`batch`** — apply multiple edits atomically to one file. Each edit is either a match/replace
or a line-range replacement (1-based, inclusive).

```json
{
  "mode": "batch",
  "file": "src/constants.ts",
  "edits": [
    { "match": "VERSION = '1.0'", "replacement": "VERSION = '2.0'" },
    { "line_start": 5, "line_end": 7, "content": "// updated header block\n" }
  ]
}
```

Per-edit `occurrence` is supported for disambiguation. Set `content` to empty string to delete
lines entirely.

**`transaction`** — atomic edits across multiple files. If any file fails, all revert.

All modes support `dry_run: true` to preview as a diff without modifying files.
---

### aft_navigate

Call graph and data-flow analysis across the workspace.

| Mode | What it does |
|------|-------------|
| `call_tree` | What does this function call? (forward, default depth 5) |
| `callers` | Where is this function called from? (reverse, default depth 1) |
| `trace_to` | How does execution reach this function from entry points? |
| `impact` | What callers are affected if this function changes? |
| `trace_data` | Follow a value through assignments and parameters. Needs `expression`. |

```json
// Find everything that would break if processPayment changes
{
  "mode": "impact",
  "file": "src/payments/processor.ts",
  "symbol": "processPayment",
  "depth": 3
}
```

---

### aft_import

Language-aware import management for TS, JS, TSX, Python, Rust, and Go.

```json
// Add named imports with auto-grouping and deduplication
{
  "op": "add",
  "file": "src/api.ts",
  "module": "react",
  "names": ["useState", "useEffect"]
}

// Remove a single named import
{ "op": "remove", "file": "src/api.ts", "module": "react", "name": "useEffect" }

// Re-sort and deduplicate all imports by language convention
{ "op": "organize", "file": "src/api.ts" }
```

---

### aft_transform

Scope-aware structural transformations that handle indentation correctly.

| Op | Description |
|----|-------------|
| `add_member` | Insert a method or field into a class, struct, or impl block |
| `add_derive` | Add Rust derive macros (deduplicates) |
| `wrap_try_catch` | Wrap a TS/JS function body in try/catch |
| `add_decorator` | Add a Python decorator to a function or class |
| `add_struct_tags` | Add or update Go struct field tags |

```json
// Add a method to a TypeScript class
{
  "op": "add_member",
  "file": "src/user.ts",
  "scope": "UserService",
  "code": "async deleteUser(id: string): Promise<void> {\n  await this.db.users.delete(id);\n}",
  "position": "last"
}
```

All ops support `dry_run` and `validate` (`"syntax"` or `"full"`).

---

### aft_refactor

Workspace-wide refactoring that updates imports and references across all files.

| Op | Description |
|----|-------------|
| `move` | Move a symbol to another file, updating all imports workspace-wide |
| `extract` | Extract a line range (1-based) into a new function (auto-detects parameters) |
| `inline` | Replace a call site (1-based `call_site_line`) with the function's body |

```json
// Move a utility function to a shared module
{
  "op": "move",
  "file": "src/pages/home.ts",
  "symbol": "formatCurrency",
  "destination": "src/utils/format.ts"
}
```

`move` saves a checkpoint before mutating anything. Use `dry_run: true` to preview as a diff.

---

### aft_safety

Backup and recovery for risky edits.

| Op | Description |
|----|-------------|
| `undo` | Undo the last edit to a file |
| `history` | List all edit snapshots for a file |
| `checkpoint` | Save a named snapshot of tracked files |
| `restore` | Restore files to a named checkpoint |
| `list` | List all available checkpoints |

```json
// Checkpoint before a multi-file refactor
{ "op": "checkpoint", "name": "before-auth-refactor" }

// Restore if something goes wrong
{ "op": "restore", "name": "before-auth-refactor" }
```

> **Note:** Backups are held in-memory for the session lifetime (lost on restart). Per-file undo
> stack is capped at 20 entries — oldest snapshots are evicted when exceeded.


### aft_ast_search

Search for AST patterns across your codebase using tree-sitter structural matching. Patterns use meta-variables (`$VAR` for single nodes, `$$$` for multiple nodes) and must be valid code fragments.

```json
{ "pattern": "console.log($MSG)", "lang": "typescript" }
```

Returns matches with file, line (1-based), column, matched text, and captured meta-variable values. Add `context: 3` to include surrounding lines.

More pattern examples:
- `def $FUNC($$$):` — find all Python function definitions
- `async function $NAME($$$) { $$$ }` — find async functions in JS/TS
- `if ($COND) { $$$ }` — find all if blocks

### aft_ast_replace

Replace AST patterns across files with structural rewriting. Dry-run by default.

```json
{ "pattern": "console.log($MSG)", "rewrite": "logger.info($MSG)", "lang": "typescript" }
```

Meta-variables captured in the pattern are available in the rewrite template. Returns unified diffs per file in dry-run mode, or writes changes with backups.

---

## Configuration

AFT uses a two-level config system: user-level defaults plus project-level overrides.
Both files are JSONC (comments allowed).

**User config** — applies to all projects:
```
~/.config/opencode/aft.jsonc
```

**Project config** — overrides user config for a specific project:
```
.opencode/aft.jsonc
```

### Config Options

```jsonc
{
  // Auto-format files after every aft_edit. Default: true
  "format_on_edit": true,

  // Auto-validate after edits: "syntax" (tree-sitter, fast) or "full" (runs type checker)
  "validate_on_edit": "syntax",

  // Per-language formatter overrides (auto-detected from project config files if omitted)
  // Keys: "typescript", "python", "rust", "go"
  // Values: "biome" | "prettier" | "deno" | "ruff" | "black" | "rustfmt" | "goimports" | "gofmt" | "none"
  "formatter": {
    "typescript": "biome",
    "rust": "rustfmt"
  },

  // Per-language type checker overrides (auto-detected if omitted)
  // Keys: "typescript", "python", "rust", "go"
  // Values: "tsc" | "biome" | "pyright" | "ruff" | "cargo" | "go" | "staticcheck" | "none"
  "checker": {
    "typescript": "biome"
  }
}
```

AFT auto-detects the formatter and checker from project config files (`biome.json` → biome,
`.prettierrc` → prettier, `Cargo.toml` → rustfmt, `pyproject.toml` → ruff/black, `go.mod` →
goimports). You only need per-language overrides if auto-detection picks the wrong tool or if
you want to pin a specific formatter for a language.

---

## Architecture

AFT is two components that talk over JSON-over-stdio:

```
OpenCode agent
     |
     | tool calls
     v
@cortexkit/aft-opencode (TypeScript plugin)
  - Registers 8 tools with OpenCode SDK
  - Manages a BridgePool (one aft process per project directory)
  - Resolves the binary path (cache → npm → PATH → cargo → download)
     |
     | JSON-over-stdio (newline-delimited)
     v
aft binary (Rust)
  - tree-sitter parsing (5 language grammars)
  - Symbol resolution, call graph, diff generation
  - Format-on-edit (shells out to biome / rustfmt / etc.)
  - Backup/checkpoint management
  - ~7 MB, zero runtime dependencies
```

The binary speaks a simple request/response protocol: the plugin writes a JSON object to stdin,
the binary writes a JSON object to stdout. One process per working directory stays alive for the
session — warm parse trees, no re-spawn overhead per call.

---

## Supported Languages

| Language | Outline | Edit | Imports | Refactor |
|----------|---------|------|---------|---------|
| TypeScript | ✓ | ✓ | ✓ | ✓ |
| JavaScript | ✓ | ✓ | ✓ | ✓ |
| TSX | ✓ | ✓ | ✓ | ✓ |
| Python | ✓ | ✓ | ✓ | ✓ |
| Rust | ✓ | ✓ | ✓ | partial |
| Go | ✓ | ✓ | ✓ | partial |
| Markdown | ✓ | ✓ | — | — |

---

## Development

AFT is a monorepo: bun workspaces for TypeScript, cargo workspace for Rust.

**Requirements:** Bun ≥ 1.0, Rust stable toolchain (1.80+).

```sh
# Install JS dependencies
bun install

# Build the Rust binary
cargo build --release

# Build the TypeScript plugin
bun run build

# Run all tests
bun run test        # TypeScript tests
cargo test          # Rust tests

# Lint and format
bun run lint        # biome check
bun run lint:fix    # biome check --write
bun run format      # biome format + cargo fmt
```

**Project layout:**

```
opencode-aft/
├── crates/
│   └── aft/              # Rust binary (tree-sitter core)
│       └── src/
├── packages/
│   ├── opencode-plugin/  # TypeScript OpenCode plugin (@cortexkit/aft-opencode)
│   │   └── src/
│   │       ├── tools/    # One file per tool group
│   │       ├── config.ts # Config loading and schema
│   │       └── downloader.ts
│   └── npm/              # Platform-specific binary packages
└── scripts/
    └── version-sync.mjs  # Keeps npm and cargo versions in sync
```

---

## Roadmap

- C/C++ language support
- LSP integration for type-aware symbol resolution (partially implemented)
- Streaming responses for large call trees
- Watch mode for live outline updates

---

## Contributing

Bug reports and pull requests are welcome. For larger changes, open an issue first to discuss
the approach.

The binary protocol is documented in `crates/aft/src/main.rs`. Adding a new command means
implementing it in Rust and adding a corresponding tool definition (or extending an existing one)
in `packages/opencode-plugin/src/tools/`.

Run `bun run format` and `cargo fmt` before submitting. The CI will reject unformatted code.

---

## License

MIT
