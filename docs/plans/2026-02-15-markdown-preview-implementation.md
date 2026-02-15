# markdown-preview Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a single-binary local server that renders markdown files with live reload, sidebar navigation, and theme switching.

**Architecture:** Axum server with shared `AppState` (file map + broadcast channel). Notify watcher on background task pushes SSE events. Embedded HTML/CSS/JS frontend with pulldown-cmark rendering server-side.

**Tech Stack:** Rust, axum, tokio, pulldown-cmark, notify, walkdir, tracing, tower-http

**Security note:** This is a localhost-only dev tool serving the user's own files. The frontend uses `innerHTML` to swap rendered markdown content — acceptable for this threat model since all content originates from local files.

---

### Task 1: Add remaining dependencies and set up module skeleton

**Files:**
- Modify: `Cargo.toml` (via cargo add)
- Create: `src/main.rs`
- Create: `src/state.rs`
- Create: `src/render.rs`
- Create: `src/discovery.rs`
- Create: `src/handlers.rs`
- Create: `src/watcher.rs`
- Create: `src/assets.rs`

**Step 1: Add missing crates**

Run:
```bash
cargo add pulldown-cmark tokio --features full tower-http --features trace serde --features derive serde_json tokio-stream --features sync
```

**Step 2: Create module skeleton**

`src/main.rs`:
```rust
mod assets;
mod discovery;
mod handlers;
mod render;
mod state;
mod watcher;

fn main() {
    println!("Hello, world!");
}
```

