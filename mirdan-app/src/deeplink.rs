//! Deep-link handler for the `mirdan://` URL scheme.
//!
//! Strips the `mirdan://install/` prefix and hands the rest verbatim
//! to `mirdan::install::run_install`. No parsing, no validation —
//! mirdan handles all of that.

use tauri::AppHandle;
use tracing::{error, info, warn};

/// Extract the package spec from a `mirdan://install/...` URL.
///
/// Returns the raw spec after `install/`, or `None` if the URL
/// doesn't match the expected scheme.
fn extract_install_spec(url: &str) -> Option<String> {
    let path = url.strip_prefix("mirdan://install/")?;
    let spec = path.trim_end_matches('/');
    if spec.is_empty() {
        return None;
    }
    // URL-decode (browsers may encode @ as %40, etc.)
    urlencoding::decode(spec).ok().map(|s| s.into_owned())
}

/// Handle an incoming deep-link URL.
///
/// Spawns the install on a background thread so it doesn't block the Tauri
/// event loop.
pub fn handle_url(_app: &AppHandle, url: String) {
    let spec = match extract_install_spec(&url) {
        Some(s) => s,
        None => {
            warn!(url, "unrecognized deep-link URL");
            return;
        }
    };

    info!(spec, "deep-link install requested");

    std::thread::spawn(move || {
        // The .app bundle's CWD is read-only. Move to $HOME so mirdan
        // can write temp files during install.
        if let Some(home) = std::env::var_os("HOME") {
            let _ = std::env::set_current_dir(home);
        }

        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!("failed to create tokio runtime for deep-link install: {e}");
                return;
            }
        };

        let result = rt.block_on(mirdan::install::run_install(
            &spec, // passed verbatim — mirdan classifies it
            None,  // agent_filter
            true,  // global
            false, // git
            None,  // skill_select
        ));

        match result {
            Ok(()) => {
                info!(spec, "installed successfully");
                // TODO: native notification
            }
            Err(e) => {
                error!(spec, "install failed: {e}");
                // TODO: native notification
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_simple() {
        assert_eq!(
            extract_install_spec("mirdan://install/no-secrets"),
            Some("no-secrets".into())
        );
    }

    #[test]
    fn extract_with_version() {
        assert_eq!(
            extract_install_spec("mirdan://install/no-secrets%401.2.0"),
            Some("no-secrets@1.2.0".into())
        );
    }

    #[test]
    fn extract_git_url() {
        assert_eq!(
            extract_install_spec("mirdan://install/https://github.com/owner/repo/skill"),
            Some("https://github.com/owner/repo/skill".into())
        );
    }

    #[test]
    fn extract_trailing_slash() {
        assert_eq!(
            extract_install_spec("mirdan://install/foo/"),
            Some("foo".into())
        );
    }

    #[test]
    fn extract_empty() {
        assert_eq!(extract_install_spec("mirdan://install/"), None);
    }

    #[test]
    fn extract_wrong_scheme() {
        assert_eq!(extract_install_spec("https://install/foo"), None);
    }

    #[test]
    fn extract_wrong_action() {
        assert_eq!(extract_install_spec("mirdan://uninstall/foo"), None);
    }

    #[test]
    fn extract_bare() {
        assert_eq!(extract_install_spec("mirdan://"), None);
    }
}
