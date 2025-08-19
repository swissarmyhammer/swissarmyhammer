//! Test utilities for CLI exclusion system testing
//!
//! This module provides common utilities, mock implementations, and helper functions
//! for testing the CLI exclusion system across all test layers.

use async_trait::async_trait;
use rmcp::model::{CallToolResult, RawContent, RawTextContent};
use rmcp::Error as McpError;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_tools::cli::{CliExclusionMarker, RegistryCliExclusionDetector, ToolCliMetadata};
use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment;

/// Mock MCP tool that implements CLI exclusion
#[derive(Default, Debug)]
pub struct ExcludedMockTool {
    pub name: String,
    pub exclusion_reason: String,
}

impl ExcludedMockTool {
    pub fn new(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exclusion_reason: reason.into(),
        }
    }
}

impl CliExclusionMarker for ExcludedMockTool {
    fn is_cli_excluded(&self) -> bool {
        true
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        // Note: This is a limitation of the trait design - we can only return static strings
        // In real implementation, this would be handled by the registry metadata
        Some("Mock tool for testing exclusion")
    }
}

#[async_trait]
impl McpTool for ExcludedMockTool {
    fn name(&self) -> &'static str {
        // SAFETY: This is only used in tests where the string lives long enough
        Box::leak(self.name.clone().into_boxed_str())
    }

    fn description(&self) -> &'static str {
        "Mock tool for testing CLI exclusion functionality"
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "test_param": {
                    "type": "string",
                    "description": "Test parameter for mock tool"
                }
            }
        })
    }

    async fn execute(
        &self,
        _arguments: serde_json::Map<String, Value>,
        _context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult {
            content: vec![RawContent::Text(RawTextContent {
                text: "Mock tool executed".to_string(),
            })],
            is_error: false,
            meta: None,
        })
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

/// Mock MCP tool that is CLI-eligible
#[derive(Default, Debug)]
pub struct IncludedMockTool {
    pub name: String,
}

impl IncludedMockTool {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl McpTool for IncludedMockTool {
    fn name(&self) -> &'static str {
        // SAFETY: This is only used in tests where the string lives long enough
        Box::leak(self.name.clone().into_boxed_str())
    }

