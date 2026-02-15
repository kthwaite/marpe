use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "path")]
pub enum SseEvent {
    FileChanged(String),
    FileAdded(String),
    FileRemoved(String),
}

pub struct AppState {
    pub root: PathBuf,
    pub files: RwLock<HashMap<String, String>>, // relative path (as string) -> rendered HTML
    pub tx: broadcast::Sender<SseEvent>,
}

impl AppState {
    pub fn new(root: PathBuf) -> Arc<Self> {
        let (tx, _rx) = broadcast::channel(64);
        Arc::new(Self {
            root,
            files: RwLock::new(HashMap::new()),
            tx,
        })
    }

    /// Get sorted list of all file paths.
    pub async fn file_list(&self) -> Vec<String> {
        let files = self.files.read().await;
        let mut paths: Vec<String> = files.keys().cloned().collect();
        paths.sort();
        paths
    }

    /// Get rendered HTML for a path, if it exists.
    pub async fn get_rendered(&self, path: &str) -> Option<String> {
        let files = self.files.read().await;
        files.get(path).cloned()
    }

    /// Insert or update a rendered file.
    pub async fn upsert(&self, path: String, html: String) -> bool {
        let mut files = self.files.write().await;
        let is_new = !files.contains_key(&path);
        files.insert(path, html);
        is_new
    }

    /// Remove a file. Returns true if it existed.
    pub async fn remove(&self, path: &str) -> bool {
        let mut files = self.files.write().await;
        files.remove(path).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_state_has_empty_file_list() {
        let state = AppState::new(PathBuf::from("."));
        assert!(state.file_list().await.is_empty());
    }

    #[tokio::test]
    async fn upsert_and_get() {
        let state = AppState::new(PathBuf::from("."));
        let is_new = state.upsert("README.md".into(), "<p>hi</p>".into()).await;
        assert!(is_new);
        assert_eq!(
            state.get_rendered("README.md").await,
            Some("<p>hi</p>".into())
        );
    }

    #[tokio::test]
    async fn upsert_existing_returns_false() {
        let state = AppState::new(PathBuf::from("."));
        state.upsert("a.md".into(), "old".into()).await;
        let is_new = state.upsert("a.md".into(), "new".into()).await;
        assert!(!is_new);
        assert_eq!(state.get_rendered("a.md").await, Some("new".into()));
    }

    #[tokio::test]
    async fn remove_existing() {
        let state = AppState::new(PathBuf::from("."));
        state.upsert("a.md".into(), "html".into()).await;
        assert!(state.remove("a.md").await);
        assert!(state.get_rendered("a.md").await.is_none());
    }

    #[tokio::test]
    async fn remove_nonexistent() {
        let state = AppState::new(PathBuf::from("."));
        assert!(!state.remove("nope.md").await);
    }

    #[tokio::test]
    async fn file_list_is_sorted() {
        let state = AppState::new(PathBuf::from("."));
        state.upsert("z.md".into(), "".into()).await;
        state.upsert("a.md".into(), "".into()).await;
        state.upsert("m.md".into(), "".into()).await;
        assert_eq!(state.file_list().await, vec!["a.md", "m.md", "z.md"]);
    }

    #[test]
    fn sse_event_serializes_as_tagged() {
        let event = SseEvent::FileChanged("test.md".into());
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"FileChanged""#));
        assert!(json.contains(r#""path":"test.md""#));
    }
}
