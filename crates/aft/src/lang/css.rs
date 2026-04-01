use tree_sitter::Language;

use super::LangSupport;

pub struct CssLang;

impl LangSupport for CssLang {
    fn id(&self) -> &'static str {
        "css"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["css", "scss"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_css::LANGUAGE.into()
    }
}
