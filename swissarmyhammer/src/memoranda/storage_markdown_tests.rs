use super::*;
use tempfile::TempDir;

fn create_test_markdown_storage() -> (MarkdownMemoStorage, TempDir) {
    let temp_dir = crate::test_utils::create_temp_dir_with_retry();
    let storage = MarkdownMemoStorage::new(temp_dir.path().join("memos"));
    (storage, temp_dir)
}

#[tokio::test]
async fn test_markdown_create_memo() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let memo = storage
        .create_memo(
            "Test Title".to_string(),
            "# Test Content\n\nThis is a test memo.".to_string(),
        )
        .await
        .unwrap();

    assert_eq!(memo.title, "Test Title");
    assert_eq!(memo.content, "# Test Content\n\nThis is a test memo.");
    assert!(!memo.id.as_str().is_empty());
}

#[tokio::test]
async fn test_markdown_get_memo() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let created_memo = storage
        .create_memo("Get Test".to_string(), "Get Content".to_string())
        .await
        .unwrap();

    let retrieved_memo = storage.get_memo(&created_memo.id).await.unwrap();
    assert_eq!(created_memo.title, retrieved_memo.title);
    assert_eq!(created_memo.content, retrieved_memo.content);
}

#[tokio::test]
async fn test_markdown_update_memo() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let created_memo = storage
        .create_memo("Update Test".to_string(), "Original Content".to_string())
        .await
        .unwrap();

    let updated_memo = storage
        .update_memo(&created_memo.id, "Updated Content".to_string())
        .await
        .unwrap();

    assert_eq!(updated_memo.content, "Updated Content");
    assert_eq!(updated_memo.title, "Update Test");
    assert_ne!(updated_memo.updated_at, created_memo.updated_at);
}

#[tokio::test]
async fn test_markdown_delete_memo() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let created_memo = storage
        .create_memo("Delete Test".to_string(), "Delete Content".to_string())
        .await
        .unwrap();

    // Verify memo exists
    storage.get_memo(&created_memo.id).await.unwrap();

    // Delete memo
    storage.delete_memo(&created_memo.id).await.unwrap();

    // Verify memo no longer exists
    let result = storage.get_memo(&created_memo.id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_markdown_list_memos() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    // Create multiple memos
    storage
        .create_memo("Title 1".to_string(), "Content 1".to_string())
        .await
        .unwrap();
    storage
        .create_memo("Title 2".to_string(), "Content 2".to_string())
        .await
        .unwrap();
    storage
        .create_memo("Title 3".to_string(), "Content 3".to_string())
        .await
        .unwrap();

    let memos = storage.list_memos().await.unwrap();
    assert_eq!(memos.len(), 3);

    // Check that all created memos are present (titles should match since they're derived from filenames)
    let memo_titles: std::collections::HashSet<&str> =
        memos.iter().map(|m| m.title.as_str()).collect();
    let expected_titles: std::collections::HashSet<&str> =
        ["Title 1", "Title 2", "Title 3"].into_iter().collect();
    assert_eq!(memo_titles, expected_titles);
}

#[tokio::test]
async fn test_markdown_filename_sanitization() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    // Test with problematic characters
    let problematic_title = "Title/with\\problematic:characters*?\"<>|";
    let memo = storage
        .create_memo(problematic_title.to_string(), "Content".to_string())
        .await
        .unwrap();

    // Should be able to retrieve the memo
    let retrieved = storage.get_memo(&memo.id).await.unwrap();
    assert_eq!(retrieved.title, "Title_with_problematic_characters_______");
    assert_eq!(retrieved.content, "Content");
}

#[tokio::test]
async fn test_markdown_unicode_content() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let unicode_title = "🚀 Test with 中文 and émojis 🎉";
    let unicode_content = "Content with Unicode: ñáéíóú, 中文测试, 🌟✨🎯";

    let memo = storage
        .create_memo(unicode_title.to_string(), unicode_content.to_string())
        .await
        .unwrap();

    let retrieved = storage.get_memo(&memo.id).await.unwrap();
    assert_eq!(retrieved.content, unicode_content);

    // Test searching with unicode
    let results = storage.search_memos("中文").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, unicode_content);
}

