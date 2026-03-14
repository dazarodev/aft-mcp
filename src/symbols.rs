use serde::Serialize;

/// The kind of a discovered symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Struct,
    Interface,
    Enum,
    TypeAlias,
}

/// Location range within a source file (line/column, 0-indexed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Range {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// A symbol discovered in a source file.
#[derive(Debug, Clone, Serialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
    /// Function/method signature, e.g. `fn foo(x: i32) -> bool`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Scope chain from outermost to innermost parent, e.g. `["ClassName"]` for a method.
    pub scope_chain: Vec<String>,
    /// Whether this symbol is exported (relevant for TS/JS).
    pub exported: bool,
    /// The direct parent symbol name, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// A resolved symbol match — a `Symbol` plus the file it was found in.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolMatch {
    pub symbol: Symbol,
    pub file: String,
}
