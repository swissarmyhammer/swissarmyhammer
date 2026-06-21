//! The single shared diagnosable-language gate.
//!
//! A path is "diagnosable" when its file extension is handled by some LSP server
//! the supervisor knows about — i.e. there is a language server that could
//! produce diagnostics for it. Both consumers of this crate (the `diagnostics`
//! MCP tool and the inline-on-edit fold-in) call this ONE helper, so the
//! `.md`/`.txt` exclusion is defined and tested in exactly one place rather than
//! reimplemented twice.
//!
//! The set of diagnosable extensions is derived from the supervisor's
//! server-spec `file_extensions` (via [`swissarmyhammer_lsp::all_servers`]); it
//! is not a hardcoded list in this crate.

use std::path::Path;

use swissarmyhammer_lsp::all_servers;

/// Return `true` when `path`'s extension is handled by some known LSP server.
///
/// The extension is matched case-insensitively against the union of every
/// registered server spec's `file_extensions`. A path with no extension (or an
/// extension no server claims, such as `.md` or `.txt`) is not diagnosable.
pub fn is_diagnosable(path: impl AsRef<Path>) -> bool {
    let ext = match path.as_ref().extension().and_then(|e| e.to_str()) {
        Some(ext) => ext.to_ascii_lowercase(),
        None => return false,
    };

    all_servers().iter().any(|spec| {
        spec.file_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(&ext))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_file_is_diagnosable() {
        assert!(is_diagnosable("src/main.rs"));
    }

    #[test]
    fn markdown_file_is_not_diagnosable() {
        assert!(!is_diagnosable("README.md"));
    }

    #[test]
    fn text_file_is_not_diagnosable() {
        assert!(!is_diagnosable("notes.txt"));
    }

    #[test]
    fn file_without_extension_is_not_diagnosable() {
        assert!(!is_diagnosable("Makefile"));
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        assert!(is_diagnosable("src/MAIN.RS"));
    }

    #[test]
    fn absolute_path_is_diagnosable_by_extension() {
        assert!(is_diagnosable("/home/user/project/src/lib.rs"));
    }
}
