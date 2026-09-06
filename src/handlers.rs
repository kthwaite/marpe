use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Redirect,
    },
    Json,
};
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

/// GET / — redirect to README.md or first file or empty state
pub async fn index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let files = state.file_list().await;
    if files.contains(&"README.md".to_string()) {
        Redirect::temporary("/view/README.md").into_response()
    } else if let Some(first) = files.first() {
        Redirect::temporary(&format!("/view/{first}")).into_response()
    } else {
        Html(state.page_shell.render_empty(&state.syntax_css_light, &state.syntax_css_dark)).into_response()
    }
}

/// GET /view/*path — full HTML page
pub async fn view_file(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.get_rendered(&path).await {
        Some(html) => Html(state.page_shell.render(&path, &html, &state.syntax_css_light, &state.syntax_css_dark)).into_response(),
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

/// GET /assets/vendor/mermaid.min.js — vendored Mermaid JS bundle
pub async fn vendor_mermaid_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        crate::assets::MERMAID_JS,
    )
}

/// GET /assets/vendor/mathjax.js — vendored MathJax TeX-SVG bundle
pub async fn vendor_mathjax_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        crate::assets::MATHJAX_JS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_vendor_mermaid_js_route() {
        let res = vendor_mermaid_js().await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(
            res.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=31536000, immutable"
        );
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, crate::assets::MERMAID_JS.as_bytes());
        assert!(!body.is_empty());
    }

    #[tokio::test]
    async fn test_vendor_mathjax_js_route() {
        let res = vendor_mathjax_js().await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(
            res.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=31536000, immutable"
        );
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, crate::assets::MATHJAX_JS.as_bytes());
        assert!(!body.is_empty());
    }

    #[tokio::test]
    async fn test_vendor_routes_via_router() {
        let app = Router::new()
            .route("/assets/vendor/mermaid.min.js", get(vendor_mermaid_js))
            .route("/assets/vendor/mathjax.js", get(vendor_mathjax_js));

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/assets/vendor/mermaid.min.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(
            res.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=31536000, immutable"
        );
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, crate::assets::MERMAID_JS.as_bytes());

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/assets/vendor/mathjax.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(
            res.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=31536000, immutable"
        );
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, crate::assets::MATHJAX_JS.as_bytes());
    }
}
