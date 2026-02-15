use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::render::render_markdown;

/// Returns true if path should be skipped (hidden dirs, node_modules)
fn should_skip(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.starts_with('.') || s == "node_modules"
    })
}

/// Walk `root` directory, find all .md files, render them.
/// Returns a map of relative path (string) -> rendered HTML.
pub fn discover_and_render(root: &Path) -> HashMap<String, String> {
    let entries: Vec<_> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return false;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                return false;
            }
            let relative = path.strip_prefix(root).unwrap_or(path);
            !should_skip(relative)
        })
        .collect();

    entries
        .into_par_iter()
        .filter_map(|entry| {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap_or(path);
            let rel_str = relative.to_string_lossy().to_string();
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let html = render_markdown(&content);
                    info!(path = %rel_str, "Rendered markdown file");
                    Some((rel_str, html))
                }
                Err(e) => {
                    warn!(path = %rel_str, error = %e, "Failed to read markdown file");
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn setup_temp_dir(name: &str) -> PathBuf {
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("_scratch/discovery_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discovers_md_files() {
        let dir = setup_temp_dir("md_files");
        fs::write(dir.join("README.md"), "# Hello").unwrap();
        fs::write(dir.join("notes.md"), "some notes").unwrap();
        fs::write(dir.join("ignore.txt"), "not markdown").unwrap();

        let files = discover_and_render(&dir);
        assert_eq!(files.len(), 2);
        assert!(files.contains_key("README.md"));
        assert!(files.contains_key("notes.md"));
        assert!(!files.contains_key("ignore.txt"));
    }

    #[test]
    fn discovers_nested_files() {
        let dir = setup_temp_dir("nested");
        fs::create_dir_all(dir.join("docs/guide")).unwrap();
        fs::write(dir.join("docs/guide/intro.md"), "# Intro").unwrap();

        let files = discover_and_render(&dir);
        assert!(files.contains_key("docs/guide/intro.md"));
    }

    #[test]
    fn skips_hidden_dirs() {
        let dir = setup_temp_dir("hidden");
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git/info.md"), "hidden").unwrap();
        fs::write(dir.join("visible.md"), "shown").unwrap();

        let files = discover_and_render(&dir);
        assert_eq!(files.len(), 1);
        assert!(files.contains_key("visible.md"));
    }

    #[test]
    fn skips_node_modules() {
        let dir = setup_temp_dir("node_modules");
        fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        fs::write(dir.join("node_modules/pkg/README.md"), "npm").unwrap();
        fs::write(dir.join("top.md"), "top").unwrap();

        let files = discover_and_render(&dir);
        assert_eq!(files.len(), 1);
        assert!(files.contains_key("top.md"));
    }

    #[test]
    fn renders_content_correctly() {
        let dir = setup_temp_dir("renders");
        fs::write(dir.join("test.md"), "**bold**").unwrap();

        let files = discover_and_render(&dir);
        let html = files.get("test.md").unwrap();
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn empty_directory() {
        let dir = setup_temp_dir("empty");
        let files = discover_and_render(&dir);
        assert!(files.is_empty());
    }
}
