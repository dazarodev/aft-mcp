//! Pluggable language registry.
//!
//! Each language implements `LangSupport` and is gated behind a cargo feature.
//! The `LangRegistry` collects all compiled-in languages and provides lookup
//! by file extension or language ID.

use tree_sitter::Language;

/// What a language needs to provide for aft analysis.
pub trait LangSupport: Send + Sync {
    /// Unique string ID (e.g. "typescript", "python").
    fn id(&self) -> &'static str;

    /// File extensions this language handles (e.g. &["ts"], &["py", "pyi"]).
    fn extensions(&self) -> &'static [&'static str];

    /// Tree-sitter grammar.
    fn grammar(&self) -> Language;

    /// Tree-sitter query for symbol extraction. None = no symbol support.
    fn symbol_query(&self) -> Option<&'static str> {
        None
    }

    /// AST node kinds that represent function/method calls.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// AST node kinds representing scope containers (class, function, module).
    fn scope_container_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Default indent style.
    fn default_indent(&self) -> IndentPreference {
        IndentPreference::Spaces(4)
    }

    /// Whether this language has an import system.
    fn has_imports(&self) -> bool {
        false
    }
}

/// Preferred indent style for a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentPreference {
    Spaces(u8),
    Tabs,
}

/// Global registry of all compiled-in languages.
pub struct LangRegistry {
    langs: Vec<Box<dyn LangSupport>>,
}

impl LangRegistry {
    pub fn new() -> Self {
        let mut registry = Self { langs: Vec::new() };
        registry.register_builtins();
        registry
    }

    /// Find language by file extension.
    pub fn detect(&self, ext: &str) -> Option<&dyn LangSupport> {
        self.langs
            .iter()
            .find(|l| l.extensions().contains(&ext))
            .map(|l| l.as_ref())
    }

    /// Find language by ID.
    pub fn get(&self, id: &str) -> Option<&dyn LangSupport> {
        self.langs
            .iter()
            .find(|l| l.id() == id)
            .map(|l| l.as_ref())
    }

    /// All registered language IDs.
    pub fn language_ids(&self) -> Vec<&'static str> {
        self.langs.iter().map(|l| l.id()).collect()
    }

    /// Retain only languages whose ID is in `active`.
    pub fn retain(&mut self, active: &[String]) {
        self.langs.retain(|l| active.iter().any(|a| a == l.id()));
    }

    fn register_builtins(&mut self) {
        #[cfg(feature = "lang-typescript")]
        {
            self.langs.push(Box::new(typescript::TypeScriptLang));
            self.langs.push(Box::new(typescript::TsxLang));
        }

        #[cfg(feature = "lang-javascript")]
        self.langs.push(Box::new(javascript::JavaScriptLang));

        #[cfg(feature = "lang-python")]
        self.langs.push(Box::new(python::PythonLang));

        #[cfg(feature = "lang-rust")]
        self.langs.push(Box::new(rust_lang::RustLang));

        #[cfg(feature = "lang-go")]
        self.langs.push(Box::new(go::GoLang));

        #[cfg(feature = "lang-markdown")]
        self.langs.push(Box::new(markdown::MarkdownLang));

        #[cfg(feature = "lang-css")]
        self.langs.push(Box::new(css::CssLang));

        #[cfg(feature = "lang-html")]
        self.langs.push(Box::new(html::HtmlLang));

        #[cfg(feature = "lang-apex")]
        self.langs.push(Box::new(apex::ApexLang));

        #[cfg(feature = "lang-java")]
        self.langs.push(Box::new(java::JavaLang));

        #[cfg(feature = "lang-ruby")]
        self.langs.push(Box::new(ruby::RubyLang));

        #[cfg(feature = "lang-c")]
        self.langs.push(Box::new(c::CLang));

        #[cfg(feature = "lang-cpp")]
        self.langs.push(Box::new(cpp::CppLang));

        #[cfg(feature = "lang-csharp")]
        self.langs.push(Box::new(csharp::CSharpLang));

        #[cfg(feature = "lang-php")]
        self.langs.push(Box::new(php::PhpLang));
    }
}

// Language modules — each gated behind its feature
#[cfg(feature = "lang-typescript")]
pub mod typescript;
#[cfg(feature = "lang-javascript")]
pub mod javascript;
#[cfg(feature = "lang-python")]
pub mod python;
#[cfg(feature = "lang-rust")]
pub mod rust_lang;
#[cfg(feature = "lang-go")]
pub mod go;
#[cfg(feature = "lang-markdown")]
pub mod markdown;
#[cfg(feature = "lang-css")]
pub mod css;
#[cfg(feature = "lang-html")]
pub mod html;
#[cfg(feature = "lang-apex")]
pub mod apex;
#[cfg(feature = "lang-java")]
pub mod java;
#[cfg(feature = "lang-ruby")]
pub mod ruby;
#[cfg(feature = "lang-c")]
pub mod c;
#[cfg(feature = "lang-cpp")]
pub mod cpp;
#[cfg(feature = "lang-csharp")]
pub mod csharp;
#[cfg(feature = "lang-php")]
pub mod php;
