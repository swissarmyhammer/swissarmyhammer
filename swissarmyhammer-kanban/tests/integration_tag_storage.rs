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

    // Add a tag (name is the human-readable slug, id is auto-generated ULID)
    let result = processor
        .process(
            &AddTag::new("bug")
                .with_color("FF0000")
                .with_description("Bug reports"),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(result["name"], "bug");
    // ID should be a ULID (26 chars)
    let tag_id = result["id"].as_str().unwrap();
    assert_eq!(tag_id.len(), 26);

    // Verify tag file was created with ULID filename
    let tag_file = kanban_dir.join("tags").join(format!("{}.yaml", tag_id));
    assert!(
        tag_file.exists(),
        "Tag file should be created with ULID name"
    );

    // Verify board.yaml does NOT contain tags array
    let board_content = std::fs::read_to_string(kanban_dir.join("board.yaml")).unwrap();
    assert!(
        !board_content.contains("tags"),
        "Board.yaml should not contain tags field"
    );

    // List tags - should read from file
    let result = processor.process(&ListTags::new(), &ctx).await.unwrap();
    assert_eq!(result["count"], 1);
    assert_eq!(result["tags"][0]["name"], "bug");

    // Get tag by name
    let result = processor.process(&GetTag::new("bug"), &ctx).await.unwrap();
    assert_eq!(result["name"], "bug");

    // Get tag by ULID
    let result = processor.process(&GetTag::new(tag_id), &ctx).await.unwrap();
    assert_eq!(result["name"], "bug");

    // Update tag color
    processor
        .process(&UpdateTag::new(tag_id).with_color("FF5555"), &ctx)
        .await
        .unwrap();

    let result = processor.process(&GetTag::new(tag_id), &ctx).await.unwrap();
    assert_eq!(result["color"], "FF5555");

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
        .process(&DeleteTag::new(tag_id), &ctx)
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

    // Tagging with a non-existent tag now auto-creates the tag object
    let result = processor
        .process(&TagTask::new(task_id, "auto-created"), &ctx)
        .await;
    assert!(result.is_ok(), "TagTask should auto-create missing tags");

    // Verify tag was auto-created
    let tag_result = processor.process(&GetTag::new("auto-created"), &ctx).await;
    assert!(tag_result.is_ok(), "Auto-created tag should be retrievable");
    assert_eq!(tag_result.unwrap()["name"], "auto-created");
}

#[tokio::test]
async fn test_tag_rename_via_update() {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    let processor = KanbanOperationProcessor::with_actor("test-user");

    processor
        .process(&InitBoard::new("Test Board"), &ctx)
        .await
        .unwrap();

    // Add a tag
    let result = processor
        .process(&AddTag::new("old-name").with_color("d73a4a"), &ctx)
        .await
        .unwrap();
    let tag_id = result["id"].as_str().unwrap().to_string();

    // Add a task and tag it
    let task_result = processor
        .process(
            &AddTask::new("Test task").with_description("Fix the #old-name issue"),
            &ctx,
        )
        .await
        .unwrap();
    let task_id = task_result["id"].as_str().unwrap();

    // Rename via update — should bulk-replace in task descriptions
    let result = processor
        .process(&UpdateTag::new(&*tag_id).with_name("new-name"), &ctx)
        .await
        .unwrap();
    assert_eq!(result["name"], "new-name");
    // Same ULID
    assert_eq!(result["id"], tag_id);

    // Verify task description was updated
    let task = ctx
        .read_task(&swissarmyhammer_kanban::TaskId::from_string(task_id))
        .await
        .unwrap();
    assert!(task.description.contains("#new-name"));
    assert!(!task.description.contains("#old-name"));

    // Tag file should still exist at same ULID path
    let tag_file = kanban_dir.join("tags").join(format!("{}.yaml", tag_id));
    assert!(tag_file.exists());
}
