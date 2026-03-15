//! AST-grep language implementations for ast-grep-core.
//!
//! Provides `AstGrepLang` enum that implements `Language` and `LanguageExt`
//! traits from ast-grep-core, mapping to our tree-sitter language grammars.

use std::borrow::Cow;

use ast_grep_core::language::Language;
use ast_grep_core::matcher::PatternError;
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::Pattern;

use crate::parser::LangId;

/// Supported languages for AST pattern matching via ast-grep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AstGrepLang {
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Rust,
    Go,
}

impl AstGrepLang {
    /// Convert from the crate's `LangId` enum.
    pub fn from_lang_id(lang_id: &LangId) -> Option<Self> {
        match lang_id {
            LangId::TypeScript => Some(Self::TypeScript),
            LangId::Tsx => Some(Self::Tsx),
            LangId::JavaScript => Some(Self::JavaScript),
            LangId::Python => Some(Self::Python),
            LangId::Rust => Some(Self::Rust),
            LangId::Go => Some(Self::Go),
            // Markdown, CSS, HTML etc. don't have meaningful AST patterns
            _ => None,
        }
    }

    /// Parse from a string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "typescript" | "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "javascript" | "js" => Some(Self::JavaScript),
            "python" | "py" => Some(Self::Python),
            "rust" | "rs" => Some(Self::Rust),
            "go" | "golang" => Some(Self::Go),
            _ => None,
        }
    }

    /// File extensions associated with this language.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::TypeScript => &["ts", "mts", "cts"],
            Self::Tsx => &["tsx"],
            Self::JavaScript => &["js", "mjs", "cjs", "jsx"],
            Self::Python => &["py", "pyi"],
            Self::Rust => &["rs"],
            Self::Go => &["go"],
        }
    }

    /// Check if a file extension matches this language.
    pub fn matches_extension(&self, ext: &str) -> bool {
        self.extensions().contains(&ext)
    }

    /// Check if a file path matches this language based on its extension.
    pub fn matches_path(&self, path: &std::path::Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        self.matches_extension(ext)
    }
}

impl Language for AstGrepLang {
    fn kind_to_id(&self, kind: &str) -> u16 {
        let ts_lang: TSLanguage = self.get_ts_language();
        ts_lang.id_for_node_kind(kind, /* named */ true)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.get_ts_language()
            .field_id_for_name(field)
            .map(|f| f.get())
    }

    fn build_pattern(
        &self,
        builder: &ast_grep_core::matcher::PatternBuilder,
    ) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, self.clone()))
    }

    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        Cow::Borrowed(query)
    }
}

impl LanguageExt for AstGrepLang {
    fn get_ts_language(&self) -> TSLanguage {
        match self {
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast_grep_core::tree_sitter::LanguageExt;

    #[test]
    fn test_from_str() {
        assert_eq!(
            AstGrepLang::from_str("typescript"),
            Some(AstGrepLang::TypeScript)
        );
        assert_eq!(AstGrepLang::from_str("tsx"), Some(AstGrepLang::Tsx));
        assert_eq!(
            AstGrepLang::from_str("javascript"),
            Some(AstGrepLang::JavaScript)
        );
        assert_eq!(AstGrepLang::from_str("python"), Some(AstGrepLang::Python));
        assert_eq!(AstGrepLang::from_str("rust"), Some(AstGrepLang::Rust));
        assert_eq!(AstGrepLang::from_str("go"), Some(AstGrepLang::Go));
        assert_eq!(AstGrepLang::from_str("markdown"), None);
    }

    #[test]
    fn test_ast_grep_basic() {
        let lang = AstGrepLang::TypeScript;
        let grep = lang.ast_grep("const x = 1;");
        let root = grep.root();
        assert!(root.find("const $X = $Y").is_some());
    }
}
