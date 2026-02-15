mod assets;
mod cli;
mod discovery;
mod handlers;
mod render;
mod state;
mod tls;
mod watcher;

use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = cli::parse_args();
    let root = args.root.canonicalize().expect("Invalid directory path");
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
