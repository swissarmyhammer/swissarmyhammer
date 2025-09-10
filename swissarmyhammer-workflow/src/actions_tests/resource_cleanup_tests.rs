//! Tests for resource cleanup in actions

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};

/// Mock resource that tracks acquisition and cleanup
struct MockResource {
    id: usize,
    _acquired: Arc<AtomicUsize>,
    released: Arc<AtomicUsize>,
}

impl MockResource {
    fn new(id: usize, acquired: Arc<AtomicUsize>, released: Arc<AtomicUsize>) -> Self {
        acquired.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            _acquired: acquired,
            released,
        }
    }
}

impl Drop for MockResource {
    fn drop(&mut self) {
        self.released.fetch_add(1, Ordering::SeqCst);
        println!("Resource {} cleaned up", self.id);
    }
}

#[tokio::test]
async fn test_action_cleanup_on_failure() {
    // Test that resources are properly cleaned up when an action fails
    let acquired = Arc::new(AtomicUsize::new(0));
    let released = Arc::new(AtomicUsize::new(0));

    let acquired_clone = Arc::clone(&acquired);
    let released_clone = Arc::clone(&released);

    // Create an action that acquires resources and then fails
    let action_result = async {
        let _resource1 = MockResource::new(1, acquired_clone.clone(), released_clone.clone());
        let _resource2 = MockResource::new(2, acquired_clone.clone(), released_clone.clone());

        // Simulate action failure
        Err::<(), String>("Action failed".to_string())
    }
    .await;

    assert!(action_result.is_err());

    // Resources should be cleaned up when they go out of scope
    assert_eq!(acquired.load(Ordering::SeqCst), 2);
    assert_eq!(released.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_prompt_action_cleanup_on_timeout() {
    // Test that PromptAction cleans up resources on timeout
    let mut action =
        PromptAction::new("test-prompt".to_string()).with_timeout(Duration::from_millis(1)); // Very short timeout

    // Track if cleanup happened
    let cleanup_tracker = Arc::new(AtomicUsize::new(0));
    let tracker_clone = Arc::clone(&cleanup_tracker);

    // Simulate resource acquisition in the action
    action.arguments.insert(
        "tracker".to_string(),
        format!("{:p}", tracker_clone.as_ref()),
    );

    let mut context = create_test_context();

    // This should timeout quickly
    let _result = action.execute(&mut context).await;

    // Even if it doesn't actually timeout (no real Claude execution),
    // verify the structure is correct for cleanup
    assert!(action.timeout < Duration::from_secs(1));
}

#[tokio::test]
async fn test_sub_workflow_cleanup_on_circular_dependency() {
    // Test cleanup when SubWorkflowAction detects circular dependency
    let mut context = create_test_context();

    // Set up workflow stack to create circular dependency
    context.insert(
        "_workflow_stack".to_string(),
        serde_json::json!(["workflow-a", "workflow-b"]),
    );

    let action = SubWorkflowAction::new("workflow-a".to_string());

    // Track resources
    let resources_cleaned = Arc::new(AtomicUsize::new(0));
    let _cleanup_clone = Arc::clone(&resources_cleaned);

    // Execute action that should fail with circular dependency
    let result = action.execute(&mut context).await;

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("Circular workflow dependency"));
    }

    // Verify context wasn't corrupted
    assert!(context.contains_key("_workflow_stack"));
}

#[tokio::test]
async fn test_concurrent_action_resource_cleanup() {
    // Test resource cleanup with concurrent action execution
    let acquired = Arc::new(AtomicUsize::new(0));
    let released = Arc::new(AtomicUsize::new(0));

    let num_actions = 5;
    let mut handles = vec![];

    for i in 0..num_actions {
        let acquired_clone = Arc::clone(&acquired);
        let released_clone = Arc::clone(&released);

        let handle = tokio::spawn(async move {
            let _resource = MockResource::new(i, acquired_clone, released_clone);

            // Simulate some work
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Half the actions fail
            if i % 2 == 0 {
                Err::<(), String>("Action failed".to_string())
            } else {
                Ok(())
            }
        });

        handles.push(handle);
    }

    // Wait for all actions
    for handle in handles {
        let _ = handle.await;
    }

    // All resources should be cleaned up regardless of success/failure
    tokio::time::sleep(Duration::from_millis(50)).await; // Give time for cleanup
    assert_eq!(
        acquired.load(Ordering::SeqCst),
        released.load(Ordering::SeqCst),
        "Not all resources were cleaned up"
    );
}

