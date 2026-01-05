//! Isolated test for todo create, read, and list operations

use swissarmyhammer_todo::{TodoItem, TodoList, TodoStorage};
use tempfile::TempDir;

#[tokio::test]
async fn test_todo_create_read_list_roundtrip() {
    // Create a temporary directory for the test
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage = TodoStorage::new(temp_dir.path().to_path_buf());

    // Create a todo item
    let (item, _gc_count): (TodoItem, usize) = storage
        .create_todo_item(
            "Fix the bug in parser".to_string(),
            Some("Check error handling in parse_token function".to_string()),
        )
        .await
        .expect("Failed to create todo item");

    println!("Created todo: {:?}", item);
    assert_eq!(item.task, "Fix the bug in parser");
    assert_eq!(
        item.context,
        Some("Check error handling in parse_token function".to_string())
    );
    assert!(!item.done);

    // Read the item back by ID (use as_str() since id is TodoId type)
    let read_item: TodoItem = storage
        .get_todo_item(item.id.as_str())
        .await
        .expect("Failed to read todo item")
        .expect("Todo item not found");

    println!("Read todo: {:?}", read_item);
    assert_eq!(read_item.id, item.id);
    assert_eq!(read_item.task, item.task);
    assert_eq!(read_item.context, item.context);
    assert_eq!(read_item.done, item.done);

    // List all todos
    let list: TodoList = storage
        .get_todo_list()
        .await
        .expect("Failed to list todos")
        .expect("No todo list");

    println!("Todo list: {:?}", list);
    assert_eq!(list.todo.len(), 1);
    assert_eq!(list.todo[0].id, item.id);

    // Create another todo
    let (item2, _): (TodoItem, usize) = storage
        .create_todo_item("Add unit tests".to_string(), None)
        .await
        .expect("Failed to create second todo");

    // List should now have 2 items
    let list: TodoList = storage
        .get_todo_list()
        .await
        .expect("Failed to list todos")
        .expect("No todo list");

    assert_eq!(list.todo.len(), 2);

    // Get next incomplete item (should be first one)
    let next: TodoItem = storage
        .get_todo_item("next")
        .await
        .expect("Failed to get next")
        .expect("No next item");

    assert_eq!(next.id, item.id);

    // Mark first item complete (id is already TodoId type)
    storage
        .mark_todo_complete(&item.id)
        .await
        .expect("Failed to mark complete");

    // Get next incomplete (should now be second item)
    let next: TodoItem = storage
        .get_todo_item("next")
        .await
        .expect("Failed to get next")
        .expect("No next item");

    assert_eq!(next.id, item2.id);

    // List all again and check states
    let list: TodoList = storage
        .get_todo_list()
        .await
        .expect("Failed to list todos")
        .expect("No todo list");

    // First item should be done, second should not
    let first = list.todo.iter().find(|t| t.id == item.id).unwrap();
    let second = list.todo.iter().find(|t| t.id == item2.id).unwrap();
    assert!(first.done, "First item should be done");
    assert!(!second.done, "Second item should not be done");

    println!("All roundtrip tests passed!");
}

#[test]
fn test_yaml_serialization_format() {
    // Test that the YAML format matches expected structure
    let item = TodoItem::new("Test task".to_string(), Some("Test context".to_string()));

    let list = TodoList {
        todo: vec![item.clone()],
    };

    let yaml = serde_yaml::to_string(&list).expect("Failed to serialize");
    println!("Serialized YAML:\n{}", yaml);

    // Verify the YAML contains expected field names
    assert!(yaml.contains("task:"), "YAML should contain 'task:' field");
    assert!(
        yaml.contains("context:"),
        "YAML should contain 'context:' field"
    );
    assert!(yaml.contains("done:"), "YAML should contain 'done:' field");
    assert!(yaml.contains("id:"), "YAML should contain 'id:' field");

    // Deserialize back
    let parsed: TodoList = serde_yaml::from_str(&yaml).expect("Failed to deserialize");
    assert_eq!(parsed.todo.len(), 1);
    assert_eq!(parsed.todo[0].task, item.task);
    assert_eq!(parsed.todo[0].context, item.context);
    assert_eq!(parsed.todo[0].done, item.done);
}

#[test]
fn test_backwards_compatible_yaml_parsing() {
    // Test parsing the old YAML format that's already on disk
    let old_format_yaml = r#"
todo:
- id: 01KE25XX8SC1NG68VFS08AQ4S6
  task: Fix the bug
  context: Some context here
  done: false
  created_at: 2026-01-03T14:59:34.297503Z
  updated_at: 2026-01-03T14:59:34.297503Z
"#;

    let parsed: TodoList =
        serde_yaml::from_str(old_format_yaml).expect("Failed to parse old format YAML");

    assert_eq!(parsed.todo.len(), 1);
    // id is a TodoId, use as_str() to compare
    assert_eq!(parsed.todo[0].id.as_str(), "01KE25XX8SC1NG68VFS08AQ4S6");
    assert_eq!(parsed.todo[0].task, "Fix the bug");
    assert_eq!(
        parsed.todo[0].context,
        Some("Some context here".to_string())
    );
    assert!(!parsed.todo[0].done);
    // Timestamps are DateTime<Utc>, not Option - just verify parsing worked
    println!("created_at: {}", parsed.todo[0].created_at);
    println!("updated_at: {}", parsed.todo[0].updated_at);
}
