use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

use crate::error::AftError;
use crate::symbols::{Range, Symbol, SymbolKind, SymbolMatch};

// --- Query patterns embedded at compile time ---

const TS_QUERY: &str = r#"
;; function declarations
(function_declaration
  name: (identifier) @fn.name) @fn.def

;; arrow functions assigned to const/let/var
(lexical_declaration
  (variable_declarator
    name: (identifier) @arrow.name
    value: (arrow_function) @arrow.body)) @arrow.def

;; class declarations
(class_declaration
  name: (type_identifier) @class.name) @class.def

;; method definitions inside classes
(class_declaration
  name: (type_identifier) @method.class_name
  body: (class_body
    (method_definition
      name: (property_identifier) @method.name) @method.def))

;; interface declarations
(interface_declaration
  name: (type_identifier) @interface.name) @interface.def

;; enum declarations
(enum_declaration
  name: (identifier) @enum.name) @enum.def

;; type alias declarations
(type_alias_declaration
  name: (type_identifier) @type_alias.name) @type_alias.def

;; export statement wrappers (top-level only)
(export_statement) @export.stmt
"#;

const JS_QUERY: &str = r#"
;; function declarations
(function_declaration
  name: (identifier) @fn.name) @fn.def

;; arrow functions assigned to const/let/var
(lexical_declaration
  (variable_declarator
    name: (identifier) @arrow.name
    value: (arrow_function) @arrow.body)) @arrow.def

;; class declarations
(class_declaration
  name: (identifier) @class.name) @class.def

;; method definitions inside classes
(class_declaration
  name: (identifier) @method.class_name
  body: (class_body
    (method_definition
      name: (property_identifier) @method.name) @method.def))

;; export statement wrappers (top-level only)
(export_statement) @export.stmt
"#;

const PY_QUERY: &str = r#"
;; function definitions (top-level and nested)
(function_definition
  name: (identifier) @fn.name) @fn.def

;; class definitions
(class_definition
  name: (identifier) @class.name) @class.def

;; decorated definitions (wraps function_definition or class_definition)
(decorated_definition
  (decorator) @dec.decorator) @dec.def
"#;

const RS_QUERY: &str = r#"
;; free functions (with optional visibility)
(function_item
  name: (identifier) @fn.name) @fn.def

;; struct items
(struct_item
  name: (type_identifier) @struct.name) @struct.def

;; enum items
(enum_item
  name: (type_identifier) @enum.name) @enum.def

;; trait items
(trait_item
  name: (type_identifier) @trait.name) @trait.def

;; impl blocks — capture the whole block to find methods
(impl_item) @impl.def

;; visibility modifiers on any item
(visibility_modifier) @vis.mod
"#;

const GO_QUERY: &str = r#"
;; function declarations
(function_declaration
  name: (identifier) @fn.name) @fn.def

;; method declarations (with receiver)
(method_declaration
  name: (field_identifier) @method.name) @method.def

;; type declarations (struct and interface)
(type_declaration
  (type_spec
    name: (type_identifier) @type.name
    type: (_) @type.body)) @type.def
"#;

/// Supported language identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LangId {
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Rust,
    Go,
}

/// Maps file extension to language identifier.
pub fn detect_language(path: &Path) -> Option<LangId> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "ts" => Some(LangId::TypeScript),
        "tsx" => Some(LangId::Tsx),
        "js" | "jsx" => Some(LangId::JavaScript),
        "py" => Some(LangId::Python),
        "rs" => Some(LangId::Rust),
        "go" => Some(LangId::Go),
        _ => None,
    }
}

/// Returns the tree-sitter Language grammar for a given LangId.
fn grammar_for(lang: LangId) -> Language {
    match lang {
        LangId::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        LangId::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        LangId::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        LangId::Python => tree_sitter_python::LANGUAGE.into(),
        LangId::Rust => tree_sitter_rust::LANGUAGE.into(),
        LangId::Go => tree_sitter_go::LANGUAGE.into(),
    }
}

/// Returns the query pattern string for a given LangId, if implemented.
fn query_for(lang: LangId) -> Option<&'static str> {
    match lang {
        LangId::TypeScript | LangId::Tsx => Some(TS_QUERY),
        LangId::JavaScript => Some(JS_QUERY),
        LangId::Python => Some(PY_QUERY),
        LangId::Rust => Some(RS_QUERY),
        LangId::Go => Some(GO_QUERY),
    }
}

/// Cached parse result: mtime at parse time + the tree.
struct CachedTree {
    mtime: SystemTime,
    tree: Tree,
}

/// Core parsing engine. Handles language detection, parse tree caching,
/// and query pattern execution via tree-sitter.
pub struct FileParser {
    cache: HashMap<PathBuf, CachedTree>,
}

impl FileParser {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Parse a file, returning the tree and detected language. Uses cache if
    /// the file hasn't been modified since last parse.
    pub fn parse(&mut self, path: &Path) -> Result<(&Tree, LangId), AftError> {
        let lang = detect_language(path).ok_or_else(|| AftError::InvalidRequest {
            message: format!(
                "unsupported file extension: {}",
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("<none>")
            ),
        })?;

        let canon = path.to_path_buf();
        let current_mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map_err(|e| AftError::FileNotFound {
                path: format!("{}: {}", path.display(), e),
            })?;

        // Check cache validity
        let needs_reparse = match self.cache.get(&canon) {
            Some(cached) => cached.mtime != current_mtime,
            None => true,
        };

        if needs_reparse {
            let source = std::fs::read_to_string(path).map_err(|e| AftError::FileNotFound {
                path: format!("{}: {}", path.display(), e),
            })?;

            let grammar = grammar_for(lang);
            let mut parser = Parser::new();
            parser.set_language(&grammar).map_err(|e| {
                eprintln!("[aft] grammar init failed for {:?}: {}", lang, e);
                AftError::ParseError {
                    message: format!("grammar init failed for {:?}: {}", lang, e),
                }
            })?;

            let tree = parser.parse(&source, None).ok_or_else(|| {
                eprintln!("[aft] parse failed for {}", path.display());
                AftError::ParseError {
                    message: format!("tree-sitter parse returned None for {}", path.display()),
                }
            })?;

            self.cache.insert(
                canon.clone(),
                CachedTree {
                    mtime: current_mtime,
                    tree,
                },
            );
        }

        let cached = self.cache.get(&canon).unwrap();
        Ok((&cached.tree, lang))
    }