`src/state.rs`, `src/render.rs`, `src/discovery.rs`, `src/handlers.rs`, `src/watcher.rs`, `src/assets.rs`: each an empty file.

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles with warnings about unused imports (that's fine)

**Step 4: Commit**

```bash
git add -A
git commit -m "Add remaining deps and module skeleton"
```

---

### Task 2: Implement markdown rendering

**Files:**
- Modify: `src/render.rs`

**Step 1: Write the tests**

In `src/render.rs`:
```rust
use pulldown_cmark::{Options, Parser, html};

/// Render markdown text to an HTML fragment string.
pub fn render_markdown(input: &str) -> String {
    todo!()
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib render`
Expected: FAIL with `todo!()` panic

**Step 3: Implement render_markdown**

Replace the `todo!()` body:
```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib render`
Expected: all 6 tests PASS

**Step 5: Commit**

```bash
git add src/render.rs
git commit -m "Implement markdown rendering with GFM extensions"
```

---

### Task 3: Implement AppState and SseEvent

**Files:**
- Modify: `src/state.rs`

**Step 1: Write the types and tests**

```rust
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "path")]
pub enum SseEvent {
    FileChanged(String),
    FileAdded(String),
    FileRemoved(String),
}

pub struct AppState {
    pub root: PathBuf,
    pub files: RwLock<HashMap<String, String>>, // relative path (as string) -> rendered HTML
    pub tx: broadcast::Sender<SseEvent>,
}

impl AppState {
    pub fn new(root: PathBuf) -> Arc<Self> {
        let (tx, _rx) = broadcast::channel(64);
        Arc::new(Self {
            root,
            files: RwLock::new(HashMap::new()),
            tx,
        })
    }

    /// Get sorted list of all file paths.
    pub async fn file_list(&self) -> Vec<String> {
        let files = self.files.read().await;
        let mut paths: Vec<String> = files.keys().cloned().collect();
        paths.sort();
        paths
    }

    /// Get rendered HTML for a path, if it exists.
    pub async fn get_rendered(&self, path: &str) -> Option<String> {
        let files = self.files.read().await;
        files.get(path).cloned()
    }

    /// Insert or update a rendered file.
    pub async fn upsert(&self, path: String, html: String) -> bool {
        let mut files = self.files.write().await;
        let is_new = !files.contains_key(&path);
        files.insert(path, html);
        is_new
    }

    /// Remove a file. Returns true if it existed.
    pub async fn remove(&self, path: &str) -> bool {
        let mut files = self.files.write().await;
        files.remove(path).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_state_has_empty_file_list() {
        let state = AppState::new(PathBuf::from("."));
        assert!(state.file_list().await.is_empty());
    }

    #[tokio::test]
    async fn upsert_and_get() {
        let state = AppState::new(PathBuf::from("."));
        let is_new = state.upsert("README.md".into(), "<p>hi</p>".into()).await;
        assert!(is_new);
        assert_eq!(
            state.get_rendered("README.md").await,
            Some("<p>hi</p>".into())
        );
    }

    #[tokio::test]
    async fn upsert_existing_returns_false() {
        let state = AppState::new(PathBuf::from("."));
        state.upsert("a.md".into(), "old".into()).await;
        let is_new = state.upsert("a.md".into(), "new".into()).await;
        assert!(!is_new);
        assert_eq!(state.get_rendered("a.md").await, Some("new".into()));
    }

    #[tokio::test]
    async fn remove_existing() {
        let state = AppState::new(PathBuf::from("."));
        state.upsert("a.md".into(), "html".into()).await;
        assert!(state.remove("a.md").await);
        assert!(state.get_rendered("a.md").await.is_none());
    }

    #[tokio::test]
    async fn remove_nonexistent() {
        let state = AppState::new(PathBuf::from("."));
        assert!(!state.remove("nope.md").await);
    }

    #[tokio::test]
    async fn file_list_is_sorted() {
        let state = AppState::new(PathBuf::from("."));
        state.upsert("z.md".into(), "".into()).await;
        state.upsert("a.md".into(), "".into()).await;
        state.upsert("m.md".into(), "".into()).await;
        assert_eq!(state.file_list().await, vec!["a.md", "m.md", "z.md"]);
    }

    #[test]
    fn sse_event_serializes_as_tagged() {
        let event = SseEvent::FileChanged("test.md".into());
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"FileChanged""#));
        assert!(json.contains(r#""path":"test.md""#));
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test --lib state`
Expected: all 7 tests PASS

**Step 3: Commit**

```bash
git add src/state.rs
git commit -m "Implement AppState and SseEvent types"
```

---

### Task 4: Implement file discovery

**Files:**
- Modify: `src/discovery.rs`

**Step 1: Write the function and tests**

```rust
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::render::render_markdown;

/// Returns true if path should be skipped (hidden dirs, node_modules)
fn should_skip(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.starts_with('.') || s == "node_modules"
    })
}

/// Walk `root` directory, find all .md files, render them.
/// Returns a map of relative path (string) -> rendered HTML.
pub fn discover_and_render(root: &Path) -> HashMap<String, String> {
    let mut files = HashMap::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(path);
        if should_skip(relative) {
            continue;
        }
        let rel_str = relative.to_string_lossy().to_string();
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let html = render_markdown(&content);
                info!(path = %rel_str, "Rendered markdown file");
                files.insert(rel_str, html);
            }
            Err(e) => {
                warn!(path = %rel_str, error = %e, "Failed to read markdown file");
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn setup_temp_dir() -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("_scratch/discovery_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discovers_md_files() {
        let dir = setup_temp_dir();
        fs::write(dir.join("README.md"), "# Hello").unwrap();
        fs::write(dir.join("notes.md"), "some notes").unwrap();
        fs::write(dir.join("ignore.txt"), "not markdown").unwrap();

        let files = discover_and_render(&dir);
        assert_eq!(files.len(), 2);
        assert!(files.contains_key("README.md"));
        assert!(files.contains_key("notes.md"));
        assert!(!files.contains_key("ignore.txt"));
    }

    #[test]
    fn discovers_nested_files() {
        let dir = setup_temp_dir();
        fs::create_dir_all(dir.join("docs/guide")).unwrap();
        fs::write(dir.join("docs/guide/intro.md"), "# Intro").unwrap();

        let files = discover_and_render(&dir);
        assert!(files.contains_key("docs/guide/intro.md"));
    }

    #[test]
    fn skips_hidden_dirs() {
        let dir = setup_temp_dir();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git/info.md"), "hidden").unwrap();
        fs::write(dir.join("visible.md"), "shown").unwrap();

        let files = discover_and_render(&dir);
        assert_eq!(files.len(), 1);
        assert!(files.contains_key("visible.md"));
    }

    #[test]
    fn skips_node_modules() {
        let dir = setup_temp_dir();
        fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        fs::write(dir.join("node_modules/pkg/README.md"), "npm").unwrap();
        fs::write(dir.join("top.md"), "top").unwrap();

        let files = discover_and_render(&dir);
        assert_eq!(files.len(), 1);
        assert!(files.contains_key("top.md"));
    }

    #[test]
    fn renders_content_correctly() {
        let dir = setup_temp_dir();
        fs::write(dir.join("test.md"), "**bold**").unwrap();

        let files = discover_and_render(&dir);
        let html = files.get("test.md").unwrap();
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn empty_directory() {
        let dir = setup_temp_dir();
        let files = discover_and_render(&dir);
        assert!(files.is_empty());
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test --lib discovery`
Expected: all 6 tests PASS

**Step 3: Commit**

```bash
git add src/discovery.rs
git commit -m "Implement file discovery with hidden dir/node_modules filtering"
```

---

### Task 5: Create embedded frontend assets

**Files:**
- Create: `src/assets/shell.html`
- Create: `src/assets/github.css`
- Create: `src/assets/gitlab.css`
- Create: `src/assets/base.css`
- Create: `src/assets/app.js`
- Modify: `src/assets.rs`

This task has no automated tests — it's static frontend content. We will integration-test it in Task 9.

**Step 1: Write the HTML shell**

`src/assets/shell.html` — uses `{title}`, `{content}`, `{github_css}`, `{gitlab_css}`, `{base_css}`, `{app_js}` as format placeholders.

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} — markdown-preview</title>
    <style id="github-theme">{github_css}</style>
    <style id="gitlab-theme">{gitlab_css}</style>
    <style>{base_css}</style>
