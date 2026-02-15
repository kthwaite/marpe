use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::{ModifyKind, RenameMode}};
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

            match event.kind {
                EventKind::Modify(ModifyKind::Name(mode)) => {
                    match mode {
                        RenameMode::Both => {
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
        }
    });

    Ok(watcher)
}
