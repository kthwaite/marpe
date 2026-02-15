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
        Html(assets::render_empty_state(&state.syntax_css_light, &state.syntax_css_dark)).into_response()
    }
}

/// GET /view/*path — full HTML page
pub async fn view_file(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.get_rendered(&path).await {
        Some(html) => Html(assets::render_page(&path, &html, &state.syntax_css_light, &state.syntax_css_dark)).into_response(),
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
