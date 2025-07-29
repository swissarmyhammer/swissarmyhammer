use crate::cli::IssueCommands;
use colored::*;
use std::io::{self, Read};
use swissarmyhammer::fs_utils::FileSystemUtils;
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};

const NAMELESS_ISSUE_NAME: &str = "";

fn format_issue_status(completed: bool) -> colored::ColoredString {
    if completed {
        "✅ Completed".green()
    } else {
        "🔄 Active".yellow()
    }
}

pub async fn handle_issue_command(
    command: IssueCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = FileSystemIssueStorage::new_default()?;

    match command {
        IssueCommands::Create {
            name,
            content,
            file,
        } => {
            create_issue(storage, name, content, file).await?;
        }
        IssueCommands::List {
            completed,
            active,
            format,
        } => {
            list_issues(storage, completed, active, format).await?;
        }
        IssueCommands::Show { name, raw } => {
            show_issue(storage, &name, raw).await?;
        }
        IssueCommands::Update {
            name,
            content,
            file,
            append,
        } => {
            update_issue(storage, &name, content, file, append).await?;
        }
        IssueCommands::Complete { name } => {
            complete_issue(storage, &name).await?;
        }
        IssueCommands::Work { name } => {
            work_issue(storage, &name).await?;
        }
        IssueCommands::Merge { name, keep_branch } => {
            merge_issue(storage, &name, keep_branch).await?;
        }
        IssueCommands::Current => {
            show_current_issue(storage).await?;
        }
        IssueCommands::Status => {
            show_status(storage).await?;
        }
        IssueCommands::Next => {
            show_next_issue(storage).await?;
        }
    }

    Ok(())
}

async fn create_issue(
    storage: FileSystemIssueStorage,
    name: Option<String>,
    content: Option<String>,
    file: Option<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_content_from_args(content, file)?;

    // Use empty string for nameless issues (matches MCP behavior)
    let issue_name = name.unwrap_or(NAMELESS_ISSUE_NAME.to_string());
    let issue = storage.create_issue(issue_name, content).await?;

    println!(
        "{} Created issue: {}",
        "✅".green(),
        issue.name.as_str().bold()
    );

    println!("📁 File: {}", issue.file_path.display());

    Ok(())
}

async fn list_issues(
    storage: FileSystemIssueStorage,
    show_completed: bool,
    show_active: bool,
    format: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let all_issues = storage.list_issues().await?;

    let issues: Vec<_> = all_issues
        .into_iter()
        .filter(|issue| {
            if show_completed && show_active {
                true // show all
            } else if show_completed {
                issue.completed
            } else if show_active {
                !issue.completed
            } else {
                true // default: show all
            }
        })
        .collect();

    match format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&issues)?);
        }
        "markdown" => {
            print_issues_markdown(&issues).await?;
        }
        _ => {
            print_issues_table(&issues).await?;
        }
    }

    Ok(())
}

async fn print_issues_table(
    issues: &[swissarmyhammer::issues::Issue],
) -> Result<(), Box<dyn std::error::Error>> {
    if issues.is_empty() {
        println!("No issues found.");
        return Ok(());
    }

    let active_issues: Vec<_> = issues.iter().filter(|i| !i.completed).collect();
    let completed_issues: Vec<_> = issues.iter().filter(|i| i.completed).collect();

    let total_issues = issues.len();
    let completed_count = completed_issues.len();
    let active_count = active_issues.len();
    let completion_percentage = if total_issues > 0 {
        (completed_count * 100) / total_issues
    } else {
        0
    };

    println!("📊 Issues: {total_issues} total");
    println!("✅ Completed: {completed_count} ({completion_percentage}%)");
    println!("🔄 Active: {active_count}");

    if active_count > 0 {
        println!();
        println!("{}", "Active Issues:".bold());
        for issue in active_issues {
            println!("  🔄 {}", issue.name);
        }
    }

    if completed_count > 0 {
        println!();
        println!("{}", "Recently Completed:".bold());
        let mut sorted_completed = completed_issues;
        sorted_completed.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        for issue in sorted_completed.iter().take(5) {
            println!("  ✅ {}", issue.name);
        }
    }

    Ok(())
}

