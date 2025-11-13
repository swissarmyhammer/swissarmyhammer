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
use swissarmyhammer_todo::{TodoItem, TodoStorage};
use tempfile::TempDir;

/// Helper to create a TodoStorage with a temporary directory
fn create_test_storage() -> (TodoStorage, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    fs::create_dir_all(temp_dir.path()).expect("Failed to create temp dir");
    let storage = TodoStorage::new(temp_dir.path().to_path_buf());
    (storage, temp_dir)
}

/// Helper to get the todo file path
fn get_todo_file_path(temp_dir: &TempDir) -> PathBuf {
    temp_dir.path().join("todo.yaml")
}

/// Test that GC removes completed todos when creating new items
#[tokio::test]
async fn test_gc_on_create_removes_completed() {
    let (storage, temp_dir) = create_test_storage();

    // Create three initial todos
    let (item1, _gc_count) = storage
        .create_todo_item("Task 1".to_string(), None)
        .await
        .expect("Failed to create item 1");

    let (item2, _gc_count) = storage
        .create_todo_item("Task 2".to_string(), None)
        .await
        .expect("Failed to create item 2");

    storage
        .create_todo_item("Task 3".to_string(), None)
        .await
        .expect("Failed to create item 3");

    // Verify we have 3 items
    let list = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");
    assert_eq!(list.todo.len(), 3, "Should have 3 todos");

    // Mark first two as complete
    storage
        .mark_todo_complete(&item1.id)
        .await
        .expect("Failed to complete item 1");
    storage
        .mark_todo_complete(&item2.id)
        .await
        .expect("Failed to complete item 2");

    // Verify we still have 3 items (2 complete, 1 incomplete)
    let list_before_gc = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");
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
    storage
        .create_todo_item("Task 4".to_string(), None)
        .await
        .expect("Failed to create item 4");

    // Verify completed items were garbage collected
    let list_after_gc = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");
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
        .map(|item| item.task.clone())
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
    let todo_file = get_todo_file_path(&temp_dir);
    assert!(todo_file.exists(), "Todo file should exist");
}

/// Test that file is deleted when all todos are completed
#[tokio::test]
async fn test_file_deleted_when_all_complete() {
    let (storage, temp_dir) = create_test_storage();

    // Create a single todo
    let (item, _gc_count) = storage
        .create_todo_item("Single task".to_string(), None)
        .await
        .expect("Failed to create item");

    // Verify file exists
    let todo_file = get_todo_file_path(&temp_dir);
    assert!(todo_file.exists(), "Todo file should exist after creation");

    // Complete the only todo
    storage
        .mark_todo_complete(&item.id)
        .await
        .expect("Failed to complete item");

    // Verify file is deleted
    assert!(
        !todo_file.exists(),
        "Todo file should be deleted when all todos are complete"
    );

    // Verify list is empty/none
    let list = storage.get_todo_list().await.expect("Failed to get list");
    assert!(list.is_none(), "List should be None after file deletion");
}

