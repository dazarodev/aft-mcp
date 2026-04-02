//! Template binding extraction for framework cross-file references.
//!
//! Parses HTML template files using tree-sitter and extracts identifiers
//! from attribute values that match binding syntax (e.g. `{handler}` in LWC).
//!
//! Only extracts bindings from actual HTML attributes — not from text content
//! between tags. This prevents false positives from prose like `{example}`.

use std::path::{Path, PathBuf};

use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};

use crate::parser::lang_registry;

/// A binding found in an HTML template attribute.
#[derive(Debug, Clone)]
pub struct TemplateBinding {
    /// The identifier referenced (e.g. "handleClick", "isLoading").
    pub identifier: String,
    /// 1-based line number in the template file.
    pub line: u32,
    /// The attribute name (e.g. "onclick", "if:true", "for:each").
    pub attribute: String,
}

/// Find the sibling template HTML file for a JS/TS controller file.
///
/// LWC convention: `component/component.js` → `component/component.html`
/// Also works for other frameworks with same-name template files.
pub fn find_sibling_template(controller_path: &Path) -> Option<PathBuf> {
    let stem = controller_path.file_stem()?.to_str()?;
    let parent = controller_path.parent()?;
    let template = parent.join(format!("{}.html", stem));
    if template.exists() {
        Some(template)
    } else {
        None
    }
}

/// Extract bindings from an HTML template file using tree-sitter.
///
/// Parses the HTML, walks all `attribute` nodes, and extracts identifiers
/// from attribute values that contain `{identifier}` patterns.
///
/// Returns empty vec if the file can't be read/parsed or has no bindings.
/// Maximum template file size to parse (10 MB). Prevents OOM on malicious files.
const MAX_TEMPLATE_SIZE: u64 = 10 * 1024 * 1024;

pub fn extract_template_bindings(template_path: &Path) -> Vec<TemplateBinding> {
    // Size guard: skip huge files to prevent OOM during tree-sitter parsing
    if let Ok(meta) = std::fs::metadata(template_path) {
        if meta.len() > MAX_TEMPLATE_SIZE {
            return Vec::new();
        }
    }

    let source = match std::fs::read_to_string(template_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let html_lang = match lang_registry().get("html") {
        Some(l) => l,
        None => return Vec::new(),
    };

    let mut parser = Parser::new();
    parser.set_language(&html_lang.grammar()).ok();

    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    // Query: find all attribute nodes with their name and value
    let query_src = r#"
        (attribute
          (attribute_name) @attr_name
          (attribute_value) @attr_value)
    "#;

    let query = match Query::new(&html_lang.grammar(), query_src) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };

    let name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "attr_name")
        .unwrap_or(0) as u32;
    let value_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "attr_value")
        .unwrap_or(1) as u32;

    let mut cursor = QueryCursor::new();
    let mut bindings = Vec::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    while let Some(m) = {
        matches.advance();
        matches.get()
    } {
        let mut attr_name_text = "";
        let mut attr_value_text = "";
        let mut value_line = 0u32;

        for cap in m.captures {
            let text = &source[cap.node.byte_range()];
            if cap.index == name_idx {
                attr_name_text = text;
            } else if cap.index == value_idx {
                attr_value_text = text;
                value_line = cap.node.start_position().row as u32 + 1;
            }
        }

        // Extract {identifier} patterns from attribute value.
        // Strip surrounding quotes if present.
        let value = attr_value_text
            .trim_start_matches('"')
            .trim_end_matches('"')
            .trim_start_matches('\'')
            .trim_end_matches('\'');

        // Match single binding: entire value is {identifier}
        if let Some(ident) = extract_single_binding(value) {
            bindings.push(TemplateBinding {
                identifier: ident.to_string(),
                line: value_line,
                attribute: attr_name_text.to_string(),
            });
        }
    }

    bindings
}

