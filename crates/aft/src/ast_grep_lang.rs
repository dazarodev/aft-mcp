//! AST-grep language implementations for ast-grep-core.
//!
//! Provides `AstGrepLang` — a registry-backed struct that implements `Language`
//! and `LanguageExt` traits from ast-grep-core. Any language registered in the
//! `LangRegistry` with non-empty `call_node_kinds` is automatically available
//! for structural pattern matching.

use std::borrow::Cow;

use ast_grep_core::language::Language;
use ast_grep_core::matcher::PatternError;
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::Pattern;

use crate::parser::{lang_registry, LangId};

/// Registry-backed language for AST pattern matching via ast-grep.
///
/// Created from any language ID in the `LangRegistry` that supports
/// structural analysis (has non-empty `call_node_kinds`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstGrepLang {
    lang_id: &'static str,
}

/// Common aliases for language IDs (short forms → canonical registry IDs).
fn resolve_alias(s: &str) -> &str {
    match s {
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" | "pyi" => "python",
        "rs" => "rust",
        "golang" => "go",
        "cc" | "cxx" | "hpp" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        other => other,
    }
}

impl AstGrepLang {
    /// Convert from the crate's `LangId` string.
    pub fn from_lang_id(lang_id: LangId) -> Option<Self> {
        Self::from_str(lang_id)
    }

    /// Parse from a string (case-insensitive, supports common aliases).
    ///
    /// Returns `None` for markup-only languages (no call_node_kinds) that
    /// don't have meaningful AST patterns.
    pub fn from_str(s: &str) -> Option<Self> {
        let lowered = s.to_lowercase();
        let canonical = resolve_alias(&lowered);
        let registry = lang_registry();
        registry.get(canonical).and_then(|lang| {
            // Skip markup-only languages with no structural patterns
            if lang.call_node_kinds().is_empty() {
                None
            } else {
                Some(Self {
                    lang_id: lang.id(),
                })
            }
        })
    }

    /// File extensions associated with this language.
    pub fn extensions(&self) -> &'static [&'static str] {
        lang_registry()
            .get(self.lang_id)
            .map(|l| l.extensions())
            .unwrap_or(&[])
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

    /// All language IDs available for ast-grep (non-markup languages), sorted.
    pub fn available_languages() -> Vec<&'static str> {
        let mut langs: Vec<&'static str> = lang_registry()
            .language_ids()
            .into_iter()
            .filter(|id| {
                lang_registry()
                    .get(id)
                    .map(|l| !l.call_node_kinds().is_empty())
                    .unwrap_or(false)
            })
            .collect();
        langs.sort_unstable();
        langs
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

    /// Some languages (Python, Rust) don't accept `$` as a valid identifier
    /// character. We replace `$` with the expando char (`µ`) in meta-variable
    /// positions so tree-sitter can parse the pattern as valid code.
    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        let expando = self.expando_char();
        if expando == '$' {
            return Cow::Borrowed(query);
        }
        let mut ret = Vec::with_capacity(query.len());
        let mut dollar_count = 0;
        for c in query.chars() {
            if c == '$' {
                dollar_count += 1;
                continue;
            }
            let need_replace = matches!(c, 'A'..='Z' | '_') || dollar_count == 3;
            let sigil = if need_replace { expando } else { '$' };
            ret.extend(std::iter::repeat(sigil).take(dollar_count));
            dollar_count = 0;
            ret.push(c);
        }
        // trailing anonymous multiple ($$$)
        let sigil = if dollar_count == 3 { expando } else { '$' };
        ret.extend(std::iter::repeat(sigil).take(dollar_count));
        Cow::Owned(ret.into_iter().collect())
    }

    fn expando_char(&self) -> char {
        lang_registry()
            .get(self.lang_id)
            .map(|l| l.expando_char())
            .unwrap_or('$')
    }
}