/// Test GC with time passage simulation
#[tokio::test]
async fn test_gc_with_time_passage_simulation() {
    let (storage, temp_dir) = create_test_storage();

    // Phase 1: Create initial todos at T0
    let t0_start = Utc::now();
    let (item1, _gc_count) = storage
        .create_todo_item("Task at T0-1".to_string(), None)
        .await
        .expect("Failed to create item 1");
    let (item2, _gc_count) = storage
        .create_todo_item("Task at T0-2".to_string(), None)
        .await
        .expect("Failed to create item 2");
    let t0_end = Utc::now();

    // Verify timestamps are within T0 range
    assert!(
        item1.created_at >= t0_start && item1.created_at <= t0_end,
        "Item 1 should be created in T0 range"
    );
    assert!(
        item2.created_at >= t0_start && item2.created_at <= t0_end,
        "Item 2 should be created in T0 range"
    );

    // Phase 2: Wait and create more todos at T1
    tokio::time::sleep(Duration::from_millis(100)).await;

    let t1_start = Utc::now();
    let (item3, _gc_count) = storage
        .create_todo_item("Task at T1".to_string(), None)
        .await
        .expect("Failed to create item 3");
    let t1_end = Utc::now();

    // Verify T1 timestamp is after T0
    assert!(
        item3.created_at > item1.created_at,
        "Item 3 should be created after item 1"
    );
    assert!(
        item3.created_at >= t1_start && item3.created_at <= t1_end,
        "Item 3 should be created in T1 range"
    );

    // Phase 3: Complete some items at T2
    tokio::time::sleep(Duration::from_millis(100)).await;

    let t2_start = Utc::now();
    storage
        .mark_todo_complete(&item1.id)
        .await
        .expect("Failed to complete item 1");
    storage
        .mark_todo_complete(&item2.id)
        .await
        .expect("Failed to complete item 2");
    let t2_end = Utc::now();

    // Verify list has correct state before GC
    let list_before_gc = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");
    assert_eq!(list_before_gc.todo.len(), 3, "Should have 3 items");
    assert_eq!(list_before_gc.complete_count(), 2, "Should have 2 complete");

    // Verify completed items have updated_at in T2 range
    let completed_items: Vec<&TodoItem> = list_before_gc
        .todo
        .iter()
        .filter(|item| item.done)
        .collect();
    for item in completed_items {
        assert!(
            item.updated_at >= t2_start && item.updated_at <= t2_end,
            "Completed item should have updated_at in T2 range"
        );
    }

    // Phase 4: Create new item at T3, triggering GC
    tokio::time::sleep(Duration::from_millis(100)).await;

    let t3_start = Utc::now();
    let (item4, _gc_count) = storage
        .create_todo_item("Task at T3".to_string(), None)
        .await
        .expect("Failed to create item 4");
    let t3_end = Utc::now();

    // Verify T3 timestamp
    assert!(
        item4.created_at >= t3_start && item4.created_at <= t3_end,
        "Item 4 should be created in T3 range"
    );

    // Verify GC removed completed items
    let list_after_gc = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");
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
    let todo_file = get_todo_file_path(&temp_dir);
    assert!(todo_file.exists(), "Todo file should exist");
}

