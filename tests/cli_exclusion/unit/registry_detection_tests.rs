//! Unit tests for registry-based CLI exclusion detection
//!
//! These tests validate the core functionality of the exclusion detection system,
//! including the CliExclusionDetector trait, ToolCliMetadata, and registry integration.

use std::collections::HashMap;
use swissarmyhammer_tools::cli::{
    CliExclusionDetector, CliExclusionMarker, RegistryCliExclusionDetector, ToolCliMetadata,
};
use super::super::common::test_utils::{create_test_metadata, assert_exclusion_detection, ExcludedMockTool, IncludedMockTool};

/// Test ToolCliMetadata creation and properties
mod tool_cli_metadata_tests {
    use super::*;

    #[test]
    fn test_included_metadata_creation() {
        let metadata = ToolCliMetadata::included("test_tool");

        assert_eq!(metadata.name, "test_tool");
        assert!(!metadata.is_cli_excluded);
        assert!(metadata.exclusion_reason.is_none());
    }

    #[test]
    fn test_excluded_metadata_creation() {
        let metadata = ToolCliMetadata::excluded("test_tool", "Test exclusion reason");

        assert_eq!(metadata.name, "test_tool");
        assert!(metadata.is_cli_excluded);
        assert_eq!(
            metadata.exclusion_reason.as_deref(),
            Some("Test exclusion reason")
        );
    }

    #[test]
    fn test_metadata_clone_and_equality() {
        let metadata1 = ToolCliMetadata::excluded("tool", "reason");
        let metadata2 = metadata1.clone();

        assert_eq!(metadata1, metadata2);
        assert_eq!(metadata1.name, metadata2.name);
        assert_eq!(metadata1.is_cli_excluded, metadata2.is_cli_excluded);
        assert_eq!(metadata1.exclusion_reason, metadata2.exclusion_reason);
    }

    #[test]
    fn test_metadata_debug_display() {
        let excluded = ToolCliMetadata::excluded("excluded_tool", "Test reason");
        let included = ToolCliMetadata::included("included_tool");

        let excluded_debug = format!("{:?}", excluded);
        let included_debug = format!("{:?}", included);

        assert!(excluded_debug.contains("excluded_tool"));
        assert!(excluded_debug.contains("Test reason"));
        assert!(included_debug.contains("included_tool"));
    }

    #[test]
    fn test_metadata_with_empty_strings() {
        let metadata = ToolCliMetadata::excluded("", "");
        
        assert_eq!(metadata.name, "");
        assert!(metadata.is_cli_excluded);
        assert_eq!(metadata.exclusion_reason.as_deref(), Some(""));
    }

    #[test]
    fn test_metadata_with_unicode() {
        let metadata = ToolCliMetadata::excluded("工具名称", "排除原因：测试");
        
        assert_eq!(metadata.name, "工具名称");
        assert!(metadata.is_cli_excluded);
        assert_eq!(metadata.exclusion_reason.as_deref(), Some("排除原因：测试"));
    }

    #[test]
    fn test_metadata_with_long_strings() {
        let long_name = "a".repeat(1000);
        let long_reason = "b".repeat(2000);
        
        let metadata = ToolCliMetadata::excluded(&long_name, &long_reason);
        
        assert_eq!(metadata.name.len(), 1000);
        assert_eq!(metadata.exclusion_reason.as_ref().unwrap().len(), 2000);
    }
}

/// Test CliExclusionMarker trait implementations
mod cli_exclusion_marker_tests {
    use super::*;

    #[test]
    fn test_default_trait_implementation() {
        #[derive(Default)]
        struct DefaultTool;

        impl CliExclusionMarker for DefaultTool {}

        let tool = DefaultTool;
        assert!(!tool.is_cli_excluded());
        assert!(tool.exclusion_reason().is_none());
    }

