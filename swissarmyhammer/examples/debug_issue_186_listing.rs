use std::path::PathBuf;
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create storage pointing to the actual issues directory
    let issues_dir = PathBuf::from("/Users/wballard/github/swissarmyhammer/issues");
    let storage = FileSystemIssueStorage::new(issues_dir)?;

    println!("=== Debugging Issue 186 Listing ===\n");

    // List all issues with extended info (includes file paths and completion status)
    let all_issues = storage.list_issues_info().await?;

    println!("Total issues found: {}", all_issues.len());

    // Look specifically for issue 000186
    let issue_186 = all_issues.iter().find(|issue_info| {
        issue_info.issue.name.contains("186")
            || issue_info
                .file_path
                .file_stem()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("")
                .contains("186")
    });

    if let Some(issue_info) = issue_186 {
        println!("\n🔍 Found Issue 186:");
        println!("  Name: '{}'", issue_info.issue.name);
        println!("  File: {}", issue_info.file_path.display());
        println!("  Completed: {}", issue_info.completed);
        println!("  Created: {}", issue_info.created_at);
        println!(
            "  Content preview: {}",
            issue_info
                .issue
                .content
                .lines()
                .take(2)
                .collect::<Vec<_>>()
                .join("\n")
        );
    } else {
        println!("\n❌ Issue 186 not found!");
    }

    // Filter to pending issues only
    let pending_issues: Vec<_> = all_issues
        .iter()
        .filter(|issue_info| !issue_info.completed)
        .collect();

    println!("\n📋 Pending issues: {}", pending_issues.len());
    for (i, issue_info) in pending_issues.iter().enumerate() {
        if i < 10 {
            // Show first 10
            println!(
                "  {}. {} ({})",
                i + 1,
                issue_info.issue.name,
                issue_info.file_path.file_name().unwrap().to_str().unwrap()
            );
        }
    }
    if pending_issues.len() > 10 {
        println!("  ... and {} more", pending_issues.len() - 10);
    }

    // Show completed issues count
    let completed_count = all_issues
        .iter()
        .filter(|issue_info| issue_info.completed)
        .count();
    println!("\n✅ Completed issues: {completed_count}");

    // Check the next issue logic specifically
    if !pending_issues.is_empty() {
        let next_issue = pending_issues[0];
        println!(
            "\n🎯 Next issue would be: '{}' ({})",
            next_issue.issue.name,
            next_issue.file_path.file_name().unwrap().to_str().unwrap()
        );
    } else {
        println!("\n🎯 No next issue - all completed!");
    }

    Ok(())
}
