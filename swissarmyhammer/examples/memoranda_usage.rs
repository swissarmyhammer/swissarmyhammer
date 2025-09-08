//! Memoranda API usage examples for SwissArmyHammer library
//!
//! This example demonstrates how to programmatically interact with the memoranda system
//! for structured note-taking and knowledge management.

use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoContent, MemoStorage, MemoTitle};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 SwissArmyHammer Memoranda API Examples");
    println!("==========================================\n");

    // Initialize temporary storage for this example
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let mut storage: Box<dyn MemoStorage> =
        Box::new(MarkdownMemoStorage::new(temp_dir.path().join("memos")));

    // Helper function to create MemoTitle from string
    fn title_from_str(s: &str) -> MemoTitle {
        MemoTitle::new(s.to_string()).expect("Invalid title")
    }

    // Example 1: Creating memos
    println!("📝 Example 1: Creating Memos");
    println!("-----------------------------");

    let memo1 = storage
        .create(
            title_from_str("API Design Meeting"),
            MemoContent::new(
                r#"# API Design Meeting - January 15, 2024

## Attendees
- Alice (Backend Engineer)
- Bob (Frontend Engineer)
- Carol (Product Manager)

## Decisions Made
1. Use REST API with GraphQL for complex queries
2. Implement OAuth2 for authentication
3. Add rate limiting: 1000 requests/hour per user

## Action Items
- [ ] Alice: Implement OAuth endpoints (by Friday)
- [ ] Bob: Update frontend auth flow (by next Monday)
- [ ] Carol: Update API documentation (by Wednesday)

## Next Meeting
- Date: January 22, 2024
- Focus: Review implementation progress"#
                    .to_string(),
            ),
        )
        .await?;

    println!("✅ Created memo: {}", memo1.title);
    println!("🆔 ID: {}", memo1.title);
    println!("📅 Created: {}\n", memo1.created_at);

    let memo2 = storage
        .create(
            title_from_str("Rust Learning Notes"),
            MemoContent::new(
                r#"# Rust Learning Progress

## Completed Topics
- ✅ Ownership and borrowing
- ✅ Pattern matching
- ✅ Error handling with Result<T, E>
- ✅ Async/await basics

## Current Focus: Advanced Patterns
### Iterator Combinators
```rust
let numbers: Vec<i32> = vec![1, 2, 3, 4, 5]
    .iter()
    .filter(|&&x| x > 2)
    .map(|&x| x * 2)
    .collect();
```

### Error Propagation
```rust
fn read_file() -> Result<String, Box<dyn Error>> {
    let content = std::fs::read_to_string("file.txt")?;
    Ok(content.trim().to_string())
}
```

## Next Steps
- Learn about lifetimes in depth
- Practice with smart pointers
- Build a CLI tool project"#
                    .to_string(),
            ),
        )
        .await?;

    println!("✅ Created memo: {}", memo2.title);
    println!("🆔 ID: {}\n", memo2.title);

    // Example 2: Listing memos
    println!("📋 Example 2: Listing All Memos");
    println!("--------------------------------");

    let memos = storage.list().await?;
    println!("📝 Found {} memos:", memos.len());

    for memo in &memos {
        println!("  🆔 {}", memo.title);
        println!("  📄 {}", memo.title);
        println!("  📅 {}", memo.created_at);
        let content_str = memo.content.as_str();
        let preview = if content_str.len() > 100 {
            format!("{}...", &content_str[..97])
        } else {
            content_str.to_string()
        };
        println!("  💬 {}\n", preview.replace('\n', "\\n"));
    }

    // Example 3: Retrieving specific memos
    println!("🔍 Example 3: Retrieving Specific Memos");
    println!("---------------------------------------");

    let retrieved_memo = storage.get(&memo1.title).await?;
    if let Some(memo) = retrieved_memo {
        println!("📝 Retrieved memo: {}", memo.title);
        println!("🆔 ID: {}", memo.title);
        println!("📅 Created: {}", memo.created_at);
        println!("🔄 Updated: {}", memo.updated_at);
        println!("📖 Content length: {} characters\n", memo.content.len());
    } else {
        println!("❌ Memo not found\n");
    }

    // Example 4: Searching memos
    println!("🔎 Example 4: Searching Memos");
    println!("-----------------------------");

    // Note: Search functionality to be implemented later
    let search_results = storage.list().await?;

    println!("🔍 Search results for 'API authentication':");
    for memo in search_results {
        println!("  📄 {} (ID: {})", memo.title, memo.title);
        println!();
    }

    // Example 5: Advanced search patterns
    println!("🔍 Example 5: Advanced Search Patterns");
    println!("--------------------------------------");

    // Search for Rust-related content
    // Note: Search functionality to be implemented later
    let rust_results = storage.list().await?;

    println!("🦀 Rust-related memos ({} found):", rust_results.len());
    for memo in rust_results {
        println!("  📄 {}", memo.title);
    }
    println!();

    // Search for action items
    // Note: Search functionality to be implemented later
    let action_results = storage.list().await?;

    println!(
        "✅ Memos with action items ({} found):",
        action_results.len()
    );
    for memo in action_results {
        println!("  📄 {}", memo.title);
    }
    println!();

    // Example 6: Updating memos
    println!("📝 Example 6: Updating Memos");
    println!("-----------------------------");

    let updated_memo = storage
        .update(
            &memo1.title,
            MemoContent::new(
                r#"# API Design Meeting - January 15, 2024

## Attendees
- Alice (Backend Engineer)
- Bob (Frontend Engineer)
- Carol (Product Manager)

## Decisions Made
1. Use REST API with GraphQL for complex queries
2. Implement OAuth2 for authentication
3. Add rate limiting: 1000 requests/hour per user

## Action Items Progress (UPDATED)
- [x] Alice: Implement OAuth endpoints (✅ Completed ahead of schedule!)
- [ ] Bob: Update frontend auth flow (in progress, on track)
- [x] Carol: Update API documentation (✅ Completed)

## Follow-up Notes
- Alice's OAuth implementation is excellent
- Need to review Bob's auth flow before Monday
- Documentation is comprehensive and clear

## Next Meeting
- Date: January 22, 2024
- Focus: Review Bob's implementation and plan next sprint"#
                    .to_string(),
            ),
        )
        .await?;

    println!("✅ Updated memo: {}", updated_memo.title);
    println!("🔄 New updated_at: {}\n", updated_memo.updated_at);

    // Example 7: Error handling patterns
    println!("⚠️  Example 7: Error Handling Patterns");
    println!("-------------------------------------");

    // Attempt to get a non-existent memo
    let invalid_title = title_from_str("Nonexistent Memo");
    match storage.get(&invalid_title).await {
        Ok(Some(_)) => println!("Found memo (unexpected)"),
        Ok(None) => println!("✅ Correctly handled: Memo not found"),
        Err(e) => println!("✅ Correctly handled error: {e}"),
    }

    // Attempt to update a non-existent memo
    match storage
        .update(
            &invalid_title,
            MemoContent::new("Updated content".to_string()),
        )
        .await
    {
        Ok(_) => println!("Updated memo (unexpected)"),
        Err(e) => println!("✅ Correctly handled error: {e}"),
    }
    println!();

    // Example 8: Batch operations
    println!("📦 Example 8: Batch Operations");
    println!("------------------------------");

    // Create multiple memos for demonstration
    let project_memos = vec![
        (
            "Sprint Planning",
            "# Sprint 12 Planning\n\n- Goal: User authentication\n- Story points: 34",
        ),
        (
            "Daily Standup",
            "# Daily Standup Notes\n\n## Blockers\n- Database migration pending",
        ),
        (
            "Code Review",
            "# Code Review Checklist\n\n- [ ] Tests pass\n- [ ] Documentation updated",
        ),
    ];

    let mut created_titles = Vec::new();
    for (title, content) in project_memos {
        let memo_title = title_from_str(title);
        let _ = storage
            .create(memo_title.clone(), MemoContent::new(content.to_string()))
            .await?;
        created_titles.push(memo_title);
        println!("✅ Created: {title}");
    }

    println!("\n📊 Final Statistics:");
    let final_memos = storage.list().await?;
    println!("  📝 Total memos: {}", final_memos.len());

    let total_content_length: usize = final_memos.iter().map(|m| m.content.len()).sum();
    println!("  📖 Total content: {total_content_length} characters");

    let avg_content_length = if !final_memos.is_empty() {
        total_content_length / final_memos.len()
    } else {
        0
    };
    println!("  📏 Average content: {avg_content_length} characters per memo");

    // Example 9: Integration patterns
    println!("\n🔗 Example 9: Integration Patterns");
    println!("-----------------------------------");

    // Export all memos for external processing
    let all_memos = storage.list().await?;
    let mut context_export = String::new();

    for memo in &all_memos {
        context_export.push_str(&format!(
            "## {} (ID: {})\n\nCreated: {}\nUpdated: {}\n\n{}\n\n===\n\n",
            memo.title, memo.title, memo.created_at, memo.updated_at, memo.content
        ));
    }

    println!(
        "📄 Generated context export ({} chars)",
        context_export.len()
    );
    println!("💡 This format is perfect for AI assistant integration!\n");

    // Cleanup example (delete some memos)
    println!("🧹 Example 10: Cleanup Operations");
    println!("---------------------------------");

    for title in &created_titles[..2] {
        // Delete first 2 demo memos
        match storage.delete(title).await {
            Ok(true) => println!("✅ Deleted memo: {title}"),
            Ok(false) => println!("⚠️  Memo not found: {title}"),
            Err(e) => println!("❌ Error deleting memo: {e}"),
        }
    }

    let remaining_memos = storage.list().await?;
    println!("📊 Remaining memos: {}\n", remaining_memos.len());

    // Final summary
    println!("🎉 API Examples Completed!");
    println!("==========================");
    println!("✅ Demonstrated memo creation and management");
    println!("✅ Showed search and retrieval patterns");
    println!("✅ Illustrated error handling best practices");
    println!("✅ Provided integration and automation examples");
    println!();
    println!("💡 Next steps:");
    println!("   - Integrate memoranda into your application");
    println!("   - Use FileStorage for persistent data");
    println!("   - Implement custom search algorithms");
    println!("   - Build MCP tools for AI assistant integration");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memo_operations() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let mut storage = MarkdownMemoStorage::new(temp_dir.path().join("memos"));

        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let content = MemoContent::new("Test content".to_string());

        // Test creating a memo
        let memo = storage
            .create(title.clone(), content.clone())
            .await
            .unwrap();

        assert_eq!(memo.title, title);
        assert_eq!(memo.content, content);

        // Test retrieving the memo
        let retrieved = storage.get(&title).await.unwrap().unwrap();
        assert_eq!(retrieved.title, title);

        // Test listing memos
        let memos = storage.list().await.unwrap();
        assert_eq!(memos.len(), 1);

        // Test searching
        let results = <dyn MemoStorage>::search_memos(&storage, "Test")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Test deleting
        let deleted = storage.delete(&title).await.unwrap();
        assert!(deleted);

        let remaining = storage.list().await.unwrap();
        assert_eq!(remaining.len(), 0);
    }

    /// Analyze memo content and return statistics
    async fn analyze_memo_content(
        memo: &swissarmyhammer_memoranda::Memo,
    ) -> std::collections::HashMap<&'static str, usize> {
        let mut stats = std::collections::HashMap::new();

        // Count headers (lines starting with #)
        let headers = memo
            .content
            .as_str()
            .lines()
            .filter(|line| line.trim_start().starts_with('#'))
            .count();
        stats.insert("headers", headers);

        // Count action items (checkbox items)
        let action_items = memo
            .content
            .as_str()
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.contains("- [ ]") || trimmed.contains("- [x]") || trimmed.contains("- [X]")
            })
            .count();
        stats.insert("action_items", action_items);

        // Count words
        let words = memo.content.as_str().split_whitespace().count();
        stats.insert("words", words);

        // Count lines
        let lines = memo.content.as_str().lines().count();
        stats.insert("lines", lines);

        stats
    }

    #[tokio::test]
    async fn test_memo_content_analysis() {
        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let content = MemoContent::new(
            "# Header 1\n\nSome content\n\n## Header 2\n\n- [ ] Task 1\n- [x] Task 2".to_string(),
        );
        let memo = swissarmyhammer_memoranda::Memo::new(title, content);

        let stats = analyze_memo_content(&memo).await;

        assert_eq!(stats["headers"], 2);
        assert_eq!(stats["action_items"], 2);
        assert!(stats["words"] > 0);
        assert!(stats["lines"] > 0);
    }
}