async fn print_issues_markdown(
    issues: &[swissarmyhammer::issues::Issue],
) -> Result<(), Box<dyn std::error::Error>> {
    println!("# Issues");
    println!();

    if issues.is_empty() {
        println!("No issues found.");
        return Ok(());
    }

    for issue in issues {
        let status = if issue.completed { "✅" } else { "🔄" };
        println!("## {} - {}", status, issue.name);
        println!();
        println!(
            "- **Status**: {}",
            if issue.completed {
                "Completed"
            } else {
                "Active"
            }
        );
        println!("- **Created**: {}", issue.created_at.format("%Y-%m-%d"));
        println!("- **File**: {}", issue.file_path.display());
        println!();
        if !issue.content.is_empty() {
            println!("### Content");
            println!();
            println!("{}", issue.content);
            println!();
        }
        println!("---");
        println!();
    }

    Ok(())
}

async fn show_issue(
    storage: FileSystemIssueStorage,
    name: &str,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let issues = storage.list_issues().await?;
    let issue = issues
        .into_iter()
        .find(|i| i.name == name)
        .ok_or_else(|| format!("Issue '{name}' not found"))?;

    if raw {
        println!("{}", issue.content);
    } else {
        let status = format_issue_status(issue.completed);

        println!("{} Issue: {}", status, issue.name.as_str().bold());
        println!("📁 File: {}", issue.file_path.display());
        println!(
            "📅 Created: {}",
            issue.created_at.format("%Y-%m-%d %H:%M:%S")
        );
        println!();
        println!("{}", issue.content);
    }

    Ok(())
}

async fn update_issue(
    storage: FileSystemIssueStorage,
    name: &str,
    content: Option<String>,
    file: Option<std::path::PathBuf>,
    append: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let new_content = get_content_from_args(content, file)?;

    let issues = storage.list_issues().await?;
    let issue = issues
        .into_iter()
        .find(|i| i.name == name)
        .ok_or_else(|| format!("Issue '{name}' not found"))?;

    let updated_content = if append {
        if issue.content.is_empty() {
            new_content
        } else {
            format!("{}\n\n{}", issue.content, new_content)
        }
    } else {
        new_content
    };

    let updated_issue = storage.update_issue(name, updated_content).await?;

    println!(
        "{} Updated issue: {}",
        "✅".green(),
        updated_issue.name.as_str().bold()
    );

    Ok(())
}

async fn complete_issue(
    storage: FileSystemIssueStorage,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let issues = storage.list_issues().await?;
    let issue = issues
        .into_iter()
        .find(|i| i.name == name)
        .ok_or_else(|| format!("Issue '{name}' not found"))?;

    if issue.completed {
        println!("ℹ️ Issue '{}' is already completed", issue.name);
        return Ok(());
    }

    let completed_issue = storage.mark_complete(name).await?;

    println!(
        "{} Marked issue '{}' as complete",
        "✅".green(),
        completed_issue.name.as_str().bold()
    );

    Ok(())
}

async fn work_issue(
    storage: FileSystemIssueStorage,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let issues = storage.list_issues().await?;
    let issue = issues
        .into_iter()
        .find(|i| i.name == name)
        .ok_or_else(|| format!("Issue '{name}' not found"))?;

    if issue.completed {
        println!("⚠️ Issue '{}' is already completed", issue.name);
        return Ok(());
    }

    let git_ops = GitOperations::new()?;
    let branch_name = git_ops.create_work_branch(&issue.name)?;

    println!(
        "{} Started working on issue: {}",
        "🔄".yellow(),
        issue.name.as_str().bold()
    );

    println!("🌿 Branch: {}", branch_name.bold());

    Ok(())
}

