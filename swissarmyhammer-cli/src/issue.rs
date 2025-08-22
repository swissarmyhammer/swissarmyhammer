use crate::cli::IssueCommands;
use crate::error::{format_component_specific_git_error, CliError};
use crate::exit_codes::EXIT_ERROR;
use crate::mcp_integration::{response_formatting, CliToolContext};
use serde_json::json;

/// Handle issue-related CLI commands
pub async fn handle_issue_command(command: IssueCommands) -> Result<(), Box<dyn std::error::Error>> {
    let context = CliToolContext::new().await?;

    // Check for Git repository requirement for issue operations
    context.require_git_repository().await.map_err(|e| {
        match e.downcast_ref::<swissarmyhammer::SwissArmyHammerError>() {
            Some(swissarmyhammer::SwissArmyHammerError::NotInGitRepository) => {
                CliError {
                    message: format_component_specific_git_error(
                        "Issue operations",
                        "Issues are stored in .swissarmyhammer/issues/ at the Git repository root and require Git for branch management."
                    ),
                    exit_code: EXIT_ERROR,
                    source: None,
                }
            }
            _ => CliError {
                message: format!("Failed to check Git repository requirement: {e}"),
                exit_code: EXIT_ERROR,
                source: None,
            }
        }
    })?;

    match command {
        IssueCommands::Create { name, content } => {
            create_issue(&context, name, content).await?;
        }
        IssueCommands::List {
            completed,
            active,
            format,
        } => {
            list_issues(&context, completed, active, format).await?;
        }
        IssueCommands::Show { name, raw } => {
            show_issue(&context, &name, raw).await?;
        }
        IssueCommands::Update {
            name,
            content,
            append,
        } => {
            update_issue(&context, &name, &content, append).await?;
        }
        IssueCommands::Complete { name } => {
            mark_complete_issue(&context, &name).await?;
        }
        IssueCommands::Work { name } => {
            work_issue(&context, &name).await?;
        }
        IssueCommands::Merge { name, keep_branch } => {
            merge_issue(&context, &name, !keep_branch).await?;
        }
        IssueCommands::Current => {
            show_current_issue(&context).await?;
        }
        IssueCommands::Status => {
            show_project_status(&context).await?;
        }
        IssueCommands::Next => {
            show_next_issue(&context).await?;
        }
    }

    Ok(())
}

/// Create a new issue
async fn create_issue(
    context: &CliToolContext,
    name: Option<String>,
    content: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Vec::new();

    if let Some(name_val) = name {
        args.push(("name", json!(name_val)));
    }

    let content_val = if let Some(content_str) = content {
        content_str
    } else {
        // For CLI usage, if no content is provided, use empty string
        // This allows creating issues without content, which is expected by tests
        String::new()
    };

    // Always add content, even if empty
    args.push(("content", json!(content_val)));

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_create", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// List all issues
async fn list_issues(
    context: &CliToolContext,
    completed: bool,
    active: bool,
    format: crate::cli::OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::cli::OutputFormat;

    let format_str = match format {
        OutputFormat::Table => "table",
        OutputFormat::Json => "json", 
        OutputFormat::Yaml => "markdown",
    };

    let args = vec![
        ("show_completed", json!(completed)),
        ("show_active", json!(active)),
        ("format", json!(format_str)),
    ];

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_list", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Show issue details
async fn show_issue(
    context: &CliToolContext,
    name: &str,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = vec![
        ("name", json!(name)),
        ("raw", json!(raw)),
    ];

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_show", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Mark an issue as complete
async fn mark_complete_issue(
    context: &CliToolContext,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = vec![("name", json!(name))];

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_mark_complete", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}


/// Update an issue
async fn update_issue(
    context: &CliToolContext,
    name: &str,
    content: &str,
    append: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = vec![
        ("name", json!(name)),
        ("content", json!(content)),
        ("append", json!(append)),
    ];

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_update", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Work on an issue (switch to work branch)
async fn work_issue(context: &CliToolContext, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let args = vec![("name", json!(name))];

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_work", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Merge issue work branch
async fn merge_issue(
    context: &CliToolContext,
    name: &str,
    delete_branch: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = vec![
        ("name", json!(name)),
        ("delete_branch", json!(delete_branch)),
    ];

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_merge", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Show current issue (based on current branch)
async fn show_current_issue(context: &CliToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let args = vec![
        ("name", json!("current")),
        ("raw", json!(false)),
    ];

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_show", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Show project status (all completed check)
async fn show_project_status(context: &CliToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let arguments = context.create_arguments(vec![]);
    let result = context.execute_tool("issue_all_complete", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

/// Show next issue to work on
async fn show_next_issue(context: &CliToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let args = vec![
        ("name", json!("next")),
        ("raw", json!(false)),
    ];

    let arguments = context.create_arguments(args);
    let result = context.execute_tool("issue_show", arguments).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}