use pulldown_cmark::{Options, Parser, html};

/// Render markdown text to an HTML fragment string.
pub fn render_markdown(input: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(input, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_paragraph() {
        let html = render_markdown("Hello, world!");
        assert_eq!(html.trim(), "<p>Hello, world!</p>");
    }

    #[test]
    fn renders_heading() {
        let html = render_markdown("# Title");
        assert_eq!(html.trim(), "<h1>Title</h1>");
    }

    #[test]
    fn renders_gfm_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = render_markdown(input);
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn renders_strikethrough() {
        let html = render_markdown("~~deleted~~");
        assert!(html.contains("<del>deleted</del>"));
    }

    #[test]
    fn renders_tasklist() {
        let html = render_markdown("- [x] done\n- [ ] todo");
        assert!(html.contains(r#"type="checkbox""#));
    }

    #[test]
    fn renders_empty_input() {
        let html = render_markdown("");
        assert_eq!(html, "");
    }
}
