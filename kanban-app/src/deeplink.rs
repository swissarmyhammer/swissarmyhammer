//! Deep-link handler for the `kanban://` URL scheme.
//!
//! Strips the `kanban://open/` prefix and hands the path to
//! `AppState::open_board`, then ensures a window is visible showing it.
//!
//! Two entry points:
//!
//! - [`handle_url_blocking`] — cold-start, called synchronously inside
//!   `tauri::Builder::setup` so the board is opened and a window is visible
//!   before the setup closure returns. Sets `AppState::deep_link_handled` so
//!   downstream setup steps stand down.
//! - [`handle_url`] — warm-start, called from the `on_open_url` callback for
//!   a second `kanban open <path>` invocation against an already-running
//!   instance.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager};
use tracing::{error, info, warn};

use crate::state::AppState;

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

/// Cold-start deep-link handler. Runs synchronously inside the Tauri `setup`
/// closure so that when it returns, the board has been opened and a window
/// is visible. Sets `AppState::deep_link_handled` so `auto_open_board` and
/// the window-restore fallback can stand down. Clears the flag on failure so
/// the user lands on the previous session instead of an empty app.
pub fn handle_url_blocking(app: &AppHandle, url: String) {
    let Some(path) = recognize(&url) else {
        return;
    };
    info!(?path, "deep-link open requested (cold start)");

    let state = app.state::<AppState>();
    state.deep_link_handled.store(true, Ordering::SeqCst);

    let handle = app.clone();
    let result = tauri::async_runtime::block_on(process_deep_link(&handle, path));
    if let Err(e) = result {
        error!("deep-link open failed: {e}");
        state.deep_link_handled.store(false, Ordering::SeqCst);
    }
}

/// Warm-start deep-link handler. Registered as the `on_open_url` callback
/// for an already-running instance. Does NOT touch `deep_link_handled` — the
/// flag is consumed only by cold-start setup, which has already completed.
pub fn handle_url(app: &AppHandle, url: String) {
    let Some(path) = recognize(&url) else {
        return;
    };
    info!(?path, "deep-link open requested (warm start)");

    let handle = app.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!("failed to create tokio runtime for deep-link: {e}");
                return;
            }
        };
        if let Err(e) = rt.block_on(process_deep_link(&handle, path)) {
            error!("deep-link open failed: {e}");
        }
    });
}

fn recognize(url: &str) -> Option<PathBuf> {
    match extract_open_path(url) {
        Some(p) => Some(p),
        None => {
            warn!(url, "unrecognized deep-link URL");
            None
        }
    }
}

/// Open `path` as a board and ensure a visible, focused window shows it.
/// Focuses an existing window mapped to the canonical path; otherwise creates
/// one via the canonical `commands::create_window_impl`.
async fn process_deep_link(app: &AppHandle, path: PathBuf) -> Result<(), String> {
    let state = app.state::<AppState>();
    let canonical = state.open_board(&path, Some(app.clone())).await?;
    let canonical_str = canonical.display().to_string();
    info!(?canonical, "board opened via deep link");

    if let Some(label) = find_window_for_board(app, &state, &canonical_str) {
        focus_existing_window(app, &label);
    } else {
        crate::commands::create_window_impl(app, &state, Some(canonical_str), None, None).await?;
    }
    Ok(())
}

fn find_window_for_board(
    app: &AppHandle,
    state: &AppState,
    canonical_path: &str,
) -> Option<String> {
    app.webview_windows()
        .into_keys()
        .find(|label| state.ui_state.window_board(label).as_deref() == Some(canonical_path))
}

fn focus_existing_window(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        warn!(label = %label, "focus target window not found");
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
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