#[tokio::test]
async fn test_markdown_duplicate_title_handling() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    // Create first memo
    storage
        .create_memo("Duplicate Title".to_string(), "First content".to_string())
        .await
        .unwrap();

    // Try to create another memo with the same title
    let result = storage
        .create_memo("Duplicate Title".to_string(), "Second content".to_string())
        .await;

    assert!(result.is_err());
    match result {
        Err(SwissArmyHammerError::MemoAlreadyExists(_)) => {}
        _ => panic!("Expected MemoAlreadyExists error"),
    }
}

#[tokio::test]
async fn test_markdown_empty_title_handling() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let memo = storage
        .create_memo("".to_string(), "Content for empty title".to_string())
        .await
        .unwrap();

    // Empty title should be converted to "untitled"
    let retrieved = storage.get_memo(&memo.id).await.unwrap();
    assert_eq!(retrieved.title, "untitled");
    assert_eq!(retrieved.content, "Content for empty title");
}

#[tokio::test]
async fn test_markdown_very_long_title() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let long_title = "A".repeat(300); // Very long title
    let memo = storage
        .create_memo(long_title.clone(), "Short content".to_string())
        .await
        .unwrap();

    // Title should be truncated to 200 characters
    let retrieved = storage.get_memo(&memo.id).await.unwrap();
    assert_eq!(retrieved.title.len(), 200);
    assert_eq!(retrieved.title, "A".repeat(200));
    assert_eq!(retrieved.content, "Short content");
}

#[tokio::test]
async fn test_markdown_search_functionality() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    // Create memos with different content
    storage
        .create_memo(
            "Rust Programming".to_string(),
            "Learning Rust language".to_string(),
        )
        .await
        .unwrap();
    storage
        .create_memo(
            "Python Guide".to_string(),
            "Python programming tutorial".to_string(),
        )
        .await
        .unwrap();
    storage
        .create_memo(
            "JavaScript Basics".to_string(),
            "Introduction to JS".to_string(),
        )
        .await
        .unwrap();

    let rust_results = storage.search_memos("Rust").await.unwrap();
    assert_eq!(rust_results.len(), 1);
    assert_eq!(rust_results[0].title, "Rust Programming");

    let programming_results = storage.search_memos("programming").await.unwrap();
    assert_eq!(programming_results.len(), 2);

    let js_results = storage.search_memos("javascript").await.unwrap();
    assert_eq!(js_results.len(), 1);
    assert_eq!(js_results[0].title, "JavaScript Basics");
}

#[tokio::test]
async fn test_markdown_advanced_search() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    storage
        .create_memo(
            "Project Meeting".to_string(),
            "Discussed project timeline and deliverables.".to_string(),
        )
        .await
        .unwrap();

    let options = crate::memoranda::SearchOptions {
        include_highlights: true,
        ..Default::default()
    };
    let results = storage
        .search_memos_advanced("project", &options)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(!results[0].highlights.is_empty());
    assert!(results[0].relevance_score > 0.0);
}

#[tokio::test]
async fn test_markdown_get_all_context() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    storage
        .create_memo("First Memo".to_string(), "First content".to_string())
        .await
        .unwrap();
    storage
        .create_memo("Second Memo".to_string(), "Second content".to_string())
        .await
        .unwrap();

    let options = crate::memoranda::ContextOptions::default();
    let context = storage.get_all_context(&options).await.unwrap();

    assert!(context.contains("First Memo"));
    assert!(context.contains("Second Memo"));
    assert!(context.contains("First content"));
    assert!(context.contains("Second content"));
}

