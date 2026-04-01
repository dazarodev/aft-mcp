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
}