    #[test]
    fn test_excluded_mock_tool_trait() {
        let tool = ExcludedMockTool::new("test_tool", "test reason");
        
        assert!(tool.is_cli_excluded());
        assert!(tool.exclusion_reason().is_some());
    }

    #[test]
    fn test_included_mock_tool_trait() {
        let tool = IncludedMockTool::new("test_tool");
        
        // Uses default trait implementation
        assert!(!tool.is_cli_excluded());
        assert!(tool.exclusion_reason().is_none());
    }

    #[test]
    fn test_custom_exclusion_implementation() {
        struct CustomExcludedTool {
            should_exclude: bool,
            reason: &'static str,
        }

        impl CliExclusionMarker for CustomExcludedTool {
            fn is_cli_excluded(&self) -> bool {
                self.should_exclude
            }

            fn exclusion_reason(&self) -> Option<&'static str> {
                if self.should_exclude {
                    Some(self.reason)
                } else {
                    None
                }
            }
        }

        let excluded_tool = CustomExcludedTool {
            should_exclude: true,
            reason: "Custom exclusion logic",
        };
        
        let included_tool = CustomExcludedTool {
            should_exclude: false,
            reason: "Should not be used",
        };

        assert!(excluded_tool.is_cli_excluded());
        assert_eq!(excluded_tool.exclusion_reason(), Some("Custom exclusion logic"));

        assert!(!included_tool.is_cli_excluded());
        assert_eq!(included_tool.exclusion_reason(), None);
    }
}

/// Test RegistryCliExclusionDetector implementation
mod registry_detector_tests {
    use super::*;

    #[test]
    fn test_empty_detector() {
        let detector = RegistryCliExclusionDetector::new(HashMap::new());

        assert!(!detector.is_cli_excluded("any_tool"));
        assert!(detector.get_excluded_tools().is_empty());
        assert!(detector.get_cli_eligible_tools().is_empty());
        assert!(detector.get_all_tool_metadata().is_empty());
    }

    #[test]
    fn test_detector_with_test_metadata() {
        let metadata = create_test_metadata();
        let detector = RegistryCliExclusionDetector::new(metadata);

        // Test individual exclusion queries
        assert!(detector.is_cli_excluded("excluded_tool_1"));
        assert!(detector.is_cli_excluded("excluded_tool_2"));
        assert!(!detector.is_cli_excluded("included_tool_1"));
        assert!(!detector.is_cli_excluded("included_tool_2"));
        assert!(!detector.is_cli_excluded("nonexistent_tool"));
    }

    #[test]
    fn test_get_excluded_tools() {
        let metadata = create_test_metadata();
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        let mut excluded_tools = detector.get_excluded_tools();
        excluded_tools.sort();
        
        assert_eq!(excluded_tools, vec!["excluded_tool_1", "excluded_tool_2"]);
    }

    #[test]
    fn test_get_cli_eligible_tools() {
        let metadata = create_test_metadata();
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        let mut eligible_tools = detector.get_cli_eligible_tools();
        eligible_tools.sort();
        
        assert_eq!(eligible_tools, vec!["included_tool_1", "included_tool_2"]);
    }

    #[test]
    fn test_get_all_tool_metadata() {
        let metadata = create_test_metadata();
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        let all_metadata = detector.get_all_tool_metadata();
        
        assert_eq!(all_metadata.len(), 4);
        
        // Count excluded and included
        let excluded_count = all_metadata
            .iter()
            .filter(|m| m.is_cli_excluded)
            .count();
        let included_count = all_metadata
            .iter()
            .filter(|m| !m.is_cli_excluded)
            .count();
        
        assert_eq!(excluded_count, 2);
        assert_eq!(included_count, 2);
    }