</head>
<body class="theme-github">
    <header>
        <span class="logo">markdown-preview</span>
        <button id="theme-toggle" title="Switch theme">GitHub</button>
    </header>
    <div class="layout">
        <nav id="sidebar">
            <div class="sidebar-header">Files</div>
            <ul id="file-tree"></ul>
        </nav>
        <main>
            <article class="markdown-body">{content}</article>
        </main>
    </div>
    <script>{app_js}</script>
</body>
</html>
```

**Step 2: Write base.css**

CSS grid layout (~80 lines): header top, sidebar left (260px), main fills rest. Sidebar styling with scroll, active highlight. Header bar with logo and toggle button.

**Step 3: Write github.css**

Scoped to `.theme-github .markdown-body`. Based on github-markdown-css (sindresorhus). Essential rules: typography, headings, code blocks, tables, blockquotes, lists, hr, images. ~200 lines.

**Step 4: Write gitlab.css**

Scoped to `.theme-gitlab .markdown-body`. GitLab's markdown style approximation: system fonts, different heading sizes, different code block colors. ~150 lines.

**Step 5: Write app.js**

Client-side JavaScript (~80 lines):
- **SSE**: `EventSource('/events')`, on `FileChanged` re-fetch `/raw/{path}` for current file and update the `<article>` content, on `FileAdded`/`FileRemoved` re-fetch sidebar
- **Sidebar**: fetch `/api/files`, build flat `<ul>` list (nested tree structure deferred to a later version), click handler fetches `/raw/{path}` and updates content area + URL via `pushState`
- **Theme toggle**: switch body class between `theme-github`/`theme-gitlab`, update button text, persist to `localStorage`
- **Init**: load sidebar, restore theme, handle `popstate`

**Step 6: Write assets.rs**

```rust
pub const SHELL_HTML: &str = include_str!("assets/shell.html");
pub const GITHUB_CSS: &str = include_str!("assets/github.css");
pub const GITLAB_CSS: &str = include_str!("assets/gitlab.css");
pub const BASE_CSS: &str = include_str!("assets/base.css");
pub const APP_JS: &str = include_str!("assets/app.js");

