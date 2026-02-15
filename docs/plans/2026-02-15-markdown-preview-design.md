# markdown-preview Design

## Summary

A single-binary local server that discovers all markdown files in a directory, renders them to HTML, and serves a live-reloading preview on `localhost:13181` with a sidebar file tree and GitHub/GitLab theme switching.

## Decisions

- **Markdown parsing**: pulldown-cmark with GFM extensions (tables, strikethrough, task lists, footnotes)
- **Live reload**: Server-Sent Events (SSE) via axum's native support
- **Themes**: CSS embedded in the binary via `include_str!()`; toggle stored in `localStorage`
- **Navigation**: persistent sidebar file tree

## Core Data Structures

```rust
struct AppState {
    root: PathBuf,                           // directory being served
    files: RwLock<HashMap<PathBuf, String>>,  // relative path -> rendered HTML
    tx: broadcast::Sender<SseEvent>,          // file change notifications
}

enum SseEvent {
    FileChanged { path: String },
    FileAdded { path: String },
    FileRemoved { path: String },
}
```

`RwLock` from tokio for concurrent reader access with occasional writer updates. Broadcast channel fans out SSE events to all connected browsers.

## File Discovery & Rendering

- Startup: `walkdir` recursively finds `*.md` files, renders each with pulldown-cmark, populates the files map
- Re-render on change: `notify` fires event, re-read and re-render the affected file, update map, broadcast SSE event
- On delete: remove from map, broadcast `FileRemoved`
- pulldown-cmark options: `ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_TASKLISTS | ENABLE_FOOTNOTES`
- No syntax highlighting in v1

## HTTP Layer

Server: axum on `0.0.0.0:13181`, tokio runtime.

### Routes

- `GET /` — redirect to `/view/README.md` if present, else first file alphabetically, else empty state
- `GET /view/*path` — full HTML page with sidebar + rendered content (404 if missing)
- `GET /raw/*path` — bare HTML fragment for JS content swap (404 if missing)
- `GET /events` — SSE stream, subscribes to broadcast channel, 30s keepalive
- `GET /api/files` — sorted JSON array of all relative file paths

### HTML Shell

Single embedded template via `format!()`:
- `<nav>` sidebar (file tree, populated client-side from `/api/files`)
- `<main>` content area (rendered HTML interpolated server-side on initial load)
- `<header>` with theme toggle button
- `<script>` block for SSE, sidebar navigation (`pushState` + `/raw/` fetch), theme switching

## File Watcher

`notify::RecommendedWatcher` on a tokio blocking task, `RecursiveMode::Recursive`.

- Filter to `*.md` files only
- Debounce ~100ms to deduplicate rapid events from a single save
- On modify/create: re-read, render, update map, broadcast
- On remove: remove from map, broadcast
- On rename: treat as remove + add
- Skip hidden directories (`.git`, `.github`, etc.) and `node_modules`

## Frontend

- CSS grid layout: sidebar ~260px fixed, content fills remainder
- Sidebar: nested `<ul>` tree built from path segments, active file highlighted
- SSE client: `EventSource('/events')`, on `FileChanged` re-fetches `/raw/{path}` for current file, on `FileAdded`/`FileRemoved` re-fetches sidebar
- Theme toggle: switches `theme-github`/`theme-gitlab` class on `<body>`, persists to `localStorage`
- Two embedded CSS files: `github.css` (based on github-markdown-css), `gitlab.css` (GitLab approximation)

## Error Handling & Logging

- `tracing_subscriber` with `fmt` layer, default level `info`
- Logs: startup info, file watcher events, HTTP requests (via tower-http `TraceLayer`)
- File read failure: log warning, skip update, keep stale content
- Watcher error: log error, continue running
- SSE disconnect: handled silently by broadcast channel
- Invalid path: 404 with simple error page
- Graceful shutdown: `ctrl_c` signal, clean shutdown of watcher and server

## Additional Dependencies Needed

- `pulldown-cmark` — markdown to HTML
- `tokio` with `full` features — async runtime
- `tower-http` with `trace` feature — HTTP request logging
