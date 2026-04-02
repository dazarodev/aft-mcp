use tree_sitter::Language;

use super::{EntryPointConfig, LangSupport};

pub struct JavaLang;

impl LangSupport for JavaLang {
    fn id(&self) -> &'static str {
        "java"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_java::LANGUAGE.into()
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "object_creation_expression"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "method_declaration",
            "interface_declaration",
        ]
    }

    fn has_imports(&self) -> bool {
        true
    }

    fn entry_point_config(&self) -> EntryPointConfig {
        EntryPointConfig {
            test_exact_names: &[],
            test_prefixes: &["test"],
            case_sensitive: false,
            lifecycle_methods: &[],
        }
    }

    fn export_modifiers(&self) -> &'static [&'static str] {
        &["public"]
    }
}