/// Render the full HTML page with content interpolated.
pub fn render_page(title: &str, content: &str) -> String {
    SHELL_HTML
        .replace("{title}", title)
        .replace("{content}", content)
        .replace("{github_css}", GITHUB_CSS)
        .replace("{gitlab_css}", GITLAB_CSS)
        .replace("{base_css}", BASE_CSS)
        .replace("{app_js}", APP_JS)
}

/// Render the empty state page (no markdown files found).
pub fn render_empty_state() -> String {
    render_page(
        "No files",
        "<p>No markdown files found in this directory.</p>",
    )
}
```

**Step 7: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 8: Commit**

```bash
git add src/assets.rs src/assets/
git commit -m "Add embedded frontend assets (HTML shell, CSS themes, JS)"
```

---

### Task 6: Implement HTTP route handlers

**Files:**
- Modify: `src/handlers.rs`

**Step 1: Write the handlers**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Redirect,
    },
    Json,
};
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::assets;
use crate::state::AppState;

/// GET / — redirect to README.md or first file or empty state
pub async fn index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let files = state.file_list().await;
    if files.contains(&"README.md".to_string()) {
        Redirect::temporary("/view/README.md").into_response()
    } else if let Some(first) = files.first() {
        Redirect::temporary(&format!("/view/{first}")).into_response()
    } else {
        Html(assets::render_empty_state()).into_response()
    }
}

/// GET /view/*path — full HTML page
pub async fn view_file(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.get_rendered(&path).await {
        Some(html) => Html(assets::render_page(&path, &html)).into_response(),
        None => (StatusCode::NOT_FOUND, Html("File not found".to_string())).into_response(),
    }
}

/// GET /raw/*path — bare HTML fragment
pub async fn raw_file(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.get_rendered(&path).await {
        Some(html) => Html(html).into_response(),
        None => (StatusCode::NOT_FOUND, "File not found".to_string()).into_response(),
    }
}

/// GET /api/files — JSON list of file paths
pub async fn file_list(State(state): State<Arc<AppState>>) -> Json<Vec<String>> {
    Json(state.file_list().await)
}

/// GET /events — SSE stream
pub async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => {
            let json = serde_json::to_string(&event).ok()?;
            Some(Ok(Event::default().data(json)))
        }
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 3: Commit**

```bash
git add src/handlers.rs
git commit -m "Implement HTTP route handlers"
```

---

### Task 7: Implement file watcher

**Files:**
- Modify: `src/watcher.rs`

**Step 1: Write the watcher**

```rust
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

use crate::render::render_markdown;
use crate::state::{AppState, SseEvent};

fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("md")
}

fn should_skip(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.starts_with('.') || s == "node_modules"
    })
}