/// Test GC preserves timestamps across multiple GC cycles
#[tokio::test]
async fn test_gc_preserves_timestamps_across_multiple_cycles() {
    let (storage, _temp_dir) = create_test_storage();

    // Create initial item
    let (initial_item, _gc_count) = storage
        .create_todo_item("Initial task".to_string(), None)
        .await
        .expect("Failed to create initial item");

    let original_created_at = initial_item.created_at;
    let original_updated_at = initial_item.updated_at;

    // Go through multiple GC cycles
    for i in 1..=5 {
        // Wait a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create a new item and complete it
        let (temp_item, _gc_count) = storage
            .create_todo_item(format!("Temp task {i}"), None)
            .await
            .expect("Failed to create temp item");

        tokio::time::sleep(Duration::from_millis(50)).await;

        storage
            .mark_todo_complete(&temp_item.id)
            .await
            .expect("Failed to complete temp item");

        // Create another item to trigger GC
        tokio::time::sleep(Duration::from_millis(50)).await;

        storage
            .create_todo_item(format!("Trigger GC {i}"), None)
            .await
            .expect("Failed to create trigger item");

        // Verify initial item timestamps remain unchanged
        let list = storage
            .get_todo_list()
            .await
            .expect("Failed to get list")
            .expect("List should exist");

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
    let (storage, temp_dir) = create_test_storage();

    // Create multiple items
    let (item1, _gc_count) = storage
        .create_todo_item("Keep this".to_string(), None)
        .await
        .expect("Failed to create item 1");

    let (item2, _gc_count) = storage
        .create_todo_item("Complete this 1".to_string(), None)
        .await
        .expect("Failed to create item 2");

    let (item3, _gc_count) = storage
        .create_todo_item("Complete this 2".to_string(), None)
        .await
        .expect("Failed to create item 3");

    let (item4, _gc_count) = storage
        .create_todo_item("Complete this 3".to_string(), None)
        .await
        .expect("Failed to create item 4");

    // Complete all but the first
    storage
        .mark_todo_complete(&item2.id)
        .await
        .expect("Failed to complete item 2");
    storage
        .mark_todo_complete(&item3.id)
        .await
        .expect("Failed to complete item 3");
    storage
        .mark_todo_complete(&item4.id)
        .await
        .expect("Failed to complete item 4");

    // Verify state before GC
    let list_before = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");
    assert_eq!(list_before.todo.len(), 4, "Should have 4 items");
    assert_eq!(list_before.complete_count(), 3, "Should have 3 complete");
    assert_eq!(
        list_before.incomplete_count(),
        1,
        "Should have 1 incomplete"
    );

    // Create new item to trigger GC
    storage
        .create_todo_item("New item".to_string(), None)
        .await
        .expect("Failed to create new item");

    // Verify GC removed completed items
    let list_after = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");
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
    let ids: Vec<String> = list_after
        .todo
        .iter()
        .map(|item| item.id.to_string())
        .collect();
    assert!(
        ids.contains(&item1.id.to_string()),
        "First item should remain"
    );

    // Verify file exists
    let todo_file = get_todo_file_path(&temp_dir);
    assert!(todo_file.exists(), "Todo file should exist");
}

/// Test rapid successive GC operations
#[tokio::test]
async fn test_rapid_successive_gc_operations() {
    let (storage, _temp_dir) = create_test_storage();

    // Create base item
    let (base_item, _gc_count) = storage
        .create_todo_item("Base item".to_string(), None)
        .await
        .expect("Failed to create base item");

    // Rapidly create and complete items, triggering multiple GC cycles
    for i in 1..=10 {
        let (temp_item, _gc_count) = storage
            .create_todo_item(format!("Temp {i}"), None)
            .await
            .expect("Failed to create temp item");

        storage
            .mark_todo_complete(&temp_item.id)
            .await
            .expect("Failed to complete temp item");

        // This will trigger GC
        storage
            .create_todo_item(format!("Next {i}"), None)
            .await
            .expect("Failed to create next item");
    }

    // Verify base item still exists
    let final_list = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");

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
    let (storage, temp_dir) = create_test_storage();

    // Create multiple items
    let (item1, _gc_count) = storage
        .create_todo_item("Task 1".to_string(), None)
        .await
        .expect("Failed to create item 1");

    let (item2, _gc_count) = storage
        .create_todo_item("Task 2".to_string(), None)
        .await
        .expect("Failed to create item 2");

    // Complete all items
    storage
        .mark_todo_complete(&item1.id)
        .await
        .expect("Failed to complete item 1");

    storage
        .mark_todo_complete(&item2.id)
        .await
        .expect("Failed to complete item 2");

    // Verify file is deleted
    let todo_file = get_todo_file_path(&temp_dir);
    assert!(
        !todo_file.exists(),
        "Todo file should be deleted after completing all items"
    );

    // Verify list is None
    let list = storage.get_todo_list().await.expect("Failed to get list");
    assert!(
        list.is_none(),
        "List should be None after all items complete"
    );

    // Create a new item - this should recreate the file
    let (new_item, _gc_count) = storage
        .create_todo_item("Fresh start".to_string(), None)
        .await
        .expect("Failed to create new item");

    // Verify file exists again
    assert!(
        todo_file.exists(),
        "Todo file should exist after creating new item"
    );

    // Verify list contains only the new item
    let new_list = storage
        .get_todo_list()
        .await
        .expect("Failed to get list")
        .expect("List should exist");
    assert_eq!(new_list.todo.len(), 1, "Should have 1 item");
    assert_eq!(new_list.todo[0].id, new_item.id, "Should have the new item");
}
