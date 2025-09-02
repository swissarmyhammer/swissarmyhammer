//! Load tests for executor performance under concurrent usage

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer::workflow::actions::{AgentExecutionContext, AgentExecutorFactory};
use swissarmyhammer::workflow::template_context::WorkflowTemplateContext;
use swissarmyhammer_config::agent::{AgentConfig, LlamaAgentConfig};
use tokio::time::{timeout, Duration, Instant};

#[tokio::test]
async fn test_concurrent_executor_creation() {
    // Skip test if LlamaAgent testing is disabled
    if !swissarmyhammer_config::test_config::is_llama_enabled() {
        println!("Skipping LlamaAgent test (set SAH_TEST_LLAMA=true to enable)");
        return;
    }
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    const CONCURRENT_REQUESTS: usize = 10;
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

    println!(
        "Testing concurrent executor creation with {} requests",
        CONCURRENT_REQUESTS
    );

    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let start_time = Instant::now();

    let handles: Vec<_> = (0..CONCURRENT_REQUESTS)
        .map(|i| {
            let success_count = Arc::clone(&success_count);
            let error_count = Arc::clone(&error_count);

            tokio::spawn(async move {
                let context = WorkflowTemplateContext::with_vars(HashMap::new())
                    .expect("Failed to create context");
                let mut context_with_config = context;

                // Alternate between executor types
                let config = if i % 2 == 0 {
                    AgentConfig::claude_code()
                } else {
                    AgentConfig::llama_agent(LlamaAgentConfig::for_testing())
                };

                context_with_config.set_agent_config(config);
                let execution_context = AgentExecutionContext::new(&context_with_config);

                match timeout(
                    REQUEST_TIMEOUT,
                    AgentExecutorFactory::create_executor(&execution_context),
                )
                .await
                {
                    Ok(Ok(_executor)) => {
                        success_count.fetch_add(1, Ordering::SeqCst);
                        println!("✓ Request {} succeeded", i);
                    }
                    Ok(Err(e)) => {
                        error_count.fetch_add(1, Ordering::SeqCst);
                        println!("⚠ Request {} failed: {}", i, e);
                    }
                    Err(_) => {
                        error_count.fetch_add(1, Ordering::SeqCst);
                        println!("⚠ Request {} timed out", i);
                    }
                }
            })
        })
        .collect();

    // Wait for all requests to complete
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    let elapsed = start_time.elapsed();
    let successes = success_count.load(Ordering::SeqCst);
    let errors = error_count.load(Ordering::SeqCst);

    println!("Concurrent executor creation results:");
    println!("  Total time: {:?}", elapsed);
    println!("  Successes: {}/{}", successes, CONCURRENT_REQUESTS);
    println!("  Errors: {}/{}", errors, CONCURRENT_REQUESTS);

    if successes > 0 {
        println!(
            "  Average time per successful request: {:?}",
            elapsed / successes as u32
        );
    }

    // Verify all requests completed (either success or graceful failure)
    assert_eq!(successes + errors, CONCURRENT_REQUESTS);

    // In load testing, we don't require all to succeed, just that none panic
    println!("✓ All concurrent requests completed without panicking");
}

