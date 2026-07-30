//! Shared helpers for MCP tool lifecycle (`init`/`deinit`) implementations.
//!
//! Tools delegate agent-specific config to the mirdan appliers, which report
//! back as a list of [`InitResult`]. The helpers here read those lists so every
//! tool interprets an applier outcome the same way.

use swissarmyhammer_common::lifecycle::{InitResult, InitStatus};

/// Collect the first error message from an applier's results, if any.
///
/// The mirdan appliers return one [`InitResult`] per aggregate; surface an
/// error so a tool's `init`/`deinit` can abort on it.
pub(crate) fn applier_error(results: &[InitResult]) -> Option<String> {
    results
        .iter()
        .find(|r| r.status == InitStatus::Error)
        .map(|r| r.message.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applier_error_is_none_when_no_result_failed() {
        let results = [
            InitResult::ok("a", "installed"),
            InitResult::skipped("b", "nothing to do"),
            InitResult::warning("c", "partially applied"),
        ];
        assert_eq!(applier_error(&results), None);
    }

    #[test]
    fn applier_error_returns_the_first_error_message() {
        let results = [
            InitResult::ok("a", "installed"),
            InitResult::error("b", "first failure"),
            InitResult::error("c", "second failure"),
        ];
        assert_eq!(applier_error(&results), Some("first failure".to_string()));
    }
}
