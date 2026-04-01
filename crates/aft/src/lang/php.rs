use tree_sitter::Language;

use super::LangSupport;

pub struct PhpLang;

impl LangSupport for PhpLang {
    fn id(&self) -> &'static str {
        "php"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_php::LANGUAGE_PHP.into()
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["function_call_expression", "method_call_expression"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "function_definition",
            "method_declaration",
            "namespace_definition",
        ]
    }

    fn has_imports(&self) -> bool {
        true
    }
}
