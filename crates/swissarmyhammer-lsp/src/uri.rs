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
}
