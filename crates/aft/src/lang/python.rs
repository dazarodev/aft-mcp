use tree_sitter::Language;

use super::LangSupport;

pub struct PythonLang;

impl LangSupport for PythonLang {
    fn id(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn symbol_query(&self) -> Option<&'static str> {
        Some(include_str!("../queries/python.scm"))
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &["class_definition", "function_definition"]
    }

    fn has_imports(&self) -> bool {
        true
    }
}
