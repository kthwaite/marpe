# Syntax Highlighting + HTTPS Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add server-side syntax highlighting via syntect and optional HTTPS via mkcert `--tls` flag.

**Architecture:** Modify the render pipeline to intercept pulldown_cmark code block events and highlight via syntect. Add CLI argument parsing and conditional TLS server binding via axum-server.

**Tech Stack:** syntect, axum-server (tls-rustls), pulldown-cmark event API

---

### Task 1: Add new dependencies

**Files:**
- Modify: `Cargo.toml` (via cargo add)

**Step 1: Add syntect and axum-server**

Run:
```bash
cargo add syntect
cargo add axum-server -F tls-rustls
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Add syntect and axum-server dependencies"
```

---

### Task 2: Add syntax highlighting to the render pipeline

**Files:**
- Modify: `src/render.rs`

**Step 1: Write the failing tests**

Add these tests to the existing `#[cfg(test)] mod tests` block in `src/render.rs`:

```rust
#[test]
fn highlights_rust_code_block() {
    let input = "```rust\nfn main() {}\n```";
    let html = render_markdown(input);
    // syntect produces <pre style="..."><span style="...">
    assert!(html.contains("<pre style="));
    assert!(html.contains("<span style="));
    // Should contain the function name
    assert!(html.contains("main"));
}

#[test]
fn highlights_python_code_block() {
    let input = "```python\ndef hello():\n    pass\n```";
    let html = render_markdown(input);
    assert!(html.contains("<pre style="));
    assert!(html.contains("hello"));
}

#[test]
fn unrecognized_language_falls_back() {
    let input = "```unknownlang\nsome code\n```";
    let html = render_markdown(input);
    // Should still render as a code block, just without highlighting spans
    assert!(html.contains("some code"));
    assert!(html.contains("<pre"));
}

#[test]
fn indented_code_block_no_highlight() {
    let input = "    indented code";
    let html = render_markdown(input);
    // Indented code blocks have no language, should render as plain <pre><code>
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test render`
Expected: FAIL — the current `render_markdown` uses `push_html` which doesn't produce `<pre style=` output

**Step 3: Implement syntax-highlighted rendering**

Replace the entire `render_markdown` function body. The approach: walk pulldown_cmark events manually instead of using `push_html`. When we encounter a fenced code block with a recognized language, collect its text and run it through syntect. Otherwise, pass events through to `push_html`.

```rust
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;
use std::sync::LazyLock;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

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
    let theme = &THEME_SET.themes["InspiredGitHub"];

    // Try to find syntax by token (extension), then by name
    let syntax = ss
        .find_syntax_by_token(lang)
        .or_else(|| ss.find_syntax_by_name(lang));

    match syntax {
        Some(syn) => {
            highlighted_html_for_string(code, ss, syn, theme).unwrap_or_else(|_| {
                plain_code_block(lang, code)
            })
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
```

Remove the old `use pulldown_cmark::{Options, Parser, html};` line at the top since we now import more items.

**Step 4: Run tests to verify they pass**

Run: `cargo test render`
Expected: all 11 tests PASS (6 existing + 5 new)

**Step 5: Commit**

```bash
git add src/render.rs
git commit -m "Add server-side syntax highlighting via syntect"
```

---

### Task 3: Add CLI argument parsing

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`

**Step 1: Write the CLI parser**

Create `src/cli.rs`:

```rust
use std::path::PathBuf;

pub struct Args {
    pub root: PathBuf,
    pub tls: bool,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

pub fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut tls = false;
    let mut cert: Option<PathBuf> = None;
    let mut key: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tls" => tls = true,
            "--cert" => cert = args.next().map(PathBuf::from),
            "--key" => key = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                eprintln!("Usage: markdown-preview [OPTIONS] [DIRECTORY]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --tls          Enable HTTPS (uses mkcert certificates)");
                eprintln!("  --cert <PATH>  TLS certificate file (PEM)");
                eprintln!("  --key <PATH>   TLS private key file (PEM)");
                eprintln!("  -h, --help     Show this help");
                std::process::exit(0);
            }
            other if !other.starts_with('-') => {
                root = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("Unknown option: {other}");
                std::process::exit(1);
            }
        }
    }

    let root = root.unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    Args { root, tls, cert, key }
}

#[cfg(test)]
mod tests {
    // CLI parsing is tested via integration/smoke tests since it reads process args
}
```

**Step 2: Add `mod cli;` to main.rs**

Add `mod cli;` after the other module declarations in `src/main.rs`.

**Step 3: Update main.rs to use parse_args()**

Replace the argument parsing section in main (lines 19-22) with:

```rust
    let args = cli::parse_args();
    let root = args.root.canonicalize().expect("Invalid directory path");
```

Remove the old `use std::path::PathBuf;` import — it's still needed but only for `shutdown_signal` return. Actually check if it's still used; if not, remove it.

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles. The `args.tls`, `args.cert`, `args.key` fields are unused for now — that's fine.

**Step 5: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "Add CLI argument parsing with --tls, --cert, --key flags"
```

---

### Task 4: Add mkcert certificate discovery

