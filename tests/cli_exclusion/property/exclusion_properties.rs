//! Property-based tests for CLI exclusion system
//!
//! These tests use the proptest crate to validate system properties with
//! randomly generated inputs, ensuring robustness and correctness across
//! a wide range of scenarios.

use proptest::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig};
use swissarmyhammer_tools::cli::{RegistryCliExclusionDetector, ToolCliMetadata};
use swissarmyhammer_tools::ToolRegistry;
use super::super::common::test_utils::{ExcludedMockTool, IncludedMockTool};

// Generators for test data

/// Generate valid tool names (alphanumeric with underscores)
fn tool_name() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Z][a-zA-Z0-9_]{2,19}")
        .expect("Valid regex for tool names")
}

/// Generate exclusion reasons
fn exclusion_reason() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Z0-9 .,!?-]{1,100}")
        .expect("Valid regex for exclusion reasons")
}

/// Generate tool metadata
fn tool_metadata() -> impl Strategy<Value = (String, bool, Option<String>)> {
    (
        tool_name(),
        any::<bool>(),
        prop::option::of(exclusion_reason())
    )
}

/// Generate a collection of tool metadata
fn tool_metadata_collection() -> impl Strategy<Value = Vec<(String, bool, Option<String>)>> {
    prop::collection::vec(tool_metadata(), 0..50)
        .prop_map(|mut tools| {
            // Ensure unique names
            tools.sort_by(|a, b| a.0.cmp(&b.0));
            tools.dedup_by(|a, b| a.0 == b.0);
            tools
        })
}

