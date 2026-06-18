//! The shared `file://` URI ↔ filesystem path conversion.
//!
//! LSP speaks `file://` URIs; in-process consumers (the diagnostics settle
//! engine, the code-context op layer) want plain filesystem paths. This is the
//! single place that conversion lives, so each consumer reuses it instead of
//! carrying its own copy of the same prefix-strip.

/// Convert a `file://` URI to a filesystem path.
///
/// Strips the `file://` scheme prefix. A URI with any other scheme (or no
/// scheme at all) is returned unchanged, so the caller still has a usable
/// identifier rather than an empty string.
pub fn file_path_from_uri(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_string()
}

/// Convert a filesystem path to a `file://` URI.
///
/// Prefixes the path with the `file://` scheme, matching the wire format the
/// [`LspSession`](crate::session::LspSession) uses when it opens documents, so a
/// uri built here keys into the session's per-uri diagnostics the same way.
/// This is the inverse of [`file_path_from_uri`].
pub fn file_uri_from_path(path: &str) -> String {
    format!("file://{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_file_scheme() {
        assert_eq!(file_path_from_uri("file:///src/main.rs"), "/src/main.rs");
    }

    #[test]
    fn passes_through_non_file_scheme() {
        assert_eq!(
            file_path_from_uri("untitled:Untitled-1"),
            "untitled:Untitled-1"
        );
    }

    #[test]
    fn passes_through_plain_path() {
        assert_eq!(file_path_from_uri("/already/a/path"), "/already/a/path");
    }

    #[test]
    fn path_to_uri_adds_file_scheme() {
        assert_eq!(file_uri_from_path("/src/main.rs"), "file:///src/main.rs");
    }

    #[test]
    fn path_and_uri_round_trip() {
        let path = "/src/lib.rs";
        assert_eq!(file_path_from_uri(&file_uri_from_path(path)), path);
    }
}