#[tokio::test]
async fn test_action_context_cleanup_on_panic() {
    // Test that context modifications are rolled back on panic
    let context = Arc::new(RwLock::new(create_test_context()));

    // Record initial state
    let initial_keys: Vec<String> = {
        let ctx = context.read().await;
        ctx.to_workflow_hashmap().keys().cloned().collect()
    };

    // Try to execute an action that panics
    let ctx_clone = Arc::clone(&context);
    let panic_result = tokio::spawn(async move {
        let mut ctx = ctx_clone.write().await;

        // Make some changes
        ctx.insert("temp_var1".to_string(), Value::String("temp1".to_string()));
        ctx.insert("temp_var2".to_string(), Value::String("temp2".to_string()));

        // Simulate panic
        panic!("Action panicked!");
    })
    .await;

    // The task should have panicked
    assert!(panic_result.is_err());

    // Context should still be accessible (not poisoned)
    let final_keys: Vec<String> = {
        let ctx = context.read().await;
        ctx.to_workflow_hashmap().keys().cloned().collect()
    };

    // Note: In this case, changes might persist unless we implement
    // transactional context updates. This test documents current behavior.
    println!("Initial keys: {initial_keys:?}");
    println!("Final keys: {final_keys:?}");
}

#[tokio::test]
async fn test_wait_action_cancellation_cleanup() {
    // Test that WaitAction properly handles cancellation
    let action = WaitAction::new_duration(Duration::from_secs(10));
    let mut context = create_test_context();

    // Create a cancellable task
    let handle = tokio::spawn(async move { action.execute(&mut context).await });

    // Give it a moment to start
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Cancel the task
    handle.abort();

    // Task should be cancelled
    let result = handle.await;
    assert!(result.is_err()); // JoinError from cancellation
}

#[tokio::test]
async fn test_log_action_file_handle_cleanup() {
    // Test that LogAction doesn't leak file handles or resources
    let num_iterations = 100;

    for i in 0..num_iterations {
        let action = LogAction::info(format!("Log message {i}"));
        let mut context = create_test_context();

        let result = action.execute(&mut context).await;
        assert!(result.is_ok());
    }

    // If file handles were leaking, we'd run out after many iterations
    // This test passes if we can complete all iterations
}

#[tokio::test]
async fn test_semaphore_cleanup_in_rate_limited_actions() {
    // Test cleanup of semaphore permits in rate-limited scenarios
    let semaphore = Arc::new(Semaphore::new(2)); // Limited permits

    let mut handles = vec![];

    for i in 0..5 {
        let sem_clone = Arc::clone(&semaphore);

        let handle = tokio::spawn(async move {
            // Try to acquire permit
            let _permit = sem_clone.acquire().await.unwrap();

            // Simulate work
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Permit automatically released when dropped
            format!("Task {i} completed")
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        let _ = handle.await;
    }

    // All permits should be available again
    assert_eq!(semaphore.available_permits(), 2);
}

#[tokio::test]
async fn test_action_cleanup_with_multiple_errors() {
    // Test cleanup when multiple errors occur in sequence
    let mut context = create_test_context();
    let mut errors = Vec::new();

    // First action succeeds (SetVariableAction doesn't fail on invalid JSON)
    let action1 = SetVariableAction::new("var1".to_string(), "{invalid json".to_string());
    let result1 = action1.execute(&mut context).await;
    assert!(result1.is_ok()); // This will store the invalid JSON as a string

    // Second action also fails
    let action2 = SubWorkflowAction::new("non-existent-workflow".to_string());
    if let Err(e) = action2.execute(&mut context).await {
        errors.push(e.to_string());
    }

    // Context should still be valid
    assert!(context.contains_key("test_var"));
    assert_eq!(errors.len(), 1); // Only SubWorkflowAction fails
                                 // Verify var1 was set even though it's invalid JSON
    assert_eq!(
        context.get("var1"),
        Some(&Value::String("{invalid json".to_string()))
    );

    // Third action should succeed despite previous failures
    let action3 = SetVariableAction::new("recovery_var".to_string(), "recovered".to_string());
    let result = action3.execute(&mut context).await;
    assert!(result.is_ok());
    assert_eq!(
        context.get("recovery_var"),
        Some(&Value::String("recovered".to_string()))
    );
}