proptest! {
    /// Property: Exclusion detection is consistent across multiple queries
    #[test]
    fn prop_exclusion_detection_consistency(
        tool_data in tool_metadata_collection()
    ) {
        // Create metadata from generated data
        let mut metadata = HashMap::new();
        for (name, is_excluded, reason) in tool_data {
            if is_excluded {
                metadata.insert(
                    name.clone(),
                    ToolCliMetadata::excluded(&name, reason.as_deref().unwrap_or("Property test exclusion"))
                );
            } else {
                metadata.insert(name.clone(), ToolCliMetadata::included(&name));
            }
        }

        let detector = RegistryCliExclusionDetector::new(metadata);

        // Property: Multiple queries for the same tool should return consistent results
        for (name, expected_excluded, _) in &tool_data {
            let result1 = detector.is_cli_excluded(name);
            let result2 = detector.is_cli_excluded(name);
            let result3 = detector.is_cli_excluded(name);

            prop_assert_eq!(result1, result2);
            prop_assert_eq!(result2, result3);
            prop_assert_eq!(result1, *expected_excluded);
        }
    }

    /// Property: Bulk queries are consistent with individual queries
    #[test]
    fn prop_bulk_query_consistency(
        tool_data in tool_metadata_collection()
    ) {
        let mut metadata = HashMap::new();
        let mut expected_excluded = Vec::new();
        let mut expected_included = Vec::new();

        for (name, is_excluded, reason) in tool_data {
            if is_excluded {
                metadata.insert(
                    name.clone(),
                    ToolCliMetadata::excluded(&name, reason.as_deref().unwrap_or("Property test"))
                );
                expected_excluded.push(name);
            } else {
                metadata.insert(name.clone(), ToolCliMetadata::included(&name));
                expected_included.push(name);
            }
        }

        let detector = RegistryCliExclusionDetector::new(metadata);

        // Property: Bulk excluded query should match individual queries
        let bulk_excluded = detector.get_excluded_tools();
        for tool_name in &expected_excluded {
            prop_assert!(detector.is_cli_excluded(tool_name));
            prop_assert!(bulk_excluded.contains(tool_name));
        }

        // Property: Bulk eligible query should match individual queries
        let bulk_eligible = detector.get_cli_eligible_tools();
        for tool_name in &expected_included {
            prop_assert!(!detector.is_cli_excluded(tool_name));
            prop_assert!(bulk_eligible.contains(tool_name));
        }

        // Property: No tool should appear in both lists
        for excluded_tool in &bulk_excluded {
            prop_assert!(!bulk_eligible.contains(excluded_tool));
        }

        for eligible_tool in &bulk_eligible {
            prop_assert!(!bulk_excluded.contains(eligible_tool));
        }
    }

    /// Property: Total metadata count equals sum of excluded and eligible tools
    #[test]
    fn prop_metadata_completeness(
        tool_data in tool_metadata_collection()
    ) {
        let mut metadata = HashMap::new();
        for (name, is_excluded, reason) in &tool_data {
            if *is_excluded {
                metadata.insert(
                    name.clone(),
                    ToolCliMetadata::excluded(name, reason.as_deref().unwrap_or("Property test"))
                );
            } else {
                metadata.insert(name.clone(), ToolCliMetadata::included(name));
            }
        }

        let detector = RegistryCliExclusionDetector::new(metadata);

        let all_metadata = detector.get_all_tool_metadata();
        let excluded_tools = detector.get_excluded_tools();
        let eligible_tools = detector.get_cli_eligible_tools();

        // Property: Total metadata count should equal sum of excluded and eligible
        prop_assert_eq!(
            all_metadata.len(),
            excluded_tools.len() + eligible_tools.len()
        );

        // Property: All metadata names should be accounted for
        let mut all_names: Vec<String> = all_metadata.iter().map(|m| m.name.clone()).collect();
        all_names.sort();

        let mut combined_names = excluded_tools.clone();
        combined_names.extend(eligible_tools);
        combined_names.sort();

        prop_assert_eq!(all_names, combined_names);
    }

    /// Property: CLI generation respects exclusion detection
    #[test]
    fn prop_cli_generation_respects_exclusions(
        tool_count in 1..20usize,
        exclusion_probability in 0.0..1.0f64
    ) {
        let mut registry = ToolRegistry::new();

        let mut expected_excluded = Vec::new();
        let mut expected_included = Vec::new();

        // Generate tools with random exclusion status
        for i in 0..tool_count {
            let tool_name = format!("prop_tool_{}", i);
            let should_exclude = (i as f64 / tool_count as f64) < exclusion_probability;

            if should_exclude {
                registry.register(Box::new(ExcludedMockTool::new(
                    tool_name.clone(),
                    "Property test exclusion"
                )));
                expected_excluded.push(tool_name);
            } else {
                registry.register(Box::new(IncludedMockTool::new(tool_name.clone())));
                expected_included.push(tool_name);
            }
        }

        // Property: CLI generation should only create commands for included tools
        let generator = CliGenerator::new(Arc::new(registry));
        let result = generator.generate_commands();

        // Generation should always succeed with valid tools
        prop_assert!(result.is_ok());
        let commands = result.unwrap();

        // Property: Number of commands should equal number of included tools
        prop_assert_eq!(commands.len(), expected_included.len());

        // Property: All generated commands should be for included tools
        let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
        for included_tool in &expected_included {
            prop_assert!(command_tool_names.contains(&included_tool));
        }

        // Property: No excluded tools should have generated commands
        for excluded_tool in &expected_excluded {
            prop_assert!(!command_tool_names.contains(&excluded_tool));
        }
    }

    /// Property: Detector behavior with nonexistent tools
    #[test]
    fn prop_nonexistent_tool_behavior(
        existing_tools in tool_metadata_collection(),
        query_tools in prop::collection::vec(tool_name(), 1..20)
    ) {
        let mut metadata = HashMap::new();
        let mut existing_names = Vec::new();

        for (name, is_excluded, reason) in existing_tools {
            existing_names.push(name.clone());
            if is_excluded {
                metadata.insert(
                    name.clone(),
                    ToolCliMetadata::excluded(&name, reason.as_deref().unwrap_or("Property test"))
                );
            } else {
                metadata.insert(name.clone(), ToolCliMetadata::included(&name));
            }
        }

        let detector = RegistryCliExclusionDetector::new(metadata);

        // Property: Queries for nonexistent tools should return false (not excluded)
        for query_tool in &query_tools {
            if !existing_names.contains(query_tool) {
                prop_assert!(!detector.is_cli_excluded(query_tool));
            }
        }

        // Property: Bulk queries should not include nonexistent tools
        let excluded_tools = detector.get_excluded_tools();
        let eligible_tools = detector.get_cli_eligible_tools();

        for query_tool in &query_tools {
            if !existing_names.contains(query_tool) {
                prop_assert!(!excluded_tools.contains(query_tool));
                prop_assert!(!eligible_tools.contains(query_tool));
            }
        }
    }

    /// Property: CLI generation config validation behavior
    #[test]
    fn prop_config_validation(
        prefix_chars in prop::option::of(prop::string::string_regex(r"[a-zA-Z0-9_-]{0,20}").unwrap()),
        max_commands in 0..1000u32
    ) {
        let config = GenerationConfig {
            command_prefix: prefix_chars.clone(),
            max_commands: max_commands as usize,
            ..Default::default()
        };

        let validation_result = config.validate();

        // Property: Empty prefix should be invalid
        if let Some(ref prefix) = prefix_chars {
            if prefix.is_empty() {
                prop_assert!(validation_result.is_err());
            } else if prefix.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                // Valid characters should pass validation (assuming other constraints are met)
                if max_commands > 0 {
                    prop_assert!(validation_result.is_ok());
                }
            }
        }

        // Property: Zero max_commands should be invalid
        if max_commands == 0 {
            prop_assert!(validation_result.is_err());
        }
    }

    /// Property: Tool metadata serialization properties
    #[test]
    fn prop_metadata_properties(
        name in tool_name(),
        is_excluded in any::<bool>(),
        reason in prop::option::of(exclusion_reason())
    ) {
        let metadata = if is_excluded {
            ToolCliMetadata::excluded(&name, reason.as_deref().unwrap_or("Property test"))
        } else {
            ToolCliMetadata::included(&name)
        };

        // Property: Name should be preserved exactly
        prop_assert_eq!(metadata.name, name);

        // Property: Exclusion status should match input
        prop_assert_eq!(metadata.is_cli_excluded, is_excluded);

        // Property: Excluded metadata should have reason, included should not
        if is_excluded {
            prop_assert!(metadata.exclusion_reason.is_some());
        } else {
            prop_assert!(metadata.exclusion_reason.is_none());
        }

        // Property: Metadata should be cloneable and equal to itself
        let cloned = metadata.clone();
        prop_assert_eq!(metadata, cloned);

        // Property: Debug representation should contain the name
        let debug_str = format!("{:?}", metadata);
        prop_assert!(debug_str.contains(&name));
    }

    /// Property: Detector performance scales reasonably with size
    #[test]
    fn prop_detector_performance_scaling(
        tool_count in 10..1000usize
    ) {
        let mut metadata = HashMap::new();
        
        // Generate large metadata set
        for i in 0..tool_count {
            let name = format!("perf_tool_{}", i);
            let is_excluded = i % 3 == 0; // Every 3rd tool is excluded
            
            if is_excluded {
                metadata.insert(name.clone(), ToolCliMetadata::excluded(&name, "Perf test"));
            } else {
                metadata.insert(name.clone(), ToolCliMetadata::included(&name));
            }
        }

        let start_time = std::time::Instant::now();
        let detector = RegistryCliExclusionDetector::new(metadata);
        let creation_time = start_time.elapsed();

        // Property: Detector creation should be reasonably fast
        prop_assert!(creation_time.as_millis() < 1000);

        // Property: Bulk queries should complete quickly
        let query_start = std::time::Instant::now();
        let excluded_tools = detector.get_excluded_tools();
        let eligible_tools = detector.get_cli_eligible_tools();
        let query_time = query_start.elapsed();

        prop_assert!(query_time.as_millis() < 500);

        // Property: Results should be consistent with expected distribution
        let total_tools = excluded_tools.len() + eligible_tools.len();
        prop_assert_eq!(total_tools, tool_count);

        // Roughly 1/3 should be excluded
        let expected_excluded = tool_count / 3;
        let actual_excluded = excluded_tools.len();
        let difference = if actual_excluded > expected_excluded {
            actual_excluded - expected_excluded
        } else {
            expected_excluded - actual_excluded
        };
        
        // Allow for small variance due to integer division
        prop_assert!(difference <= 2);
    }

    /// Property: Registry modifications don't affect existing detectors
    #[test]
    fn prop_detector_isolation(
        initial_tools in prop::collection::vec((tool_name(), any::<bool>()), 1..20),
        additional_tools in prop::collection::vec((tool_name(), any::<bool>()), 1..10)
    ) {
        let mut registry = ToolRegistry::new();

        // Register initial tools
        for (name, is_excluded) in &initial_tools {
            if *is_excluded {
                registry.register(Box::new(ExcludedMockTool::new(name, "Initial tool")));
            } else {
                registry.register(Box::new(IncludedMockTool::new(name)));
            }
        }

        // Create detector from initial state
        let initial_detector = registry.as_exclusion_detector();
        let initial_count = initial_detector.get_all_tool_metadata().len();

        // Add more tools
        for (name, is_excluded) in &additional_tools {
            if initial_tools.iter().any(|(existing_name, _)| existing_name == name) {
                continue; // Skip duplicate names
            }

            if *is_excluded {
                registry.register(Box::new(ExcludedMockTool::new(name, "Additional tool")));
            } else {
                registry.register(Box::new(IncludedMockTool::new(name)));
            }
        }

        // Create new detector
        let new_detector = registry.as_exclusion_detector();
        let new_count = new_detector.get_all_tool_metadata().len();

        // Property: Original detector should maintain original count
        prop_assert_eq!(initial_detector.get_all_tool_metadata().len(), initial_count);

        // Property: New detector should see more tools (unless all were duplicates)
        prop_assert!(new_count >= initial_count);

        // Property: Original detector queries should remain consistent
        for (name, expected_excluded) in &initial_tools {
            prop_assert_eq!(initial_detector.is_cli_excluded(name), *expected_excluded);
        }
    }
}

