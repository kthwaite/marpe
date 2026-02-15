// Temporary stubs — will be replaced by Task 5
pub fn render_page(title: &str, content: &str) -> String {
    format!("<html><body>{}</body></html>", content)
}

pub fn render_empty_state() -> String {
    render_page("empty", "<p>No files</p>")
}
