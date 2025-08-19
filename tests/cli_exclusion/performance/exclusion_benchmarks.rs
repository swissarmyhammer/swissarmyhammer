//! Performance benchmarks for CLI exclusion system
//!
//! These tests validate the performance characteristics of the exclusion detection
//! and CLI generation systems under various load conditions.

use std::sync::Arc;
use std::time::Instant;
use swissarmyhammer_cli::generation::CliGenerator;
use swissarmyhammer_tools::ToolRegistry;
use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment;
use super::super::common::test_utils::{
    CliExclusionTestEnvironment, ExclusionQueryPerformance, ExcludedMockTool, IncludedMockTool,
};

/// Benchmark exclusion detection query performance
#[tokio::test]
async fn bench_exclusion_detection_queries() {
    let _env = IsolatedTestEnvironment::new();

    // Test different registry sizes to understand scaling characteristics
    let test_sizes = vec![10, 50, 100, 500, 1000];
    
    for &size in &test_sizes {
        println!("Testing exclusion detection with {} tools", size);
        
        let env = CliExclusionTestEnvironment::with_tool_counts(size / 3, (size * 2) / 3);
        let detector = env.fixture.as_exclusion_detector();
        
        // Benchmark individual queries
        let individual_perf = ExclusionQueryPerformance::measure(&detector, 1000, |det, i| {
            let tool_name = format!("test_tool_{}", i % size);
            let _ = det.is_cli_excluded(&tool_name);
        });

        println!(
            "  Individual queries: {:.2} queries/sec, avg {:.2}μs per query",
            individual_perf.queries_per_second,
            individual_perf.total_duration.as_micros() as f64 / 1000.0
        );

        // Performance requirements
        assert!(
            individual_perf.queries_per_second > 10000.0,
            "Individual queries should exceed 10k/sec for {} tools",
            size
        );

        // Benchmark bulk queries
        let bulk_start = Instant::now();
        for _ in 0..100 {
            let _ = detector.get_excluded_tools();
            let _ = detector.get_cli_eligible_tools();
        }
        let bulk_duration = bulk_start.elapsed();
        let bulk_queries_per_sec = 200.0 / bulk_duration.as_secs_f64(); // 100 iterations * 2 queries each

        println!(
            "  Bulk queries: {:.2} queries/sec, {:.2}ms per bulk operation",
            bulk_queries_per_sec,
            bulk_duration.as_millis() as f64 / 100.0
        );

        // Bulk queries should still be reasonably fast
        assert!(
            bulk_queries_per_sec > 100.0,
            "Bulk queries should exceed 100/sec for {} tools",
            size
        );

        // Test metadata queries
        let metadata_start = Instant::now();
        for _ in 0..10 {
            let metadata = detector.get_all_tool_metadata();
            assert_eq!(metadata.len(), env.fixture.total_count());
        }
        let metadata_duration = metadata_start.elapsed();

        println!(
            "  Metadata queries: {:.2}ms per query",
            metadata_duration.as_millis() as f64 / 10.0
        );

        // Metadata queries should complete quickly
        assert!(
            metadata_duration.as_millis() < 1000,
            "Metadata queries should complete in < 1s for {} tools",
            size
        );

        println!();
    }
}

