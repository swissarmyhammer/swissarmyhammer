//! Integration test for file-based tag storage

use swissarmyhammer_kanban::{
    board::InitBoard,
    tag::{AddTag, DeleteTag, GetTag, ListTags, UpdateTag},
    task::{AddTask, TagTask},
    KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use tempfile::TempDir;

#[tokio::test]
async fn test_tag_file_based_storage() {
    // Setup
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    let processor = KanbanOperationProcessor::with_actor("test-user");

    // Initialize board
    processor
        .process(&InitBoard::new("Test Board"), &ctx)
        .await
        .unwrap();

    // Verify tags directory was created
    assert!(
        kanban_dir.join("tags").exists(),
        "Tags directory should be created"
    );

    // Add a tag
    let result = processor
        .process(
            &AddTag::new("bug", "Bug", "FF0000").with_description("Bug reports"),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(result["id"], "bug");
    assert_eq!(result["name"], "Bug");

    // Verify tag file was created
    let tag_file = kanban_dir.join("tags").join("bug.json");
    assert!(tag_file.exists(), "Tag file should be created");

    // Verify board.json does NOT contain tags array
    let board_content = std::fs::read_to_string(kanban_dir.join("board.json")).unwrap();
    assert!(
        !board_content.contains("\"tags\""),
        "Board.json should not contain tags field"
    );

    // List tags - should read from file
    let result = processor.process(&ListTags::new(), &ctx).await.unwrap();
    assert_eq!(result["count"], 1);
    assert_eq!(result["tags"][0]["id"], "bug");

    // Get tag - should read from file
    let result = processor.process(&GetTag::new("bug"), &ctx).await.unwrap();
    assert_eq!(result["id"], "bug");
    assert_eq!(result["name"], "Bug");

    // Update tag - should update file
    processor
        .process(&UpdateTag::new("bug").with_name("Critical Bug"), &ctx)
        .await
        .unwrap();

    let result = processor.process(&GetTag::new("bug"), &ctx).await.unwrap();
    assert_eq!(result["name"], "Critical Bug");

    // Add task and tag it
    let task_result = processor
        .process(&AddTask::new("Fix issue"), &ctx)
        .await
        .unwrap();
    let task_id = task_result["id"].as_str().unwrap();

    processor
        .process(&TagTask::new(task_id, "bug"), &ctx)
        .await
        .unwrap();

    // Delete tag - should cascade to tasks and delete file
    processor
        .process(&DeleteTag::new("bug"), &ctx)
        .await
        .unwrap();

    // Verify tag file was deleted
    assert!(!tag_file.exists(), "Tag file should be deleted");

    // Verify tag no longer in list
    let result = processor.process(&ListTags::new(), &ctx).await.unwrap();
    assert_eq!(result["count"], 0);
}

#[tokio::test]
async fn test_tag_validation() {
    // Setup
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    let processor = KanbanOperationProcessor::with_actor("test-user");

    processor
        .process(&InitBoard::new("Test Board"), &ctx)
        .await
        .unwrap();

    // Try to get non-existent tag
    let result = processor.process(&GetTag::new("nonexistent"), &ctx).await;
    assert!(result.is_err(), "Should error on non-existent tag");

    // Add a task
    let task_result = processor
        .process(&AddTask::new("Test task"), &ctx)
        .await
        .unwrap();
    let task_id = task_result["id"].as_str().unwrap();

    // Try to tag task with non-existent tag
    let result = processor
        .process(&TagTask::new(task_id, "nonexistent"), &ctx)
        .await;
    assert!(
        result.is_err(),
        "Should error when tagging with non-existent tag"
    );
}
