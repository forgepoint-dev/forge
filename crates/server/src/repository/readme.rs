use pulldown_cmark::{html, Options, Parser};

/// Detects README file in a list of repository entries
pub fn detect_readme_file(entries: &[super::models::RepositoryEntryNode]) -> Option<String> {
    let readme_names = [
        "README.md",
        "readme.md",
        "Readme.md",
        "README.markdown",
        "readme.markdown",
        "README.adoc",
        "readme.adoc",
        "README.asciidoc",
        "readme.asciidoc",
    ];

    for name in &readme_names {
        if let Some(entry) = entries.iter().find(|e| {
            e.name == *name && e.kind == super::models::RepositoryEntryKind::File
        }) {
            return Some(entry.path.clone());
        }
    }

    None
}

/// Renders markdown content to HTML
pub fn render_markdown(content: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(content, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Renders AsciiDoc content to HTML (placeholder for now)
/// AsciiDoc support in pure Rust is limited, so we return a message for now
pub fn render_asciidoc(_content: &str) -> String {
    "<p><em>AsciiDoc rendering not yet supported on the server. Please use Markdown format.</em></p>".to_string()
}

/// Renders README content based on file extension
pub fn render_readme(content: &str, filename: &str) -> String {
    let ext = filename
        .rsplit('.')
        .next()
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "adoc" | "asciidoc" => render_asciidoc(content),
        _ => render_markdown(content),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::models::{RepositoryEntryKind, RepositoryEntryNode};

    #[test]
    fn test_detect_readme_file_uppercase() {
        let entries = vec![
            RepositoryEntryNode {
                name: "README.md".to_string(),
                path: "README.md".to_string(),
                kind: RepositoryEntryKind::File,
                size: Some(100),
            },
            RepositoryEntryNode {
                name: "src".to_string(),
                path: "src".to_string(),
                kind: RepositoryEntryKind::Directory,
                size: None,
            },
        ];

        let result = detect_readme_file(&entries);
        assert_eq!(result, Some("README.md".to_string()));
    }

    #[test]
    fn test_detect_readme_file_lowercase() {
        let entries = vec![RepositoryEntryNode {
            name: "readme.md".to_string(),
            path: "readme.md".to_string(),
            kind: RepositoryEntryKind::File,
            size: Some(100),
        }];

        let result = detect_readme_file(&entries);
        assert_eq!(result, Some("readme.md".to_string()));
    }

    #[test]
    fn test_detect_readme_file_not_found() {
        let entries = vec![RepositoryEntryNode {
            name: "index.html".to_string(),
            path: "index.html".to_string(),
            kind: RepositoryEntryKind::File,
            size: Some(100),
        }];

        let result = detect_readme_file(&entries);
        assert_eq!(result, None);
    }

    #[test]
    fn test_detect_readme_ignores_directories() {
        let entries = vec![RepositoryEntryNode {
            name: "README.md".to_string(),
            path: "README.md".to_string(),
            kind: RepositoryEntryKind::Directory,
            size: None,
        }];

        let result = detect_readme_file(&entries);
        assert_eq!(result, None);
    }

    #[test]
    fn test_render_markdown_basic() {
        let content = "# Hello\n\nThis is **bold** text.";
        let html = render_markdown(content);

        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_render_markdown_table() {
        let content = "| Header |\n|--------|\n| Cell   |";
        let html = render_markdown(content);

        assert!(html.contains("<table>"));
    }

    #[test]
    fn test_render_readme_markdown() {
        let content = "# Test";
        let html = render_readme(content, "README.md");

        assert!(html.contains("<h1>"));
    }

    #[test]
    fn test_render_readme_asciidoc() {
        let html = render_readme("= Title", "README.adoc");

        assert!(html.contains("not yet supported"));
    }
}