#[tokio::test]
async fn test_markdown_filesystem_timestamps() {
    let (storage, temp_dir) = create_test_markdown_storage();

    // Create a memo
    let memo = storage
        .create_memo("Timestamp Test".to_string(), "Original content".to_string())
        .await
        .unwrap();

    // Check that the markdown file was created
    let path = temp_dir.path().join("memos").join("Timestamp Test.md");
    assert!(path.exists());

    // Verify file content is pure markdown (not JSON)
    let file_content = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(file_content, "Original content");

    // Verify timestamps are reasonable (created within the last minute)
    let now = Utc::now();
    let time_diff = now.signed_duration_since(memo.created_at);
    assert!(time_diff.num_seconds() < 60);
}

#[tokio::test]
async fn test_markdown_newlines_and_formatting() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let formatted_content = "# Heading\n\n## Subheading\n\n- List item 1\n- List item 2\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
    let memo = storage
        .create_memo(
            "Formatted Content".to_string(),
            formatted_content.to_string(),
        )
        .await
        .unwrap();

    let retrieved = storage.get_memo(&memo.id).await.unwrap();
    assert_eq!(retrieved.content, formatted_content);
}

#[tokio::test]
async fn test_markdown_concurrent_creation() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let tasks = (0..10).map(|i| {
        let storage_ref = &storage;
        async move {
            storage_ref
                .create_memo(format!("Concurrent Title {i}"), format!("Content {i}"))
                .await
        }
    });

    let results = futures::future::try_join_all(tasks).await.unwrap();
    assert_eq!(results.len(), 10);

    // Verify all titles are unique
    let mut titles: Vec<_> = results.iter().map(|memo| &memo.title).collect();
    titles.sort();
    titles.dedup();
    assert_eq!(titles.len(), 10);
}

#[tokio::test]
async fn test_markdown_special_characters_in_title() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let special_title = "Title with \"quotes\" and <brackets> & symbols!";
    let memo = storage
        .create_memo(special_title.to_string(), "Content".to_string())
        .await
        .unwrap();

    let retrieved = storage.get_memo(&memo.id).await.unwrap();
    assert_eq!(
        retrieved.title,
        "Title with _quotes_ and _brackets_ & symbols!"
    );
    assert_eq!(retrieved.content, "Content");
}

#[tokio::test]
async fn test_markdown_whitespace_in_title() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let whitespace_title = "  Title with\twhitespace\n  ";
    let memo = storage
        .create_memo(whitespace_title.to_string(), "Content".to_string())
        .await
        .unwrap();

    let retrieved = storage.get_memo(&memo.id).await.unwrap();
    assert_eq!(retrieved.title, "Title with whitespace");
    assert_eq!(retrieved.content, "Content");
}

#[tokio::test]
async fn test_markdown_get_nonexistent_memo() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let fake_id = MemoId::new();
    let result = storage.get_memo(&fake_id).await;

    assert!(result.is_err());
    match result {
        Err(SwissArmyHammerError::MemoNotFound(_)) => {}
        _ => panic!("Expected MemoNotFound error"),
    }
}

#[tokio::test]
async fn test_markdown_delete_nonexistent_memo() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let fake_id = MemoId::new();
    let result = storage.delete_memo(&fake_id).await;

    assert!(result.is_err());
    // The error will likely be MemoNotFound since get_memo is called first
    match result {
        Err(SwissArmyHammerError::MemoNotFound(_)) => {}
        _ => panic!("Expected MemoNotFound error"),
    }
}

#[tokio::test]
async fn test_markdown_update_nonexistent_memo() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let fake_id = MemoId::new();
    let result = storage
        .update_memo(&fake_id, "New content".to_string())
        .await;

    assert!(result.is_err());
    match result {
        Err(SwissArmyHammerError::MemoNotFound(_)) => {}
        _ => panic!("Expected MemoNotFound error"),
    }
}

#[tokio::test]
async fn test_markdown_list_empty_directory() {
    let (storage, _temp_dir) = create_test_markdown_storage();

    let memos = storage.list_memos().await.unwrap();
    assert_eq!(memos.len(), 0);
}
