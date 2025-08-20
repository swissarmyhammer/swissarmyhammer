use anyhow::Result;
use colored::*;

use crate::cli::{OutputFormat, PromptSourceArg};



/// Run prompt search command with advanced search options
pub fn run_search_command(
    query: String,
    _fields: Option<Vec<String>>,
    _regex: bool,
    _fuzzy: bool,
    _case_sensitive: bool,
    _source_filter: Option<PromptSourceArg>,
    _has_arg: Option<String>,
    _no_args: bool,
    _full: bool,
    _format: OutputFormat,
    _highlight: bool,
    _limit: Option<usize>,
) -> Result<()> {
    // Simplified implementation for prompt search
    // This functionality is being migrated to use the dynamic CLI system
    println!("{}", "🔍 Searching prompts...".cyan());
    println!("Query: {}", query.bright_yellow());
    println!();
    println!(
        "{}",
        "Prompt search functionality is being updated to use the new dynamic CLI system.".yellow()
    );
    println!(
        "{}",
        "Please use 'sah prompt list' to see available prompts for now.".bright_blue()
    );
    
    Ok(())
}

// Removed display_search_results and display_table_format functions
// These will be reimplemented using the dynamic CLI system

/// Generate excerpt from content with optional highlighting
pub fn generate_excerpt(content: &str, query: &str, highlight: bool) -> Option<String> {
    let excerpt_length = 200;
    generate_excerpt_with_long_text(content, query, excerpt_length)
        .lines()
        .next()
        .map(|line| {
            if highlight && line.contains(query) {
                line.replace(query, &format!("{}", query.bright_yellow().bold()))
            } else {
                line.to_string()
            }
        })
}

/// Generate excerpt with configurable length
pub fn generate_excerpt_with_long_text(content: &str, query: &str, max_length: usize) -> String {
    let query_lower = query.to_lowercase();
    let content_lower = content.to_lowercase();

    if let Some(pos) = content_lower.find(&query_lower) {
        let start = if pos >= max_length / 2 {
            pos.saturating_sub(max_length / 2)
        } else {
            0
        };
        let end = (start + max_length).min(content.len());
        let excerpt = &content[start..end];

        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if end < content.len() { "..." } else { "" };

        format!("{prefix}{excerpt}{suffix}")
    } else {
        // If query not found, return beginning of content
        let end = max_length.min(content.len());
        let excerpt = &content[..end];
        if end < content.len() {
            format!("{excerpt}...")
        } else {
            excerpt.to_string()
        }
    }
}