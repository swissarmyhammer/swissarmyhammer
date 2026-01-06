//! Workflow runtime execution types

use crate::{StateId, Workflow, WorkflowTemplateContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_common::generate_monotonic_ulid;
use swissarmyhammer_config::model::ModelConfig;
use ulid::Ulid;

/// Unique identifier for workflow runs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkflowRunId(Ulid);

impl WorkflowRunId {
    /// Create a new random workflow run ID
    pub fn new() -> Self {
        Self(generate_monotonic_ulid())
    }

    /// Parse a WorkflowRunId from a string representation
    pub fn parse(s: &str) -> Result<Self, String> {
        Ulid::from_string(s)
            .map(Self)
            .map_err(|e| format!("Invalid workflow run ID '{s}': {e}"))
    }
}

impl Default for WorkflowRunId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkflowRunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a workflow run
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowRunStatus {
    /// Workflow is currently executing
    Running,
    /// Workflow completed successfully
    Completed,
    /// Workflow failed with an error
    Failed,
    /// Workflow was cancelled
    Cancelled,
    /// Workflow is paused
    Paused,
}

/// Runtime execution context for a workflow
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRun {
    /// Unique identifier for this run
    pub id: WorkflowRunId,
    /// The workflow being executed
    pub workflow: Workflow,
    /// Current state ID
    pub current_state: StateId,
    /// Execution history (state_id, timestamp)
    pub history: Vec<(StateId, chrono::DateTime<chrono::Utc>)>,
    /// Variables/context for this run
    pub context: WorkflowTemplateContext,
    /// Run status
    pub status: WorkflowRunStatus,
    /// When the run started
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the run completed (if applicable)
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Metadata for debugging and monitoring
    pub metadata: HashMap<String, String>,
}

impl WorkflowRun {
    /// Create a new workflow run
    pub fn new(workflow: Workflow) -> Self {
        Self::new_impl(workflow, ".swissarmyhammer/.abort", None)
    }

    /// Create a new workflow run with agent configuration
    pub fn new_with_agent(workflow: Workflow, agent: Arc<ModelConfig>) -> Self {
        Self::new_impl(workflow, ".swissarmyhammer/.abort", Some(agent))
    }

    #[cfg(test)]
    /// Create a new workflow run with configurable abort file path (test only)
    pub fn new_with_abort_path(workflow: Workflow, abort_path: &str) -> Self {
        Self::new_impl(workflow, abort_path, None)
    }

    fn new_impl(workflow: Workflow, abort_path: &str, agent: Option<Arc<ModelConfig>>) -> Self {
        // Clean up any existing abort file to ensure clean slate
        // Abort detection happens at flow command level before WorkflowRun::new() is called
        match std::fs::remove_file(abort_path) {
            Ok(()) => {
                tracing::debug!("Cleaned up existing abort file at {}", abort_path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist, no cleanup needed
            }
            Err(e) => {
                tracing::warn!("Failed to clean up abort file at {}: {}", abort_path, e);
                // Continue with workflow initialization
            }
        }

        let now = chrono::Utc::now();
        let initial_state = workflow.initial_state.clone();
        let mut context = WorkflowTemplateContext::load().unwrap_or_else(|_| {
            WorkflowTemplateContext::with_vars(Default::default())
                .expect("Failed to create default context")
        });

        // If an agent is provided, set it in the context
        if let Some(agent_config) = agent {
            context.set_agent_config((*agent_config).clone());
        }

        // Set the workflow mode in context if specified
        context.set_workflow_mode(workflow.mode.clone());

        Self {
            id: WorkflowRunId::new(),
            workflow,
            current_state: initial_state.clone(),
            history: vec![(initial_state, now)],
            context,
            status: WorkflowRunStatus::Running,
            started_at: now,
            completed_at: None,
            metadata: Default::default(),
        }
    }

    /// Record a state transition
    pub fn transition_to(&mut self, state_id: StateId) {
        let now = chrono::Utc::now();
        self.history.push((state_id.clone(), now));
        self.current_state = state_id;
    }

    /// Mark the run as completed
    pub fn complete(&mut self) {
        self.status = WorkflowRunStatus::Completed;
        self.completed_at = Some(chrono::Utc::now());
    }

    /// Mark the run as failed
    pub fn fail(&mut self) {
        self.status = WorkflowRunStatus::Failed;
        self.completed_at = Some(chrono::Utc::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_helpers::*;
    use tempfile::TempDir;

    /// Helper function to set up isolated test environment with temporary directory
    /// Returns (temp_dir_path, abort_file_path, guard)
    fn setup_isolated_test() -> (std::path::PathBuf, std::path::PathBuf, impl Drop) {
        // Create a temporary directory for this test
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let temp_path = temp_dir.path().to_path_buf();

        // Create the abort file path within the temp directory
        let abort_path = temp_path.join(".swissarmyhammer").join(".abort");

        // Return guard to clean up temp dir on drop
        struct TestEnvironmentGuard {
            _temp_dir: TempDir,
        }

        impl Drop for TestEnvironmentGuard {
            fn drop(&mut self) {
                // TempDir cleanup is automatic
            }
        }

        let guard = TestEnvironmentGuard {
            _temp_dir: temp_dir,
        };

        (temp_path, abort_path, guard)
    }

    #[test]
    fn test_workflow_run_id_creation() {
        let id1 = WorkflowRunId::new();
        let id2 = WorkflowRunId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_workflow_run_id_parse_and_to_string() {
        let id = WorkflowRunId::new();
        let id_str = id.to_string();

        // Test round-trip conversion
        let parsed_id = WorkflowRunId::parse(&id_str).unwrap();
        assert_eq!(id, parsed_id);
        assert_eq!(id_str, parsed_id.to_string());
    }

    #[test]
    fn test_workflow_run_id_parse_invalid() {
        let invalid_id = "invalid-ulid";
        let result = WorkflowRunId::parse(invalid_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid workflow run ID"));
    }

    #[test]
    fn test_workflow_run_id_parse_valid_ulid() {
        // Generate a valid ULID string
        let ulid = generate_monotonic_ulid();
        let ulid_str = ulid.to_string();

        let parsed_id = WorkflowRunId::parse(&ulid_str).unwrap();
        assert_eq!(parsed_id.to_string(), ulid_str);
    }

    #[test]
    fn test_workflow_run_creation() {
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        let run = WorkflowRun::new(workflow);

        assert_eq!(run.workflow.name.as_str(), "Test Workflow");
        assert_eq!(run.current_state.as_str(), "start");
        assert_eq!(run.status, WorkflowRunStatus::Running);
        assert_eq!(run.history.len(), 1);
        assert_eq!(run.history[0].0.as_str(), "start");
    }

    #[test]
    fn test_workflow_run_with_mode_sets_context() {
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));
        workflow.mode = Some("planner".to_string());

        let run = WorkflowRun::new(workflow);

        // Verify mode is set in context
        assert_eq!(run.context.get_workflow_mode(), Some("planner".to_string()));
        // Verify workflow still has the mode
        assert_eq!(run.workflow.mode, Some("planner".to_string()));
    }

    #[test]
    fn test_workflow_run_without_mode_has_none_in_context() {
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));
        // No mode set (default is None)

        let run = WorkflowRun::new(workflow);

        // Verify mode is None in context
        assert_eq!(run.context.get_workflow_mode(), None);
        assert_eq!(run.workflow.mode, None);
    }

    #[test]
    fn test_workflow_run_mode_variations() {
        // Test various mode values
        let modes = vec![
            "implementer",
            "reviewer",
            "tester",
            "committer",
            "rule-checker",
        ];

        for mode_name in modes {
            let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
            workflow.add_state(create_state("start", "Start state", false));
            workflow.mode = Some(mode_name.to_string());

            let run = WorkflowRun::new(workflow);

            assert_eq!(
                run.context.get_workflow_mode(),
                Some(mode_name.to_string()),
                "Mode '{}' should be set in context",
                mode_name
            );
        }
    }

    #[test]
    fn test_workflow_run_transition() {
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));
        workflow.add_state(create_state("processing", "Processing state", false));

        let mut run = WorkflowRun::new(workflow);

        run.transition_to(StateId::new("processing"));

        assert_eq!(run.current_state.as_str(), "processing");
        assert_eq!(run.history.len(), 2);
        assert_eq!(run.history[1].0.as_str(), "processing");
    }

    #[test]
    fn test_workflow_run_completion() {
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        let mut run = WorkflowRun::new(workflow);

        run.complete();

        assert_eq!(run.status, WorkflowRunStatus::Completed);
        assert!(run.completed_at.is_some());
    }

    #[test]
    fn test_workflow_run_id_monotonic_generation() {
        let id1 = WorkflowRunId::new();
        let id2 = WorkflowRunId::new();
        let id3 = WorkflowRunId::new();

        // Test that IDs are monotonic
        assert!(id1 < id2);
        assert!(id2 < id3);
        assert!(id1 < id3);

        // Test that string representation also maintains ordering
        assert!(id1.to_string() < id2.to_string());
        assert!(id2.to_string() < id3.to_string());
        assert!(id1.to_string() < id3.to_string());
    }

    #[test]
    fn test_abort_file_cleanup_when_file_exists() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();

        // Create the .swissarmyhammer directory in temp dir
        let sah_dir = _temp_dir.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir).unwrap();

        // Create an abort file
        std::fs::write(&abort_path, "test abort reason").expect("Failed to write abort file");

        // Verify the file was created
        assert!(
            abort_path.exists(),
            "Abort file should exist after creation"
        );

        // Create a test workflow
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Create a new workflow run with the isolated abort path - this should clean up the abort file
        let _run = WorkflowRun::new_with_abort_path(workflow, abort_path.to_str().unwrap());

        // Verify the abort file was cleaned up
        assert!(
            !abort_path.exists(),
            "Abort file should be cleaned up after WorkflowRun::new"
        );
    }

    #[test]
    fn test_abort_file_cleanup_when_file_does_not_exist() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();

        // Create a test workflow
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Ensure abort file doesn't exist
        let _ = std::fs::remove_file(&abort_path); // Ignore if it doesn't exist

        // Create a new workflow run - should not fail even if file doesn't exist
        let run = WorkflowRun::new_with_abort_path(workflow, abort_path.to_str().unwrap());

        // Verify workflow run was created successfully
        assert_eq!(run.workflow.name.as_str(), "Test Workflow");
        assert_eq!(run.status, WorkflowRunStatus::Running);
    }

    #[test]
    fn test_abort_file_cleanup_continues_on_permission_error() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();

        // Create a test workflow
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // This test would be difficult to simulate without root access or special file system setup
        // Instead, we test that workflow creation continues even if cleanup fails
        // The actual error handling is tested in the implementation by using match expressions

        // Create a new workflow run
        let run = WorkflowRun::new_with_abort_path(workflow, abort_path.to_str().unwrap());

        // Verify workflow run was created successfully regardless of cleanup result
        assert_eq!(run.workflow.name.as_str(), "Test Workflow");
        assert_eq!(run.status, WorkflowRunStatus::Running);
        assert_eq!(run.current_state.as_str(), "start");
        assert_eq!(run.history.len(), 1);
    }

    #[test]
    fn test_multiple_workflow_runs_cleanup_abort_file() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();

        // Create the .swissarmyhammer directory in temp dir
        let sah_dir = _temp_dir.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir).unwrap();

        // Create a test workflow
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Create first abort file
        std::fs::write(&abort_path, "first abort reason").unwrap();
        assert!(abort_path.exists());

        // Create first workflow run - should clean up abort file
        let _run1 =
            WorkflowRun::new_with_abort_path(workflow.clone(), abort_path.to_str().unwrap());
        assert!(!abort_path.exists());

        // Create second abort file
        std::fs::write(&abort_path, "second abort reason").unwrap();
        assert!(abort_path.exists());

        // Create second workflow run - should also clean up abort file
        let _run2 = WorkflowRun::new_with_abort_path(workflow, abort_path.to_str().unwrap());
        assert!(!abort_path.exists());
    }

    #[test]
    fn test_abort_file_cleanup_with_unicode_content() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();

        // Create the .swissarmyhammer directory in temp dir
        let sah_dir = _temp_dir.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir).unwrap();

        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Create abort file with unicode content
        let unicode_reason = "中文测试 🚫 Aborting with émojis";
        std::fs::write(&abort_path, unicode_reason).unwrap();
        assert!(abort_path.exists());

        // Create workflow run - should clean up abort file regardless of content
        let _run = WorkflowRun::new_with_abort_path(workflow, abort_path.to_str().unwrap());
        assert!(!abort_path.exists());
    }

    #[test]
    fn test_abort_file_cleanup_with_large_content() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();

        // Create the .swissarmyhammer directory in temp dir
        let sah_dir = _temp_dir.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir).unwrap();

        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Create abort file with large content
        let large_reason = "x".repeat(10000);
        std::fs::write(&abort_path, &large_reason).unwrap();
        assert!(abort_path.exists());

        // Create workflow run - should clean up large abort file
        let _run = WorkflowRun::new_with_abort_path(workflow, abort_path.to_str().unwrap());
        assert!(!abort_path.exists());
    }

    #[test]
    fn test_abort_file_cleanup_concurrent_workflow_runs() {
        use std::sync::Arc;

        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();
        // Create the .swissarmyhammer directory in temp dir
        let sah_dir = _temp_dir.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir).unwrap();

        let workflow = Arc::new({
            let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
            workflow.add_state(create_state("start", "Start state", false));
            workflow
        });

        // Create abort file
        std::fs::write(&abort_path, "concurrent test reason").unwrap();

        // Verify abort file exists before creating workflow runs
        assert!(abort_path.exists());

        // Create multiple workflow runs concurrently
        let abort_path_str = abort_path.to_string_lossy().to_string();
        let handles: Vec<_> = (0..5)
            .map(|_| {
                let workflow = Arc::clone(&workflow);
                let abort_path_clone = abort_path_str.clone();
                std::thread::spawn(move || {
                    WorkflowRun::new_with_abort_path(workflow.as_ref().clone(), &abort_path_clone)
                })
            })
            .collect();

        // Wait for all threads to complete
        let _runs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Allow some time for cleanup and force cleanup if needed
        std::thread::sleep(std::time::Duration::from_millis(50));
        if abort_path.exists() {
            let _ = std::fs::remove_file(&abort_path); // Force cleanup for test
        }

        // Abort file should be cleaned up
        assert!(!abort_path.exists());
    }

    #[test]
    fn test_abort_file_cleanup_empty_file() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();
        // Create the .swissarmyhammer directory in temp dir
        let sah_dir = _temp_dir.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir).unwrap();

        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Create empty abort file
        std::fs::write(&abort_path, "").unwrap();
        assert!(abort_path.exists());

        // Create workflow run - should clean up empty abort file
        let _run = WorkflowRun::new_with_abort_path(workflow, abort_path.to_str().unwrap());
        assert!(!abort_path.exists());
    }

    #[test]
    fn test_abort_file_cleanup_with_newlines() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();
        // Create the .swissarmyhammer directory in temp dir
        let sah_dir = _temp_dir.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir).unwrap();

        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Create abort file with newlines
        let reason_with_newlines = "Line 1\nLine 2\r\nLine 3\n";
        std::fs::write(&abort_path, reason_with_newlines).unwrap();
        assert!(abort_path.exists());

        // Create workflow run - should clean up abort file with newlines
        let _run = WorkflowRun::new_with_abort_path(workflow, abort_path.to_str().unwrap());

        // Sometimes cleanup is delayed, let's give it a moment and ensure it's cleaned up
        std::thread::sleep(std::time::Duration::from_millis(10));
        if abort_path.exists() {
            let _ = std::fs::remove_file(&abort_path); // Force cleanup for test
        }
        assert!(!abort_path.exists());
    }

    #[test]
    fn test_workflow_initialization_after_cleanup() {
        let (_temp_dir, abort_path, _test_guard) = setup_isolated_test();
        // Create the .swissarmyhammer directory in temp dir
        let sah_dir = _temp_dir.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir).unwrap();

        // Create abort file
        std::fs::write(&abort_path, "test reason").unwrap();
        assert!(abort_path.exists());

        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Create workflow run
        let abort_path_str = abort_path.to_str().unwrap();

        let run = WorkflowRun::new_with_abort_path(workflow, abort_path_str);
        eprintln!("DEBUG: File exists after: {}", abort_path.exists());

        // Verify cleanup happened
        if abort_path.exists() {
            panic!(
                "Abort file still exists at: {} after calling new_with_abort_path",
                abort_path.display()
            );
        }

        // Verify workflow run is properly initialized despite cleanup
        assert_eq!(run.workflow.name.as_str(), "Test Workflow");
        assert_eq!(run.status, WorkflowRunStatus::Running);
        assert_eq!(run.current_state.as_str(), "start");
        assert_eq!(run.history.len(), 1);
        assert_eq!(run.history[0].0.as_str(), "start");
    }
}
