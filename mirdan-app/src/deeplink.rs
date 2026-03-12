//! Deep-link handler for the `mirdan://` URL scheme.
//!
//! Parses incoming URLs and dispatches to the appropriate mirdan library
//! functions. Currently supports:
//!
//! - `mirdan://install/{package}` — install a package by name (optionally `name@version`)

use tauri::AppHandle;

/// A parsed deep-link action.
#[derive(Debug, PartialEq, Eq)]
pub enum DeepLinkAction {
    Install { package: String },
}

/// Parse a `mirdan://` URL into an action.
///
/// Returns `None` for unrecognized or malformed URLs.
pub fn parse_url(url: &str) -> Option<DeepLinkAction> {
    // Strip the scheme prefix.
    let path = url.strip_prefix("mirdan://")?;

    // Split on '/' and ignore empty segments (handles trailing slashes).
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    match segments.as_slice() {
        ["install", package] if !package.is_empty() => {
            // URL-decode the package name (browsers may encode @ as %40, etc.)
            let decoded = urlencoding::decode(package).ok()?;
            Some(DeepLinkAction::Install {
                package: decoded.into_owned(),
            })
        }
        _ => None,
    }
}

/// Handle an incoming deep-link URL.
///
/// Spawns the install on a background thread so it doesn't block the Tauri
/// event loop, then posts a native notification with the result.
pub fn handle_url(_app: &AppHandle, url: String) {
    let action = match parse_url(&url) {
        Some(a) => a,
        None => {
            eprintln!("[mirdan] unrecognized deep-link URL: {url}");
            return;
        }
    };

    match action {
        DeepLinkAction::Install { package } => {
            eprintln!("[mirdan] deep-link install: {package}");

            // Run the async install on a dedicated tokio runtime so we don't
            // block the Tauri event loop.
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new()
                    .expect("failed to create tokio runtime for deep-link install");

                let result = rt.block_on(mirdan::install::run_install(
                    &package, // package_spec
                    None,     // agent_filter — install for all agents
                    true,     // global — tray app has no project CWD
                    false,    // git
                    None,     // skill_select
                ));

                match result {
                    Ok(()) => {
                        eprintln!("[mirdan] installed {package} successfully");
                        // TODO: native notification
                    }
                    Err(e) => {
                        eprintln!("[mirdan] install failed for {package}: {e}");
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
}
