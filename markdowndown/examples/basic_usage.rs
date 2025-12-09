//! Basic usage examples for the markdowndown library.
//!
//! This example demonstrates simple URL conversion for different types of URLs
//! using the default configuration.

use markdowndown::types::{Markdown, MarkdownError};
use markdowndown::{convert_url, detect_url_type};

const PREVIEW_LENGTH: usize = 200;
const MAX_PREVIEW_LINES: usize = 3;
const MAX_ERROR_SUGGESTIONS: usize = 2;

/// Display the result of a successful URL conversion
fn display_conversion_result(markdown: &Markdown) {
    let content_length = markdown.as_str().len();
    let line_count = markdown.as_str().lines().count();

    println!("   ✅ Successfully converted!");
    println!("   📊 Content: {content_length} characters, {line_count} lines");

    let preview = if content_length > PREVIEW_LENGTH {
        format!("{}...", &markdown.as_str()[..PREVIEW_LENGTH])
    } else {
        markdown.as_str().to_string()
    };

    println!("   📝 Preview:");
    for line in preview.lines().take(MAX_PREVIEW_LINES) {
        println!("      {line}");
    }

    if let Some(frontmatter) = markdown.frontmatter() {
        println!("   📋 Has YAML frontmatter ({} chars)", frontmatter.len());
    } else {
        println!("   📋 No frontmatter");
    }
}

/// Display conversion error and suggestions
fn display_conversion_error(error: &MarkdownError) {
    eprintln!("   ❌ Failed to convert: {error}");

    let suggestions = error.suggestions();
    if !suggestions.is_empty() {
        eprintln!("   💡 Suggestions:");
        for suggestion in suggestions.iter().take(MAX_ERROR_SUGGESTIONS) {
            eprintln!("      - {suggestion}");
        }
    }
}

/// Process a single URL by detecting its type and converting it to markdown
async fn process_url(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    match detect_url_type(url) {
        Ok(url_type) => {
            println!("   Type: {url_type}");
        }
        Err(e) => {
            eprintln!("   ❌ Failed to detect URL type: {e}");
            return Ok(());
        }
    }

    match convert_url(url).await {
        Ok(markdown) => {
            display_conversion_result(&markdown);
        }
        Err(e) => {
            display_conversion_error(&e);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 markdowndown Basic Usage Examples\n");

    let urls = [
        "https://blog.rust-lang.org/2024/01/15/Rust-1.75.0.html",
        "https://doc.rust-lang.org/book/ch01-00-getting-started.html",
        "https://docs.google.com/document/d/1ZzWTwAmWe0QE24qRV9_xL8B7q8i3rCtO2tVJx8VrIHs/edit",
        "https://github.com/rust-lang/rust/issues/100000",
    ];

    println!("Converting {} URLs to markdown...\n", urls.len());

    for (i, url) in urls.iter().enumerate() {
        println!("{}. Processing: {}", i + 1, url);
        process_url(url).await?;
        println!();
    }

    println!("🎉 Basic usage examples completed!");
    Ok(())
}
