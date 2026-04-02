use tree_sitter::Language;

use super::LangSupport;

pub struct ApexLang;

impl LangSupport for ApexLang {
    fn id(&self) -> &'static str {
        "apex"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cls", "trigger", "apex"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_sfapex::apex::LANGUAGE.into()
    }

    fn symbol_query(&self) -> Option<&'static str> {
        Some(include_str!("../queries/apex.scm"))
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "interface_declaration", "enum_declaration", "trigger_declaration"]
    }
}
