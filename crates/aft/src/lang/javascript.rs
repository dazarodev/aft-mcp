use tree_sitter::Language;

use super::{EntryPointConfig, IndentPreference, LangSupport};

pub struct JavaScriptLang;

impl LangSupport for JavaScriptLang {
    fn id(&self) -> &'static str {
        "javascript"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "jsx"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn symbol_query(&self) -> Option<&'static str> {
        Some(include_str!("../queries/javascript.scm"))
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