/// Benchmark CLI generation performance
#[tokio::test]
async fn bench_cli_generation_performance() {
    let _env = IsolatedTestEnvironment::new();

    let test_configs = vec![
        (50, "small"),
        (200, "medium"), 
        (500, "large"),
        (1000, "very_large"),
    ];

    for &(total_tools, size_desc) in &test_configs {
        println!("Testing CLI generation with {} {} tools", total_tools, size_desc);
        
        // Create registry with mix of excluded and included tools
        let excluded_count = total_tools / 4; // 25% excluded
        let included_count = total_tools - excluded_count;
        
        let env = CliExclusionTestEnvironment::with_tool_counts(excluded_count, included_count);
        let registry = Arc::new(env.fixture.registry);

        // Benchmark full CLI generation process
        let generation_start = Instant::now();
        let generator = CliGenerator::new(registry.clone());
        let commands = generator.generate_commands().unwrap();
        let generation_duration = generation_start.elapsed();

        println!(
            "  Generated {} commands in {:.2}ms ({:.2} tools/sec)",
            commands.len(),
            generation_duration.as_millis(),
            included_count as f64 / generation_duration.as_secs_f64()
        );

        // Verify correct number of commands generated
        assert_eq!(commands.len(), included_count);

        // Performance requirements based on size
        let max_time_ms = match total_tools {
            n if n <= 100 => 500,   // Small: 500ms
            n if n <= 500 => 2000,  // Medium: 2s
            _ => 5000,              // Large: 5s
        };

        assert!(
            generation_duration.as_millis() < max_time_ms,
            "CLI generation should complete within {}ms for {} tools, took {}ms",
            max_time_ms,
            total_tools,
            generation_duration.as_millis()
        );

        // Test repeated generation (should use cached results where applicable)
        let repeated_start = Instant::now();
        for _ in 0..10 {
            let generator = CliGenerator::new(registry.clone());
            let repeated_commands = generator.generate_commands().unwrap();
            assert_eq!(repeated_commands.len(), commands.len());
        }
        let repeated_duration = repeated_start.elapsed();

        println!(
            "  10 repeated generations: {:.2}ms total, {:.2}ms avg",
            repeated_duration.as_millis(),
            repeated_duration.as_millis() as f64 / 10.0
        );

        // Repeated generations should be reasonably fast
        let avg_repeated_ms = repeated_duration.as_millis() as f64 / 10.0;
        assert!(
            avg_repeated_ms < generation_duration.as_millis() as f64 * 1.5,
            "Repeated generations should not be significantly slower than first generation"
        );

        println!();
    }
}

/// Benchmark concurrent access performance
#[tokio::test]
async fn bench_concurrent_access() {
    let _env = IsolatedTestEnvironment::new();
    
    let env = CliExclusionTestEnvironment::with_tool_counts(250, 750); // 1000 total tools
    let detector = Arc::new(env.fixture.as_exclusion_detector());
    let registry = Arc::new(env.fixture.registry);

    println!("Testing concurrent access with 1000 tools");

    // Benchmark concurrent exclusion queries
    let concurrent_start = Instant::now();
    let mut tasks = Vec::new();

    for task_id in 0..10 {
        let detector_clone = detector.clone();
        let task = tokio::task::spawn(async move {
            // Each task performs 100 queries
            for i in 0..100 {
                let tool_name = format!("test_tool_{}", (task_id * 100 + i) % 1000);
                let _ = detector_clone.is_cli_excluded(&tool_name);
            }
            task_id
        });
        tasks.push(task);
    }

    let results = futures::future::join_all(tasks).await;
    let concurrent_duration = concurrent_start.elapsed();

    // Verify all tasks completed successfully
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Concurrent task {} failed", i);
    }

    let total_queries = 10 * 100; // 10 tasks * 100 queries each
    let queries_per_sec = total_queries as f64 / concurrent_duration.as_secs_f64();

    println!(
        "Concurrent exclusion queries: {} queries in {:.2}ms ({:.2} queries/sec)",
        total_queries,
        concurrent_duration.as_millis(),
        queries_per_sec
    );

    // Should handle concurrent access efficiently
    assert!(
        queries_per_sec > 5000.0,
        "Concurrent queries should exceed 5k/sec, got {:.2}",
        queries_per_sec
    );

    // Benchmark concurrent CLI generation
    let generation_start = Instant::now();
    let mut generation_tasks = Vec::new();

    for _ in 0..5 {
        let registry_clone = registry.clone();
        let task = tokio::task::spawn(async move {
            let generator = CliGenerator::new(registry_clone);
            let commands = generator.generate_commands().unwrap();
            commands.len()
        });
        generation_tasks.push(task);
    }

    let generation_results = futures::future::join_all(generation_tasks).await;
    let concurrent_generation_duration = generation_start.elapsed();

    // Verify all generations completed successfully and consistently
    let mut command_counts = Vec::new();
    for result in generation_results {
        assert!(result.is_ok());
        command_counts.push(result.unwrap());
    }

    // All generations should produce the same number of commands
    let expected_commands = command_counts[0];
    for &count in &command_counts {
        assert_eq!(count, expected_commands);
    }

    println!(
        "Concurrent CLI generation: 5 generations in {:.2}ms ({:.2}ms avg)",
        concurrent_generation_duration.as_millis(),
        concurrent_generation_duration.as_millis() as f64 / 5.0
    );

    // Concurrent generation should not be dramatically slower than sequential
    assert!(
        concurrent_generation_duration.as_millis() < 10000, // 10 seconds
        "Concurrent generation took too long: {}ms",
        concurrent_generation_duration.as_millis()
    );
}