    #[test]
    fn test_detector_consistency() {
        let metadata = create_test_metadata();
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        // Multiple queries should return consistent results
        for _ in 0..100 {
            assert!(detector.is_cli_excluded("excluded_tool_1"));
            assert!(!detector.is_cli_excluded("included_tool_1"));
        }
        
        // Bulk queries should be consistent too
        let excluded1 = detector.get_excluded_tools();
        let excluded2 = detector.get_excluded_tools();
        assert_eq!(excluded1, excluded2);
        
        let eligible1 = detector.get_cli_eligible_tools();
        let eligible2 = detector.get_cli_eligible_tools();
        assert_eq!(eligible1, eligible2);
    }

    #[test]
    fn test_detector_with_single_tool() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "only_tool".to_string(),
            ToolCliMetadata::excluded("only_tool", "Only tool in registry"),
        );
        
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        assert!(detector.is_cli_excluded("only_tool"));
        assert_eq!(detector.get_excluded_tools(), vec!["only_tool"]);
        assert!(detector.get_cli_eligible_tools().is_empty());
        
        let all_metadata = detector.get_all_tool_metadata();
        assert_eq!(all_metadata.len(), 1);
        assert!(all_metadata[0].is_cli_excluded);
    }

    #[test]
    fn test_detector_with_all_included_tools() {
        let mut metadata = HashMap::new();
        metadata.insert("tool1".to_string(), ToolCliMetadata::included("tool1"));
        metadata.insert("tool2".to_string(), ToolCliMetadata::included("tool2"));
        metadata.insert("tool3".to_string(), ToolCliMetadata::included("tool3"));
        
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        assert!(detector.get_excluded_tools().is_empty());
        assert_eq!(detector.get_cli_eligible_tools().len(), 3);
        
        for tool in ["tool1", "tool2", "tool3"] {
            assert!(!detector.is_cli_excluded(tool));
        }
    }

    #[test]
    fn test_detector_with_all_excluded_tools() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "tool1".to_string(),
            ToolCliMetadata::excluded("tool1", "Reason 1"),
        );
        metadata.insert(
            "tool2".to_string(),
            ToolCliMetadata::excluded("tool2", "Reason 2"),
        );
        
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        assert!(detector.get_cli_eligible_tools().is_empty());
        assert_eq!(detector.get_excluded_tools().len(), 2);
        
        for tool in ["tool1", "tool2"] {
            assert!(detector.is_cli_excluded(tool));
        }
    }

    #[test]
    fn test_assert_exclusion_detection_helper() {
        let metadata = create_test_metadata();
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        // This should not panic if working correctly
        assert_exclusion_detection(
            &detector,
            &["excluded_tool_1", "excluded_tool_2"],
            &["included_tool_1", "included_tool_2"],
        );
    }

    #[test]
    #[should_panic(expected = "should be marked as CLI-excluded")]
    fn test_assert_exclusion_detection_failure_case() {
        let metadata = create_test_metadata();
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        // This should panic because included_tool_1 is not excluded
        assert_exclusion_detection(
            &detector,
            &["included_tool_1"], // Wrong - this is actually included
            &["excluded_tool_1"],
        );
    }
}

