use tree_sitter::Language;

use super::{EntryPointConfig, LangSupport};

pub struct RubyLang;

impl LangSupport for RubyLang {
    fn id(&self) -> &'static str {
        "ruby"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rb"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call", "method_call"]
    }

    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &["class", "method", "module"]
    }

    fn has_imports(&self) -> bool {
        true
    }

    fn entry_point_config(&self) -> EntryPointConfig {
        EntryPointConfig {
            test_exact_names: &["describe", "it", "context"],
            test_prefixes: &["test_"],
            case_sensitive: false,
            lifecycle_methods: &[],
        }
    }
}
