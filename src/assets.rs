pub const SHELL_HTML: &str = include_str!("assets/shell.html");
pub const GITHUB_CSS: &str = include_str!("assets/github.css");
pub const GITLAB_CSS: &str = include_str!("assets/gitlab.css");
pub const BASE_CSS: &str = include_str!("assets/base.css");
pub const APP_JS: &str = include_str!("assets/app.js");

/// A pre-built page shell with all static assets baked in.
/// Only `{title}`, `{content}`, `{syntax_css_light}`, and `{syntax_css_dark}` remain as placeholders.
pub struct PageShell {
    template: String,
}

impl PageShell {
    pub fn new() -> Self {
        let template = SHELL_HTML
            .replace("{github_css}", GITHUB_CSS)
            .replace("{gitlab_css}", GITLAB_CSS)
            .replace("{base_css}", BASE_CSS)
            .replace("{app_js}", APP_JS);
        Self { template }
    }

    pub fn render(&self, title: &str, content: &str, syntax_css_light: &str, syntax_css_dark: &str) -> String {
        self.template
            .replace("{title}", title)
            .replace("{content}", content)
            .replace("{syntax_css_light}", syntax_css_light)
            .replace("{syntax_css_dark}", syntax_css_dark)
    }

    pub fn render_empty(&self, syntax_css_light: &str, syntax_css_dark: &str) -> String {
        self.render(
            "No files",
            "<p>No markdown files found in this directory.</p>",
            syntax_css_light,
            syntax_css_dark,
        )
    }
}
