use tree_sitter::Language;

use super::LangSupport;

pub struct CSharpLang;

impl LangSupport for CSharpLang {
    fn id(&self) -> &'static str {
        "csharp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cs"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["invocation_expression", "object_creation_expression"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "method_declaration",
            "interface_declaration",
            "namespace_declaration",
        ]
    }

    fn has_imports(&self) -> bool {
        true
    }
}
