use tree_sitter::Language;

use super::{EntryPointConfig, LangSupport};

pub struct RustLang;

impl LangSupport for RustLang {
    fn id(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn symbol_query(&self) -> Option<&'static str> {
        Some(include_str!("../queries/rust.scm"))
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "macro_invocation"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &[
            "function_item",
            "struct_item",
            "impl_item",
            "trait_item",
            "mod_item",
        ]
    }

    fn has_imports(&self) -> bool {
        true
    }

    fn entry_point_config(&self) -> EntryPointConfig {
        EntryPointConfig {
            test_exact_names: &[],
            test_prefixes: &["test_"],
            case_sensitive: false,
            lifecycle_methods: &[],
        }
    }

    fn expando_char(&self) -> char {
        '\u{00B5}'
    }
}