impl LanguageExt for AstGrepLang {
    fn get_ts_language(&self) -> TSLanguage {
        use crate::parser::grammar_for;
        grammar_for(self.lang_id).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast_grep_core::tree_sitter::LanguageExt;

    #[test]
    fn test_from_str_standard() {
        assert!(AstGrepLang::from_str("typescript").is_some());
        assert!(AstGrepLang::from_str("tsx").is_some());
        assert!(AstGrepLang::from_str("javascript").is_some());
        assert!(AstGrepLang::from_str("python").is_some());
        assert!(AstGrepLang::from_str("rust").is_some());
        assert!(AstGrepLang::from_str("go").is_some());
    }

    #[test]
    fn test_from_str_new_languages() {
        assert!(AstGrepLang::from_str("apex").is_some());
        assert!(AstGrepLang::from_str("java").is_some());
        assert!(AstGrepLang::from_str("ruby").is_some());
        assert!(AstGrepLang::from_str("c").is_some());
        assert!(AstGrepLang::from_str("cpp").is_some());
        assert!(AstGrepLang::from_str("csharp").is_some());
        assert!(AstGrepLang::from_str("php").is_some());
    }

    #[test]
    fn test_from_str_aliases() {
        assert!(AstGrepLang::from_str("ts").is_some());
        assert!(AstGrepLang::from_str("js").is_some());
        assert!(AstGrepLang::from_str("py").is_some());
        assert!(AstGrepLang::from_str("rs").is_some());
        assert!(AstGrepLang::from_str("golang").is_some());
        assert!(AstGrepLang::from_str("cs").is_some());
        assert!(AstGrepLang::from_str("rb").is_some());
    }

    #[test]
    fn test_from_str_markup_rejected() {
        assert!(AstGrepLang::from_str("markdown").is_none());
        assert!(AstGrepLang::from_str("css").is_none());
        assert!(AstGrepLang::from_str("html").is_none());
    }

    #[test]
    fn test_ast_grep_basic() {
        let lang = AstGrepLang::from_str("typescript").unwrap();
        let grep = lang.ast_grep("const x = 1;");
        let root = grep.root();
        assert!(root.find("const $X = $Y").is_some());
    }

    #[test]
    fn test_python_function_pattern() {
        let lang = AstGrepLang::from_str("python").unwrap();
        let source = "def add(a, b):\n    return a + b\n";
        let grep = lang.ast_grep(source);
        let root = grep.root();
        let found = root.find("def $FUNC($$$):\n    return $X");
        assert!(found.is_some(), "Python function pattern should match");
        let node = found.unwrap();
        assert_eq!(node.text(), "def add(a, b):\n    return a + b");
    }

    #[test]
    fn test_python_expression_pattern() {
        let lang = AstGrepLang::from_str("python").unwrap();
        let source = "x = self.value + 1\n";
        let grep = lang.ast_grep(source);
        let root = grep.root();
        let found = root.find("self.$ATTR + $X");
        assert!(found.is_some(), "Python expression pattern should match");
    }

    #[test]
    fn test_rust_function_pattern() {
        let lang = AstGrepLang::from_str("rust").unwrap();
        let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let grep = lang.ast_grep(source);
        let root = grep.root();
        let found = root.find("fn $NAME($$$) -> $RET { $$$BODY }");
        assert!(found.is_some(), "Rust function pattern should match");
    }

    #[test]
    fn test_expando_char() {
        assert_eq!(
            AstGrepLang::from_str("python").unwrap().expando_char(),
            '\u{00B5}'
        );
        assert_eq!(
            AstGrepLang::from_str("rust").unwrap().expando_char(),
            '\u{00B5}'
        );
        assert_eq!(
            AstGrepLang::from_str("typescript").unwrap().expando_char(),
            '$'
        );
        assert_eq!(
            AstGrepLang::from_str("javascript").unwrap().expando_char(),
            '$'
        );
        assert_eq!(
            AstGrepLang::from_str("go").unwrap().expando_char(),
            '$'
        );
    }

    #[test]
    fn test_pre_process_pattern_python() {
        let lang = AstGrepLang::from_str("python").unwrap();
        let result = lang.pre_process_pattern("def $FUNC($$$):");
        assert!(result.contains('\u{00B5}'), "Should contain µ expando char");
        assert!(
            !result.contains('$'),
            "Should not contain $ after preprocessing"
        );
    }

    #[test]
    fn test_available_languages() {
        let available = AstGrepLang::available_languages();
        assert!(available.contains(&"typescript"));
        assert!(available.contains(&"apex"));
        assert!(available.contains(&"java"));
        assert!(!available.contains(&"markdown"));
        assert!(!available.contains(&"css"));
    }
}
