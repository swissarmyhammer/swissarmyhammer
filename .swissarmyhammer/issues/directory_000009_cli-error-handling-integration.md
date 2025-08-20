# CLI Error Handling Integration for Git Repository Requirements

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Update CLI command handlers and error messaging to gracefully handle Git repository requirements and provide clear guidance to users when commands are run outside Git repositories.

## Current State
Many CLI commands will start failing when components are migrated to require Git repositories. Users need clear, helpful error messages that guide them to the correct solution.

## Implementation Approach

### Centralized Error Handling
```rust
impl From<SwissArmyHammerError> for CliError {
    fn from(err: SwissArmyHammerError) -> Self {
        match err {
            SwissArmyHammerError::NotInGitRepository => CliError {
                message: format_git_repository_requirement_error(),
                exit_code: EXIT_ERROR,
                source: Some(Box::new(err)),
            },
            SwissArmyHammerError::DirectoryCreation(io_err) => CliError {
                message: format_directory_creation_error(&io_err),
                exit_code: EXIT_ERROR, 
                source: Some(Box::new(err)),
            },
            // ... other error mappings
        }
    }
}

fn format_git_repository_requirement_error() -> String {
    format!(
        "❌ Git repository required\n\n\
        SwissArmyHammer operations require a Git repository context.\n\
        \n\
        Solutions:\n\
        • Run this command from within a Git repository\n\
        • Initialize a Git repository: git init\n\
        • Clone an existing repository: git clone <url>\n\
        \n\
        Current directory: {}", 
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unable to determine>".to_string())
    )
}
```

### Command-Specific Error Handling
Update command handlers to provide context-specific guidance:

```rust
// Memo commands
async fn handle_memo_create(/* ... */) -> Result<(), CliError> {
    let context = CliToolContext::new().await?;
    
    // This will now properly propagate NotInGitRepository errors
    let result = memo_operations::create_memo(&context, name, content).await
        .map_err(|e| match e {
            SwissArmyHammerError::NotInGitRepository => CliError {
                message: format!(
                    "❌ Memo operations require a Git repository\n\n\
                    Memos are stored in .swissarmyhammer/memos/ at the Git repository root.\n\
                    Please run this command from within a Git repository."
                ),
                exit_code: EXIT_ERROR,
                source: Some(Box::new(e)),
            },
            other => CliError::from(other),
        })?;
        
    println!("{}", format_memo_success(&result));
    Ok(())
}
```

## Error Message Strategy

### Consistent Error Format
```
❌ [Component] requires a Git repository

[Component-specific explanation]

Solutions:
• Run this command from within a Git repository  
• Initialize a Git repository: git init
• Clone an existing repository: git clone <url>

Current directory: /path/to/current/dir
```

### Component-Specific Messages
- **Memos**: "Memos are stored in .swissarmyhammer/memos/ at the Git repository root"
- **Todos**: "Todo lists are stored in .swissarmyhammer/todo/ at the Git repository root"  
- **Search**: "Search index is stored in .swissarmyhammer/semantic.db at the Git repository root"
- **Workflows**: "Workflow runs are tracked in .swissarmyhammer/runs/ at the Git repository root"

## Help Text Updates
Update command help text to mention Git repository requirements:

```rust
#[derive(Parser)]
pub struct MemoCreate {
    /// Memo name (requires Git repository context)
    pub name: String,
    // ...
}
```

## Tasks
1. Update `CliError` implementation with Git repository error handling
2. Add centralized error message formatting functions
3. Update all MCP tool integrations with proper error propagation
4. Add component-specific error context for each command group:
   - Memo commands
   - Todo commands  
   - Search commands
   - Workflow commands
   - Issue commands (if affected)
5. Update CLI help text to mention Git repository requirements
6. Add integration tests covering:
   - Error handling outside Git repository for each command
   - Error message formatting and clarity
   - Exit code consistency
7. Test user experience with clear error scenarios

## Documentation Updates
- Update README with Git repository requirements
- Add troubleshooting section for common Git repository issues
- Document migration process from non-Git usage

## Dependencies
- Depends on: directory_000006_memoranda-system-migration
- Depends on: directory_000007_todo-system-migration  
- Depends on: directory_000005_search-system-migration

## Success Criteria
- All CLI commands provide clear error messages when Git repository is required
- Error messages are helpful and actionable 
- Consistent error formatting across all components
- Users understand what they need to do to resolve issues
- Integration tests validate error handling scenarios
- Documentation clearly explains Git repository requirements