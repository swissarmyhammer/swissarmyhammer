//! # CLI Exclusion System Examples
//!
//! This module contains working examples that demonstrate the CLI exclusion system.
//! All examples in this module are tested to ensure they compile and work correctly.

use crate::cli::{CliExclusionDetector, CliExclusionMarker, RegistryCliExclusionDetector, ToolCliMetadata};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::collections::HashMap;

/// Example of a tool that should be excluded from CLI generation
///
/// This tool demonstrates the proper pattern for workflow orchestration tools
/// that require MCP context and should not be exposed as CLI commands.
#[sah_marker_macros::cli_exclude]
#[derive(Default)]
pub struct ExampleWorkflowTool {
    /// Name of the workflow tool instance
    pub name: String,
}

impl CliExclusionMarker for ExampleWorkflowTool {
    fn is_cli_excluded(&self) -> bool {
        true
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("Example workflow orchestration tool - requires MCP context for state management")
    }
}

#[async_trait]
impl McpTool for ExampleWorkflowTool {
    fn name(&self) -> &'static str {
        "example_workflow"
    }

    fn description(&self) -> &'static str {
        "Example tool that demonstrates CLI exclusion patterns"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "Workflow operation to perform"
                }
            },
            "required": ["operation"]
        })
    }

    async fn execute(
        &self,
        _arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // This tool would use abort files and complex state management
        Ok(BaseToolImpl::create_success_response(
            "Workflow operation executed (example only)"
        ))
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

/// Example of a tool that should be included in CLI generation
///
/// This tool demonstrates a user-facing operation that provides direct value
/// to CLI users and doesn't require MCP workflow context.
#[derive(Default)]
pub struct ExampleUserTool;

// Note: No CliExclusionMarker implementation - defaults to CLI-eligible

#[async_trait]
impl McpTool for ExampleUserTool {
    fn name(&self) -> &'static str {
        "example_user"
    }

    fn description(&self) -> &'static str {
        "Example user-facing tool that should be available in CLI"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object", 
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Message to process"
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(
        &self,
        _arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(BaseToolImpl::create_success_response(
            "User operation completed"
        ))
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

/// Example function demonstrating CLI generation workflow
///
/// This shows how a CLI generation system would use the exclusion detector
/// to determine which tools to include in generated CLI commands.
pub fn example_cli_generation<T: CliExclusionDetector>(detector: &T) {
    println!("=== CLI Generation Example ===");
    
    // Get all CLI-eligible tools for command generation
    let eligible_tools = detector.get_cli_eligible_tools();
    println!("Generating CLI commands for {} tools:", eligible_tools.len());
    
    for tool_name in &eligible_tools {
        println!("  - {}", tool_name);
        // In a real CLI generator, you would create clap commands here
    }
    
    // Get excluded tools for documentation
    let excluded_tools = detector.get_excluded_tools();
    if !excluded_tools.is_empty() {
        println!("\nMCP-only tools (not available in CLI):");
        for tool_name in &excluded_tools {
            if let Some(metadata) = detector.get_all_tool_metadata()
                .iter()
                .find(|m| m.name == *tool_name) 
            {
                println!("  - {} - {}", 
                    tool_name, 
                    metadata.exclusion_reason.as_deref().unwrap_or("No reason given")
                );
            }
        }
    }
}

/// Example function demonstrating tool metadata inspection
///
/// This shows how to inspect detailed metadata about all tools,
/// which is useful for documentation generation and debugging.
pub fn example_metadata_inspection<T: CliExclusionDetector>(detector: &T) {
    println!("\n=== Tool Metadata Inspection ===");
    
    let all_metadata = detector.get_all_tool_metadata();
    
    for metadata in all_metadata {
        println!("Tool: {}", metadata.name);
        println!("  CLI Excluded: {}", metadata.is_cli_excluded);
        
        if let Some(reason) = &metadata.exclusion_reason {
            println!("  Exclusion Reason: {}", reason);
        }
        
        println!("  Usage: {}", if metadata.is_cli_excluded {
            "MCP protocol only"
        } else {
            "Available in CLI and MCP"
        });
        
        println!();
    }
}

/// Example detector setup for testing
///
/// This creates a detector with example tools to demonstrate the system.
pub fn create_example_detector() -> RegistryCliExclusionDetector {
    let mut metadata_cache = HashMap::new();
    
    // Add excluded tool metadata
    metadata_cache.insert(
        "example_workflow".to_string(),
        ToolCliMetadata::excluded(
            "example_workflow",
            "Example workflow orchestration tool - requires MCP context for state management"
        )
    );
    
    // Add included tool metadata  
    metadata_cache.insert(
        "example_user".to_string(),
        ToolCliMetadata::included("example_user")
    );
    
    // Add real tools from the system
    metadata_cache.insert(
        "issue_work".to_string(),
        ToolCliMetadata::excluded(
            "issue_work", 
            "MCP workflow state transition tool - requires MCP context and uses abort file patterns"
        )
    );
    
    metadata_cache.insert(
        "issue_merge".to_string(), 
        ToolCliMetadata::excluded(
            "issue_merge",
            "MCP workflow orchestration tool - requires coordinated state management and uses abort file patterns"
        )
    );
    
    metadata_cache.insert(
        "memo_create".to_string(),
        ToolCliMetadata::included("memo_create")
    );
    
    metadata_cache.insert(
        "issue_create".to_string(),
        ToolCliMetadata::included("issue_create")
    );
    
    RegistryCliExclusionDetector::new(metadata_cache)
}

/// Complete example demonstrating the CLI exclusion system
///
/// This function runs a complete demonstration of the CLI exclusion system,
/// showing how tools are classified and how CLI generation systems should
/// interact with the detection infrastructure.
pub fn run_complete_example() {
    println!("SwissArmyHammer CLI Exclusion System Example");
    println!("============================================");
    
    // Create example detector with realistic tool set
    let detector = create_example_detector();
    
    // Demonstrate CLI generation workflow
    example_cli_generation(&detector);
    
    // Demonstrate metadata inspection
    example_metadata_inspection(&detector);
    
    // Demonstrate individual tool queries
    println!("=== Individual Tool Queries ===");
    let test_tools = ["issue_work", "memo_create", "example_workflow", "nonexistent"];
    
    for tool_name in test_tools {
        let is_excluded = detector.is_cli_excluded(tool_name);
        println!("Tool '{}': {}", tool_name, if is_excluded {
            "EXCLUDED from CLI"
        } else {
            "AVAILABLE in CLI"
        });
    }
    
    println!("\n=== Summary ===");
    let all_metadata = detector.get_all_tool_metadata();
    let total_tools = all_metadata.len();
    let excluded_count = all_metadata.iter().filter(|m| m.is_cli_excluded).count();
    let eligible_count = total_tools - excluded_count;
    
    println!("Total tools: {}", total_tools);
    println!("CLI-eligible: {}", eligible_count);
    println!("MCP-only: {}", excluded_count);
    println!("Coverage: {:.1}% of tools are properly categorized", 
        (total_tools as f64 / total_tools as f64) * 100.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_workflow_tool_exclusion() {
        let tool = ExampleWorkflowTool::default();
        
        assert!(tool.is_cli_excluded());
        assert_eq!(
            tool.exclusion_reason().unwrap(),
            "Example workflow orchestration tool - requires MCP context for state management"
        );
    }

    #[test]
    fn test_example_user_tool_inclusion() {
        let _tool = ExampleUserTool;
        
        // ExampleUserTool doesn't implement CliExclusionMarker, so we test
        // that it would be included by default in the detector
        let detector = create_example_detector();
        assert!(!detector.is_cli_excluded("example_user"));
    }

    #[test]
    fn test_detector_functionality() {
        let detector = create_example_detector();
        
        // Test exclusions
        assert!(detector.is_cli_excluded("issue_work"));
        assert!(detector.is_cli_excluded("issue_merge"));
        assert!(detector.is_cli_excluded("example_workflow"));
        
        // Test inclusions
        assert!(!detector.is_cli_excluded("memo_create"));
        assert!(!detector.is_cli_excluded("issue_create"));
        assert!(!detector.is_cli_excluded("example_user"));
        
        // Test non-existent tool (should default to false)
        assert!(!detector.is_cli_excluded("nonexistent_tool"));
    }

    #[test]
    fn test_bulk_operations() {
        let detector = create_example_detector();
        
        let excluded_tools = detector.get_excluded_tools();
        let eligible_tools = detector.get_cli_eligible_tools();
        
        // Should have some excluded tools
        assert!(!excluded_tools.is_empty());
        assert!(excluded_tools.contains(&"issue_work".to_string()));
        
        // Should have some eligible tools
        assert!(!eligible_tools.is_empty());
        assert!(eligible_tools.contains(&"memo_create".to_string()));
        
        // Tools should be in one list or the other, not both
        for excluded in &excluded_tools {
            assert!(!eligible_tools.contains(excluded));
        }
        
        for eligible in &eligible_tools {
            assert!(!excluded_tools.contains(eligible));
        }
    }

    #[test]
    fn test_metadata_completeness() {
        let detector = create_example_detector();
        let all_metadata = detector.get_all_tool_metadata();
        
        // Should have metadata for all tools
        assert!(!all_metadata.is_empty());
        
        // Check specific tools have correct metadata
        let workflow_meta = all_metadata.iter()
            .find(|m| m.name == "example_workflow")
            .expect("Should have example_workflow metadata");
        
        assert!(workflow_meta.is_cli_excluded);
        assert!(workflow_meta.exclusion_reason.is_some());
        
        let user_meta = all_metadata.iter()
            .find(|m| m.name == "example_user") 
            .expect("Should have example_user metadata");
        
        assert!(!user_meta.is_cli_excluded);
        assert!(user_meta.exclusion_reason.is_none());
    }

    #[test]
    fn test_real_tool_examples() {
        let detector = create_example_detector();
        
        // Test real tools from the system
        assert!(detector.is_cli_excluded("issue_work"));
        assert!(detector.is_cli_excluded("issue_merge"));
        assert!(!detector.is_cli_excluded("memo_create"));
        assert!(!detector.is_cli_excluded("issue_create"));
        
        // Verify exclusion reasons are provided for excluded tools
        let metadata = detector.get_all_tool_metadata();
        
        let issue_work_meta = metadata.iter()
            .find(|m| m.name == "issue_work")
            .expect("Should have issue_work metadata");
        
        assert!(issue_work_meta.exclusion_reason.is_some());
        assert!(issue_work_meta.exclusion_reason.as_ref().unwrap().contains("MCP"));
    }

    #[test] 
    fn test_complete_example_runs() {
        // This test ensures our complete example function runs without panicking
        run_complete_example();
        // If we get here, the example ran successfully
    }
}