//! Deep-link handler for the `mirdan://` URL scheme.
//!
//! Parses incoming URLs and dispatches to the appropriate mirdan library
//! functions. Currently supports:
//!
//! - `mirdan://install/{package}` — install a package by name (optionally `name@version`)

use tauri::AppHandle;
use tracing::{error, info, warn};

/// A parsed deep-link action.
#[derive(Debug, PartialEq, Eq)]
pub enum DeepLinkAction {
    Install { package: String },
}

/// Check that a decoded package spec contains only safe characters.
///
/// Package specs can be:
/// - Simple names: `no-secrets`, `no-secrets@1.2.0`
/// - Git URLs: `https://github.com/owner/repo/skill@latest`
///
/// Rejects `..` sequences and shell metacharacters.
fn is_valid_package_spec(spec: &str) -> bool {
    !spec.is_empty()
        && !spec.contains("..")
        && spec.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '-' | '_' | '.' | '@' | '/' | ':' | ' ' | '+' | '~')
        })
}

/// Parse a `mirdan://` URL into an action.
///
/// Supports:
/// - `mirdan://install/no-secrets`
/// - `mirdan://install/no-secrets@1.2.0`
/// - `mirdan://install/https://github.com/owner/repo/skill@latest`
///
/// Returns `None` for unrecognized or malformed URLs.
pub fn parse_url(url: &str) -> Option<DeepLinkAction> {
    // Strip the scheme prefix.
    let path = url.strip_prefix("mirdan://")?;

    // Extract the action (first segment) and the rest as the package spec.
    let path = path.trim_matches('/');
    let (action, spec) = path.split_once('/')?;

    if action != "install" || spec.is_empty() {
        return None;
    }

    // Strip trailing slashes from the spec.
    let spec = spec.trim_end_matches('/');

    // URL-decode the package spec (browsers may encode @ as %40, etc.)
    let decoded = urlencoding::decode(spec).ok()?;

    if !is_valid_package_spec(&decoded) {
        return None;
    }

    Some(DeepLinkAction::Install {
        package: decoded.into_owned(),
    })
}

/// Handle an incoming deep-link URL.
///
/// Spawns the install on a background thread so it doesn't block the Tauri
/// event loop, then posts a native notification with the result.
pub fn handle_url(_app: &AppHandle, url: String) {
    let action = match parse_url(&url) {
        Some(a) => a,
        None => {
            warn!(url, "unrecognized deep-link URL");
            return;
        }
    };

    match action {
        DeepLinkAction::Install { package } => {
            info!(package, "deep-link install requested");

            // Run the async install on a dedicated tokio runtime so we don't
            // block the Tauri event loop.
            std::thread::spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        error!("failed to create tokio runtime for deep-link install: {e}");
                        return;
                    }
                };

                let is_git = package.starts_with("https://") || package.starts_with("http://");
                let result = rt.block_on(mirdan::install::run_install(
                    &package, // package_spec
                    None,     // agent_filter — install for all agents
                    true,     // global — tray app has no project CWD
                    is_git,   // git — auto-detect from URL
                    None,     // skill_select
                ));

                match result {
                    Ok(()) => {
                        info!(package, "installed successfully");
                        // TODO: native notification
                    }
                    Err(e) => {
                        error!(package, "install failed: {e}");
                        // TODO: native notification
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_install_simple() {
        assert_eq!(
            parse_url("mirdan://install/no-secrets"),
            Some(DeepLinkAction::Install {
                package: "no-secrets".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_install_with_version() {
        assert_eq!(
            parse_url("mirdan://install/no-secrets@1.2.0"),
            Some(DeepLinkAction::Install {
                package: "no-secrets@1.2.0".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_install_trailing_slash() {
        assert_eq!(
            parse_url("mirdan://install/foo/"),
            Some(DeepLinkAction::Install {
                package: "foo".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_unknown_action() {
        assert_eq!(parse_url("mirdan://unknown/thing"), None);
    }

    #[test]
    fn test_parse_empty_package() {
        assert_eq!(parse_url("mirdan://install/"), None);
    }

    #[test]
    fn test_parse_wrong_scheme() {
        assert_eq!(parse_url("https://install/foo"), None);
    }

    #[test]
    fn test_parse_bare_scheme() {
        assert_eq!(parse_url("mirdan://"), None);
    }

    #[test]
    fn test_parse_url_encoded_at() {
        // Browsers may encode @ as %40
        assert_eq!(
            parse_url("mirdan://install/no-secrets%401.2.0"),
            Some(DeepLinkAction::Install {
                package: "no-secrets@1.2.0".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_url_encoded_spaces() {
        assert_eq!(
            parse_url("mirdan://install/my%20package"),
            Some(DeepLinkAction::Install {
                package: "my package".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_rejects_path_traversal() {
        assert_eq!(parse_url("mirdan://install/..%2F..%2Fetc%2Fpasswd"), None);
    }

    #[test]
    fn test_parse_rejects_shell_metacharacters() {
        assert_eq!(parse_url("mirdan://install/foo;rm%20-rf"), None);
    }

    #[test]
    fn test_parse_git_url_package() {
        assert_eq!(
            parse_url("mirdan://install/https://github.com/0xdarkmatter/claude-mods/explain@latest"),
            Some(DeepLinkAction::Install {
                package: "https://github.com/0xdarkmatter/claude-mods/explain@latest".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_git_url_no_version() {
        assert_eq!(
            parse_url("mirdan://install/https://github.com/owner/repo"),
            Some(DeepLinkAction::Install {
                package: "https://github.com/owner/repo".to_string(),
            })
        );
    }
}
