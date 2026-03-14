# Project

## What This Is

Agent File Toolkit (AFT) — a Rust binary + OpenCode TypeScript plugin that gives AI coding agents semantic file manipulation and code navigation primitives. Replaces the read-grep-edit-read cycle with operations that match how agents reason about code: edit by symbol name, trace call paths in one call, manage imports with a single operation, checkpoint/restore without git overhead.

Two components: a Rust binary (`aft`) that embeds tree-sitter grammars for 6 languages and does all computation, and a thin TypeScript OpenCode plugin that bridges tool calls to the binary via JSON over stdin/stdout.

## Core Value

One-call semantic file operations that eliminate mechanical token waste — agents edit by symbol name instead of line numbers, read file structure instead of entire files, and trace call chains in a single operation.

## Current State

M001/S01 and S02 complete. The `aft` Rust binary runs as a persistent process with NDJSON protocol and now embeds tree-sitter grammars for 6 languages (TypeScript, JavaScript, TSX, Python, Rust, Go). `TreeSitterProvider` implements the `LanguageProvider` trait — `list_symbols()` extracts functions, classes, methods, structs, interfaces, enums, and type aliases with names, kinds, ranges, signatures, scope chains, and export status. 57 tests pass (53 unit + 4 integration). Parse tree caching with mtime invalidation is in place. Next: S03 (outline/zoom commands) or S04 (safety/recovery system).

## Architecture / Key Patterns

- **Persistent binary process:** The Rust binary runs as a long-lived process, receiving JSON commands on stdin and writing JSON responses to stdout. Keeps tree-sitter parse state, call graph cache, checkpoint store, and edit history in memory.
- **LSP-aware provider interface:** Symbol resolution has a clean provider abstraction — tree-sitter is the default backend, LSP-derived data can be injected via optional `lsp_hints` fields in command JSON.
- **Plugin as bridge, not brain:** The TypeScript plugin manages binary lifecycle (spawn, health, restart), registers tools with OpenCode, and mediates LSP data. All logic lives in the Rust binary.
- **Web-first language priority:** TS/JS/TSX first (shared query patterns), then Python, then Rust and Go.
- **Binary distribution:** npm platform packages following the esbuild/turbo pattern, with `cargo install aft` fallback.

## Capability Contract

See `.gsd/REQUIREMENTS.md` for the explicit capability contract, requirement status, and coverage mapping.

## Milestone Sequence

- [ ] M001: Foundation — Rust binary, tree-sitter for 6 languages, three-layer editing, safety system, OpenCode plugin, binary distribution
- [ ] M002: Language Intelligence — Import management, scope-aware insertion, compound operations, auto-format, transactions
- [ ] M003: Call Graph Navigation — Lazy/incremental call graph, forward/reverse traces, impact analysis, data flow tracking
- [ ] M004: Refactoring Primitives — Move symbol, extract function, inline symbol, LSP integration
