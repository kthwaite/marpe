pub const SHELL_HTML: &str = include_str!("assets/shell.html");
pub const GITHUB_CSS: &str = include_str!("assets/github.css");
pub const GITLAB_CSS: &str = include_str!("assets/gitlab.css");
pub const BASE_CSS: &str = include_str!("assets/base.css");
pub const APP_JS: &str = include_str!("assets/app.js");

/// Render the full HTML page with content interpolated.
pub fn render_page(title: &str, content: &str, syntax_css_light: &str, syntax_css_dark: &str) -> String {
    SHELL_HTML
        .replace("{title}", title)
        .replace("{content}", content)
        .replace("{github_css}", GITHUB_CSS)
        .replace("{gitlab_css}", GITLAB_CSS)
        .replace("{base_css}", BASE_CSS)
        .replace("{app_js}", APP_JS)
        .replace("{syntax_css_light}", syntax_css_light)
        .replace("{syntax_css_dark}", syntax_css_dark)
}

/// Render the empty state page (no markdown files found).
pub fn render_empty_state(syntax_css_light: &str, syntax_css_dark: &str) -> String {
    render_page(
        "No files",
        "<p>No markdown files found in this directory.</p>",
        syntax_css_light,
        syntax_css_dark,
    )
}
