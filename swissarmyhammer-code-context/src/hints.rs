//! Next-step hints for code context operations.
//!
//! Pure functions that map operation names to suggested next-step strings.
//! No DB access needed -- these are static recommendations to guide users
//! through a typical workflow.

/// Returns a suggested next-step hint for the given operation name.
///
/// Every recognized operation gets a non-empty hint string. Unknown
/// operations receive a generic suggestion.
///
/// # Arguments
///
/// * `operation` - The operation name (e.g. `"get_status"`, `"get_symbol"`).
///
/// # Returns
///
/// A static string with a next-step suggestion.
pub fn hint_for_operation(operation: &str) -> &'static str {
    match operation {
        "get_status" => {
            "If indexing is incomplete, run 'build_status' to trigger re-indexing of stale layers."
        }
        "build_status" => {
            "Files have been marked for re-indexing. The leader process will pick them up on its next cycle. Run 'get_status' to monitor progress."
        }
        "clear_status" => {
            "Index has been wiped. Run 'build_status' to trigger a full re-index, or wait for the leader to re-index automatically."
        }
        "get_symbol" => {
            "Use 'get_callgraph' to explore call relationships, or 'get_blastradius' to see downstream impact."
        }
        "get_callgraph" => {
            "Use 'get_blastradius' to see the full impact of changes, or 'grep_code' to find usage patterns."
        }
        "get_blastradius" => {
            "Review affected symbols and consider running tests for impacted files. Use 'get_symbol' to inspect specific symbols."
        }
        "grep_code" => {
            "Use 'get_symbol' to locate definitions and retrieve source text for a match."
        }
        "list_symbols" => {
            "Use 'get_symbol' with a specific name for exact lookup, or 'search_symbol' for fuzzy matching."
        }
        "search_symbol" => {
            "Use 'get_symbol' to retrieve the full source of a match, or 'get_callgraph' to trace callers/callees."
        }
        _ => "Run 'get_status' to check index health, or 'list_symbols' to explore the codebase.",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hints_are_non_empty_for_known_operations() {
        let ops = [
            "get_status",
            "build_status",
            "clear_status",
            "get_symbol",
            "get_callgraph",
            "get_blastradius",
            "grep_code",
            "list_symbols",
            "search_symbol",
        ];

        for op in &ops {
            let hint = hint_for_operation(op);
            assert!(!hint.is_empty(), "hint for '{}' must be non-empty", op);
        }
    }

    #[test]
    fn test_hint_for_unknown_operation_is_non_empty() {
        let hint = hint_for_operation("unknown_op");
        assert!(!hint.is_empty());
    }

    #[test]
    fn test_hint_content_is_relevant() {
        let hint = hint_for_operation("get_status");
        assert!(
            hint.contains("build_status"),
            "get_status hint should mention build_status"
        );

        let hint = hint_for_operation("build_status");
        assert!(
            hint.contains("get_status"),
            "build_status hint should mention get_status"
        );
    }
}