    /// Extract symbols from a file using language-specific query patterns.
    pub fn extract_symbols(&mut self, path: &Path) -> Result<Vec<Symbol>, AftError> {
        let source = std::fs::read_to_string(path).map_err(|e| AftError::FileNotFound {
            path: format!("{}: {}", path.display(), e),
        })?;

        let (tree, lang) = self.parse(path)?;
        let root = tree.root_node();

        let query_src = query_for(lang).ok_or_else(|| AftError::InvalidRequest {
            message: format!("no query patterns implemented for {:?} yet", lang),
        })?;

        let grammar = grammar_for(lang);
        let query = Query::new(&grammar, query_src).map_err(|e| {
            eprintln!("[aft] query compile failed for {:?}: {}", lang, e);
            AftError::ParseError {
                message: format!("query compile error for {:?}: {}", lang, e),
            }
        })?;

        match lang {
            LangId::TypeScript | LangId::Tsx => {
                extract_ts_symbols(&source, &root, &query)
            }
            LangId::JavaScript => extract_js_symbols(&source, &root, &query),
            LangId::Python => extract_py_symbols(&source, &root, &query),
            LangId::Rust => extract_rs_symbols(&source, &root, &query),
            LangId::Go => extract_go_symbols(&source, &root, &query),
        }
    }
}

/// Build a Range from a tree-sitter Node.
fn node_range(node: &Node) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start_line: start.row as u32,
        start_col: start.column as u32,
        end_line: end.row as u32,
        end_col: end.column as u32,
    }
}

/// Extract the text of a node from source.
fn node_text<'a>(source: &'a str, node: &Node) -> &'a str {
    &source[node.byte_range()]
}

/// Collect byte ranges of all export_statement nodes from query matches.
fn collect_export_ranges(
    source: &str,
    root: &Node,
    query: &Query,
) -> Vec<std::ops::Range<usize>> {
    let export_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "export.stmt");
    let export_idx = match export_idx {
        Some(i) => i as u32,
        None => return vec![],
    };

    let mut cursor = QueryCursor::new();
    let mut ranges = Vec::new();
    let mut matches = cursor.matches(query, *root, source.as_bytes());

    while let Some(m) = {
        matches.advance();
        matches.get()
    } {
        for cap in m.captures {
            if cap.index == export_idx {
                ranges.push(cap.node.byte_range());
            }
        }
    }
    ranges
}

/// Check if a node's byte range is contained within any export statement.
fn is_exported(node: &Node, export_ranges: &[std::ops::Range<usize>]) -> bool {
    let r = node.byte_range();
    export_ranges
        .iter()
        .any(|er| er.start <= r.start && r.end <= er.end)
}

/// Extract the first line of a node as its signature.
fn extract_signature(source: &str, node: &Node) -> String {
    let text = node_text(source, node);
    let first_line = text.lines().next().unwrap_or(text);
    // Trim trailing opening brace if present
    let trimmed = first_line.trim_end();
    let trimmed = trimmed.strip_suffix('{').unwrap_or(trimmed).trim_end();
    trimmed.to_string()
}

/// Extract symbols from TypeScript / TSX source.
fn extract_ts_symbols(
    source: &str,
    root: &Node,
    query: &Query,
) -> Result<Vec<Symbol>, AftError> {
    let capture_names = query.capture_names();

    let export_ranges = collect_export_ranges(source, root, query);

    let mut symbols = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, *root, source.as_bytes());

    while let Some(m) = {
        matches.advance();
        matches.get()
    } {
        // Determine what kind of match this is by looking at capture names
        let mut fn_name_node = None;
        let mut fn_def_node = None;
        let mut arrow_name_node = None;
        let mut arrow_def_node = None;
        let mut class_name_node = None;
        let mut class_def_node = None;
        let mut method_class_name_node = None;
        let mut method_name_node = None;
        let mut method_def_node = None;
        let mut interface_name_node = None;
        let mut interface_def_node = None;
        let mut enum_name_node = None;
        let mut enum_def_node = None;
        let mut type_alias_name_node = None;
        let mut type_alias_def_node = None;

        for cap in m.captures {
            let name = capture_names[cap.index as usize];
            match name {
                "fn.name" => fn_name_node = Some(cap.node),
                "fn.def" => fn_def_node = Some(cap.node),
                "arrow.name" => arrow_name_node = Some(cap.node),
                "arrow.def" => arrow_def_node = Some(cap.node),
                "class.name" => class_name_node = Some(cap.node),
                "class.def" => class_def_node = Some(cap.node),
                "method.class_name" => method_class_name_node = Some(cap.node),
                "method.name" => method_name_node = Some(cap.node),
                "method.def" => method_def_node = Some(cap.node),
                "interface.name" => interface_name_node = Some(cap.node),
                "interface.def" => interface_def_node = Some(cap.node),
                "enum.name" => enum_name_node = Some(cap.node),
                "enum.def" => enum_def_node = Some(cap.node),
                "type_alias.name" => type_alias_name_node = Some(cap.node),
                "type_alias.def" => type_alias_def_node = Some(cap.node),
                _ => {}
            }
        }

        // Function declaration
        if let (Some(name_node), Some(def_node)) = (fn_name_node, fn_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Function,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }

        // Arrow function
        if let (Some(name_node), Some(def_node)) = (arrow_name_node, arrow_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Function,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }

        // Class declaration
        if let (Some(name_node), Some(def_node)) = (class_name_node, class_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Class,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }

        // Method definition
        if let (Some(class_name_node), Some(name_node), Some(def_node)) =
            (method_class_name_node, method_name_node, method_def_node)
        {
            let class_name = node_text(source, &class_name_node).to_string();
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Method,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![class_name.clone()],
                exported: false, // methods inherit export from class
                parent: Some(class_name),
            });
        }

        // Interface declaration
        if let (Some(name_node), Some(def_node)) = (interface_name_node, interface_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Interface,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }

        // Enum declaration
        if let (Some(name_node), Some(def_node)) = (enum_name_node, enum_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Enum,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }

        // Type alias
        if let (Some(name_node), Some(def_node)) = (type_alias_name_node, type_alias_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::TypeAlias,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }
    }

    // Deduplicate: methods can appear as both class and method captures
    dedup_symbols(&mut symbols);
    Ok(symbols)
}

