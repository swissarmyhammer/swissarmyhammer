# CLI Integration for Worktree Support

## Overview
Update the CLI commands to support worktree operations, ensuring the command-line interface works seamlessly with the new worktree-based workflow.

## Implementation

### Update Issue CLI Module (`swissarmyhammer-cli/src/issue.rs`)

Update the work subcommand handler:

```rust
pub async fn run_issue(subcommand: IssueSubcommand) -> Result<()> {
    match subcommand {
        IssueSubcommand::Work { name } => {
            let issue_storage = create_issue_storage().await?;
            let git_ops = GitOperations::new(std::env::current_dir()?);
            
            // Get the issue
            let issue = issue_storage.get_issue(&name).await
                .with_context(|| format!("Failed to get issue '{}'", name))?;
            
            // Create worktree
            let worktree_path = git_ops.create_work_worktree(&issue.name)?;
            
            println!("✅ Created worktree for issue '{}' at:", issue.name);
            println!("   {}", worktree_path.display());
            println!();
            println!("To start working:");
            println!("   cd {}", worktree_path.display());
            
            Ok(())
        }
        IssueSubcommand::Current => {
            let git_ops = GitOperations::new(std::env::current_dir()?);
            
            // List all issue worktrees
            let worktrees = git_ops.list_issue_worktrees()?;
            if !worktrees.is_empty() {
                println!("Active issue worktrees:");
                for wt in &worktrees {
                    println!("  - {} at {}", wt.issue_name, wt.path.display());
                }
                println!();
            }
            
            // Check current context
            match git_ops.get_current_issue()? {
                Some(issue) => {
                    println!("Current context: working on issue '{}'", issue);
                }
                None => {
                    let branch = git_ops.current_branch()?;
                    println!("Not on an issue branch. Current branch: {}", branch);
                }
            }
            
            Ok(())
        }
        IssueSubcommand::Merge { name, delete_branch } => {
            let issue_storage = create_issue_storage().await?;
            let git_ops = GitOperations::new(std::env::current_dir()?);
            
            // Validate issue is completed
            let issue = issue_storage.get_issue(&name).await?;
            if !issue.completed {
                return Err(anyhow!("Issue '{}' must be completed before merging", name));
            }
            
            // Perform merge with worktree cleanup
            git_ops.merge_issue_worktree(&issue.name, delete_branch)?;
            
            println!("✅ Merged issue '{}' to main", issue.name);
            
            // Get commit info
            if let Ok(info) = git_ops.get_last_commit_info() {
                let parts: Vec<&str> = info.split('|').collect();
                if parts.len() >= 4 {
                    println!("📝 Merge commit: {}", &parts[0][..8]);
                    println!("   Message: {}", parts[1]);
                }
            }
            
            Ok(())
        }
        // ... other subcommands remain unchanged
    }
}
```

### Add New CLI Subcommands (Optional)

Add worktree-specific commands for maintenance:

```rust
#[derive(Subcommand, Debug, Clone)]
pub enum IssueSubcommand {
    // ... existing commands ...
    
    /// List all active issue worktrees
    Worktrees,
    
    /// Clean up orphaned worktrees
    CleanupWorktrees,
}
```

Implementation:

```rust
IssueSubcommand::Worktrees => {
    let git_ops = GitOperations::new(std::env::current_dir()?);
    let worktrees = git_ops.list_issue_worktrees()?;
    
    if worktrees.is_empty() {
        println!("No active issue worktrees");
    } else {
        println!("Active issue worktrees:");
        for wt in worktrees {
            println!("  {} -> {}", wt.issue_name, wt.path.display());
            if let Some(branch) = wt.branch {
                println!("    Branch: {}", branch);
            }
        }
    }
    Ok(())
}

IssueSubcommand::CleanupWorktrees => {
    let git_ops = GitOperations::new(std::env::current_dir()?);
    let cleaned = git_ops.cleanup_orphaned_worktrees()?;
    
    if cleaned.is_empty() {
        println!("No orphaned worktrees found");
    } else {
        println!("Cleaned up {} orphaned worktrees:", cleaned.len());
        for path in cleaned {
            println!("  - {}", path);
        }
    }
    Ok(())
}
```

### Update Help Text

Update command help text to reflect worktree usage:

```rust
/// Work on an issue by creating a dedicated worktree
#[arg(help = "Creates or switches to a worktree for the specified issue")]
Work { name: String },

/// Merge an issue back to main and clean up its worktree
#[arg(help = "Merges the issue branch and removes its worktree")]
Merge { 
    name: String,
    #[arg(long, help = "Delete the branch after merging")]
    delete_branch: bool,
},
```

## Dependencies
- Requires all previous worktree implementation steps

## Testing
1. Test CLI work command creates worktree
2. Test CLI current command shows worktrees
3. Test CLI merge command cleans up worktree
4. Test new worktree maintenance commands
5. Test help text accuracy

## Context
This step updates the CLI to use the new worktree operations, providing a consistent command-line experience with helpful output that guides users through the worktree workflow.