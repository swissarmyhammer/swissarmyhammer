//! Tool definition and execution types.
//!
//! This module contains types for defining, calling, and managing tools,
//! including parallel execution configuration and dependency analysis.

use serde::{Deserialize, Serialize};
// Note: serde_json::Value removed as unused
use std::collections::HashMap;

use crate::types::ids::ToolCallId;

/// Definition of a tool available through MCP servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub server_name: String,
}

/// A call to execute a specific tool with arguments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: ToolCallId,
    pub result: serde_json::Value,
    pub error: Option<String>,
}

/// Configuration for parallel tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelConfig {
    pub max_parallel_tools: usize,
    pub conflict_detection: bool,
    pub resource_analysis: bool,
    pub never_parallel: Vec<(String, String)>,
    pub tool_conflicts: Vec<ToolConflict>,
    pub resource_access_patterns: std::collections::HashMap<String, Vec<ResourceAccess>>,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_parallel_tools: 4,
            conflict_detection: true,
            resource_analysis: true,
            never_parallel: Vec::new(),
            tool_conflicts: Vec::new(),
            resource_access_patterns: HashMap::new(),
        }
    }
}

/// Type alias for parallel execution configuration.
pub type ParallelExecutionConfig = ParallelConfig;

/// Type of access a tool requires to a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessType {
    Read,
    Write,
    ReadWrite,
    Delete,
}

/// Definition of resource access pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAccess {
    pub resource: ResourceType,
    pub access_type: AccessType,
    pub exclusive: bool,
}

/// Type of resource that can be accessed by tools.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResourceType {
    File(String),
    FileSystem(String),
    Network(String),
    Database(String),
    Memory,
    System,
    Other(String),
}

/// Type of conflict between tools.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictType {
    ResourceConflict,
    DependencyConflict,
    OrderDependency,
    MutualExclusion,
}

/// Definition of a conflict between two tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConflict {
    pub tool1: String,
    pub tool2: String,
    pub conflict_type: ConflictType,
    pub description: String,
}

/// Type of parameter reference in tool dependencies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReferenceType {
    Input,
    Output,
    Context,
    DirectOutput,
}

/// Reference to a parameter in tool dependency chains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterReference {
    pub parameter_name: String,
    pub parameter_path: String,
    pub reference_type: ReferenceType,
    pub target_tool: Option<String>,
    pub referenced_tool: String,
}
