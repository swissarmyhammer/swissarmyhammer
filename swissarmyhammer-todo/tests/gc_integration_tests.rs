//! Integration tests for todo garbage collection workflow
//!
//! This test suite verifies that the garbage collection mechanism works correctly
//! through the complete workflow, simulating time passage and testing that:
//! - Completed todos are automatically removed when creating new items
//! - The file is deleted when all todos are complete
//! - Timestamps are preserved correctly through the GC process

use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_todo::{TodoId, TodoItem, TodoItemExt, TodoList, TodoStorage};

/// Test fixture that encapsulates TodoStorage and temporary directory
struct TestFixture {
    storage: TodoStorage,
    _env: IsolatedTestEnvironment,
}

impl TestFixture {
    /// Create a new test fixture with TodoStorage and temporary directory
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let temp_dir = env.temp_dir();
        fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
        let storage = TodoStorage::new(temp_dir);
        Self { storage, _env: env }
    }

    /// Get the path to the todo file
    fn todo_file_path(&self) -> PathBuf {
        self._env.temp_dir().join("todo.yaml")
    }

    /// Create a todo item with simplified error handling
    async fn create_item(&self, task: &str) -> TodoItem {
        self.storage
            .create_todo_item(task.to_string(), None)
            .await
            .expect("Failed to create todo item")
            .0
    }

    /// Get the todo list, expecting it to exist
    async fn get_list(&self) -> TodoList {
        self.storage
            .get_todo_list()
            .await
            .expect("Failed to get list")
            .expect("List should exist")
    }

    /// Get the todo list as an option
    async fn get_list_opt(&self) -> Option<TodoList> {
        self.storage
            .get_todo_list()
            .await
            .expect("Failed to get list")
    }

    /// Mark a todo item as complete
    async fn complete_item(&self, id: &str) {
        let todo_id = TodoId::from_string(id.to_string()).expect("Invalid ID");
        self.storage
            .mark_todo_complete(&todo_id)
            .await
            .expect("Failed to complete todo item")
    }
}

/// Assert that a timestamp is within a given range
fn assert_timestamp_in_range(
    timestamp: chrono::DateTime<chrono::Utc>,
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
    context: &str,
) {
    assert!(
        timestamp >= start && timestamp <= end,
        "{} should be in time range",
        context
    );
}

/// Test that GC removes completed todos when creating new items
#[tokio::test]
async fn test_gc_on_create_removes_completed() {
    let fixture = TestFixture::new();

    // Create three initial todos
    let item1 = fixture.create_item("Task 1").await;
    let item2 = fixture.create_item("Task 2").await;
    fixture.create_item("Task 3").await;

    // Verify we have 3 items
    let list = fixture.get_list().await;
    assert_eq!(list.todo.len(), 3, "Should have 3 todos");

    // Mark first two as complete
    fixture.complete_item(&item1.id).await;
    fixture.complete_item(&item2.id).await;

    // Verify we still have 3 items (2 complete, 1 incomplete)
    let list_before_gc = fixture.get_list().await;
    assert_eq!(
        list_before_gc.todo.len(),
        3,
        "Should still have 3 todos before GC"
    );
    assert_eq!(
        list_before_gc.complete_count(),
        2,
        "Should have 2 completed"
    );
    assert_eq!(
        list_before_gc.incomplete_count(),
        1,
        "Should have 1 incomplete"
    );

    // Create a new item - this should trigger GC
    fixture.create_item("Task 4").await;

    // Verify completed items were garbage collected
    let list_after_gc = fixture.get_list().await;
    assert_eq!(
        list_after_gc.todo.len(),
        2,
        "Should have 2 todos after GC (Task 3 and Task 4)"
    );
    assert_eq!(
        list_after_gc.complete_count(),
        0,
        "Should have 0 completed after GC"
    );
    assert_eq!(
        list_after_gc.incomplete_count(),
        2,
        "Should have 2 incomplete after GC"
    );

    // Verify the correct tasks remain
    let tasks: Vec<String> = list_after_gc
        .todo
        .iter()
        .map(|item| item.task().to_string())
        .collect();
    assert!(
        tasks.contains(&"Task 3".to_string()),
        "Task 3 should remain"
    );
    assert!(
        tasks.contains(&"Task 4".to_string()),
        "Task 4 should remain"
    );
    assert!(
        !tasks.contains(&"Task 1".to_string()),
        "Task 1 should be GC'd"
    );
    assert!(
        !tasks.contains(&"Task 2".to_string()),
        "Task 2 should be GC'd"
    );

    // Verify file exists
    let todo_file = fixture.todo_file_path();
    assert!(todo_file.exists(), "Todo file should exist");
}