async fn merge_issue(
    storage: FileSystemIssueStorage,
    name: &str,
    keep_branch: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let issues = storage.list_issues().await?;
    let issue = issues
        .into_iter()
        .find(|i| i.name == name)
        .ok_or_else(|| format!("Issue '{name}' not found"))?;

    if !issue.completed {
        println!("⚠️ Issue '{}' is not completed", issue.name);
        println!("Complete the issue first with: swissarmyhammer issue complete {name}");
        return Ok(());
    }

    let git_ops = GitOperations::new()?;

    git_ops.merge_issue_branch(&issue.name)?;

    if !keep_branch {
        // Delete the branch
        let branch_name = format!("issue/{}", issue.name);
        if let Err(e) = git_ops.delete_branch(&branch_name) {
            eprintln!("Warning: Failed to delete branch {branch_name}: {e}");
        }
    }

    println!(
        "{} Merged issue '{}' to main",
        "✅".green(),
        issue.name.as_str().bold()
    );

    Ok(())
}

async fn show_current_issue(
    storage: FileSystemIssueStorage,
) -> Result<(), Box<dyn std::error::Error>> {
    let git_ops = GitOperations::new()?;
    let current_branch = git_ops.current_branch()?;

    if current_branch.starts_with("issue/") {
        let issue_name = current_branch.strip_prefix("issue/").unwrap();

        let issues = storage.list_issues().await?;
        match issues.into_iter().find(|i| i.name == issue_name) {
            Some(issue) => {
                let status = format_issue_status(issue.completed);

                println!("{} Current issue: {}", status, issue.name.as_str().bold());
                println!("🌿 Branch: {}", current_branch.bold());
                println!("📁 File: {}", issue.file_path.display());
            }
            None => {
                println!("⚠️ On issue branch '{current_branch}' but issue not found");
            }
        }
    } else {
        println!("ℹ️ Not currently working on a specific issue");
        println!("🌿 Current branch: {}", current_branch.bold());
    }

    Ok(())
}

async fn show_status(storage: FileSystemIssueStorage) -> Result<(), Box<dyn std::error::Error>> {
    let all_issues = storage.list_issues().await?;
    let active_count = all_issues.iter().filter(|i| !i.completed).count();
    let completed_count = all_issues.iter().filter(|i| i.completed).count();
    let total = all_issues.len();

    println!("📊 Project Status");
    println!("  Total issues: {total}");
    println!("  🔄 Active: {active_count}");
    println!("  ✅ Completed: {completed_count}");

    if total > 0 {
        let percentage = (completed_count * 100) / total;
        println!("  📈 Completion: {percentage}%");
    }

    Ok(())
}

fn get_content_from_args(
    content: Option<String>,
    file: Option<std::path::PathBuf>,
) -> Result<String, Box<dyn std::error::Error>> {
    match (content, file) {
        (Some(content), None) => {
            if content == "-" {
                // Read from stdin
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer)?;
                Ok(buffer.trim().to_string())
            } else {
                Ok(content)
            }
        }
        (None, Some(path)) => {
            let fs = FileSystemUtils::new();
            let content = fs.read_text(&path)?;
            Ok(content.trim().to_string())
        }
        (Some(_), Some(_)) => Err("Cannot specify both --content and --file options".into()),
        (None, None) => {
            // Allow empty content for nameless issues
            Ok(String::new())
        }
    }
}

async fn show_next_issue(
    storage: FileSystemIssueStorage,
) -> Result<(), Box<dyn std::error::Error>> {
    match storage.get_next_issue().await? {
        Some(issue) => {
            let status = format_issue_status(issue.completed);

            println!("{} Next issue: {}", status, issue.name.as_str().bold());
            println!("📁 File: {}", issue.file_path.display());
            println!(
                "📅 Created: {}",
                issue.created_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!();
            println!("{}", issue.content);
        }
        None => {
            println!("🎉 No pending issues found. All issues are completed!");
        }
    }

    Ok(())
}
