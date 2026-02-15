# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**marpe** — a local markdown preview server. Discovers all `.md` files under a directory, renders them with syntax highlighting, and serves live-reloading HTML on localhost. Supports GitHub/GitLab styling, light/dark mode, and optional TLS via mkcert.

## Build & Run

```sh
cargo build                # debug build
cargo build --release      # release build
cargo run                  # serve CWD on port 13181
cargo run -- --port 8080 --open /path/to/docs   # custom port, auto-open browser
cargo run -- --tls         # HTTPS via mkcert
```

## Tests

```sh
cargo test                 # all tests
cargo test -- --test-threads=1   # if tests conflict on filesystem
cargo test render::tests   # tests in a specific module
cargo test discovers_md    # single test by name substring
```

Tests use `_scratch/` subdirectories (under `CARGO_MANIFEST_DIR`) for temp files; this directory is gitignored.

## Architecture

Single-binary axum server with these modules:

- **`main.rs`** — Wires everything together: CLI parsing, initial discovery, watcher startup, router construction, port binding with auto-increment fallback, graceful shutdown.
- **`cli.rs`** — Hand-rolled arg parser (no clap). `Args` struct holds all CLI options.
- **`state.rs`** — `AppState` (shared via `Arc`): holds the root path, an `RwLock<BTreeMap<String, String>>` mapping relative paths to rendered HTML, a `broadcast::Sender<SseEvent>` for live reload, precomputed syntax-highlight CSS scoped to `.theme-light`/`.theme-dark`, and a `PageShell` for efficient page rendering.
- **`discovery.rs`** — Walks the root directory with `walkdir`, filters to `.md` files (skipping hidden dirs and `node_modules`), renders them in parallel via `rayon`.
- **`render.rs`** — `pulldown-cmark` with GFM extensions (tables, strikethrough, tasklists, footnotes). Fenced code blocks are syntax-highlighted with `syntect` using CSS class-based styling.
- **`watcher.rs`** — `notify` filesystem watcher. On create/modify/remove of `.md` files, re-renders and pushes `SseEvent` through the broadcast channel.
- **`handlers.rs`** — Axum route handlers: index redirect, `/view/*path` (full page), `/raw/*path` (HTML fragment for client-side swap), `/api/files` (JSON), `/events` (SSE stream).
- **`assets.rs`** — `include_str!` embeds for HTML shell, CSS (github/gitlab/base), JS, and `Monokai.tmtheme`. `PageShell` pre-bakes static assets into the template at startup; per-request rendering only substitutes title, content, and syntax CSS.
- **`tls.rs`** — Resolves TLS certs: uses explicit paths if given, otherwise locates/generates mkcert localhost certs in CAROOT.

### Key data flow

1. Startup: `discovery::discover_and_render` → parallel render → populate `AppState.files`
2. Runtime: `watcher` detects FS changes → re-renders single file → updates `AppState.files` → broadcasts `SseEvent`
3. Client: SSE listener in `app.js` receives event → fetches `/raw/{path}` → swaps `.markdown-body` innerHTML (no full page reload)

### Routes

| Route | Handler | Purpose |
|---|---|---|
| `GET /` | `index` | Redirect to README.md or first file |
| `GET /view/{*path}` | `view_file` | Full HTML page |
| `GET /raw/{*path}` | `raw_file` | Bare HTML fragment (for live reload) |
| `GET /api/files` | `file_list` | JSON array of paths |
| `GET /events` | `events` | SSE stream |

## Conventions

- Static assets are embedded at compile time via `include_str!` in `src/assets/`. Changes to CSS/JS/HTML require recompilation.
- Syntax highlighting uses CSS classes (not inline styles) — theme CSS is generated from syntect at startup, scoped to `.theme-light`/`.theme-dark`, and injected into pages. Default themes: InspiredGitHub (light), Monokai (dark, bundled as `src/assets/Monokai.tmtheme`).
- File filtering: hidden directories (`.` prefix) and `node_modules` are always skipped, in both discovery and watcher.
- Rust edition 2024.