#[cfg(test)]
mod deterministic_property_tests {
    use super::*;

    /// Test specific edge cases that should always hold
    #[test]
    fn test_empty_detector_properties() {
        let detector = RegistryCliExclusionDetector::new(HashMap::new());

        // Properties that should always hold for empty detector
        assert_eq!(detector.get_excluded_tools().len(), 0);
        assert_eq!(detector.get_cli_eligible_tools().len(), 0);
        assert_eq!(detector.get_all_tool_metadata().len(), 0);
        assert!(!detector.is_cli_excluded("any_tool"));
        assert!(!detector.is_cli_excluded(""));
        assert!(!detector.is_cli_excluded("nonexistent"));
    }

    #[test]
    fn test_single_tool_properties() {
        // Test excluded tool
        let mut metadata = HashMap::new();
        metadata.insert(
            "single_excluded".to_string(),
            ToolCliMetadata::excluded("single_excluded", "Test reason"),
        );

        let detector = RegistryCliExclusionDetector::new(metadata);

        assert_eq!(detector.get_excluded_tools(), vec!["single_excluded"]);
        assert_eq!(detector.get_cli_eligible_tools().len(), 0);
        assert!(detector.is_cli_excluded("single_excluded"));
        assert!(!detector.is_cli_excluded("other_tool"));

        // Test included tool
        let mut metadata = HashMap::new();
        metadata.insert(
            "single_included".to_string(),
            ToolCliMetadata::included("single_included"),
        );

        let detector = RegistryCliExclusionDetector::new(metadata);

        assert_eq!(detector.get_excluded_tools().len(), 0);
        assert_eq!(detector.get_cli_eligible_tools(), vec!["single_included"]);
        assert!(!detector.is_cli_excluded("single_included"));
        assert!(!detector.is_cli_excluded("other_tool"));
    }

    #[test]
    fn test_cli_generation_empty_registry_property() {
        let empty_registry = Arc::new(ToolRegistry::new());
        let generator = CliGenerator::new(empty_registry);

        let result = generator.generate_commands();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_metadata_equality_properties() {
        let meta1 = ToolCliMetadata::included("test");
        let meta2 = ToolCliMetadata::included("test");
        let meta3 = ToolCliMetadata::excluded("test", "reason");

        assert_eq!(meta1, meta2);
        assert_ne!(meta1, meta3);
        assert_eq!(meta1.clone(), meta1);
    }
}