/// Extract symbols from JavaScript source.
fn extract_js_symbols(
    source: &str,
    root: &Node,
    query: &Query,
) -> Result<Vec<Symbol>, AftError> {
    let capture_names = query.capture_names();

    let export_ranges = collect_export_ranges(source, root, query);

    let mut symbols = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, *root, source.as_bytes());

    while let Some(m) = {
        matches.advance();
        matches.get()
    } {
        let mut fn_name_node = None;
        let mut fn_def_node = None;
        let mut arrow_name_node = None;
        let mut arrow_def_node = None;
        let mut class_name_node = None;
        let mut class_def_node = None;
        let mut method_class_name_node = None;
        let mut method_name_node = None;
        let mut method_def_node = None;

        for cap in m.captures {
            let name = capture_names[cap.index as usize];
            match name {
                "fn.name" => fn_name_node = Some(cap.node),
                "fn.def" => fn_def_node = Some(cap.node),
                "arrow.name" => arrow_name_node = Some(cap.node),
                "arrow.def" => arrow_def_node = Some(cap.node),
                "class.name" => class_name_node = Some(cap.node),
                "class.def" => class_def_node = Some(cap.node),
                "method.class_name" => method_class_name_node = Some(cap.node),
                "method.name" => method_name_node = Some(cap.node),
                "method.def" => method_def_node = Some(cap.node),
                _ => {}
            }
        }

        if let (Some(name_node), Some(def_node)) = (fn_name_node, fn_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Function,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }

        if let (Some(name_node), Some(def_node)) = (arrow_name_node, arrow_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Function,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }

        if let (Some(name_node), Some(def_node)) = (class_name_node, class_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Class,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_exported(&def_node, &export_ranges),
                parent: None,
            });
        }

        if let (Some(class_name_node), Some(name_node), Some(def_node)) =
            (method_class_name_node, method_name_node, method_def_node)
        {
            let class_name = node_text(source, &class_name_node).to_string();
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Method,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![class_name.clone()],
                exported: false,
                parent: Some(class_name),
            });
        }
    }

    dedup_symbols(&mut symbols);
    Ok(symbols)
}

/// Walk parent nodes to build a scope chain for Python symbols.
/// A function inside `class_definition > block` gets the class name in its scope.
fn py_scope_chain(node: &Node, source: &str) -> Vec<String> {
    let mut chain = Vec::new();
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "class_definition" {
            if let Some(name_node) = parent.child_by_field_name("name") {
                chain.push(node_text(source, &name_node).to_string());
            }
        }
        current = parent.parent();
    }
    chain.reverse();
    chain
}