/// Test that file is deleted when all todos are completed
#[tokio::test]
async fn test_file_deleted_when_all_complete() {
    let fixture = TestFixture::new();

    // Create a single todo
    let item = fixture.create_item("Single task").await;

    // Verify file exists
    let todo_file = fixture.todo_file_path();
    assert!(todo_file.exists(), "Todo file should exist after creation");

    // Complete the only todo
    fixture.complete_item(&item.id).await;

    // Verify file is deleted
    assert!(
        !todo_file.exists(),
        "Todo file should be deleted when all todos are complete"
    );

    // Verify list is empty/none
    let list = fixture.get_list_opt().await;
    assert!(list.is_none(), "List should be None after file deletion");
}

/// Test GC with time passage simulation
#[tokio::test]
async fn test_gc_with_time_passage_simulation() {
    let fixture = TestFixture::new();

    // Phase 1: Create initial todos at T0
    let t0_start = Utc::now();
    let item1 = fixture.create_item("Task at T0-1").await;
    let item2 = fixture.create_item("Task at T0-2").await;
    let t0_end = Utc::now();

    // Verify timestamps are within T0 range
    assert_timestamp_in_range(
        item1.created_at.expect("created_at should be set"),
        t0_start,
        t0_end,
        "Item 1 created_at",
    );
    assert_timestamp_in_range(
        item2.created_at.expect("created_at should be set"),
        t0_start,
        t0_end,
        "Item 2 created_at",
    );

    // Phase 2: Wait and create more todos at T1
    tokio::time::sleep(Duration::from_millis(100)).await;

    let t1_start = Utc::now();
    let item3 = fixture.create_item("Task at T1").await;
    let t1_end = Utc::now();

    // Verify T1 timestamp is after T0
    assert!(
        item3.created_at.expect("created_at should be set")
            > item1.created_at.expect("created_at should be set"),
        "Item 3 should be created after item 1"
    );
    assert_timestamp_in_range(
        item3.created_at.expect("created_at should be set"),
        t1_start,
        t1_end,
        "Item 3 created_at",
    );

    // Phase 3: Complete some items at T2
    tokio::time::sleep(Duration::from_millis(100)).await;

    let t2_start = Utc::now();
    fixture.complete_item(&item1.id).await;
    fixture.complete_item(&item2.id).await;
    let t2_end = Utc::now();

    // Verify list has correct state before GC
    let list_before_gc = fixture.get_list().await;
    assert_eq!(list_before_gc.todo.len(), 3, "Should have 3 items");
    assert_eq!(list_before_gc.complete_count(), 2, "Should have 2 complete");

    // Verify completed items have updated_at in T2 range
    let completed_items: Vec<&TodoItem> = list_before_gc
        .todo
        .iter()
        .filter(|item| item.done())
        .collect();
    for item in completed_items {
        assert_timestamp_in_range(
            item.updated_at.expect("updated_at should be set"),
            t2_start,
            t2_end,
            "Completed item updated_at",
        );
    }

    // Phase 4: Create new item at T3, triggering GC
    tokio::time::sleep(Duration::from_millis(100)).await;

    let t3_start = Utc::now();
    let item4 = fixture.create_item("Task at T3").await;
    let t3_end = Utc::now();

    // Verify T3 timestamp
    assert_timestamp_in_range(
        item4.created_at.expect("created_at should be set"),
        t3_start,
        t3_end,
        "Item 4 created_at",
    );

    // Verify GC removed completed items
    let list_after_gc = fixture.get_list().await;
    assert_eq!(list_after_gc.todo.len(), 2, "Should have 2 items after GC");
    assert_eq!(
        list_after_gc.complete_count(),
        0,
        "Should have 0 complete after GC"
    );

    // Verify timestamps were preserved for remaining items
    let remaining_item3 = list_after_gc
        .todo
        .iter()
        .find(|item| item.id == item3.id)
        .expect("Item 3 should remain");
    assert_eq!(
        remaining_item3.created_at, item3.created_at,
        "Item 3 created_at should be preserved"
    );
    assert_eq!(
        remaining_item3.updated_at, item3.updated_at,
        "Item 3 updated_at should be preserved"
    );

    let remaining_item4 = list_after_gc
        .todo
        .iter()
        .find(|item| item.id == item4.id)
        .expect("Item 4 should remain");
    assert_eq!(
        remaining_item4.created_at, item4.created_at,
        "Item 4 created_at should be preserved"
    );

    // Verify the file exists
    let todo_file = fixture.todo_file_path();
    assert!(todo_file.exists(), "Todo file should exist");
}