fn relative_path(path: &Path, root: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

pub fn start_watcher(
    state: Arc<AppState>,
) -> notify::Result<RecommendedWatcher> {
    let root = state.root.clone();
    let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(256);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.blocking_send(res);
        },
        notify::Config::default(),
    )?;

    watcher.watch(&root, RecursiveMode::Recursive)?;
    info!(path = %root.display(), "Watching for file changes");

    tokio::spawn(async move {
        while let Some(result) = rx.recv().await {
            let event = match result {
                Ok(e) => e,
                Err(e) => {
                    error!(error = %e, "File watcher error");
                    continue;
                }
            };

            for path in &event.paths {
                if !is_markdown(path) || should_skip(path) {
                    continue;
                }
                let rel = match relative_path(path, &root) {
                    Some(r) => r,
                    None => continue,
                };

                match event.kind {
                    EventKind::Create(_) => {
                        if let Ok(content) = tokio::fs::read_to_string(path).await {
                            let html = render_markdown(&content);
                            state.upsert(rel.clone(), html).await;
                            info!(path = %rel, "File added");
                            let _ = state.tx.send(SseEvent::FileAdded(rel));
                        }
                    }
                    EventKind::Modify(_) => {
                        if let Ok(content) = tokio::fs::read_to_string(path).await {
                            let html = render_markdown(&content);
                            state.upsert(rel.clone(), html).await;
                            info!(path = %rel, "File changed");
                            let _ = state.tx.send(SseEvent::FileChanged(rel));
                        }
                    }
                    EventKind::Remove(_) => {
                        if state.remove(&rel).await {
                            info!(path = %rel, "File removed");
                            let _ = state.tx.send(SseEvent::FileRemoved(rel));
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    Ok(watcher)
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 3: Commit**

```bash
git add src/watcher.rs
git commit -m "Implement file watcher with notify"
```

---

### Task 8: Wire everything together in main.rs

**Files:**
- Modify: `src/main.rs`

**Step 1: Write main**

```rust
mod assets;
mod discovery;
mod handlers;
mod render;
mod state;
mod watcher;

use axum::Router;
use axum::routing::get;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let root = root.canonicalize().expect("Invalid directory path");
    info!(path = %root.display(), "Serving markdown files from");

    let state = state::AppState::new(root.clone());

    // Initial file discovery
    let files = discovery::discover_and_render(&root);
    let count = files.len();
    {
        let mut map = state.files.write().await;
        *map = files;
    }
    info!(count, "Discovered markdown files");

    // Start file watcher
    let _watcher = watcher::start_watcher(Arc::clone(&state))
        .expect("Failed to start file watcher");

    // Build router
    let app = Router::new()
        .route("/", get(handlers::index))
        .route("/view/{*path}", get(handlers::view_file))
        .route("/raw/{*path}", get(handlers::raw_file))
        .route("/api/files", get(handlers::file_list))
        .route("/events", get(handlers::events))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = "0.0.0.0:13181";
    info!(addr, "Server listening");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install ctrl+c handler");
    info!("Shutting down");
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles clean

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Wire up server, router, watcher, and graceful shutdown"
```

---

### Task 9: Smoke test the running server

**Files:** none (manual verification)

**Step 1: Create test markdown files**

```bash
mkdir -p _scratch/smoke
echo "# Hello" > _scratch/smoke/README.md
echo "## Notes\n\n- Item 1\n- Item 2" > _scratch/smoke/notes.md
mkdir -p _scratch/smoke/docs
echo "# Guide\n\nSome guide content." > _scratch/smoke/docs/guide.md
```

**Step 2: Run the server**

Run: `cargo run -- _scratch/smoke`

Verify in logs:
- "Serving markdown files from" with the path
- "Discovered markdown files" with count 3
- "Watching for file changes"
- "Server listening" at 0.0.0.0:13181

**Step 3: Test each endpoint**

- `curl -s -o /dev/null -w '%{http_code}' http://localhost:13181/` — should be `307` redirect
- `curl http://localhost:13181/view/README.md` — full HTML page with sidebar and rendered content
- `curl http://localhost:13181/raw/README.md` — just the `<h1>Hello</h1>` fragment
- `curl http://localhost:13181/api/files` — `["README.md","docs/guide.md","notes.md"]`
- `curl http://localhost:13181/view/nonexistent.md` — 404 status

**Step 4: Test live reload**

In another terminal:
```bash
echo "# Updated" > _scratch/smoke/README.md
```
Server logs should show "File changed" for README.md.

**Step 5: Fix any issues found, commit fixes**

```bash
git add -A && git commit -m "Fix issues found during smoke testing"
```

---

### Task 10: Add .gitignore and final cleanup

**Files:**
- Modify: `.gitignore`

**Step 1: Write .gitignore**

```
/target
/_scratch
```

**Step 2: Commit project files**

```bash
git add .gitignore Cargo.toml Cargo.lock
git commit -m "Add .gitignore, commit Cargo.toml and lockfile"
```

---

## Task Dependencies

```
Task 1 (skeleton) -> Task 2 (render) -> Task 4 (discovery)
Task 1 (skeleton) -> Task 3 (state) -> Task 4 (discovery)
Task 3 (state) -> Task 5 (assets)
Task 3 (state) -> Task 6 (handlers)
Task 3 (state) + Task 2 (render) -> Task 7 (watcher)
Tasks 4-7 -> Task 8 (main)
Task 8 -> Task 9 (smoke test)
Task 9 -> Task 10 (cleanup)
```

Tasks 2 and 3 can be done in parallel. Tasks 5, 6, and 7 can be done in parallel after their dependencies.