/// Extract identifier from a single binding expression like `{handleClick}`.
///
/// Returns None if the value is not a simple binding (e.g. contains spaces,
/// operators, or nested expressions).
fn extract_single_binding(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }
    let inner = trimmed[1..trimmed.len() - 1].trim_start_matches('!').trim();
    // Must be a simple identifier: alphanumeric + underscore, starts with letter
    if inner.is_empty() {
        return None;
    }
    let first = inner.chars().next()?;
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }
    // Allow dotted paths like item.Id but extract just the root identifier
    let root = inner.split('.').next()?;
    if root
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        Some(root)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_html(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn extract_event_handler_binding() {
        let dir = TempDir::new().unwrap();
        let html = write_html(
            &dir,
            "test.html",
            r#"<template>
    <button onclick={handleClick}>Click</button>
</template>"#,
        );
        let bindings = extract_template_bindings(&html);
        assert!(
            bindings.iter().any(|b| b.identifier == "handleClick"),
            "should find handleClick binding, got: {:?}",
            bindings
        );
    }

    #[test]
    fn extract_property_binding_in_attribute() {
        let dir = TempDir::new().unwrap();
        let html = write_html(
            &dir,
            "test.html",
            r#"<template>
    <div class={computedClass}>
        <lightning-input value={inputValue} disabled={isDisabled}></lightning-input>
    </div>
</template>"#,
        );
        let bindings = extract_template_bindings(&html);
        let ids: Vec<&str> = bindings.iter().map(|b| b.identifier.as_str()).collect();
        assert!(ids.contains(&"computedClass"), "missing computedClass");
        assert!(ids.contains(&"inputValue"), "missing inputValue");
        assert!(ids.contains(&"isDisabled"), "missing isDisabled");
    }

    #[test]
    fn extract_directive_bindings() {
        let dir = TempDir::new().unwrap();
        let html = write_html(
            &dir,
            "test.html",
            r#"<template>
    <template if:true={showSection}>
        <template for:each={items} for:item="item">
            <div key={item.Id}>{item.name}</div>
        </template>
    </template>
</template>"#,
        );
        let bindings = extract_template_bindings(&html);
        let ids: Vec<&str> = bindings.iter().map(|b| b.identifier.as_str()).collect();
        assert!(ids.contains(&"showSection"), "missing showSection");
        assert!(ids.contains(&"items"), "missing items");
        // item.Id → extracts "item" (root identifier)
        assert!(ids.contains(&"item"), "missing item from item.Id");
    }

    #[test]
    fn no_text_content_bindings() {
        let dir = TempDir::new().unwrap();
        let html = write_html(
            &dir,
            "test.html",
            r#"<template>
    <p>This is {notABinding} text</p>
    <span>{alsoNotABinding}</span>
</template>"#,
        );
        let bindings = extract_template_bindings(&html);
        // Text content between tags should NOT be extracted
        assert!(
            !bindings
                .iter()
                .any(|b| b.identifier == "notABinding" || b.identifier == "alsoNotABinding"),
            "should not extract text content, got: {:?}",
            bindings
        );
    }

    #[test]
    fn find_sibling_template_works() {
        let dir = TempDir::new().unwrap();
        write_html(&dir, "myComponent.html", "<template></template>");
        let js_path = dir.path().join("myComponent.js");
        std::fs::write(&js_path, "// controller").unwrap();

        let found = find_sibling_template(&js_path);
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("myComponent.html"));
    }

    #[test]
    fn find_sibling_template_missing() {
        let dir = TempDir::new().unwrap();
        let js_path = dir.path().join("noTemplate.js");
        std::fs::write(&js_path, "// controller").unwrap();

        assert!(find_sibling_template(&js_path).is_none());
    }

    #[test]
    fn extract_single_binding_edge_cases() {
        assert_eq!(extract_single_binding("{valid}"), Some("valid"));
        assert_eq!(extract_single_binding("{item.Id}"), Some("item"));
        assert_eq!(extract_single_binding("{_private}"), Some("_private"));
        assert_eq!(extract_single_binding("{}"), None);
        assert_eq!(extract_single_binding("plain"), None);
        assert_eq!(extract_single_binding("{has space}"), None);
        assert_eq!(extract_single_binding("{123invalid}"), None);
        // Negation
        assert_eq!(extract_single_binding("{!condition}"), Some("condition"));
        assert_eq!(extract_single_binding("{!}"), None);
        assert_eq!(extract_single_binding("{!!double}"), Some("double"));
    }
}
