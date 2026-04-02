use tree_sitter::Language;

use super::{EntryPointConfig, IndentPreference, LangSupport};

pub struct TypeScriptLang;

impl LangSupport for TypeScriptLang {
    fn id(&self) -> &'static str {
        "typescript"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn symbol_query(&self) -> Option<&'static str> {
        Some(include_str!("../queries/typescript.scm"))
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "new_expression"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "function_declaration",
            "arrow_function",
            "method_definition",
        ]
    }

    fn default_indent(&self) -> IndentPreference {
        IndentPreference::Spaces(2)
    }

    fn has_imports(&self) -> bool {
        true
    }

    fn entry_point_config(&self) -> EntryPointConfig {
        EntryPointConfig {
            test_exact_names: &["describe", "it", "test"],
            test_prefixes: &["test", "spec"],
            case_sensitive: false,
            lifecycle_methods: &[],
        }
    }
}

pub struct TsxLang;

impl LangSupport for TsxLang {
    fn id(&self) -> &'static str {
        "tsx"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tsx"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    }

    fn symbol_query(&self) -> Option<&'static str> {
        Some(include_str!("../queries/typescript.scm"))
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "new_expression"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "function_declaration",
            "arrow_function",
            "method_definition",
        ]
    }

    fn default_indent(&self) -> IndentPreference {
        IndentPreference::Spaces(2)
    }

    fn has_imports(&self) -> bool {
        true
    }

    fn entry_point_config(&self) -> EntryPointConfig {
        EntryPointConfig {
            test_exact_names: &["describe", "it", "test"],
            test_prefixes: &["test", "spec"],
            case_sensitive: false,
            lifecycle_methods: &[],
        }
    }
}