    fn description(&self) -> &'static str {
        "Mock tool for testing CLI inclusion functionality"
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name parameter"
                },
                "optional_param": {
                    "type": "string",
                    "description": "Optional parameter"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(
        &self,
        _arguments: serde_json::Map<String, Value>,
        _context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult {
            content: vec![RawContent::Text(RawTextContent {
                text: "Mock included tool executed".to_string(),
            })],
            is_error: false,
            meta: None,
        })
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

/// Test fixture containing a pre-configured registry with known excluded and included tools
pub struct TestRegistryFixture {
    pub registry: ToolRegistry,
    pub excluded_tool_names: Vec<String>,
    pub included_tool_names: Vec<String>,
}

impl TestRegistryFixture {
    /// Create a new test registry with a mix of excluded and included tools
    pub fn new() -> Self {
        let mut registry = ToolRegistry::new();

        // Add excluded tools
        let excluded_tools = vec![
            ("excluded_tool_1", "MCP workflow orchestration only"),
            ("excluded_tool_2", "Requires specific MCP context"),
            ("test_workflow_tool", "Internal workflow management"),
        ];

        // Add included tools
        let included_tools = vec![
            "included_tool_1",
            "included_tool_2", 
            "test_memo_tool",
            "test_file_tool",
        ];

        let excluded_tool_names: Vec<String> = excluded_tools.iter().map(|(name, _)| name.to_string()).collect();
        let included_tool_names: Vec<String> = included_tools.iter().map(|name| name.to_string()).collect();

        // Register excluded tools
        for (name, reason) in excluded_tools {
            registry.register(Box::new(ExcludedMockTool::new(name, reason)));
        }

        // Register included tools  
        for name in &included_tools {
            registry.register(Box::new(IncludedMockTool::new(name)));
        }

        Self {
            registry,
            excluded_tool_names,
            included_tool_names,
        }
    }

    /// Create a registry with a specific number of tools for performance testing
    pub fn new_with_size(excluded_count: usize, included_count: usize) -> Self {
        let mut registry = ToolRegistry::new();
        let mut excluded_tool_names = Vec::new();
        let mut included_tool_names = Vec::new();

        // Add excluded tools
        for i in 0..excluded_count {
            let name = format!("excluded_tool_{}", i);
            excluded_tool_names.push(name.clone());
            registry.register(Box::new(ExcludedMockTool::new(
                &name,
                "Performance test exclusion"
            )));
        }

        // Add included tools
        for i in 0..included_count {
            let name = format!("included_tool_{}", i);
            included_tool_names.push(name.clone());
            registry.register(Box::new(IncludedMockTool::new(&name)));
        }

        Self {
            registry,
            excluded_tool_names,
            included_tool_names,
        }
    }

    /// Get a CLI exclusion detector from this registry
    pub fn as_exclusion_detector(&self) -> RegistryCliExclusionDetector {
        self.registry.as_exclusion_detector()
    }

    /// Get the total number of tools in this registry
    pub fn total_count(&self) -> usize {
        self.excluded_tool_names.len() + self.included_tool_names.len()
    }
}

/// Test environment with isolated filesystem for CLI exclusion tests
pub struct CliExclusionTestEnvironment {
    pub _env: IsolatedTestEnvironment,
    pub fixture: TestRegistryFixture,
}

impl CliExclusionTestEnvironment {
    /// Create a new isolated test environment with CLI exclusion test fixtures
    pub fn new() -> Self {
        Self {
            _env: IsolatedTestEnvironment::new(),
            fixture: TestRegistryFixture::new(),
        }
    }

    /// Create environment with custom tool counts
    pub fn with_tool_counts(excluded_count: usize, included_count: usize) -> Self {
        Self {
            _env: IsolatedTestEnvironment::new(),
            fixture: TestRegistryFixture::new_with_size(excluded_count, included_count),
        }
    }
}

/// Helper function to create metadata for testing
pub fn create_test_metadata() -> HashMap<String, ToolCliMetadata> {
    let mut metadata = HashMap::new();
    
    // Add excluded tools
    metadata.insert(
        "excluded_tool_1".to_string(),
        ToolCliMetadata::excluded("excluded_tool_1", "Test exclusion 1"),
    );
    metadata.insert(
        "excluded_tool_2".to_string(), 
        ToolCliMetadata::excluded("excluded_tool_2", "Test exclusion 2"),
    );
    
    // Add included tools
    metadata.insert(
        "included_tool_1".to_string(),
        ToolCliMetadata::included("included_tool_1"),
    );
    metadata.insert(
        "included_tool_2".to_string(),
        ToolCliMetadata::included("included_tool_2"),
    );
    
    metadata
}

/// Assert that the exclusion detector correctly identifies excluded tools
pub fn assert_exclusion_detection(
    detector: &RegistryCliExclusionDetector,
    expected_excluded: &[&str],
    expected_included: &[&str],
) {
    // Test individual exclusion queries
    for tool_name in expected_excluded {
        assert!(
            detector.is_cli_excluded(tool_name),
            "Tool '{}' should be marked as CLI-excluded",
            tool_name
        );
    }

    for tool_name in expected_included {
        assert!(
            !detector.is_cli_excluded(tool_name),
            "Tool '{}' should not be marked as CLI-excluded",
            tool_name
        );
    }

    // Test bulk queries
    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();

    assert_eq!(excluded_tools.len(), expected_excluded.len());
    assert_eq!(eligible_tools.len(), expected_included.len());

    // Verify all expected tools are present
    for expected in expected_excluded {
        assert!(
            excluded_tools.contains(&expected.to_string()),
            "Expected excluded tool '{}' not found in exclusion list",
            expected
        );
    }

    for expected in expected_included {
        assert!(
            eligible_tools.contains(&expected.to_string()),
            "Expected included tool '{}' not found in eligible list", 
            expected
        );
    }
}

/// Performance measurement utility for exclusion queries
pub struct ExclusionQueryPerformance {
    pub query_times: Vec<std::time::Duration>,
    pub total_duration: std::time::Duration,
    pub queries_per_second: f64,
}

impl ExclusionQueryPerformance {
    pub fn measure<F>(detector: &RegistryCliExclusionDetector, query_count: usize, query_fn: F) -> Self
    where
        F: Fn(&RegistryCliExclusionDetector, usize) -> (),
    {
        let start_time = std::time::Instant::now();
        let mut query_times = Vec::with_capacity(query_count);

        for i in 0..query_count {
            let query_start = std::time::Instant::now();
            query_fn(detector, i);
            query_times.push(query_start.elapsed());
        }

        let total_duration = start_time.elapsed();
        let queries_per_second = query_count as f64 / total_duration.as_secs_f64();

        Self {
            query_times,
            total_duration,
            queries_per_second,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_excluded_mock_tool() {
        let tool = ExcludedMockTool::new("test_tool", "test reason");
        assert!(tool.is_cli_excluded());
        assert!(tool.exclusion_reason().is_some());
        assert_eq!(tool.name, "test_tool");
    }

    #[test]
    fn test_included_mock_tool() {
        let tool = IncludedMockTool::new("test_tool");
        assert!(!tool.is_cli_excluded()); // Default implementation
        assert!(tool.exclusion_reason().is_none()); // Default implementation
        assert_eq!(tool.name, "test_tool");
    }

    #[test]
    fn test_registry_fixture() {
        let fixture = TestRegistryFixture::new();
        
        assert!(!fixture.excluded_tool_names.is_empty());
        assert!(!fixture.included_tool_names.is_empty());
        assert_eq!(
            fixture.registry.len(),
            fixture.excluded_tool_names.len() + fixture.included_tool_names.len()
        );
    }

    #[test]
    fn test_registry_fixture_with_size() {
        let fixture = TestRegistryFixture::new_with_size(5, 10);
        
        assert_eq!(fixture.excluded_tool_names.len(), 5);
        assert_eq!(fixture.included_tool_names.len(), 10);
        assert_eq!(fixture.total_count(), 15);
        assert_eq!(fixture.registry.len(), 15);
    }

    #[test]
    fn test_cli_exclusion_test_environment() {
        let env = CliExclusionTestEnvironment::new();
        assert!(!env.fixture.excluded_tool_names.is_empty());
        assert!(!env.fixture.included_tool_names.is_empty());
    }

    #[test]
    fn test_create_test_metadata() {
        let metadata = create_test_metadata();
        
        assert!(metadata.len() >= 4); // At least 2 excluded, 2 included
        
        assert!(metadata.get("excluded_tool_1").unwrap().is_cli_excluded);
        assert!(!metadata.get("included_tool_1").unwrap().is_cli_excluded);
    }

    #[test]
    fn test_assert_exclusion_detection() {
        let metadata = create_test_metadata();
        let detector = RegistryCliExclusionDetector::new(metadata);

        // This should not panic if the exclusion detection is working correctly
        assert_exclusion_detection(
            &detector,
            &["excluded_tool_1", "excluded_tool_2"],
            &["included_tool_1", "included_tool_2"],
        );
    }
}