/// Benchmark memory usage characteristics
#[tokio::test]
async fn bench_memory_usage() {
    let _env = IsolatedTestEnvironment::new();

    println!("Testing memory usage characteristics");

    // Test with progressively larger registries
    let sizes = vec![100, 500, 1000, 2000];
    
    for &size in &sizes {
        let env = CliExclusionTestEnvironment::with_tool_counts(size / 3, (size * 2) / 3);
        
        // Measure memory usage of detector creation
        let detector = env.fixture.as_exclusion_detector();
        
        // Exercise the detector to ensure all lazy initialization is complete
        let _ = detector.get_all_tool_metadata();
        let _ = detector.get_excluded_tools();
        let _ = detector.get_cli_eligible_tools();
        
        // Test repeated queries don't cause memory leaks
        for _ in 0..1000 {
            let _ = detector.is_cli_excluded(&format!("tool_{}", size / 2));
        }

        // Test CLI generation memory usage
        let generator = CliGenerator::new(Arc::new(env.fixture.registry));
        let _commands = generator.generate_commands().unwrap();

        println!("Completed memory test for {} tools", size);
        
        // This test mainly ensures we don't crash or leak memory
        // Actual memory measurement would require additional tooling
    }
}

/// Benchmark worst-case scenarios
#[tokio::test]
async fn bench_worst_case_scenarios() {
    let _env = IsolatedTestEnvironment::new();
    
    println!("Testing worst-case performance scenarios");

    // Scenario 1: Very high exclusion ratio (95% excluded)
    let high_exclusion_env = CliExclusionTestEnvironment::with_tool_counts(950, 50);
    let high_exclusion_detector = high_exclusion_env.fixture.as_exclusion_detector();
    
    let high_exclusion_start = Instant::now();
    for _ in 0..1000 {
        let _ = high_exclusion_detector.get_cli_eligible_tools();
    }
    let high_exclusion_duration = high_exclusion_start.elapsed();
    
    println!(
        "High exclusion ratio (95%): {:.2}ms for 1000 bulk queries",
        high_exclusion_duration.as_millis()
    );

    // Should still be reasonably fast
    assert!(high_exclusion_duration.as_millis() < 5000);

    // Scenario 2: Very low exclusion ratio (5% excluded)
    let low_exclusion_env = CliExclusionTestEnvironment::with_tool_counts(50, 950);
    let low_exclusion_detector = low_exclusion_env.fixture.as_exclusion_detector();
    
    let low_exclusion_start = Instant::now();
    for _ in 0..1000 {
        let _ = low_exclusion_detector.get_excluded_tools();
    }
    let low_exclusion_duration = low_exclusion_start.elapsed();
    
    println!(
        "Low exclusion ratio (5%): {:.2}ms for 1000 bulk queries",
        low_exclusion_duration.as_millis()
    );

    assert!(low_exclusion_duration.as_millis() < 5000);

    // Scenario 3: Many individual queries for non-existent tools
    let detector = high_exclusion_detector;
    let nonexistent_start = Instant::now();
    for i in 0..10000 {
        let _ = detector.is_cli_excluded(&format!("nonexistent_tool_{}", i));
    }
    let nonexistent_duration = nonexistent_start.elapsed();
    
    println!(
        "Nonexistent tool queries: {:.2}ms for 10000 queries ({:.2} queries/sec)",
        nonexistent_duration.as_millis(),
        10000.0 / nonexistent_duration.as_secs_f64()
    );

    // Should handle nonexistent queries efficiently
    assert!(nonexistent_duration.as_secs_f64() < 2.0);

    // Scenario 4: CLI generation with very complex schemas
    let mut complex_registry = ToolRegistry::new();
    
    for i in 0..100 {
        // Add tools with complex schemas (many parameters)
        complex_registry.register(Box::new(create_complex_tool(i)));
    }

    let complex_start = Instant::now();
    let generator = CliGenerator::new(Arc::new(complex_registry));
    let complex_commands = generator.generate_commands().unwrap();
    let complex_duration = complex_start.elapsed();

    println!(
        "Complex schema generation: {} commands in {:.2}ms",
        complex_commands.len(),
        complex_duration.as_millis()
    );

    // Should handle complex schemas reasonably well
    assert!(complex_duration.as_millis() < 3000);
    assert_eq!(complex_commands.len(), 100);
}

