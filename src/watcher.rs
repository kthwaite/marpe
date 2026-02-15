use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, error};

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
