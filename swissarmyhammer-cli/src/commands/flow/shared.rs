//! Shared utilities for flow command subcommands

use swissarmyhammer::WorkflowRunId;

/// Helper to convert WorkflowRunId to string
#[allow(dead_code)]
pub fn workflow_run_id_to_string(id: &WorkflowRunId) -> String {
    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_run_id_to_string() {
        let id = WorkflowRunId::new();
        let id_str = workflow_run_id_to_string(&id);

        // Test that the conversion produces a valid string
        assert!(!id_str.is_empty());
        assert_eq!(id_str, id.to_string());
    }
}