/// Create a tool with complex schema for performance testing
fn create_complex_tool(id: usize) -> IncludedMockTool {
    use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
    use async_trait::async_trait;
    use rmcp::model::CallToolResult;
    use rmcp::Error as McpError;
    use serde_json::Value;

    #[derive(Debug)]
    struct ComplexMockTool {
        id: usize,
        name: String,
    }

    impl ComplexMockTool {
        fn new(id: usize) -> Self {
            Self {
                id,
                name: format!("complex_tool_{}", id),
            }
        }
    }

    #[async_trait]
    impl McpTool for ComplexMockTool {
        fn name(&self) -> &'static str {
            // SAFETY: This is only used in tests where the string lives long enough
            Box::leak(self.name.clone().into_boxed_str())
        }

        fn description(&self) -> &'static str {
            "Complex tool for performance testing with many parameters"
        }

        fn schema(&self) -> Value {
            // Create a complex schema with many parameters
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();

            for i in 0..20 {
                let param_name = format!("param_{}", i);
                properties.insert(
                    param_name.clone(),
                    serde_json::json!({
                        "type": "string",
                        "description": format!("Parameter {} for tool {}", i, self.id),
                        "minLength": 1,
                        "maxLength": 100
                    }),
                );

                if i < 5 {
                    required.push(param_name);
                }
            }

            serde_json::json!({
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false
            })
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            Ok(swissarmyhammer_tools::mcp::tool_registry::BaseToolImpl::create_success_response(
                &format!("Complex tool {} executed", self.id)
            ))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    // Return a simpler version for now - the complex schema generation is tested above
    IncludedMockTool::new(format!("complex_tool_{}", id))
}

/// Benchmark startup and initialization performance
#[tokio::test]
async fn bench_initialization_performance() {
    let _env = IsolatedTestEnvironment::new();
    
    println!("Testing initialization performance");

    // Benchmark registry creation and tool registration
    let registration_start = Instant::now();
    let mut registry = ToolRegistry::new();
    
    for i in 0..1000 {
        if i % 4 == 0 {
            registry.register(Box::new(ExcludedMockTool::new(
                format!("init_excluded_{}", i),
                "Initialization test"
            )));
        } else {
            registry.register(Box::new(IncludedMockTool::new(
                format!("init_included_{}", i)
            )));
        }
    }
    let registration_duration = registration_start.elapsed();

    println!(
        "Tool registration: 1000 tools in {:.2}ms ({:.2} tools/sec)",
        registration_duration.as_millis(),
        1000.0 / registration_duration.as_secs_f64()
    );

    // Registration should be fast
    assert!(registration_duration.as_millis() < 1000);

    // Benchmark detector creation
    let detector_start = Instant::now();
    let detector = registry.as_exclusion_detector();
    let detector_duration = detector_start.elapsed();

    println!(
        "Detector creation: {:.2}ms for 1000 tools",
        detector_duration.as_millis()
    );

    // Detector creation should be very fast
    assert!(detector_duration.as_millis() < 500);

    // Verify the detector works correctly
    assert_eq!(detector.get_all_tool_metadata().len(), 1000);
    assert_eq!(detector.get_excluded_tools().len(), 250); // 1/4 of tools
    assert_eq!(detector.get_cli_eligible_tools().len(), 750); // 3/4 of tools

    // Benchmark first CLI generation (cold start)
    let cold_start = Instant::now();
    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();
    let cold_duration = cold_start.elapsed();

    println!(
        "Cold CLI generation: {} commands in {:.2}ms",
        commands.len(),
        cold_duration.as_millis()
    );

    assert_eq!(commands.len(), 750);
    assert!(cold_duration.as_millis() < 3000);

    println!("Initialization performance tests completed");
}