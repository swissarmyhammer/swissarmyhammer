//! Agent reasoning and thought generation types

use std::time::SystemTime;

/// Agent reasoning phases for thought generation
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ReasoningPhase {
    /// Initial analysis of the user's prompt
    PromptAnalysis,
    /// Planning the overall strategy and approach
    StrategyPlanning,
    /// Selecting appropriate tools for the task
    ToolSelection,
    /// Breaking down complex problems into smaller parts
    ProblemDecomposition,
    /// Executing the planned approach
    Execution,
    /// Evaluating results and determining next steps
    ResultEvaluation,
}

/// Agent thought content with contextual information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentThought {
    /// The reasoning phase this thought belongs to
    pub phase: ReasoningPhase,
    /// Human-readable thought content
    pub content: String,
    /// Optional structured context data
    pub context: Option<serde_json::Value>,
    /// Timestamp when the thought was generated
    pub timestamp: SystemTime,
}

impl AgentThought {
    /// Create a new agent thought for a specific reasoning phase
    pub fn new(phase: ReasoningPhase, content: impl Into<String>) -> Self {
        Self {
            phase,
            content: content.into(),
            context: None,
            timestamp: SystemTime::now(),
        }
    }

    /// Create a new agent thought with additional context
    pub fn with_context(
        phase: ReasoningPhase,
        content: impl Into<String>,
        context: serde_json::Value,
    ) -> Self {
        Self {
            phase,
            content: content.into(),
            context: Some(context),
            timestamp: SystemTime::now(),
        }
    }
}
