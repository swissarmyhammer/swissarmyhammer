//! Integration tests for memoranda storage functionality
//!
//! Tests core memo operations:
//! - Creating, reading, updating, deleting memos
//! - Listing memos and getting all context
//! - Error handling and edge cases

use serial_test::serial;
use std::sync::Arc;
use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoContent, MemoStorage, MemoTitle};
use tokio::sync::Mutex;

/// Helper to create a test memo storage
async fn create_test_storage() -> Arc<Mutex<MarkdownMemoStorage>> {
    let temp_dir = tempfile::tempdir().unwrap();
    let memos_dir = temp_dir.path().join("memos");
    std::fs::create_dir_all(&memos_dir).unwrap();
    Arc::new(Mutex::new(MarkdownMemoStorage::new(memos_dir)))
}

/// Test creating a memo
#[tokio::test]
#[serial]
async fn test_memo_create() {
    let storage = create_test_storage().await;

    let title = MemoTitle::new("Test Memo".to_string()).unwrap();
    let content = MemoContent::from("Test content for memo".to_string());

    let memo = storage
        .lock()
        .await
        .create(title.clone(), content.clone())
        .await
        .unwrap();
    assert_eq!(memo.title.as_str(), "Test Memo");
    assert_eq!(memo.content.as_str(), "Test content for memo");
}

/// Test retrieving a memo
#[tokio::test]
#[serial]
async fn test_memo_get() {
    let storage = create_test_storage().await;

    let title = MemoTitle::new("Get Test Memo".to_string()).unwrap();
    let content = MemoContent::from("Content for get test".to_string());

    // Create memo first
    storage
        .lock()
        .await
        .create(title.clone(), content.clone())
        .await
        .unwrap();

    // Retrieve it
    let retrieved = storage.lock().await.get(&title).await.unwrap();
    assert!(retrieved.is_some());
    let memo = retrieved.unwrap();
    assert_eq!(memo.title.as_str(), "Get Test Memo");
    assert_eq!(memo.content.as_str(), "Content for get test");
}

/// Test listing memos
#[tokio::test]
#[serial]
async fn test_memo_list() {
    let storage = create_test_storage().await;

    // Create multiple memos
    let titles = vec!["Memo 1", "Memo 2", "Memo 3"];
    for (i, title_str) in titles.iter().enumerate() {
        let title = MemoTitle::new(title_str.to_string()).unwrap();
        let content = MemoContent::from(format!("Content for memo {}", i + 1));
        storage.lock().await.create(title, content).await.unwrap();
    }

    let memos = storage.lock().await.list().await.unwrap();
    assert_eq!(memos.len(), 3);

    let memo_titles: Vec<&str> = memos.iter().map(|m| m.title.as_str()).collect();
    for title in &titles {
        assert!(memo_titles.contains(title));
    }
}

/// Test updating a memo
#[tokio::test]
#[serial]
async fn test_memo_update() {
    let storage = create_test_storage().await;

    let title = MemoTitle::new("Update Test Memo".to_string()).unwrap();
    let original_content = MemoContent::from("Original content".to_string());
    let updated_content = MemoContent::from("Updated content".to_string());

    // Create memo
    storage
        .lock()
        .await
        .create(title.clone(), original_content)
        .await
        .unwrap();

    // Update it
    let updated_memo = storage
        .lock()
        .await
        .update(&title, updated_content.clone())
        .await
        .unwrap();
    assert_eq!(updated_memo.content.as_str(), "Updated content");

    // Verify the update persisted
    let retrieved = storage.lock().await.get(&title).await.unwrap().unwrap();
    assert_eq!(retrieved.content.as_str(), "Updated content");
}

/// Test deleting a memo
#[tokio::test]
#[serial]
async fn test_memo_delete() {
    let storage = create_test_storage().await;

    let title = MemoTitle::new("Delete Test Memo".to_string()).unwrap();
    let content = MemoContent::from("Content to be deleted".to_string());

    // Create memo
    storage
        .lock()
        .await
        .create(title.clone(), content)
        .await
        .unwrap();

    // Verify it exists
    let retrieved = storage.lock().await.get(&title).await.unwrap();
    assert!(retrieved.is_some());

    // Delete it
    let deleted = storage.lock().await.delete(&title).await.unwrap();
    assert!(deleted);

    // Verify it's gone
    let retrieved = storage.lock().await.get(&title).await.unwrap();
    assert!(retrieved.is_none());
}

/// Test error handling for invalid titles
#[tokio::test]
#[serial]
async fn test_memo_invalid_title() {
    // Test empty title
    let result = MemoTitle::new("".to_string());
    assert!(result.is_err());

    // Test title with invalid characters
    let result = MemoTitle::new("Invalid/Title".to_string());
    assert!(result.is_err());
}

/// Test getting non-existent memo
#[tokio::test]
#[serial]
async fn test_memo_get_nonexistent() {
    let storage = create_test_storage().await;

    let title = MemoTitle::new("Nonexistent Memo".to_string()).unwrap();
    let result = storage.lock().await.get(&title).await.unwrap();
    assert!(result.is_none());
}

/// Test deleting non-existent memo
#[tokio::test]
#[serial]
async fn test_memo_delete_nonexistent() {
    let storage = create_test_storage().await;

    let title = MemoTitle::new("Nonexistent Memo".to_string()).unwrap();
    let deleted = storage.lock().await.delete(&title).await.unwrap();
    assert!(!deleted);
}
