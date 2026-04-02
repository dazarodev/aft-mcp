# aft-mcp

Tree-sitter powered code analysis MCP server for Claude Code.

Semantic navigation, call-graph analysis, and structural search — as MCP tools your AI agent can call directly.

---

## What it does

| Tool         | Description                                         |
| ------------ | --------------------------------------------------- |
| `outline`    | File structure — symbols, signatures, line ranges   |
| `zoom`       | Symbol detail with call annotations                 |
| `callers`    | Who calls this symbol                               |
| `call_tree`  | What this symbol calls (recursive)                  |
| `impact`     | What breaks if this symbol changes                  |
| `trace_to`   | Execution path from entry points to a symbol        |
| `trace_data` | Data flow analysis through a symbol                 |
| `ast_search` | Structural pattern matching via tree-sitter queries |
| `read`       | File content with line ranges                       |

## Supported languages

TypeScript, TSX, JavaScript, Python, Rust, Go, Markdown, CSS, HTML, Apex, Java, Ruby, C, C++, C#, PHP

## Installation

### Claude Code marketplace (recommended)

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "aft": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@anthropic/aft-mcp@latest"]
    }
  }
}
```

Or install as a plugin:

```json
{
  "extraKnownMarketplaces": {
    "aft-mcp": {
      "source": { "source": "github", "repo": "dazarodev/aft-mcp" }
    }
  }
}
```

### Manual (binary)

Download the binary for your platform from [Releases](https://github.com/dazarodev/aft-mcp/releases), then add to `~/.claude/.mcp.json`:

```json
{
  "aft": {
    "command": "/path/to/aft-mcp"
  }
}
```

### Install script

```bash
curl -fsSL https://raw.githubusercontent.com/dazarodev/aft-mcp/main/scripts/install.sh | bash
```

## Usage

Once installed, Claude Code automatically discovers the MCP tools. Ask your agent to:

- "outline this file" — get the structure of any source file
- "who calls this function?" — trace callers across the codebase
- "what breaks if I change this?" — impact analysis before refactoring
- "trace data flow through this variable" — understand how data moves

## Configuration

Create `aft.toml` in your project root or `~/.config/aft/config.toml`:

```toml
# Activate only specific languages (default: all)
languages = ["typescript", "javascript", "python", "rust"]

# Project root override (default: auto-detect)
# root = "/path/to/project"
```

## Building from source

```bash
git clone https://github.com/dazarodev/aft-mcp.git
cd aft-mcp
cargo build --release
# Binary at target/release/aft-mcp
```

## Adding language support

See [docs/ADDING-LANGUAGE.md](docs/ADDING-LANGUAGE.md) for instructions on adding new tree-sitter grammars.

## Attribution

Based on [cortexkit/aft](https://github.com/cortexkit/aft) v0.7.3 by Ufuk Altinok. Extended with MCP protocol support, pluggable language registry, and Claude Code marketplace integration.

## License

MIT — see [LICENSE](LICENSE).