/// Test GC preserves timestamps across multiple GC cycles
#[tokio::test]
async fn test_gc_preserves_timestamps_across_multiple_cycles() {
    let fixture = TestFixture::new();

    // Create initial item
    let initial_item = fixture.create_item("Initial task").await;

    let original_created_at = initial_item.created_at;
    let original_updated_at = initial_item.updated_at;

    // Go through multiple GC cycles
    for i in 1..=5 {
        // Wait a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create a new item and complete it
        let temp_item = fixture.create_item(&format!("Temp task {i}")).await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        fixture.complete_item(&temp_item.id).await;

        // Create another item to trigger GC
        tokio::time::sleep(Duration::from_millis(50)).await;

        fixture.create_item(&format!("Trigger GC {i}")).await;

        // Verify initial item timestamps remain unchanged
        let list = fixture.get_list().await;

        let preserved_item = list
            .todo
            .iter()
            .find(|item| item.id == initial_item.id)
            .expect("Initial item should still exist");

        assert_eq!(
            preserved_item.created_at, original_created_at,
            "Created timestamp should remain unchanged after GC cycle {i}"
        );
        assert_eq!(
            preserved_item.updated_at, original_updated_at,
            "Updated timestamp should remain unchanged after GC cycle {i}"
        );
    }
}

/// Test that GC works correctly when all but one item is completed
#[tokio::test]
async fn test_gc_with_single_incomplete_item() {
    let fixture = TestFixture::new();

    // Create multiple items
    let item1 = fixture.create_item("Keep this").await;
    let item2 = fixture.create_item("Complete this 1").await;
    let item3 = fixture.create_item("Complete this 2").await;
    let item4 = fixture.create_item("Complete this 3").await;

    // Complete all but the first
    fixture.complete_item(&item2.id).await;
    fixture.complete_item(&item3.id).await;
    fixture.complete_item(&item4.id).await;

    // Verify state before GC
    let list_before = fixture.get_list().await;
    assert_eq!(list_before.todo.len(), 4, "Should have 4 items");
    assert_eq!(list_before.complete_count(), 3, "Should have 3 complete");
    assert_eq!(
        list_before.incomplete_count(),
        1,
        "Should have 1 incomplete"
    );

    // Create new item to trigger GC
    fixture.create_item("New item").await;

    // Verify GC removed completed items
    let list_after = fixture.get_list().await;
    assert_eq!(
        list_after.todo.len(),
        2,
        "Should have 2 items after GC (the incomplete one and the new one)"
    );
    assert_eq!(
        list_after.complete_count(),
        0,
        "Should have 0 complete after GC"
    );
    assert_eq!(
        list_after.incomplete_count(),
        2,
        "Should have 2 incomplete after GC"
    );

    // Verify the correct item remained
    let ids: Vec<String> = list_after.todo.iter().map(|item| item.id.clone()).collect();
    assert!(ids.contains(&item1.id), "First item should remain");

    // Verify file exists
    let todo_file = fixture.todo_file_path();
    assert!(todo_file.exists(), "Todo file should exist");
}

/// Test rapid successive GC operations
#[tokio::test]
async fn test_rapid_successive_gc_operations() {
    let fixture = TestFixture::new();

    // Create base item
    let base_item = fixture.create_item("Base item").await;

    // Rapidly create and complete items, triggering multiple GC cycles
    for i in 1..=10 {
        let temp_item = fixture.create_item(&format!("Temp {i}")).await;

        fixture.complete_item(&temp_item.id).await;

        // This will trigger GC
        fixture.create_item(&format!("Next {i}")).await;
    }

    // Verify base item still exists
    let final_list = fixture.get_list().await;

    let base_still_exists = final_list.todo.iter().any(|item| item.id == base_item.id);
    assert!(base_still_exists, "Base item should survive all GC cycles");

    // Verify no completed items remain
    assert_eq!(
        final_list.complete_count(),
        0,
        "Should have 0 completed items after all GC cycles"
    );
}

/// Test that GC handles empty list after all items completed
#[tokio::test]
async fn test_gc_after_completing_all_items() {
    let fixture = TestFixture::new();

    // Create multiple items
    let item1 = fixture.create_item("Task 1").await;
    let item2 = fixture.create_item("Task 2").await;

    // Complete all items
    fixture.complete_item(&item1.id).await;
    fixture.complete_item(&item2.id).await;

    // Verify file is deleted
    let todo_file = fixture.todo_file_path();
    assert!(
        !todo_file.exists(),
        "Todo file should be deleted after completing all items"
    );

    // Verify list is None
    let list = fixture.get_list_opt().await;
    assert!(
        list.is_none(),
        "List should be None after all items complete"
    );

    // Create a new item - this should recreate the file
    let new_item = fixture.create_item("Fresh start").await;

    // Verify file exists again
    assert!(
        todo_file.exists(),
        "Todo file should exist after creating new item"
    );

    // Verify list contains only the new item
    let new_list = fixture.get_list().await;
    assert_eq!(new_list.todo.len(), 1, "Should have 1 item");
    assert_eq!(new_list.todo[0].id, new_item.id, "Should have the new item");
}