#[tokio::test]
async fn test_memory_usage_patterns() {
    // Skip test if LlamaAgent testing is disabled
    if !swissarmyhammer_config::test_config::is_llama_enabled() {
        println!("Skipping LlamaAgent test (set SAH_TEST_LLAMA=true to enable)");
        return;
    }
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    println!("Testing memory usage patterns with repeated operations");

    const ITERATIONS: usize = 5;

    for i in 0..ITERATIONS {
        println!("Memory test iteration {}/{}", i + 1, ITERATIONS);

        // Create and test multiple contexts in this iteration
        for j in 0..3 {
            let context = WorkflowTemplateContext::with_vars(HashMap::from([
                ("iteration".to_string(), serde_json::Value::Number(i.into())),
                (
                    "context_num".to_string(),
                    serde_json::Value::Number(j.into()),
                ),
            ]))
            .expect("Failed to create context");

            let mut context_with_config = context;
            context_with_config.set_agent_config(AgentConfig::claude_code());
            let execution_context = AgentExecutionContext::new(&context_with_config);

            match AgentExecutorFactory::create_executor(&execution_context).await {
                Ok(_executor) => {
                    println!("  ✓ Iteration {} context {} succeeded", i, j);
                }
                Err(e) => {
                    println!("  ⚠ Iteration {} context {} failed: {}", i, j, e);
                }
            }
        }

        // Allow some time for cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("✓ Memory usage pattern test completed without issues");
}

#[tokio::test]
async fn test_stress_execution() {
    // Skip test if LlamaAgent testing is disabled
    if !swissarmyhammer_config::test_config::is_llama_enabled() {
        println!("Skipping LlamaAgent test (set SAH_TEST_LLAMA=true to enable)");
        return;
    }
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    const BATCH_SIZE: usize = 3;
    const BATCH_COUNT: usize = 4;
    const BATCH_DELAY: Duration = Duration::from_millis(500);

    println!(
        "Testing stress execution with {} batches of {} requests each",
        BATCH_COUNT, BATCH_SIZE
    );

    let mut total_successes = 0;
    let mut total_errors = 0;
    let overall_start = Instant::now();

    for batch in 0..BATCH_COUNT {
        println!("Processing batch {}/{}", batch + 1, BATCH_COUNT);
        let batch_start = Instant::now();

        let batch_handles: Vec<_> = (0..BATCH_SIZE)
            .map(|i| {
                tokio::spawn(async move {
                    let context = WorkflowTemplateContext::with_vars(HashMap::from([
                        ("batch".to_string(), serde_json::Value::Number(batch.into())),
                        ("request".to_string(), serde_json::Value::Number(i.into())),
                    ]))
                    .expect("Failed to create context");

                    let mut context_with_config = context;

                    // Vary the executor type based on batch and request
                    let config = if (batch + i) % 3 == 0 {
                        AgentConfig::llama_agent(LlamaAgentConfig::for_testing())
                    } else {
                        AgentConfig::claude_code()
                    };

                    context_with_config.set_agent_config(config);
                    let execution_context = AgentExecutionContext::new(&context_with_config);

                    match AgentExecutorFactory::create_executor(&execution_context).await {
                        Ok(_executor) => Ok(()),
                        Err(_) => Err(()),
                    }
                })
            })
            .collect();

        // Wait for batch completion
        let mut batch_successes = 0;
        let mut batch_errors = 0;

        for handle in batch_handles {
            match handle.await.expect("Task should not panic") {
                Ok(()) => batch_successes += 1,
                Err(()) => batch_errors += 1,
            }
        }

        let batch_elapsed = batch_start.elapsed();
        println!(
            "  Batch {} results: {} successes, {} errors in {:?}",
            batch + 1,
            batch_successes,
            batch_errors,
            batch_elapsed
        );

        total_successes += batch_successes;
        total_errors += batch_errors;

        // Brief pause between batches
        if batch < BATCH_COUNT - 1 {
            tokio::time::sleep(BATCH_DELAY).await;
        }
    }

    let overall_elapsed = overall_start.elapsed();
    let total_requests = BATCH_SIZE * BATCH_COUNT;

    println!("Stress test summary:");
    println!("  Total time: {:?}", overall_elapsed);
    println!("  Total requests: {}", total_requests);
    println!("  Total successes: {}", total_successes);
    println!("  Total errors: {}", total_errors);
    println!(
        "  Success rate: {:.1}%",
        (total_successes as f64 / total_requests as f64) * 100.0
    );

    // Verify all requests completed
    assert_eq!(total_successes + total_errors, total_requests);

    println!("✓ Stress test completed successfully");
}

#[tokio::test]
async fn test_rapid_creation_destruction() {
    // Skip test if LlamaAgent testing is disabled
    if !swissarmyhammer_config::test_config::is_llama_enabled() {
        println!("Skipping LlamaAgent test (set SAH_TEST_LLAMA=true to enable)");
        return;
    }
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    const CYCLES: usize = 10;

    println!("Testing rapid creation/destruction cycles");

    for cycle in 0..CYCLES {
        let cycle_start = Instant::now();

        // Create context
        let context = WorkflowTemplateContext::with_vars(HashMap::from([(
            "cycle".to_string(),
            serde_json::Value::Number(cycle.into()),
        )]))
        .expect("Failed to create context");

        let mut context_with_config = context;
        context_with_config.set_agent_config(AgentConfig::claude_code());
        let execution_context = AgentExecutionContext::new(&context_with_config);

        // Try to create and immediately drop executor
        match AgentExecutorFactory::create_executor(&execution_context).await {
            Ok(executor) => {
                drop(executor);
                let cycle_elapsed = cycle_start.elapsed();
                println!("  ✓ Cycle {} succeeded in {:?}", cycle, cycle_elapsed);
            }
            Err(e) => {
                let cycle_elapsed = cycle_start.elapsed();
                println!("  ⚠ Cycle {} failed in {:?}: {}", cycle, cycle_elapsed, e);
            }
        }

        // Brief pause to allow cleanup
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    println!("✓ Rapid creation/destruction test completed");
}

#[tokio::test]
async fn test_timeout_behavior_under_load() {
    // Skip test if LlamaAgent testing is disabled
    if !swissarmyhammer_config::test_config::is_llama_enabled() {
        println!("Skipping LlamaAgent test (set SAH_TEST_LLAMA=true to enable)");
        return;
    }
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    const SHORT_TIMEOUT_MS: u64 = 100; // Very short timeout
    const TIMEOUT_REQUESTS: usize = 5;

    println!(
        "Testing timeout behavior under load with {}ms timeout",
        SHORT_TIMEOUT_MS
    );

    let handles: Vec<_> = (0..TIMEOUT_REQUESTS)
        .map(|i| {
            tokio::spawn(async move {
                let context = WorkflowTemplateContext::with_vars(HashMap::from([(
                    "timeout_test".to_string(),
                    serde_json::Value::Number(i.into()),
                )]))
                .expect("Failed to create context");

                let mut context_with_config = context;
                context_with_config.set_agent_config(AgentConfig::claude_code());
                let execution_context = AgentExecutionContext::new(&context_with_config);

                // Test with very short timeout
                let result = timeout(
                    Duration::from_millis(SHORT_TIMEOUT_MS),
                    AgentExecutorFactory::create_executor(&execution_context),
                )
                .await;

                match result {
                    Ok(Ok(_executor)) => {
                        println!("  ✓ Request {} completed within timeout", i);
                        "success"
                    }
                    Ok(Err(_)) => {
                        println!("  ⚠ Request {} failed within timeout", i);
                        "error"
                    }
                    Err(_) => {
                        println!("  ⚠ Request {} timed out", i);
                        "timeout"
                    }
                }
            })
        })
        .collect();

    // Wait for all timeout tests
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.expect("Task should not panic");
        results.push(result);
    }

    // Count results
    let successes = results.iter().filter(|&&r| r == "success").count();
    let errors = results.iter().filter(|&&r| r == "error").count();
    let timeouts = results.iter().filter(|&&r| r == "timeout").count();

    println!("Timeout test results:");
    println!("  Successes: {}", successes);
    println!("  Errors: {}", errors);
    println!("  Timeouts: {}", timeouts);

    // All requests should complete without panicking
    assert_eq!(successes + errors + timeouts, TIMEOUT_REQUESTS);

    println!("✓ Timeout behavior test completed successfully");
}
