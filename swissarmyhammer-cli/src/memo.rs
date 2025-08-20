use crate::cli::MemoCommands;
use crate::error::{format_component_specific_git_error, CliError};
use crate::exit_codes::EXIT_ERROR;
use crate::mcp_integration::CliToolContext;
use rmcp::model::CallToolResult;
use serde_json::json;
use std::io::{self, Read};

pub async fn handle_memo_command(command: MemoCommands) -> Result<(), Box<dyn std::error::Error>> {
    let context = CliToolContext::new().await?;

    // Check for Git repository requirement for memo operations
    context.require_git_repository().await.map_err(|e| match e
        .downcast_ref::<swissarmyhammer::SwissArmyHammerError>()
    {
        Some(swissarmyhammer::SwissArmyHammerError::NotInGitRepository) => CliError {
            message: format_component_specific_git_error(
                "Memo operations",
                "Memos are stored in .swissarmyhammer/memos/ at the Git repository root.",
            ),
            exit_code: EXIT_ERROR,
            source: None,
        },
        _ => CliError {
            message: format!("Failed to check Git repository requirement: {e}"),
            exit_code: EXIT_ERROR,
            source: None,
        },
    })?;

    match command {
        MemoCommands::Create { title, content } => {
            create_memo(&context, title, content).await?;
        }
        MemoCommands::List => {
            list_memos(&context).await?;
        }
        MemoCommands::Get { id } => {
            get_memo(&context, &id).await?;
        }
        MemoCommands::Update { id, content } => {
            update_memo(&context, &id, content).await?;
        }
        MemoCommands::Delete { id } => {
            delete_memo(&context, &id).await?;
        }
        MemoCommands::Search { query } => {
            search_memos(&context, &query).await?;
        }
        MemoCommands::Context => {
            get_context(&context).await?;
        }
    }

    Ok(())
}

async fn create_memo(
    context: &CliToolContext,
    title: String,
    content: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_content_input(content)?;

    let args = context.create_arguments(vec![("title", json!(title)), ("content", json!(content))]);

    let result = context.execute_tool("memo_create", args).await?;

    println!("{}", format_create_memo_response(&result, &title));
    Ok(())
}

async fn list_memos(context: &CliToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![]);
    let result = context.execute_tool("memo_list", args).await?;

    println!("{}", format_list_memo_response(&result));
    Ok(())
}

async fn get_memo(context: &CliToolContext, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![("id", json!(id))]);
    let result = context.execute_tool("memo_get", args).await;

    match result {
        Ok(result) => {
            println!("{}", format_get_memo_response(&result));
            Ok(())
        }
        Err(e) => {
            // Handle client-side validation errors
            let error_msg = e.to_string();
            if error_msg.contains("Invalid memo ID format:") {
                eprintln!("Memo ID contains invalid character");
                std::process::exit(1);
            } else {
                eprintln!("Error: {error_msg}");
                std::process::exit(1);
            }
        }
    }
}

async fn update_memo(
    context: &CliToolContext,
    id: &str,
    content: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_content_input(content)?;

    let args = context.create_arguments(vec![("id", json!(id)), ("content", json!(content))]);

    let result = context.execute_tool("memo_update", args).await;

    match result {
        Ok(result) => {
            println!("{}", format_update_memo_response(&result));
            Ok(())
        }
        Err(e) => {
            // Handle client-side validation errors
            let error_msg = e.to_string();
            if error_msg.contains("Invalid memo ID format:") {
                eprintln!("Memo ID contains invalid character");
                std::process::exit(1);
            } else {
                eprintln!("Error: {error_msg}");
                std::process::exit(1);
            }
        }
    }
}

async fn delete_memo(context: &CliToolContext, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![("id", json!(id))]);
    let result = context.execute_tool("memo_delete", args).await;

    match result {
        Ok(result) => {
            println!("{}", format_delete_memo_response(&result));
            Ok(())
        }
        Err(e) => {
            // Handle client-side validation errors
            let error_msg = e.to_string();
            if error_msg.contains("Invalid memo ID format:") {
                eprintln!("Memo ID contains invalid character");
                std::process::exit(1);
            } else {
                eprintln!("Error: {error_msg}");
                std::process::exit(1);
            }
        }
    }
}

async fn search_memos(
    context: &CliToolContext,
    query: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![("query", json!(query))]);
    let result = context.execute_tool("memo_search", args).await?;

    println!("{}", format_search_memo_response(&result, query));
    Ok(())
}

async fn get_context(context: &CliToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let args = context.create_arguments(vec![]);
    let result = context.execute_tool("memo_get_all_context", args).await?;

    println!("{}", format_context_memo_response(&result));
    Ok(())
}

/// Represents different sources of content input
enum ContentInput {
    Direct(String),
    Stdin,
    Interactive,
}