/// Extract symbols from Python source.
fn extract_py_symbols(
    source: &str,
    root: &Node,
    query: &Query,
) -> Result<Vec<Symbol>, AftError> {
    let capture_names = query.capture_names();

    let mut symbols = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, *root, source.as_bytes());

    // Track decorated definitions to avoid double-counting
    let mut decorated_fn_lines = std::collections::HashSet::new();

    // First pass: collect decorated definition info
    {
        let mut cursor2 = QueryCursor::new();
        let mut matches2 = cursor2.matches(query, *root, source.as_bytes());
        while let Some(m) = {
            matches2.advance();
            matches2.get()
        } {
            let mut dec_def_node = None;
            let mut dec_decorator_node = None;

            for cap in m.captures {
                let name = capture_names[cap.index as usize];
                match name {
                    "dec.def" => dec_def_node = Some(cap.node),
                    "dec.decorator" => dec_decorator_node = Some(cap.node),
                    _ => {}
                }
            }

            if let (Some(def_node), Some(_dec_node)) = (dec_def_node, dec_decorator_node) {
                // Find the inner function_definition or class_definition
                let mut child_cursor = def_node.walk();
                if child_cursor.goto_first_child() {
                    loop {
                        let child = child_cursor.node();
                        if child.kind() == "function_definition"
                            || child.kind() == "class_definition"
                        {
                            decorated_fn_lines
                                .insert(child.start_position().row);
                        }
                        if !child_cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
        }
    }

    while let Some(m) = {
        matches.advance();
        matches.get()
    } {
        let mut fn_name_node = None;
        let mut fn_def_node = None;
        let mut class_name_node = None;
        let mut class_def_node = None;

        for cap in m.captures {
            let name = capture_names[cap.index as usize];
            match name {
                "fn.name" => fn_name_node = Some(cap.node),
                "fn.def" => fn_def_node = Some(cap.node),
                "class.name" => class_name_node = Some(cap.node),
                "class.def" => class_def_node = Some(cap.node),
                _ => {}
            }
        }

        // Function definition
        if let (Some(name_node), Some(def_node)) = (fn_name_node, fn_def_node) {
            let scope = py_scope_chain(&def_node, source);
            let is_method = !scope.is_empty();
            let name = node_text(source, &name_node).to_string();
            // Skip __init__ and other dunders as separate symbols — they're methods
            let kind = if is_method {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };

            // Build signature — include decorator if this is a decorated function
            let sig = if decorated_fn_lines.contains(&def_node.start_position().row) {
                // Find the decorated_definition parent to get decorator text
                let mut sig_parts = Vec::new();
                let mut parent = def_node.parent();
                while let Some(p) = parent {
                    if p.kind() == "decorated_definition" {
                        // Get decorator lines
                        let mut dc = p.walk();
                        if dc.goto_first_child() {
                            loop {
                                if dc.node().kind() == "decorator" {
                                    sig_parts.push(node_text(source, &dc.node()).to_string());
                                }
                                if !dc.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                        break;
                    }
                    parent = p.parent();
                }
                sig_parts.push(extract_signature(source, &def_node));
                Some(sig_parts.join("\n"))
            } else {
                Some(extract_signature(source, &def_node))
            };

            symbols.push(Symbol {
                name,
                kind,
                range: node_range(&def_node),
                signature: sig,
                scope_chain: scope.clone(),
                exported: false, // Python has no export concept
                parent: scope.last().cloned(),
            });
        }

        // Class definition
        if let (Some(name_node), Some(def_node)) = (class_name_node, class_def_node) {
            let scope = py_scope_chain(&def_node, source);

            // Build signature — include decorator if decorated
            let sig = if decorated_fn_lines.contains(&def_node.start_position().row) {
                let mut sig_parts = Vec::new();
                let mut parent = def_node.parent();
                while let Some(p) = parent {
                    if p.kind() == "decorated_definition" {
                        let mut dc = p.walk();
                        if dc.goto_first_child() {
                            loop {
                                if dc.node().kind() == "decorator" {
                                    sig_parts.push(node_text(source, &dc.node()).to_string());
                                }
                                if !dc.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                        break;
                    }
                    parent = p.parent();
                }
                sig_parts.push(extract_signature(source, &def_node));
                Some(sig_parts.join("\n"))
            } else {
                Some(extract_signature(source, &def_node))
            };

            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Class,
                range: node_range(&def_node),
                signature: sig,
                scope_chain: scope.clone(),
                exported: false,
                parent: scope.last().cloned(),
            });
        }
    }

    dedup_symbols(&mut symbols);
    Ok(symbols)
}

/// Extract symbols from Rust source.
/// Handles: free functions, struct, enum, trait (as Interface), impl methods with scope chains.
fn extract_rs_symbols(
    source: &str,
    root: &Node,
    query: &Query,
) -> Result<Vec<Symbol>, AftError> {
    let capture_names = query.capture_names();

    // Collect all visibility_modifier byte ranges first
    let mut vis_ranges: Vec<std::ops::Range<usize>> = Vec::new();
    {
        let vis_idx = capture_names
            .iter()
            .position(|n| *n == "vis.mod");
        if let Some(idx) = vis_idx {
            let idx = idx as u32;
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, *root, source.as_bytes());
            while let Some(m) = {
                matches.advance();
                matches.get()
            } {
                for cap in m.captures {
                    if cap.index == idx {
                        vis_ranges.push(cap.node.byte_range());
                    }
                }
            }
        }
    }

    let is_pub = |node: &Node| -> bool {
        // Check if the node has a visibility_modifier as a direct child
        let mut child_cursor = node.walk();
        if child_cursor.goto_first_child() {
            loop {
                if child_cursor.node().kind() == "visibility_modifier" {
                    return true;
                }
                if !child_cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    };

    let mut symbols = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, *root, source.as_bytes());

    while let Some(m) = {
        matches.advance();
        matches.get()
    } {
        let mut fn_name_node = None;
        let mut fn_def_node = None;
        let mut struct_name_node = None;
        let mut struct_def_node = None;
        let mut enum_name_node = None;
        let mut enum_def_node = None;
        let mut trait_name_node = None;
        let mut trait_def_node = None;
        let mut impl_def_node = None;

        for cap in m.captures {
            let name = capture_names[cap.index as usize];
            match name {
                "fn.name" => fn_name_node = Some(cap.node),
                "fn.def" => fn_def_node = Some(cap.node),
                "struct.name" => struct_name_node = Some(cap.node),
                "struct.def" => struct_def_node = Some(cap.node),
                "enum.name" => enum_name_node = Some(cap.node),
                "enum.def" => enum_def_node = Some(cap.node),
                "trait.name" => trait_name_node = Some(cap.node),
                "trait.def" => trait_def_node = Some(cap.node),
                "impl.def" => impl_def_node = Some(cap.node),
                _ => {}
            }
        }

        // Free function (not inside impl block — check parent)
        if let (Some(name_node), Some(def_node)) = (fn_name_node, fn_def_node) {
            let parent = def_node.parent();
            let in_impl = parent
                .map(|p| p.kind() == "declaration_list")
                .unwrap_or(false);
            if !in_impl {
                symbols.push(Symbol {
                    name: node_text(source, &name_node).to_string(),
                    kind: SymbolKind::Function,
                    range: node_range(&def_node),
                    signature: Some(extract_signature(source, &def_node)),
                    scope_chain: vec![],
                    exported: is_pub(&def_node),
                    parent: None,
                });
            }
        }

        // Struct
        if let (Some(name_node), Some(def_node)) = (struct_name_node, struct_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Struct,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_pub(&def_node),
                parent: None,
            });
        }

        // Enum
        if let (Some(name_node), Some(def_node)) = (enum_name_node, enum_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Enum,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_pub(&def_node),
                parent: None,
            });
        }

        // Trait (mapped to Interface kind)
        if let (Some(name_node), Some(def_node)) = (trait_name_node, trait_def_node) {
            symbols.push(Symbol {
                name: node_text(source, &name_node).to_string(),
                kind: SymbolKind::Interface,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                exported: is_pub(&def_node),
                parent: None,
            });
        }

        // Impl block — extract methods from inside
        if let Some(impl_node) = impl_def_node {
            // Find the type name(s) from the impl
            // `impl TypeName { ... }` → scope = ["TypeName"]
            // `impl Trait for TypeName { ... }` → scope = ["Trait for TypeName"]
            let mut type_names: Vec<String> = Vec::new();
            let mut child_cursor = impl_node.walk();
            if child_cursor.goto_first_child() {
                loop {
                    let child = child_cursor.node();
                    if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                        type_names.push(node_text(source, &child).to_string());
                    }
                    if !child_cursor.goto_next_sibling() {
                        break;
                    }
                }
            }

            let scope_name = if type_names.len() >= 2 {
                // impl Trait for Type
                format!("{} for {}", type_names[0], type_names[1])
            } else if type_names.len() == 1 {
                type_names[0].clone()
            } else {
                String::new()
            };

            let parent_name = type_names.last().cloned().unwrap_or_default();

            // Find declaration_list and extract function_items
            let mut child_cursor = impl_node.walk();
            if child_cursor.goto_first_child() {
                loop {
                    let child = child_cursor.node();
                    if child.kind() == "declaration_list" {
                        let mut fn_cursor = child.walk();
                        if fn_cursor.goto_first_child() {
                            loop {
                                let fn_node = fn_cursor.node();
                                if fn_node.kind() == "function_item" {
                                    if let Some(name_node) =
                                        fn_node.child_by_field_name("name")
                                    {
                                        symbols.push(Symbol {
                                            name: node_text(source, &name_node)
                                                .to_string(),
                                            kind: SymbolKind::Method,
                                            range: node_range(&fn_node),
                                            signature: Some(extract_signature(
                                                source, &fn_node,
                                            )),
                                            scope_chain: if scope_name.is_empty() {
                                                vec![]
                                            } else {
                                                vec![scope_name.clone()]
                                            },
                                            exported: is_pub(&fn_node),
                                            parent: if parent_name.is_empty() {
                                                None
                                            } else {
                                                Some(parent_name.clone())
                                            },
                                        });
                                    }
                                }
                                if !fn_cursor.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                    }
                    if !child_cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    dedup_symbols(&mut symbols);
    Ok(symbols)
}

/// Extract symbols from Go source.
/// Handles: functions, methods (with receiver scope chain), struct/interface types,
/// uppercase-first-letter export detection.
fn extract_go_symbols(
    source: &str,
    root: &Node,
    query: &Query,
) -> Result<Vec<Symbol>, AftError> {
    let capture_names = query.capture_names();

    let is_go_exported = |name: &str| -> bool {
        name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
    };

    let mut symbols = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, *root, source.as_bytes());

    while let Some(m) = {
        matches.advance();
        matches.get()
    } {
        let mut fn_name_node = None;
        let mut fn_def_node = None;
        let mut method_name_node = None;
        let mut method_def_node = None;
        let mut type_name_node = None;
        let mut type_body_node = None;
        let mut type_def_node = None;

        for cap in m.captures {
            let name = capture_names[cap.index as usize];
            match name {
                "fn.name" => fn_name_node = Some(cap.node),
                "fn.def" => fn_def_node = Some(cap.node),
                "method.name" => method_name_node = Some(cap.node),
                "method.def" => method_def_node = Some(cap.node),
                "type.name" => type_name_node = Some(cap.node),
                "type.body" => type_body_node = Some(cap.node),
                "type.def" => type_def_node = Some(cap.node),
                _ => {}
            }
        }

        // Function declaration
        if let (Some(name_node), Some(def_node)) = (fn_name_node, fn_def_node) {
            let name = node_text(source, &name_node).to_string();
            symbols.push(Symbol {
                exported: is_go_exported(&name),
                name,
                kind: SymbolKind::Function,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                parent: None,
            });
        }

        // Method declaration (with receiver)
        if let (Some(name_node), Some(def_node)) = (method_name_node, method_def_node) {
            let name = node_text(source, &name_node).to_string();

            // Extract receiver type from the first parameter_list
            let receiver_type = extract_go_receiver_type(&def_node, source);
            let scope_chain = if let Some(ref rt) = receiver_type {
                vec![rt.clone()]
            } else {
                vec![]
            };

            symbols.push(Symbol {
                exported: is_go_exported(&name),
                name,
                kind: SymbolKind::Method,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain,
                parent: receiver_type,
            });
        }

        // Type declarations (struct or interface)
        if let (Some(name_node), Some(body_node), Some(def_node)) =
            (type_name_node, type_body_node, type_def_node)
        {
            let name = node_text(source, &name_node).to_string();
            let kind = match body_node.kind() {
                "struct_type" => SymbolKind::Struct,
                "interface_type" => SymbolKind::Interface,
                _ => SymbolKind::TypeAlias,
            };

            symbols.push(Symbol {
                exported: is_go_exported(&name),
                name,
                kind,
                range: node_range(&def_node),
                signature: Some(extract_signature(source, &def_node)),
                scope_chain: vec![],
                parent: None,
            });
        }
    }

    dedup_symbols(&mut symbols);
    Ok(symbols)
}

/// Extract the receiver type from a Go method_declaration node.
/// e.g. `func (m *MyStruct) String()` → Some("MyStruct")
fn extract_go_receiver_type(method_node: &Node, source: &str) -> Option<String> {
    // The first parameter_list is the receiver
    let mut child_cursor = method_node.walk();
    if child_cursor.goto_first_child() {
        loop {
            let child = child_cursor.node();
            if child.kind() == "parameter_list" {
                // Walk into parameter_list to find type_identifier
                return find_type_identifier_recursive(&child, source);
            }
            if !child_cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Recursively find the first type_identifier node in a subtree.
fn find_type_identifier_recursive(node: &Node, source: &str) -> Option<String> {
    if node.kind() == "type_identifier" {
        return Some(node_text(source, node).to_string());
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if let Some(result) = find_type_identifier_recursive(&cursor.node(), source) {
                return Some(result);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Remove duplicate symbols based on (name, kind, start_line).
/// Class declarations can match both "class" and "method" patterns,
/// producing duplicates.
fn dedup_symbols(symbols: &mut Vec<Symbol>) {
    let mut seen = std::collections::HashSet::new();
    symbols.retain(|s| {
        let key = (s.name.clone(), format!("{:?}", s.kind), s.range.start_line);
        seen.insert(key)
    });
}

/// Provider that uses tree-sitter for real symbol extraction.
/// Implements the `LanguageProvider` trait from `language.rs`.
pub struct TreeSitterProvider {
    parser: RefCell<FileParser>,
}

impl TreeSitterProvider {
    pub fn new() -> Self {
        Self {
            parser: RefCell::new(FileParser::new()),
        }
    }
}

impl crate::language::LanguageProvider for TreeSitterProvider {
    fn resolve_symbol(
        &self,
        file: &Path,
        name: &str,
    ) -> Result<Vec<SymbolMatch>, AftError> {
        let symbols = self.parser.borrow_mut().extract_symbols(file)?;

        let matches: Vec<SymbolMatch> = symbols
            .into_iter()
            .filter(|s| s.name == name)
            .map(|s| SymbolMatch {
                file: file.display().to_string(),
                symbol: s,
            })
            .collect();

        if matches.is_empty() {
            Err(AftError::SymbolNotFound {
                name: name.to_string(),
                file: file.display().to_string(),
            })
        } else {
            Ok(matches)
        }
    }

    fn list_symbols(&self, file: &Path) -> Result<Vec<Symbol>, AftError> {
        self.parser.borrow_mut().extract_symbols(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LanguageProvider;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    // --- Language detection ---

    #[test]
    fn detect_ts() {
        assert_eq!(detect_language(Path::new("foo.ts")), Some(LangId::TypeScript));
    }

    #[test]
    fn detect_tsx() {
        assert_eq!(detect_language(Path::new("foo.tsx")), Some(LangId::Tsx));
    }

    #[test]
    fn detect_js() {
        assert_eq!(detect_language(Path::new("foo.js")), Some(LangId::JavaScript));
    }

    #[test]
    fn detect_jsx() {
        assert_eq!(detect_language(Path::new("foo.jsx")), Some(LangId::JavaScript));
    }

    #[test]
    fn detect_py() {
        assert_eq!(detect_language(Path::new("foo.py")), Some(LangId::Python));
    }

    #[test]
    fn detect_rs() {
        assert_eq!(detect_language(Path::new("foo.rs")), Some(LangId::Rust));
    }

    #[test]
    fn detect_go() {
        assert_eq!(detect_language(Path::new("foo.go")), Some(LangId::Go));
    }

    #[test]
    fn detect_unknown_returns_none() {
        assert_eq!(detect_language(Path::new("foo.txt")), None);
    }

    // --- Unsupported extension error ---

    #[test]
    fn unsupported_extension_returns_invalid_request() {
        // Use a file that exists but has an unsupported extension
        let path = fixture_path("sample.ts");
        let bad_path = path.with_extension("txt");
        // Create a dummy file so the error comes from language detection, not I/O
        std::fs::write(&bad_path, "hello").unwrap();
        let provider = TreeSitterProvider::new();
        let result = provider.list_symbols(&bad_path);
        std::fs::remove_file(&bad_path).ok();
        match result {
            Err(AftError::InvalidRequest { message }) => {
                assert!(message.contains("unsupported file extension"), "msg: {}", message);
                assert!(message.contains("txt"), "msg: {}", message);
            }
            other => panic!("expected InvalidRequest, got {:?}", other),
        }
    }

    // --- TypeScript extraction ---

    #[test]
    fn ts_extracts_all_symbol_kinds() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.ts")).unwrap();

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"), "missing function greet: {:?}", names);
        assert!(names.contains(&"add"), "missing arrow fn add: {:?}", names);
        assert!(names.contains(&"UserService"), "missing class UserService: {:?}", names);
        assert!(names.contains(&"Config"), "missing interface Config: {:?}", names);
        assert!(names.contains(&"Status"), "missing enum Status: {:?}", names);
        assert!(names.contains(&"UserId"), "missing type alias UserId: {:?}", names);
        assert!(names.contains(&"internalHelper"), "missing non-exported fn: {:?}", names);

        // At least 6 unique symbols as required
        assert!(
            symbols.len() >= 6,
            "expected ≥6 symbols, got {}: {:?}",
            symbols.len(),
            names
        );
    }

    #[test]
    fn ts_symbol_kinds_correct() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.ts")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        assert_eq!(find("greet").kind, SymbolKind::Function);
        assert_eq!(find("add").kind, SymbolKind::Function); // arrow fn → Function
        assert_eq!(find("UserService").kind, SymbolKind::Class);
        assert_eq!(find("Config").kind, SymbolKind::Interface);
        assert_eq!(find("Status").kind, SymbolKind::Enum);
        assert_eq!(find("UserId").kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn ts_export_detection() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.ts")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        assert!(find("greet").exported, "greet should be exported");
        assert!(find("add").exported, "add should be exported");
        assert!(find("UserService").exported, "UserService should be exported");
        assert!(find("Config").exported, "Config should be exported");
        assert!(find("Status").exported, "Status should be exported");
        assert!(!find("internalHelper").exported, "internalHelper should not be exported");
    }

    #[test]
    fn ts_method_scope_chain() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.ts")).unwrap();

        let methods: Vec<&Symbol> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .collect();
        assert!(!methods.is_empty(), "should have at least one method");

        for method in &methods {
            assert_eq!(
                method.scope_chain,
                vec!["UserService"],
                "method {} should have UserService in scope chain",
                method.name
            );
            assert_eq!(method.parent.as_deref(), Some("UserService"));
        }
    }

    #[test]
    fn ts_signatures_present() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.ts")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        let greet_sig = find("greet").signature.as_ref().unwrap();
        assert!(
            greet_sig.contains("greet"),
            "signature should contain function name: {}",
            greet_sig
        );
    }

    #[test]
    fn ts_ranges_valid() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.ts")).unwrap();

        for s in &symbols {
            assert!(
                s.range.end_line >= s.range.start_line,
                "symbol {} has invalid range: {:?}",
                s.name,
                s.range
            );
        }
    }

    // --- JavaScript extraction ---

    #[test]
    fn js_extracts_core_symbols() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.js")).unwrap();

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"multiply"), "missing function multiply: {:?}", names);
        assert!(names.contains(&"divide"), "missing arrow fn divide: {:?}", names);
        assert!(names.contains(&"EventEmitter"), "missing class EventEmitter: {:?}", names);
        assert!(names.contains(&"main"), "missing default export fn main: {:?}", names);

        assert!(
            symbols.len() >= 4,
            "expected ≥4 symbols, got {}: {:?}",
            symbols.len(),
            names
        );
    }

    #[test]
    fn js_arrow_fn_correctly_named() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.js")).unwrap();

        let divide = symbols.iter().find(|s| s.name == "divide").unwrap();
        assert_eq!(divide.kind, SymbolKind::Function);
        assert!(divide.exported, "divide should be exported");

        let internal = symbols.iter().find(|s| s.name == "internalUtil").unwrap();
        assert_eq!(internal.kind, SymbolKind::Function);
        assert!(!internal.exported, "internalUtil should not be exported");
    }

    #[test]
    fn js_method_scope_chain() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.js")).unwrap();

        let methods: Vec<&Symbol> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .collect();

        for method in &methods {
            assert_eq!(
                method.scope_chain,
                vec!["EventEmitter"],
                "method {} should have EventEmitter in scope chain",
                method.name
            );
        }
    }

    // --- TSX extraction ---

    #[test]
    fn tsx_extracts_react_component() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.tsx")).unwrap();

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Button"), "missing React component Button: {:?}", names);
        assert!(names.contains(&"Counter"), "missing class Counter: {:?}", names);
        assert!(names.contains(&"formatLabel"), "missing function formatLabel: {:?}", names);

        assert!(
            symbols.len() >= 2,
            "expected ≥2 symbols, got {}: {:?}",
            symbols.len(),
            names
        );
    }

    #[test]
    fn tsx_jsx_doesnt_break_parser() {
        // Main assertion: TSX grammar handles JSX without errors
        let provider = TreeSitterProvider::new();
        let result = provider.list_symbols(&fixture_path("sample.tsx"));
        assert!(result.is_ok(), "TSX parsing should succeed: {:?}", result.err());
    }

    // --- resolve_symbol ---

    #[test]
    fn resolve_symbol_finds_match() {
        let provider = TreeSitterProvider::new();
        let matches = provider
            .resolve_symbol(&fixture_path("sample.ts"), "greet")
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].symbol.name, "greet");
        assert_eq!(matches[0].symbol.kind, SymbolKind::Function);
    }

    #[test]
    fn resolve_symbol_not_found() {
        let provider = TreeSitterProvider::new();
        let result = provider.resolve_symbol(&fixture_path("sample.ts"), "nonexistent");
        assert!(matches!(result, Err(AftError::SymbolNotFound { .. })));
    }

    // --- Parse tree caching ---

    #[test]
    fn parse_cache_returns_same_tree() {
        let mut parser = FileParser::new();
        let path = fixture_path("sample.ts");

        let (tree1, _) = parser.parse(&path).unwrap();
        let tree1_root = tree1.root_node().byte_range();

        let (tree2, _) = parser.parse(&path).unwrap();
        let tree2_root = tree2.root_node().byte_range();

        // Same tree (cache hit) should return identical root node range
        assert_eq!(tree1_root, tree2_root);
    }

    // --- Python extraction ---

    #[test]
    fn py_extracts_all_symbols() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.py")).unwrap();

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"top_level_function"),
            "missing top_level_function: {:?}",
            names
        );
        assert!(names.contains(&"MyClass"), "missing MyClass: {:?}", names);
        assert!(
            names.contains(&"instance_method"),
            "missing method instance_method: {:?}",
            names
        );
        assert!(
            names.contains(&"decorated_function"),
            "missing decorated_function: {:?}",
            names
        );

        // Plan requires ≥4 symbols
        assert!(
            symbols.len() >= 4,
            "expected ≥4 symbols, got {}: {:?}",
            symbols.len(),
            names
        );
    }

    #[test]
    fn py_symbol_kinds_correct() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.py")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        assert_eq!(find("top_level_function").kind, SymbolKind::Function);
        assert_eq!(find("MyClass").kind, SymbolKind::Class);
        assert_eq!(find("instance_method").kind, SymbolKind::Method);
        assert_eq!(find("decorated_function").kind, SymbolKind::Function);
        assert_eq!(find("OuterClass").kind, SymbolKind::Class);
        assert_eq!(find("InnerClass").kind, SymbolKind::Class);
        assert_eq!(find("inner_method").kind, SymbolKind::Method);
        assert_eq!(find("outer_method").kind, SymbolKind::Method);
    }

    #[test]
    fn py_method_scope_chain() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.py")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        // Method inside MyClass
        assert_eq!(
            find("instance_method").scope_chain,
            vec!["MyClass"],
            "instance_method should have MyClass in scope chain"
        );
        assert_eq!(find("instance_method").parent.as_deref(), Some("MyClass"));

        // Method inside OuterClass > InnerClass
        assert_eq!(
            find("inner_method").scope_chain,
            vec!["OuterClass", "InnerClass"],
            "inner_method should have nested scope chain"
        );

        // InnerClass itself should have OuterClass in scope
        assert_eq!(
            find("InnerClass").scope_chain,
            vec!["OuterClass"],
            "InnerClass should have OuterClass in scope"
        );

        // Top-level function has empty scope
        assert!(
            find("top_level_function").scope_chain.is_empty(),
            "top-level function should have empty scope chain"
        );
    }

    #[test]
    fn py_decorated_function_signature() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.py")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        let sig = find("decorated_function").signature.as_ref().unwrap();
        assert!(
            sig.contains("@staticmethod"),
            "decorated function signature should include decorator: {}",
            sig
        );
        assert!(
            sig.contains("def decorated_function"),
            "signature should include function def: {}",
            sig
        );
    }

    #[test]
    fn py_ranges_valid() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.py")).unwrap();

        for s in &symbols {
            assert!(
                s.range.end_line >= s.range.start_line,
                "symbol {} has invalid range: {:?}",
                s.name,
                s.range
            );
        }
    }

    // --- Rust extraction ---

    #[test]
    fn rs_extracts_all_symbols() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.rs")).unwrap();

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"public_function"),
            "missing public_function: {:?}",
            names
        );
        assert!(
            names.contains(&"private_function"),
            "missing private_function: {:?}",
            names
        );
        assert!(
            names.contains(&"MyStruct"),
            "missing MyStruct: {:?}",
            names
        );
        assert!(names.contains(&"Color"), "missing enum Color: {:?}", names);
        assert!(
            names.contains(&"Drawable"),
            "missing trait Drawable: {:?}",
            names
        );
        // impl methods
        assert!(
            names.contains(&"new"),
            "missing impl method new: {:?}",
            names
        );

        // Plan requires ≥6 symbols
        assert!(
            symbols.len() >= 6,
            "expected ≥6 symbols, got {}: {:?}",
            symbols.len(),
            names
        );
    }

    #[test]
    fn rs_symbol_kinds_correct() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.rs")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        assert_eq!(find("public_function").kind, SymbolKind::Function);
        assert_eq!(find("private_function").kind, SymbolKind::Function);
        assert_eq!(find("MyStruct").kind, SymbolKind::Struct);
        assert_eq!(find("Color").kind, SymbolKind::Enum);
        assert_eq!(find("Drawable").kind, SymbolKind::Interface); // trait → Interface
        assert_eq!(find("new").kind, SymbolKind::Method);
    }

    #[test]
    fn rs_pub_export_detection() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.rs")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        assert!(
            find("public_function").exported,
            "pub fn should be exported"
        );
        assert!(
            !find("private_function").exported,
            "non-pub fn should not be exported"
        );
        assert!(find("MyStruct").exported, "pub struct should be exported");
        assert!(find("Color").exported, "pub enum should be exported");
        assert!(
            find("Drawable").exported,
            "pub trait should be exported"
        );
        assert!(
            find("new").exported,
            "pub fn inside impl should be exported"
        );
        assert!(
            !find("helper").exported,
            "non-pub fn inside impl should not be exported"
        );
    }

    #[test]
    fn rs_impl_method_scope_chain() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.rs")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        // `impl MyStruct { fn new() }` → scope chain = ["MyStruct"]
        assert_eq!(
            find("new").scope_chain,
            vec!["MyStruct"],
            "impl method should have type in scope chain"
        );
        assert_eq!(find("new").parent.as_deref(), Some("MyStruct"));

        // Free function has empty scope chain
        assert!(
            find("public_function").scope_chain.is_empty(),
            "free function should have empty scope chain"
        );
    }

    #[test]
    fn rs_trait_impl_scope_chain() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.rs")).unwrap();

        // `impl Drawable for MyStruct { fn draw() }` → scope = ["Drawable for MyStruct"]
        let draw = symbols.iter().find(|s| s.name == "draw").unwrap();
        assert_eq!(
            draw.scope_chain,
            vec!["Drawable for MyStruct"],
            "trait impl method should have 'Trait for Type' scope"
        );
        assert_eq!(draw.parent.as_deref(), Some("MyStruct"));
    }

    #[test]
    fn rs_ranges_valid() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.rs")).unwrap();

        for s in &symbols {
            assert!(
                s.range.end_line >= s.range.start_line,
                "symbol {} has invalid range: {:?}",
                s.name,
                s.range
            );
        }
    }

    // --- Go extraction ---

    #[test]
    fn go_extracts_all_symbols() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.go")).unwrap();

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"ExportedFunction"),
            "missing ExportedFunction: {:?}",
            names
        );
        assert!(
            names.contains(&"unexportedFunction"),
            "missing unexportedFunction: {:?}",
            names
        );
        assert!(
            names.contains(&"MyStruct"),
            "missing struct MyStruct: {:?}",
            names
        );
        assert!(
            names.contains(&"Reader"),
            "missing interface Reader: {:?}",
            names
        );
        // receiver method
        assert!(
            names.contains(&"String"),
            "missing receiver method String: {:?}",
            names
        );

        // Plan requires ≥4 symbols
        assert!(
            symbols.len() >= 4,
            "expected ≥4 symbols, got {}: {:?}",
            symbols.len(),
            names
        );
    }

    #[test]
    fn go_symbol_kinds_correct() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.go")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        assert_eq!(find("ExportedFunction").kind, SymbolKind::Function);
        assert_eq!(find("unexportedFunction").kind, SymbolKind::Function);
        assert_eq!(find("MyStruct").kind, SymbolKind::Struct);
        assert_eq!(find("Reader").kind, SymbolKind::Interface);
        assert_eq!(find("String").kind, SymbolKind::Method);
        assert_eq!(find("helper").kind, SymbolKind::Method);
    }

    #[test]
    fn go_uppercase_export_detection() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.go")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        assert!(
            find("ExportedFunction").exported,
            "ExportedFunction (uppercase) should be exported"
        );
        assert!(
            !find("unexportedFunction").exported,
            "unexportedFunction (lowercase) should not be exported"
        );
        assert!(
            find("MyStruct").exported,
            "MyStruct (uppercase) should be exported"
        );
        assert!(
            find("Reader").exported,
            "Reader (uppercase) should be exported"
        );
        assert!(
            find("String").exported,
            "String method (uppercase) should be exported"
        );
        assert!(
            !find("helper").exported,
            "helper method (lowercase) should not be exported"
        );
    }

    #[test]
    fn go_receiver_method_scope_chain() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.go")).unwrap();

        let find = |name: &str| symbols.iter().find(|s| s.name == name).unwrap();

        // `func (m *MyStruct) String()` → scope chain = ["MyStruct"]
        assert_eq!(
            find("String").scope_chain,
            vec!["MyStruct"],
            "receiver method should have type in scope chain"
        );
        assert_eq!(find("String").parent.as_deref(), Some("MyStruct"));

        // Regular function has empty scope chain
        assert!(
            find("ExportedFunction").scope_chain.is_empty(),
            "regular function should have empty scope chain"
        );
    }

    #[test]
    fn go_ranges_valid() {
        let provider = TreeSitterProvider::new();
        let symbols = provider.list_symbols(&fixture_path("sample.go")).unwrap();

        for s in &symbols {
            assert!(
                s.range.end_line >= s.range.start_line,
                "symbol {} has invalid range: {:?}",
                s.name,
                s.range
            );
        }
    }

    // --- Cross-language ---

    #[test]
    fn cross_language_all_six_produce_symbols() {
        let provider = TreeSitterProvider::new();

        let fixtures = [
            ("sample.ts", "TypeScript"),
            ("sample.tsx", "TSX"),
            ("sample.js", "JavaScript"),
            ("sample.py", "Python"),
            ("sample.rs", "Rust"),
            ("sample.go", "Go"),
        ];

        for (fixture, lang) in &fixtures {
            let symbols = provider
                .list_symbols(&fixture_path(fixture))
                .unwrap_or_else(|e| panic!("{} ({}) failed: {:?}", lang, fixture, e));
            assert!(
                symbols.len() >= 2,
                "{} should produce ≥2 symbols, got {}: {:?}",
                lang,
                symbols.len(),
                symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
            );
        }
    }
}
