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
        Self::new_impl(workflow, None)
    }

    /// Create a new workflow run with agent configuration
    pub fn new_with_agent(workflow: Workflow, agent: Arc<ModelConfig>) -> Self {
        Self::new_impl(workflow, Some(agent))
    }

    fn new_impl(workflow: Workflow, agent: Option<Arc<ModelConfig>>) -> Self {
        // Note: Abort state is managed via JS (js_set("abort", "true"))
        // No file cleanup needed

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
