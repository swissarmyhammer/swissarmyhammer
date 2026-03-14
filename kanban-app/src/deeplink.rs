//! Deep-link handler for the `kanban://` URL scheme.
//!
//! Strips the `kanban://open/` prefix and hands the path to
//! `AppState::open_board`.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tracing::{error, info, warn};

/// Extract a filesystem path from a `kanban://open/...` URL.
fn extract_open_path(url: &str) -> Option<PathBuf> {
    let path = url.strip_prefix("kanban://open/")?;
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        return None;
    }
    urlencoding::decode(path)
        .ok()
        .map(|s| PathBuf::from(s.into_owned()))
}

/// Handle an incoming deep-link URL.
pub fn handle_url(app: &AppHandle, url: String) {
    let path = match extract_open_path(&url) {
        Some(p) => p,
        None => {
            warn!(url, "unrecognized deep-link URL");
            return;
        }
    };

    info!(?path, "deep-link open requested");

    let handle = app.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!("failed to create tokio runtime for deep-link: {e}");
                return;
            }
        };

        let state = handle.state::<crate::state::AppState>();
        match rt.block_on(state.open_board(&path, Some(handle.clone()))) {
            Ok(canonical) => {
                info!(?canonical, "board opened via deep link");
            }
            Err(e) => {
                error!(?path, "deep-link open failed: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_absolute_path() {
        assert_eq!(
            extract_open_path("kanban://open/%2FUsers%2Fme%2Fproject"),
            Some(PathBuf::from("/Users/me/project"))
        );
    }

    #[test]
    fn extract_relative_path() {
        assert_eq!(
            extract_open_path("kanban://open/some-project"),
            Some(PathBuf::from("some-project"))
        );
    }

    #[test]
    fn extract_encoded_spaces() {
        assert_eq!(
            extract_open_path("kanban://open/%2FUsers%2Fme%2Fmy%20project"),
            Some(PathBuf::from("/Users/me/my project"))
        );
    }

    #[test]
    fn extract_dot() {
        assert_eq!(
            extract_open_path("kanban://open/."),
            Some(PathBuf::from("."))
        );
    }

    #[test]
    fn extract_trailing_slash() {
        assert_eq!(
            extract_open_path("kanban://open/foo/"),
            Some(PathBuf::from("foo"))
        );
    }

    #[test]
    fn extract_empty() {
        assert_eq!(extract_open_path("kanban://open/"), None);
    }

    #[test]
    fn extract_wrong_scheme() {
        assert_eq!(extract_open_path("https://open/foo"), None);
    }

    #[test]
    fn extract_wrong_action() {
        assert_eq!(extract_open_path("kanban://install/foo"), None);
    }

    #[test]
    fn extract_bare() {
        assert_eq!(extract_open_path("kanban://"), None);
    }
}
