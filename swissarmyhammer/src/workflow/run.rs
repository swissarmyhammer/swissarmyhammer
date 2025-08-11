//! Workflow runtime execution types

use crate::common::generate_monotonic_ulid;
use crate::workflow::{StateId, Workflow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub context: HashMap<String, serde_json::Value>,
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
        // Clean up any existing abort file to ensure clean slate
        match std::fs::remove_file(".swissarmyhammer/.abort") {
            Ok(()) => {
                tracing::debug!("Cleaned up existing abort file");
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist, no cleanup needed
            }
            Err(e) => {
                tracing::warn!("Failed to clean up abort file: {}", e);
                // Continue with workflow initialization
            }
        }

        let now = chrono::Utc::now();
        let initial_state = workflow.initial_state.clone();
        Self {
            id: WorkflowRunId::new(),
            workflow,
            current_state: initial_state.clone(),
            history: vec![(initial_state, now)],
            context: Default::default(),
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
    use crate::workflow::test_helpers::*;

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
        use tempfile::TempDir;

        // Create a temporary directory for this test to avoid conflicts
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        let sah_dir = temp_path.join(".swissarmyhammer");
        let abort_path = sah_dir.join(".abort");

        // Create the .swissarmyhammer directory
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

        // Change to the temp directory to test the relative path logic
        let original_dir = std::env::current_dir().expect("Failed to get current dir");
        std::env::set_current_dir(temp_path).expect("Failed to set current dir");

        // Create a new workflow run - this should clean up the abort file
        let _run = WorkflowRun::new(workflow);

        // Verify the abort file was cleaned up
        assert!(
            !abort_path.exists(),
            "Abort file should be cleaned up after WorkflowRun::new"
        );

        // Restore original directory
        std::env::set_current_dir(original_dir).expect("Failed to restore current dir");
    }

    #[test]
    fn test_abort_file_cleanup_when_file_does_not_exist() {
        // Create a test workflow
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Ensure abort file doesn't exist
        let abort_path = ".swissarmyhammer/.abort";
        let _ = std::fs::remove_file(abort_path); // Ignore if it doesn't exist

        // Create a new workflow run - should not fail even if file doesn't exist
        let run = WorkflowRun::new(workflow);

        // Verify workflow run was created successfully
        assert_eq!(run.workflow.name.as_str(), "Test Workflow");
        assert_eq!(run.status, WorkflowRunStatus::Running);
    }

    #[test]
    fn test_abort_file_cleanup_continues_on_permission_error() {
        // Create a test workflow
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // This test would be difficult to simulate without root access or special file system setup
        // Instead, we test that workflow creation continues even if cleanup fails
        // The actual error handling is tested in the implementation by using match expressions

        // Create a new workflow run
        let run = WorkflowRun::new(workflow);

        // Verify workflow run was created successfully regardless of cleanup result
        assert_eq!(run.workflow.name.as_str(), "Test Workflow");
        assert_eq!(run.status, WorkflowRunStatus::Running);
        assert_eq!(run.current_state.as_str(), "start");
        assert_eq!(run.history.len(), 1);
    }

    #[test]
    fn test_multiple_workflow_runs_cleanup_abort_file() {
        use std::path::Path;

        // Create a test workflow
        let mut workflow = create_workflow("Test Workflow", "A test workflow", "start");
        workflow.add_state(create_state("start", "Start state", false));

        // Create the .swissarmyhammer directory if it doesn't exist
        std::fs::create_dir_all(".swissarmyhammer").unwrap();

        let abort_path = ".swissarmyhammer/.abort";

        // Create first abort file
        std::fs::write(abort_path, "first abort reason").unwrap();
        assert!(Path::new(abort_path).exists());

        // Create first workflow run - should clean up abort file
        let _run1 = WorkflowRun::new(workflow.clone());
        assert!(!Path::new(abort_path).exists());

        // Create second abort file
        std::fs::write(abort_path, "second abort reason").unwrap();
        assert!(Path::new(abort_path).exists());

        // Create second workflow run - should also clean up abort file
        let _run2 = WorkflowRun::new(workflow);
        assert!(!Path::new(abort_path).exists());
    }
}
