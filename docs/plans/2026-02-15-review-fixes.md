# Review Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all 11 findings from the correctness/security/efficiency review.

**Architecture:** Each fix is isolated to 1-2 files. No new crates needed. Tasks ordered by severity, grouped where fixes interact (e.g. watcher rename handling touches the same match arm as the blocking_send fix).

**Tech Stack:** Rust (edition 2024), axum, notify 8.x, pulldown-cmark, syntect, vanilla JS

**Scope note:** Finding #3 (raw HTML passthrough) is intentionally deferred. This is a local-docs tool and sanitizing would break legitimate HTML in markdown. If the user wants it later, add `ammonia` behind `--sanitize`.

---

### Task 1: Fix XSS in code-fence language attribute (Finding #2)

**Files:**
- Modify: `src/render.rs:72-81`
- Test: `src/render.rs` (inline tests)

**Step 1: Write the failing test**

Add to `src/render.rs` tests module:

```rust
#[test]
fn lang_attribute_is_escaped() {
    let input = "```foo\"onmouseover=\"alert(1)\ncode\n```";
    let html = render_markdown(input);
    assert!(!html.contains(r#"onmouseover"#));
    assert!(html.contains("&quot;") || html.contains("<pre><code>"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test lang_attribute_is_escaped`
Expected: FAIL -- the unescaped `"` injects into the attribute.

**Step 3: Write minimal implementation**

Replace the `plain_code_block` function in `src/render.rs:72-82`:

```rust
fn plain_code_block(lang: &str, code: &str) -> String {
    let escaped = code
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let sanitized_lang: String = lang.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '+' || *c == '.')
        .collect();
    if sanitized_lang.is_empty() {
        format!("<pre><code>{escaped}</code></pre>\n")
    } else {
        format!("<pre><code class=\"language-{sanitized_lang}\">{escaped}</code></pre>\n")
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test lang_attribute_is_escaped`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo test`
Expected: All pass

**Step 6: Commit**

```
fix: sanitize language token in code fence to prevent XSS
```

---

### Task 2: Handle rename events in watcher (Finding #1)

**Files:**
- Modify: `src/watcher.rs:1-92`

**Step 1: Update imports**

Add `ModifyKind` and `RenameMode` to the notify import at `src/watcher.rs:1`:

```rust
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::{ModifyKind, RenameMode}};
```

**Step 2: Rewrite the match arm for Modify**

Replace the match block at `src/watcher.rs:62-86` with logic that distinguishes rename from content modification. On macOS (fsevent), renames arrive as `Modify(Name(Any))` with only `event.paths` telling us what happened. With `RenameMode::Both`, paths are `[from, to]`.

The practical approach for a local preview tool: treat `Name(From)` as a remove, `Name(To)` as a create, and `Name(Both)` as remove-old + create-new using the two paths. For `Name(Any)`, check if the file still exists -- if yes, upsert; if not, remove. This covers all backends.

Replace `src/watcher.rs:52-87` (the `for path in &event.paths` loop and match block) with:

```rust
            match event.kind {
                EventKind::Modify(ModifyKind::Name(mode)) => {
                    match mode {
                        RenameMode::Both => {
                            // paths[0] = old, paths[1] = new
                            if let (Some(from), Some(to)) = (event.paths.first(), event.paths.get(1)) {
                                if is_markdown(from) && !should_skip(from) {
                                    if let Some(rel) = relative_path(from, &root) {
                                        if state.remove(&rel).await {
                                            info!(path = %rel, "File renamed away");
                                            let _ = state.tx.send(SseEvent::FileRemoved(rel));
                                        }
                                    }
                                }
                                if is_markdown(to) && !should_skip(to) {
                                    if let Some(rel) = relative_path(to, &root) {
                                        if let Ok(content) = tokio::fs::read_to_string(to).await {
                                            let html = render_markdown(&content);
                                            state.upsert(rel.clone(), html).await;
                                            info!(path = %rel, "File renamed to");
                                            let _ = state.tx.send(SseEvent::FileAdded(rel));
                                        }
                                    }
                                }
                            }
                        }
                        RenameMode::From => {
                            for path in &event.paths {
                                if !is_markdown(path) || should_skip(path) { continue; }
                                if let Some(rel) = relative_path(path, &root) {
                                    if state.remove(&rel).await {
                                        info!(path = %rel, "File renamed away");
                                        let _ = state.tx.send(SseEvent::FileRemoved(rel));
                                    }
                                }
                            }
                        }
                        RenameMode::To => {
                            for path in &event.paths {
                                if !is_markdown(path) || should_skip(path) { continue; }
                                if let Some(rel) = relative_path(path, &root) {
                                    if let Ok(content) = tokio::fs::read_to_string(path).await {
                                        let html = render_markdown(&content);
                                        state.upsert(rel.clone(), html).await;
                                        info!(path = %rel, "File renamed to");
                                        let _ = state.tx.send(SseEvent::FileAdded(rel));
                                    }
                                }
                            }
                        }
                        _ => {
                            // RenameMode::Any (macOS fsevent) -- check if file exists
                            for path in &event.paths {
                                if !is_markdown(path) || should_skip(path) { continue; }
                                if let Some(rel) = relative_path(path, &root) {
                                    if path.exists() {
                                        if let Ok(content) = tokio::fs::read_to_string(path).await {
                                            let html = render_markdown(&content);
                                            let is_new = state.upsert(rel.clone(), html).await;
                                            if is_new {
                                                info!(path = %rel, "File appeared (rename)");
                                                let _ = state.tx.send(SseEvent::FileAdded(rel));
                                            } else {
                                                info!(path = %rel, "File changed (rename)");
                                                let _ = state.tx.send(SseEvent::FileChanged(rel));
                                            }
                                        }
                                    } else if state.remove(&rel).await {
                                        info!(path = %rel, "File gone (rename)");
                                        let _ = state.tx.send(SseEvent::FileRemoved(rel));
                                    }
                                }
                            }
                        }
                    }
                }
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in &event.paths {
                        if !is_markdown(path) || should_skip(path) { continue; }
                        let rel = match relative_path(path, &root) {
                            Some(r) => r,
                            None => continue,
                        };
                        if let Ok(content) = tokio::fs::read_to_string(path).await {
                            let html = render_markdown(&content);
                            let is_new = state.upsert(rel.clone(), html).await;
                            let event_kind = if is_new { "added" } else { "changed" };
                            info!(path = %rel, "File {}", event_kind);
                            let _ = state.tx.send(if is_new {
                                SseEvent::FileAdded(rel)
                            } else {
                                SseEvent::FileChanged(rel)
                            });
                        }
                    }
                }
                EventKind::Remove(_) => {
                    for path in &event.paths {
                        if !is_markdown(path) || should_skip(path) { continue; }
                        if let Some(rel) = relative_path(path, &root) {
                            if state.remove(&rel).await {
                                info!(path = %rel, "File removed");
                                let _ = state.tx.send(SseEvent::FileRemoved(rel));
                            }
                        }
                    }
                }
                _ => {}
            }
```

Note: The `Create` and `Modify` (non-rename) arms are collapsed into one since the logic is identical.

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All pass (no existing unit tests for watcher; it is integration-level)

**Step 4: Verify compilation**

Run: `cargo check`
Expected: No errors or warnings

**Step 5: Commit**

```
fix: handle rename events in watcher to prevent stale file entries
```

---

### Task 3: Use non-blocking try_send in watcher callback (Finding #7)

**Files:**
- Modify: `src/watcher.rs:33-36`

**Step 1: Replace blocking_send with try_send**

Change `src/watcher.rs:34-35`:

```rust
        move |res| {
            let _ = tx.try_send(res);
        },
```

Using `try_send` means under extreme event flood, some events get dropped rather than stalling the notify producer thread. This is acceptable because the watcher is advisory and the SSE client can always full-refresh.

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All pass

**Step 4: Commit**

```
fix: use non-blocking try_send in watcher callback to prevent stall
```

---

### Task 4: Fix client-side URL encoding and popstate double-push (Findings #4, #5)

**Files:**
- Modify: `src/assets/app.js`

**Step 1: Rewrite app.js with URL encoding and split navigateTo/renderPath**

Key changes:
- Add `encodePath()` that encodes each path segment individually
- All URL construction uses `encodePath()` instead of raw concatenation
- Split `navigateTo()` into `renderPath()` (render only) and `navigateTo()` (render + pushState) -- fixes Finding #5 (popstate double-push)
- `onpopstate` calls `renderPath()` instead of `navigateTo()`
- Sidebar active link comparison uses decoded pathname for reliable matching

Replace `src/assets/app.js` entirely:

```javascript
(function() {
    function encodePath(path) {
        return path.split('/').map(encodeURIComponent).join('/');
    }

    const currentPath = () => decodeURIComponent(location.pathname.replace(/^\/view\//, ''));

    // SSE
    const es = new EventSource('/events');
    es.onmessage = (e) => {
        const event = JSON.parse(e.data);
        if (event.type === 'FileChanged' && event.path === currentPath()) {
            fetch('/raw/' + encodePath(currentPath()))
                .then(r => r.text())
                .then(html => { document.querySelector('.markdown-body').innerHTML = html; }); // eslint-disable-line no-param-reassign
        }
        if (event.type === 'FileAdded' || event.type === 'FileRemoved') {
            loadSidebar();
        }
    };

    // Sidebar
    async function loadSidebar() {
        const res = await fetch('/api/files');
        const files = await res.json();
        const tree = document.getElementById('file-tree');
        tree.innerHTML = ''; // eslint-disable-line no-param-reassign
        files.forEach(f => {
            const li = document.createElement('li');
            const a = document.createElement('a');
            a.href = '/view/' + encodePath(f);
            a.textContent = f;
            a.onclick = (e) => {
                e.preventDefault();
                navigateTo(f);
            };
            if (f === currentPath()) a.classList.add('active');
            li.appendChild(a);
            tree.appendChild(li);
        });
    }

    async function renderPath(path) {
        const res = await fetch('/raw/' + encodePath(path));
        const html = await res.text();
        document.querySelector('.markdown-body').innerHTML = html; // eslint-disable-line no-param-reassign
        document.querySelectorAll('#file-tree a').forEach(a => {
            a.classList.toggle('active', decodeURIComponent(a.pathname) === '/view/' + path);
        });
    }

    async function navigateTo(path) {
        await renderPath(path);
        history.pushState(null, '', '/view/' + encodePath(path));
    }

    window.onpopstate = () => {
        const path = currentPath();
        if (path) renderPath(path);
    };

    // Theme (Light/Dark)
    const themeToggle = document.getElementById('theme-toggle');
    function setTheme(theme) {
        document.body.classList.remove('theme-light', 'theme-dark');
        document.body.classList.add('theme-' + theme);
        themeToggle.textContent = theme.charAt(0).toUpperCase() + theme.slice(1);
        localStorage.setItem('md-preview-theme', theme);
    }
    themeToggle.onclick = () => {
        setTheme(document.body.classList.contains('theme-light') ? 'dark' : 'light');
    };
    const savedTheme = localStorage.getItem('md-preview-theme') || 'light';
    setTheme(savedTheme);

    // Style (GitHub/GitLab)
    const styleToggle = document.getElementById('style-toggle');
    function setStyle(style) {
        document.body.classList.remove('style-github', 'style-gitlab');
        document.body.classList.add('style-' + style);
        styleToggle.textContent = style.charAt(0).toUpperCase() + style.slice(1);
        localStorage.setItem('md-preview-style', style);
    }
    styleToggle.onclick = () => {
        setStyle(document.body.classList.contains('style-github') ? 'gitlab' : 'github');
    };
    const savedStyle = localStorage.getItem('md-preview-style') || 'github';
    setStyle(savedStyle);

    loadSidebar();
})();
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors (assets are embedded at compile time via `include_str!`)

**Step 3: Commit**

```
fix: encode URL path segments and fix popstate double-push in client JS
```

---

### Task 5: Prune directory traversal in discovery (Finding #6)

**Files:**
- Modify: `src/discovery.rs:19-34`
- Test: `src/discovery.rs` (inline tests)

**Step 1: Replace filter with filter_entry for early pruning**

Replace `src/discovery.rs:19-34`:

```rust
pub fn discover_and_render(root: &Path) -> HashMap<String, String> {
    let entries: Vec<_> = WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap_or(path);
            !should_skip(relative)
        })
        .filter_map(|e| e.ok())
        .filter(|entry| {
            let path = entry.path();
            path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md")
        })
        .collect();
```

`filter_entry` prevents `WalkDir` from descending into directories that match `should_skip`, cutting entire subtrees.

**Step 2: Run full test suite**

Run: `cargo test`
Expected: All pass including `skips_hidden_dirs`, `skips_node_modules`

**Step 3: Commit**

```
perf: use filter_entry to prune skipped directories during discovery walk
```

---

### Task 6: Validate --cert/--key both-or-none (Finding #8)

**Files:**
- Modify: `src/cli.rs:25-81`

**Step 1: Fix silent None on missing value for --cert and --key**

Replace lines 29-30 of `src/cli.rs`:

```rust
            "--cert" => {
                cert = Some(PathBuf::from(args.next().unwrap_or_else(|| {
                    eprintln!("Missing path for --cert");
                    std::process::exit(1);
                })));
            }
            "--key" => {
                key = Some(PathBuf::from(args.next().unwrap_or_else(|| {
                    eprintln!("Missing path for --key");
                    std::process::exit(1);
                })));
            }
```

**Step 2: Add both-or-none validation**

Insert after the `while` loop, before the `let root =` line:

```rust
    if cert.is_some() != key.is_some() {
        eprintln!("Error: --cert and --key must be provided together");
        std::process::exit(1);
    }
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 4: Commit**

```
fix: validate --cert and --key are provided together, error on missing values
```

---

### Task 7: Fix port probe overflow near u16::MAX (Finding #9)

**Files:**
- Modify: `src/main.rs:54`

**Step 1: Replace arithmetic with saturating_add**

Replace `src/main.rs:54`:

```rust
    let end_port = args.port.saturating_add(9);
    for p in args.port..=end_port {
```

Also update the error message at line 70:

```rust
    let listener = listener.expect("Could not find a free port in range");
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```
fix: prevent port probe overflow near u16::MAX with saturating_add
```

---

### Task 8: Switch state to BTreeMap for sorted file list (Finding #11)

**Files:**
- Modify: `src/state.rs`
- Modify: `src/discovery.rs`

**Step 1: Replace HashMap with BTreeMap in state.rs**

Change `src/state.rs` line 2 import:

```rust
use std::collections::BTreeMap;
```

Change the field type at line 17:

```rust
    pub files: RwLock<BTreeMap<String, String>>,
```

Change the constructor at line 42:

```rust
            files: RwLock::new(BTreeMap::new()),
```

Simplify `file_list` (lines 50-55) since BTreeMap iterates in order:

```rust
    pub async fn file_list(&self) -> Vec<String> {
        let files = self.files.read().await;
        files.keys().cloned().collect()
    }
```

**Step 2: Update discovery.rs return type**

Change `src/discovery.rs` line 2 import:

```rust
use std::collections::BTreeMap;
```

Change the function signature at line 19:

```rust
pub fn discover_and_render(root: &Path) -> BTreeMap<String, String> {
```

The `.collect()` at line 54 will now collect into `BTreeMap` automatically.

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All pass. `file_list_is_sorted` test still works since BTreeMap is inherently sorted.

**Step 4: Commit**

```
perf: use BTreeMap for file storage to avoid re-sorting on every request
```

---

### Task 9: Prebuild static page shell (Finding #10)

**Files:**
- Modify: `src/assets.rs`
- Modify: `src/state.rs`
- Modify: `src/handlers.rs`

**Step 1: Create PageShell struct in assets.rs**

Replace `src/assets.rs` entirely:

```rust
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
```

**Step 2: Store PageShell in AppState**

Add to `src/state.rs`:

Import at top:
```rust
use crate::assets::PageShell;
```

Add field to AppState struct:
```rust
    pub page_shell: PageShell,
```

Add to constructor (inside `Arc::new(Self { ... })`):
```rust
            page_shell: PageShell::new(),
```

**Step 3: Update handlers**

In `src/handlers.rs`, replace calls:

In `index()`:
```rust
Html(state.page_shell.render_empty(&state.syntax_css_light, &state.syntax_css_dark)).into_response()
```

In `view_file()`:
```rust
Html(state.page_shell.render(&path, &html, &state.syntax_css_light, &state.syntax_css_dark)).into_response()
```

Remove `use crate::assets;` from handlers.rs since it is no longer used there.

**Step 4: Run full test suite**

Run: `cargo test`
Expected: All pass

**Step 5: Commit**

```
perf: prebuild page shell at startup, substitute only dynamic parts per request
```

---

### Task 10: Final verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All pass

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No errors

**Step 3: Verify release build**

Run: `cargo build --release`
Expected: Clean build
