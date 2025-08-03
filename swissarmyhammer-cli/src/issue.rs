use crate::cli::{IssueCommands, OutputFormat};
use crate::mcp_integration::{response_formatting, CliToolContext};
use serde_json::json;
use std::io::{self, Read};
use swissarmyhammer::config::Config;

pub async fn handle_issue_command(
    command: IssueCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = CliToolContext::new().await?;

    match command {
        IssueCommands::Create {
            name,
            content,
            file,
        } => {
            create_issue(&context, name, content, file).await?;
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
            file,
            append,
        } => {
            update_issue(&context, &name, content, file, append).await?;
        }
        IssueCommands::Complete { name } => {
            complete_issue(&context, &name).await?;
        }
        IssueCommands::Work { name } => {
            work_issue(&context, &name).await?;
        }
        IssueCommands::Merge { name, keep_branch } => {
            merge_issue(&context, &name, keep_branch).await?;
        }
        IssueCommands::Current => {
            show_current_issue(&context).await?;
        }
        IssueCommands::Status => {
            show_status(&context).await?;
        }
        IssueCommands::Next => {
            show_next_issue(&context).await?;
        }
    }

    Ok(())
}

async fn create_issue(
    context: &CliToolContext,
    name: Option<String>,
    content: Option<String>,
    file: Option<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_content_from_args(content, file)?;
    // Ensure content is not empty for MCP tool compatibility
    let content = if content.is_empty() {
        Config::global().default_issue_content.clone()
    } else {
        content
    };

    let args = if let Some(issue_name) = name {
        context.create_arguments(vec![
            ("name", json!(issue_name)),
            ("content", json!(content)),
        ])
    } else {
        // For nameless issues, don't pass a name argument
        context.create_arguments(vec![("content", json!(content))])
    };

    let result = context.execute_tool("issue_create", args).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

async fn list_issues(
    context: &CliToolContext,
    show_completed: bool,
    show_active: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let format_str = match format {
        OutputFormat::Table => "table",
        OutputFormat::Json => "json",
        OutputFormat::Yaml => "markdown", // MCP tool uses "markdown" for YAML-like output
    };
    let args = context.create_arguments(vec![
        ("show_completed", json!(show_completed)),
        ("show_active", json!(show_active)),
        ("format", json!(format_str)),
    ]);

    let result = context.execute_tool("issue_list", args).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

async fn show_issue(
    context: &CliToolContext,
    name: &str,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![("name", json!(name)), ("raw", json!(raw))]);

    let result = context.execute_tool("issue_show", args).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

async fn update_issue(
    context: &CliToolContext,
    name: &str,
    content: Option<String>,
    file: Option<std::path::PathBuf>,
    append: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let new_content = get_content_from_args(content, file)?;

    let args = context.create_arguments(vec![
        ("name", json!(name)),
        ("content", json!(new_content)),
        ("append", json!(append)),
    ]);

    let result = context.execute_tool("issue_update", args).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

async fn complete_issue(
    context: &CliToolContext,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![("name", json!(name))]);
    let result = context.execute_tool("issue_mark_complete", args).await?;

    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

async fn work_issue(
    context: &CliToolContext,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![("name", json!(name))]);
    let result = context.execute_tool("issue_work", args).await?;

    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

async fn merge_issue(
    context: &CliToolContext,
    name: &str,
    keep_branch: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![
        ("name", json!(name)),
        ("delete_branch", json!(!keep_branch)),
    ]);

    let result = context.execute_tool("issue_merge", args).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

async fn show_current_issue(context: &CliToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![("name", json!("current"))]);
    let result = context.execute_tool("issue_show", args).await?;

    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}

async fn show_status(context: &CliToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![]);
    let result = context.execute_tool("issue_all_complete", args).await?;

    println!("{}", response_formatting::format_success_response(&result));
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
            let content = std::fs::read_to_string(path)?;
            Ok(content.trim().to_string())
        }
        (Some(_), Some(_)) => Err("Cannot specify both --content and --file options".into()),
        (None, None) => {
            // Allow empty content for nameless issues
            Ok(String::new())
        }
    }
}

async fn show_next_issue(context: &CliToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![("name", json!("next"))]);
    let result = context.execute_tool("issue_show", args).await?;

    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}
