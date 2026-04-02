use tree_sitter::Language;

use super::{EntryPointConfig, IndentPreference, LangSupport};

pub struct GoLang;

impl LangSupport for GoLang {
    fn id(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn symbol_query(&self) -> Option<&'static str> {
        Some(include_str!("../queries/go.scm"))
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &[
            "function_declaration",
            "method_declaration",
            "type_declaration",
        ]
    }

    fn default_indent(&self) -> IndentPreference {
        IndentPreference::Tabs
    }

    fn has_imports(&self) -> bool {
        true
    }

    fn entry_point_config(&self) -> EntryPointConfig {
        EntryPointConfig {
            test_exact_names: &[],
            test_prefixes: &["Test"],
            case_sensitive: true,
            lifecycle_methods: &[],
        }
    }
}
