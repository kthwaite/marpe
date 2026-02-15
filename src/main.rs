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
use std::time::Duration;
use tower_http::trace::TraceLayer;
use tracing::info;

const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = cli::parse_args();
    let root = args.root.canonicalize().expect("Invalid directory path");
    info!(path = %root.display(), "Serving markdown files from");

    let state = state::AppState::new(root.clone(), &args.syntax_theme_light, &args.syntax_theme_dark);

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

    let mut listener = None;
    let mut port = args.port;
    let end_port = args.port.saturating_add(9);
    for p in args.port..=end_port {
        let addr = format!("0.0.0.0:{}", p);
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => {
                listener = Some(l);
                port = p;
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                info!(port = p, "Port already in use, trying next one");
                continue;
            }
            Err(e) => panic!("Failed to bind to port {}: {}", p, e),
        }
    }

    let listener = listener.expect("Could not find a free port in range");
    let protocol = if args.tls { "https" } else { "http" };
    let url = format!("{}://localhost:{}", protocol, port);
    info!(port, "Server listening on {}", url);

    if args.open {
        if let Err(e) = open::that(&url) {
            tracing::error!("Failed to open browser: {}", e);
        }
    }

    let std_listener = listener.into_std().unwrap();

    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_handle.graceful_shutdown(Some(SHUTDOWN_GRACE_PERIOD));
    });

    if args.tls {
        let (cert_path, key_path) = tls::resolve_certs(args.cert, args.key)
            .expect("Failed to resolve TLS certificates");

        let rustls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_path, &key_path)
            .await
            .expect("Failed to load TLS certificates");

        axum_server::from_tcp_rustls(std_listener, rustls_config)
            .unwrap()
            .handle(handle)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        axum_server::from_tcp(std_listener)
            .unwrap()
            .handle(handle)
            .serve(app.into_make_service())
            .await
            .unwrap();
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install ctrl+c handler");
    info!("Shutting down");
}
