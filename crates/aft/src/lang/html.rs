use tree_sitter::Language;

use super::LangSupport;

pub struct HtmlLang;

impl LangSupport for HtmlLang {
    fn id(&self) -> &'static str {
        "html"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["html", "htm"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_html::LANGUAGE.into()
    }
}