/// Get content from various input sources
fn get_content_input(content: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    let input_type = match content {
        Some(c) if c == "-" => ContentInput::Stdin,
        Some(c) => ContentInput::Direct(c),
        None => ContentInput::Interactive,
    };

    match input_type {
        ContentInput::Direct(content) => Ok(content),
        ContentInput::Stdin => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            Ok(buffer.trim().to_string())
        }
        ContentInput::Interactive => {
            println!("📝 Enter memo content:");
            println!("   💡 Type or paste your content, then press Ctrl+D (or Cmd+D on Mac) when finished");
            println!("   💡 You can enter multiple lines - just keep typing and press Enter for new lines");
            println!("   💡 To cancel, press Ctrl+C");
            println!();
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            Ok(buffer.trim().to_string())
        }
    }
}

/// Custom response formatters for memo CLI commands to match expected test format
mod memo_response_formatting {
    use colored::*;
    use once_cell::sync::Lazy;
    use regex::Regex;
    use rmcp::model::{CallToolResult, RawContent};

    static MEMO_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"with ID: ([A-Z0-9]+)").unwrap());
    static SEARCH_COUNT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"Found (\d+) memo").unwrap());

    /// Format memo create response to match CLI expectations
    pub fn format_create_memo_response(result: &CallToolResult, title: &str) -> String {
        if result.is_error.unwrap_or(false) {
            return extract_text_content(result)
                .unwrap_or_else(|| "An error occurred creating memo".to_string())
                .red()
                .to_string();
        }

        // Extract memo ID from the MCP response if available
        let response_text = extract_text_content(result)
            .unwrap_or_else(|| format!("Successfully created memo '{title}'"));

        let memo_id = extract_memo_id(&response_text);

        // Format in the expected CLI style
        let mut output = format!("{} Created memo: {}", "✅".green(), title.bold());
        if let Some(id) = memo_id {
            output.push_str(&format!("\n🆔 ID: {}", id.blue()));

            // Use current time since ULID parsing is complex
            let timestamp = chrono::Utc::now();
            output.push_str(&format!(
                "\n📅 Created: {}",
                timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }
        output
    }

    /// Format memo search response to match CLI expectations  
    pub fn format_search_memo_response(result: &CallToolResult, query: &str) -> String {
        if result.is_error.unwrap_or(false) {
            return extract_text_content(result)
                .unwrap_or_else(|| "An error occurred searching memos".to_string())
                .red()
                .to_string();
        }

        let response_text =
            extract_text_content(result).unwrap_or_else(|| "No results found".to_string());

        // Handle different search response formats
        if response_text.contains("No memos found matching query:") {
            // Transform: "No memos found matching query: 'query'" -> "ℹ️ No memos found matching 'query'"
            let result = response_text
                .replace(
                    "No memos found matching query: '",
                    "No memos found matching '",
                )
                .replace(
                    "No memos found matching query: \"",
                    "No memos found matching \"",
                );
            format!("{} {}", "ℹ️".blue(), result)
        } else if let Some(count) = extract_search_count(&response_text) {
            if count == 0 {
                format!("{} No memos found matching '{}'", "ℹ️".blue(), query)
            } else {
                // Replace the start of the response with emoji version
                response_text.replace(
                    &format!("Found {count} memo"),
                    &format!("{} Found {count} memo", "🔍".blue()),
                )
            }
        } else {
            // If we can't parse the count, just add the emoji
            format!("{} {}", "🔍".blue(), response_text)
        }
    }

    /// Format memo context response to match CLI expectations
    pub fn format_context_memo_response(result: &CallToolResult) -> String {
        if result.is_error.unwrap_or(false) {
            return extract_text_content(result)
                .unwrap_or_else(|| "An error occurred getting context".to_string())
                .red()
                .to_string();
        }

        let response_text =
            extract_text_content(result).unwrap_or_else(|| "No memos available".to_string());

        // Handle empty context case
        if response_text.contains("No memos available") {
            format!("{} No memos available for context", "ℹ️".blue())
        } else {
            // Add document emoji to the context header
            response_text.replace("All memo context", &format!("{} All memo context", "📄"))
        }
    }

    /// Format memo list response to match CLI expectations
    pub fn format_list_memo_response(result: &CallToolResult) -> String {
        if result.is_error.unwrap_or(false) {
            return extract_text_content(result)
                .unwrap_or_else(|| "An error occurred listing memos".to_string())
                .red()
                .to_string();
        }

        let response_text =
            extract_text_content(result).unwrap_or_else(|| "No memos found".to_string());

        // Handle different list response formats
        if response_text.contains("No memos found") {
            format!("{} No memos found", "ℹ️".blue())
        } else if let Some(count_match) = response_text.find("Found ") {
            // Replace "Found X memo(s):" with "📝 Found X memo(s)" and add 🆔, 📄 emojis
            let mut result = response_text.clone();
            if let Some(colon_pos) = result.find(':') {
                result.replace_range(
                    count_match..colon_pos + 1,
                    &format!("{} {}", "📝".blue(), &result[count_match..colon_pos]),
                );
            } else {
                result.replace_range(
                    count_match..,
                    &format!("{} {}", "📝".blue(), &result[count_match..]),
                );
            }

            // Add emojis to the individual memo entries
            result = result.replace("Created:", &format!("{} Created:", "📅"));
            result = result.replace("Updated:", &format!("{} Updated:", "🔄"));
            result = result.replace("Preview:", &format!("{} Preview:", "📄"));

            // Add ID emoji to the title lines - format: • Title (ID) -> • Title (🆔 ID)
            let id_regex = regex::Regex::new(r"• ([^(]+) \(([A-Z0-9]+)\)").unwrap();
            result = id_regex.replace_all(&result, "• $1 (🆔 $2)").to_string();

            result
        } else {
            response_text
        }
    }

    /// Format memo get response to match CLI expectations
    pub fn format_get_memo_response(result: &CallToolResult) -> String {
        if result.is_error.unwrap_or(false) {
            let error_text = extract_text_content(result)
                .unwrap_or_else(|| "An error occurred retrieving memo".to_string());

            // Transform error message to match test expectations
            if error_text.contains("Invalid memo ID format:") {
                return "Memo ID contains invalid character"
                    .to_string()
                    .red()
                    .to_string();
            }

            return error_text.red().to_string();
        }

        let response_text =
            extract_text_content(result).unwrap_or_else(|| "Memo not found".to_string());

        // Add emojis to match test expectations
        let mut result = response_text;
        result = result.replace("ID:", &format!("{} ID:", "🆔"));
        result = result.replace("Created:", &format!("{} Created:", "📅"));
        result = result.replace("Updated:", &format!("{} Updated:", "🔄"));

        result
    }

    /// Format memo update response to match CLI expectations
    pub fn format_update_memo_response(result: &CallToolResult) -> String {
        if result.is_error.unwrap_or(false) {
            let error_text = extract_text_content(result)
                .unwrap_or_else(|| "An error occurred updating memo".to_string());

            // Transform error message to match test expectations
            if error_text.contains("Invalid memo ID format:") {
                return "Memo ID contains invalid character"
                    .to_string()
                    .red()
                    .to_string();
            }

            return error_text.red().to_string();
        }

        let response_text =
            extract_text_content(result).unwrap_or_else(|| "Memo updated".to_string());

        // Add emojis and update prefix
        let mut result = response_text;
        if result.starts_with("Successfully updated memo") {
            result = result.replace("Successfully updated memo", "✅ Updated memo");
        }
        result = result.replace("ID:", &format!("{} ID:", "🆔"));
        result = result.replace("Updated:", &format!("{} Updated:", "🔄"));

        result
    }

    /// Format memo delete response to match CLI expectations
    pub fn format_delete_memo_response(result: &CallToolResult) -> String {
        if result.is_error.unwrap_or(false) {
            let error_text = extract_text_content(result)
                .unwrap_or_else(|| "An error occurred deleting memo".to_string());

            // Transform error message to match test expectations
            if error_text.contains("Invalid memo ID format:") {
                return "Memo ID contains invalid character"
                    .to_string()
                    .red()
                    .to_string();
            }

            return error_text.red().to_string();
        }

        let response_text =
            extract_text_content(result).unwrap_or_else(|| "Memo deleted".to_string());

        // Add delete emoji and format the response
        let result = response_text;
        if result.contains("Successfully deleted memo") || result.contains("deleted memo") {
            // Extract the memo ID if present in the response
            if let Some(id_start) = result.find("ID: ") {
                let id_end = result[id_start + 4..]
                    .find(' ')
                    .unwrap_or(result.len() - id_start - 4);
                let memo_id = &result[id_start + 4..id_start + 4 + id_end];
                format!("{} Deleted memo: {}", "🗑️", memo_id)
            } else {
                format!("{} Deleted memo", "🗑️")
            }
        } else {
            result
        }
    }

    /// Extract text content from CallToolResult
    fn extract_text_content(result: &CallToolResult) -> Option<String> {
        result
            .content
            .first()
            .and_then(|content| match &content.raw {
                RawContent::Text(text_content) => Some(text_content.text.clone()),
                _ => None,
            })
    }

    /// Extract memo ID from response text using regex
    fn extract_memo_id(text: &str) -> Option<String> {
        MEMO_ID_REGEX
            .captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Extract search result count from response text
    fn extract_search_count(text: &str) -> Option<usize> {
        SEARCH_COUNT_REGEX
            .captures(text)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse().ok())
    }
}

/// Use the custom formatting functions
fn format_create_memo_response(result: &CallToolResult, title: &str) -> String {
    memo_response_formatting::format_create_memo_response(result, title)
}

fn format_search_memo_response(result: &CallToolResult, query: &str) -> String {
    memo_response_formatting::format_search_memo_response(result, query)
}

fn format_context_memo_response(result: &CallToolResult) -> String {
    memo_response_formatting::format_context_memo_response(result)
}

fn format_list_memo_response(result: &CallToolResult) -> String {
    memo_response_formatting::format_list_memo_response(result)
}

fn format_get_memo_response(result: &CallToolResult) -> String {
    memo_response_formatting::format_get_memo_response(result)
}

fn format_update_memo_response(result: &CallToolResult) -> String {
    memo_response_formatting::format_update_memo_response(result)
}

fn format_delete_memo_response(result: &CallToolResult) -> String {
    memo_response_formatting::format_delete_memo_response(result)
}
