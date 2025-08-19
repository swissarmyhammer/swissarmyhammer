//! Integration tests for CLI exclusion detection with actual tool registry
//!
//! These tests validate that the exclusion detection works properly with
//! real MCP tools and registry instances.

#[cfg(test)]
mod tests {
    use crate::cli::CliExclusionDetector;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_registry_detects_excluded_issue_tools() {
        let mut registry = ToolRegistry::new();
        
        // Register some tools including the excluded ones
        registry.register(crate::mcp::tools::issues::work::WorkIssueTool::new());
        registry.register(crate::mcp::tools::issues::merge::MergeIssueTool::new());
        registry.register(crate::mcp::tools::issues::create::CreateIssueTool::new());
        registry.register(crate::mcp::tools::issues::list::ListIssuesTool::new());

        let detector = registry.create_cli_exclusion_detector();

        // Check that issue_work and issue_merge are excluded
        assert!(detector.is_cli_excluded("issue_work"));
        assert!(detector.is_cli_excluded("issue_merge"));
        
        // Check that other issue tools are included
        assert!(!detector.is_cli_excluded("issue_create"));
        assert!(!detector.is_cli_excluded("issue_list"));

        // Verify the lists contain expected tools
        let excluded_tools = detector.get_excluded_tools();
        let eligible_tools = detector.get_cli_eligible_tools();

        assert!(excluded_tools.contains(&"issue_work".to_string()));
        assert!(excluded_tools.contains(&"issue_merge".to_string()));
        assert!(!excluded_tools.contains(&"issue_create".to_string()));

        assert!(eligible_tools.contains(&"issue_create".to_string()));
        assert!(eligible_tools.contains(&"issue_list".to_string()));
        assert!(!eligible_tools.contains(&"issue_work".to_string()));
        assert!(!eligible_tools.contains(&"issue_merge".to_string()));
    }

    #[test]
    fn test_registry_metadata_includes_exclusion_reasons() {
        let mut registry = ToolRegistry::new();
        registry.register(crate::mcp::tools::issues::work::WorkIssueTool::new());
        registry.register(crate::mcp::tools::issues::create::CreateIssueTool::new());

        let detector = registry.create_cli_exclusion_detector();
        let all_metadata = detector.get_all_tool_metadata();

        // Find the work tool metadata
        let work_metadata = all_metadata.iter()
            .find(|m| m.name == "issue_work")
            .expect("issue_work metadata should be present");

        assert!(work_metadata.is_cli_excluded);
        assert!(work_metadata.exclusion_reason.is_some());

        // Find the create tool metadata  
        let create_metadata = all_metadata.iter()
            .find(|m| m.name == "issue_create")
            .expect("issue_create metadata should be present");

        assert!(!create_metadata.is_cli_excluded);
        assert!(create_metadata.exclusion_reason.is_none());
    }

    #[test]
    fn test_registry_convenience_methods() {
        let mut registry = ToolRegistry::new();
        registry.register(crate::mcp::tools::issues::work::WorkIssueTool::new());
        registry.register(crate::mcp::tools::issues::create::CreateIssueTool::new());

        let excluded = registry.get_excluded_tool_names();
        let eligible = registry.get_cli_eligible_tool_names();

        assert!(excluded.contains(&"issue_work".to_string()));
        assert!(!excluded.contains(&"issue_create".to_string()));

        assert!(eligible.contains(&"issue_create".to_string()));
        assert!(!eligible.contains(&"issue_work".to_string()));
    }

    #[test]
    fn test_empty_registry_detection() {
        let registry = ToolRegistry::new();
        let detector = registry.create_cli_exclusion_detector();

        assert!(detector.get_excluded_tools().is_empty());
        assert!(detector.get_cli_eligible_tools().is_empty());
        assert!(!detector.is_cli_excluded("any_tool"));
    }
}