use tree_sitter::Language;

use super::LangSupport;

pub struct CLang;

impl LangSupport for CLang {
    fn id(&self) -> &'static str {
        "c"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["c", "h"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_c::LANGUAGE.into()
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn has_imports(&self) -> bool {
        true
    }
}
