use tree_sitter::Language;

use super::LangSupport;

pub struct CppLang;

impl LangSupport for CppLang {
    fn id(&self) -> &'static str {
        "cpp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cpp", "cc", "cxx", "hpp"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "new_expression"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "class_specifier", "namespace_definition"]
    }

    fn has_imports(&self) -> bool {
        true
    }
}