**Files:**
- Create: `src/tls.rs`

**Step 1: Write the mkcert discovery module**

```rust
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

/// Resolve TLS certificate and key paths.
/// If explicit paths are given, use those.
/// Otherwise, look for mkcert certs in the CAROOT, generating if needed.
pub fn resolve_certs(
    cert: Option<PathBuf>,
    key: Option<PathBuf>,
) -> Result<(PathBuf, PathBuf), String> {
    if let (Some(c), Some(k)) = (cert, key) {
        if !c.exists() {
            return Err(format!("Certificate file not found: {}", c.display()));
        }
        if !k.exists() {
            return Err(format!("Key file not found: {}", k.display()));
        }
        return Ok((c, k));
    }

    // Find mkcert CAROOT
    let caroot_output = Command::new("mkcert")
        .arg("-CAROOT")
        .output()
        .map_err(|_| "mkcert is not installed. Install it with: brew install mkcert".to_string())?;

    if !caroot_output.status.success() {
        return Err("Failed to run mkcert -CAROOT".to_string());
    }

    let caroot = String::from_utf8_lossy(&caroot_output.stdout)
        .trim()
        .to_string();
    let caroot = PathBuf::from(caroot);

    let cert_path = caroot.join("localhost.pem");
    let key_path = caroot.join("localhost-key.pem");

    if cert_path.exists() && key_path.exists() {
        info!(
            cert = %cert_path.display(),
            key = %key_path.display(),
            "Using existing mkcert certificates"
        );
        return Ok((cert_path, key_path));
    }

    // Generate certs
    info!("Generating mkcert certificates for localhost");
    let gen_output = Command::new("mkcert")
        .current_dir(&caroot)
        .arg("localhost")
        .output()
        .map_err(|e| format!("Failed to run mkcert: {e}"))?;

    if !gen_output.status.success() {
        let stderr = String::from_utf8_lossy(&gen_output.stderr);
        return Err(format!("mkcert failed: {stderr}"));
    }

    if cert_path.exists() && key_path.exists() {
        info!(
            cert = %cert_path.display(),
            key = %key_path.display(),
            "Generated mkcert certificates"
        );
        Ok((cert_path, key_path))
    } else {
        Err("mkcert ran but certificates not found at expected paths".to_string())
    }
}
```

**Step 2: Add `mod tls;` to main.rs**

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 4: Commit**

```bash
git add src/tls.rs src/main.rs
git commit -m "Add mkcert certificate discovery and generation"
```

---

### Task 5: Wire TLS into the server

**Files:**
- Modify: `src/main.rs`

**Step 1: Update main.rs to conditionally bind with TLS**

Replace the server binding section (lines 52-59) in `src/main.rs`. The new main function should look like this after the router is built:

```rust
    let addr = "0.0.0.0:13181";

    if args.tls {
        let (cert_path, key_path) = tls::resolve_certs(args.cert, args.key)
            .expect("Failed to resolve TLS certificates");

        let rustls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_path, &key_path)
            .await
            .expect("Failed to load TLS certificates");

        info!(addr, "Server listening on https://localhost:13181");
        axum_server::bind_rustls(addr.parse().unwrap(), rustls_config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        info!(addr, "Server listening on http://localhost:13181");
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .unwrap();
    }
```

Note: `axum_server::bind_rustls` uses its own graceful shutdown mechanism. For simplicity in v1, the TLS path doesn't wire `shutdown_signal` — ctrl+c will still terminate the process.

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles clean

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Wire TLS support into server with --tls flag"
```

---

### Task 6: Smoke test both features

**Files:** none (manual verification)

**Step 1: Test syntax highlighting**

```bash
mkdir -p _scratch/highlight
cat > _scratch/highlight/README.md << 'MDEOF'
# Code Examples

```rust
fn main() {
    println!("Hello, world!");
}
```

```python
def greet(name):
    return f"Hello, {name}!"
```

```
no language specified
```
MDEOF

cargo run -- _scratch/highlight
```

In another terminal:
```bash
curl -s http://localhost:13181/raw/README.md | grep -c 'style='
```
Expected: multiple matches (syntect inline styles present)

```bash
curl -s http://localhost:13181/raw/README.md | grep 'no language specified'
```
Expected: present in plain code block

Kill the server.

**Step 2: Test TLS (if mkcert is installed)**

```bash
cargo run -- --tls _scratch/highlight
```

In another terminal:
```bash
curl -sk https://localhost:13181/api/files
```
Expected: `["README.md"]`

Kill the server.

**Step 3: Test --help**

```bash
cargo run -- --help
```
Expected: usage text with --tls, --cert, --key options

**Step 4: Fix any issues, commit**

```bash
git add -A && git commit -m "Fix issues found during smoke testing"
```

---

## Task Dependencies

```
Task 1 (deps) -> Task 2 (syntax highlighting)
Task 1 (deps) -> Task 3 (CLI) -> Task 4 (mkcert) -> Task 5 (TLS wiring)
Tasks 2 + 5 -> Task 6 (smoke test)
```

Tasks 2 and 3 can run in parallel after Task 1.
