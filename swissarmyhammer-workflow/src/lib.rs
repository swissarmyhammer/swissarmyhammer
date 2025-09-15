//! Workflow system data structures and types
//!
//! This module provides the core types for representing and executing workflows
//! based on Mermaid state diagrams.

mod action_parser;
pub mod actions;
#[cfg(test)]
mod actions_tests;
mod agents;

mod definition;
pub mod error;
mod error_utils;
#[cfg(test)]
mod examples_tests;
mod executor;
#[cfg(test)]
mod executor_utils;
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


pub use actions::{
    is_valid_env_var_name, parse_action_from_description,
    parse_action_from_description_with_context, validate_command,
    validate_environment_variables_security, validate_working_directory_security, Action,
    ActionError, ActionResult, AgentExecutionContext, AgentExecutor, AgentExecutorFactory,
    LogAction, LogLevel, PromptAction, SetVariableAction, ShellAction, SubWorkflowAction,
    WaitAction,
};
pub use agents::LlamaAgentExecutor;

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
    CompressedWorkflowStorage, FileSystemWorkflowRunStorage, FileSystemWorkflowStorage,
    MemoryWorkflowRunStorage, MemoryWorkflowStorage, WorkflowResolver, WorkflowRunStorageBackend,
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
