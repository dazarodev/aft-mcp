use tree_sitter::Language;

use super::LangSupport;

pub struct MarkdownLang;

impl LangSupport for MarkdownLang {
    fn id(&self) -> &'static str {
        "markdown"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["md", "markdown", "mdx"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_md::LANGUAGE.into()
    }
}
