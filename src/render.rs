use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};
use std::sync::LazyLock;
use syntect::html::{ClassedHTMLGenerator, ClassStyle};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

/// Render markdown text to an HTML fragment string with syntax highlighting.
pub fn render_markdown(input: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(input, options);

    let mut output = String::new();
    let mut code_buf: Option<(String, String)> = None; // (language, accumulated text)

    let events: Vec<Event> = parser.collect();
    let mut highlighted_events: Vec<Event> = Vec::new();

    for event in events {
        match &event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                let lang_str = lang.split_whitespace().next().unwrap_or("").to_string();
                code_buf = Some((lang_str, String::new()));
                continue;
            }
            Event::Text(text) if code_buf.is_some() => {
                code_buf.as_mut().unwrap().1.push_str(text);
                continue;
            }
            Event::End(TagEnd::CodeBlock) if code_buf.is_some() => {
                let (lang, code) = code_buf.take().unwrap();
                let highlighted = try_highlight(&lang, &code);
                highlighted_events.push(Event::Html(CowStr::from(highlighted)));
                continue;
            }
            _ => {}
        }
        highlighted_events.push(event);
    }

    html::push_html(&mut output, highlighted_events.into_iter());
    output
}

fn try_highlight(lang: &str, code: &str) -> String {
    let ss = &*SYNTAX_SET;

    let syntax = ss
        .find_syntax_by_token(lang)
        .or_else(|| ss.find_syntax_by_name(lang));

    match syntax {
        Some(syn) => {
            let mut html_generator =
                ClassedHTMLGenerator::new_with_class_style(syn, ss, ClassStyle::Spaced);
            for line in LinesWithEndings::from(code) {
                let _ = html_generator.parse_html_for_line_which_includes_newline(line);
            }
            format!(
                "<pre class=\"highlight\"><code>{}</code></pre>",
                html_generator.finalize()
            )
        }
        None => plain_code_block(lang, code),
    }
}

fn plain_code_block(lang: &str, code: &str) -> String {
    let escaped = code
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if lang.is_empty() {
        format!("<pre><code>{escaped}</code></pre>\n")
    } else {
        format!("<pre><code class=\"language-{lang}\">{escaped}</code></pre>\n")
    }
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

    #[test]
    fn highlights_rust_code_block() {
        let input = "```rust\nfn main() {}\n```";
        let html = render_markdown(input);
        assert!(html.contains("<pre class=\"highlight\">"));
        assert!(html.contains("main"));
    }

    #[test]
    fn highlights_python_code_block() {
        let input = "```python\ndef hello():\n    pass\n```";
        let html = render_markdown(input);
        assert!(html.contains("<pre class=\"highlight\">"));
        assert!(html.contains("hello"));
    }

    #[test]
    fn unrecognized_language_falls_back() {
        let input = "```unknownlang\nsome code\n```";
        let html = render_markdown(input);
        assert!(html.contains("some code"));
        assert!(html.contains("<pre"));
    }

    #[test]
    fn indented_code_block_no_highlight() {
        let input = "    indented code";
        let html = render_markdown(input);
        assert!(html.contains("<pre><code>"));
        assert!(html.contains("indented code"));
    }

    #[test]
    fn fenced_block_no_language() {
        let input = "```\nplain code\n```";
        let html = render_markdown(input);
        assert!(html.contains("plain code"));
        assert!(html.contains("<pre"));
    }
}
