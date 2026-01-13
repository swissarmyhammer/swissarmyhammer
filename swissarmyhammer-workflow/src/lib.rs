//! Workflow system data structures and types
//!
//! This module provides the core types for representing and executing workflows
//! based on Mermaid state diagrams.

pub mod acp;
mod action_parser;
pub mod actions;
#[cfg(test)]
mod actions_tests;

mod definition;
pub mod error;
mod error_utils;
#[cfg(test)]
mod examples_tests;
mod executor;
mod graph;
#[cfg(test)]
mod graph_tests;
mod mcp_integration;
mod metrics;
mod parser;
mod run;
mod state;
mod storage;
pub mod template_context;
#[cfg(test)]
mod template_context_integration_test;
#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod test_liquid_rendering;
mod transition;
mod transition_key;

pub use acp::{
    create_agent, execute_prompt, AcpError, AcpResult, AgentResponse, AgentResponseType,
    McpServerConfig,
};
pub use actions::{
    is_valid_env_var_name, parse_action_from_description,
    parse_action_from_description_with_context, validate_command,
    validate_environment_variables_security, validate_working_directory_security, Action,
    ActionError, ActionResult, LogAction, LogLevel, PromptAction, SetVariableAction, ShellAction,
    SubWorkflowAction, WaitAction,
};

pub use definition::{Workflow, WorkflowError, WorkflowName, WorkflowResult};
pub use error_utils::{
    command_succeeded, extract_stderr, extract_stdout, handle_claude_command_error,
    handle_command_error, handle_command_error_with_mapper,
};
pub use executor::{
    ExecutionEvent, ExecutionEventType, ExecutorError, ExecutorResult, WorkflowExecutor,
};
pub use graph::{GraphError, GraphResult, WorkflowGraphAnalyzer};
pub use mcp_integration::{response_processing, WorkflowShellContext};
pub use metrics::{
    GlobalMetrics, MemoryMetrics, ResourceTrends, RunMetrics, StateExecutionCount, WorkflowMetrics,
    WorkflowSummaryMetrics,
};
pub use parser::{MermaidParser, ParseError, ParseResult};
pub use run::{WorkflowRun, WorkflowRunId, WorkflowRunStatus};
pub use state::{
    CompensationKey, ErrorContext, State, StateError, StateId, StateResult, StateType,
};
pub use storage::{
    CompressedWorkflowStorage, FileSystemWorkflowStorage, MemoryWorkflowStorage, WorkflowResolver,
    WorkflowStorage, WorkflowStorageBackend,
};
pub use template_context::WorkflowTemplateContext;
pub use transition::{ConditionType, Transition, TransitionCondition};
pub use transition_key::TransitionKey;

/// Convenience function to parse a workflow from a string
pub fn parse_workflow_from_string(input: &str) -> ParseResult<Workflow> {
    let workflow_name = WorkflowName::from("test_workflow");
    MermaidParser::parse(input, workflow_name)
}

// Initialize common workflow CEL variables when the module loads
fn init_workflow_cel_variables() {
    let cel_state = swissarmyhammer_cel::CelState::global();
    // Initialize are_tests_passing to false by default
    // This ensures the variable exists for workflows that depend on it
    if cel_state.get("are_tests_passing").is_err() {
        let _ = cel_state.set("are_tests_passing", "false");
    }
}

// Call initialization when module loads
#[ctor::ctor]
fn init() {
    init_workflow_cel_variables();
}
