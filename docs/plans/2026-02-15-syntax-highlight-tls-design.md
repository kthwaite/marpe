# Syntax Highlighting + HTTPS/TLS Design

## Summary

Two additions to markdown-preview: server-side syntax highlighting via syntect, and optional HTTPS via mkcert with a `--tls` CLI flag.

## Feature 1: Syntax Highlighting

**Crate**: `syntect` — bundled syntax definitions and color themes.

**Rendering change**: Replace `pulldown_cmark::html::push_html` with a custom event walker that intercepts `CodeBlock` events:

1. Parse markdown with pulldown_cmark (unchanged)
2. Walk the event stream; on `Start(CodeBlock(kind))`, extract the language from the fence info string
3. Collect the text content of the code block
4. Run through `syntect::html::highlighted_html_for_string` with the appropriate syntax and a neutral light theme
5. Emit the highlighted HTML as a raw HTML event
6. All other events pass through to `push_html` unchanged

**Theme**: Use a bundled neutral light theme (e.g. `InspiredGitHub`) that produces inline `style` attributes on `<span>` elements, working under both GitHub and GitLab CSS themes.

**Fallback**: Unrecognized languages fall back to plain `<pre><code>` with no highlighting.

## Feature 2: HTTPS with mkcert

**CLI**: `--tls` flag enables HTTPS. Optional `--cert` and `--key` flags for explicit PEM paths.

**Certificate discovery** (when `--tls` without explicit paths):
1. Run `mkcert -CAROOT` to find the CA root directory
2. Look for `localhost.pem` and `localhost-key.pem` there
3. If not found, run `mkcert localhost` to generate them
4. If `mkcert` isn't installed, exit with a helpful error message

**Server change**: When TLS enabled, use `axum-server` with `RustlsConfig` loaded from PEM files instead of plain `axum::serve`. Graceful shutdown preserved.

**Dependencies**: `axum-server` with `tls-rustls` feature.

**Port**: Same `13181` for both HTTP and HTTPS. Log shows protocol in URL.
