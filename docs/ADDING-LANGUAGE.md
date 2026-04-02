# Adding a New Language to aft-mcp

## Steps

1. Add tree-sitter grammar crate to `crates/aft/Cargo.toml` as an optional dependency
2. Add cargo feature `lang-xxx = ["dep:tree-sitter-xxx"]`
3. Add the feature to the `lang-all` list
4. Create `crates/aft/src/lang/xxx.rs` implementing `LangSupport`
5. (Optional) Create `crates/aft/src/queries/xxx.scm` with symbol extraction query
6. Register in `crates/aft/src/lang/mod.rs` — add `#[cfg(feature = "lang-xxx")]` module declaration and registration in `register_builtins()`
7. Add file extensions to `SOURCE_EXTENSIONS` in `src/main.rs`
8. Run `cargo test`

## Template

Copy `crates/aft/src/lang/markdown.rs` (simplest) and fill in:

- `id()` — lowercase language name (e.g. `"ruby"`)
- `extensions()` — file extensions (e.g. `&["rb"]`)
- `grammar()` — tree-sitter grammar reference
- `call_node_kinds()` — AST node kinds for function calls (empty if N/A)
- `scope_container_kinds()` — AST node kinds for scope containers
- `default_indent()` — `IndentPreference::Spaces(4)` or `IndentPreference::Tabs`
- `has_imports()` — `true` if language has import/include system

## Symbol Queries

If you want symbol extraction support (for `outline`, `zoom` commands), create a `.scm` file in `crates/aft/src/queries/` and return it from `symbol_query()` via `include_str!`. See `typescript.scm` for a full example.

Languages without a symbol query will fall back to tree-walking for headings/sections.

## Build Variants

```bash
# Build with all languages (default)
cargo build

# Build with only web languages
cargo build --no-default-features --features lang-web

# Build with specific languages
cargo build --no-default-features --features lang-typescript,lang-python

# Available meta-features: lang-all, lang-web
```

No other files need changing. The registry discovers the language automatically.