/// Test edge cases and error conditions
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_tool_names_with_special_characters() {
        let mut metadata = HashMap::new();
        
        // Test various special characters in tool names
        let special_names = [
            "tool-with-dashes",
            "tool_with_underscores", 
            "tool.with.dots",
            "tool:with:colons",
            "tool with spaces",
            "tool/with/slashes",
            "tool@with@symbols",
            "123numeric_start",
            "UPPERCASE_TOOL",
            "MixedCase_Tool",
        ];
        
        for (i, name) in special_names.iter().enumerate() {
            if i % 2 == 0 {
                metadata.insert(
                    name.to_string(),
                    ToolCliMetadata::excluded(name, "Test exclusion"),
                );
            } else {
                metadata.insert(name.to_string(), ToolCliMetadata::included(name));
            }
        }
        
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        // Test that all names are handled correctly
        for (i, name) in special_names.iter().enumerate() {
            let should_be_excluded = i % 2 == 0;
            assert_eq!(
                detector.is_cli_excluded(name),
                should_be_excluded,
                "Tool '{}' exclusion status incorrect",
                name
            );
        }
    }

    #[test] 
    fn test_very_large_metadata_set() {
        let mut metadata = HashMap::new();
        
        // Create a large number of tools
        for i in 0..10000 {
            let name = format!("tool_{}", i);
            if i % 3 == 0 {
                metadata.insert(
                    name.clone(),
                    ToolCliMetadata::excluded(&name, "Large set test exclusion"),
                );
            } else {
                metadata.insert(name.clone(), ToolCliMetadata::included(&name));
            }
        }
        
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        // Test that queries work efficiently even with large datasets
        let excluded_count = detector.get_excluded_tools().len();
        let eligible_count = detector.get_cli_eligible_tools().len();
        let total_count = detector.get_all_tool_metadata().len();
        
        assert_eq!(total_count, 10000);
        assert_eq!(excluded_count + eligible_count, total_count);
        
        // Roughly 1/3 should be excluded (every 3rd tool)
        assert!((excluded_count as f64 / total_count as f64 - 0.33).abs() < 0.01);
    }

    #[test]
    fn test_metadata_with_identical_names() {
        // This tests behavior when metadata has duplicate keys (HashMap behavior)
        let mut metadata = HashMap::new();
        
        // Insert same name twice - second insertion should overwrite
        metadata.insert(
            "duplicate_tool".to_string(),
            ToolCliMetadata::excluded("duplicate_tool", "First insertion"),
        );
        metadata.insert(
            "duplicate_tool".to_string(),
            ToolCliMetadata::included("duplicate_tool"),
        );
        
        let detector = RegistryCliExclusionDetector::new(metadata);
        
        // Should reflect the last insertion (included)
        assert!(!detector.is_cli_excluded("duplicate_tool"));
        assert_eq!(detector.get_cli_eligible_tools(), vec!["duplicate_tool"]);
        assert!(detector.get_excluded_tools().is_empty());
    }

    #[test]
    fn test_exclusion_reason_edge_cases() {
        let mut metadata = HashMap::new();
        
        // Test various reason strings
        metadata.insert(
            "empty_reason".to_string(),
            ToolCliMetadata::excluded("empty_reason", ""),
        );
        metadata.insert(
            "long_reason".to_string(),
            ToolCliMetadata::excluded("long_reason", &"a".repeat(10000)),
        );
        metadata.insert(
            "unicode_reason".to_string(),
            ToolCliMetadata::excluded("unicode_reason", "原因: 日本語 🚀"),
        );
        metadata.insert(
            "multiline_reason".to_string(),
            ToolCliMetadata::excluded("multiline_reason", "Line 1\nLine 2\nLine 3"),
        );
        
        let detector = RegistryCliExclusionDetector::new(metadata);
        let all_metadata = detector.get_all_tool_metadata();
        
        assert_eq!(all_metadata.len(), 4);
        
        // All should be marked as excluded
        for metadata in &all_metadata {
            assert!(metadata.is_cli_excluded);
            assert!(metadata.exclusion_reason.is_some());
        }
        
        // Find specific metadata and test reasons
        let empty_meta = all_metadata
            .iter()
            .find(|m| m.name == "empty_reason")
            .unwrap();
        assert_eq!(empty_meta.exclusion_reason.as_deref(), Some(""));
        
        let long_meta = all_metadata
            .iter()
            .find(|m| m.name == "long_reason")
            .unwrap();
        assert_eq!(long_meta.exclusion_reason.as_ref().unwrap().len(), 10000);
        
        let unicode_meta = all_metadata
            .iter()
            .find(|m| m.name == "unicode_reason")
            .unwrap();
        assert_eq!(unicode_meta.exclusion_reason.as_deref(), Some("原因: 日本語 🚀"));
    }
}