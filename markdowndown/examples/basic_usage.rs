//! Basic usage examples for the markdowndown library.
//!
//! This example demonstrates simple URL conversion for different types of URLs
//! using the default configuration.

use markdowndown::{convert_url, detect_url_type};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 markdowndown Basic Usage Examples\n");

    // Example URLs for different types
    let urls = [
        "https://blog.rust-lang.org/2024/01/15/Rust-1.75.0.html",
        "https://doc.rust-lang.org/book/ch01-00-getting-started.html",
        "https://docs.google.com/document/d/1ZzWTwAmWe0QE24qRV9_xL8B7q8i3rCtO2tVJx8VrIHs/edit",
        "https://github.com/rust-lang/rust/issues/100000",
    ];

    println!("Converting {} URLs to markdown...\n", urls.len());

    for (i, url) in urls.iter().enumerate() {
        println!("{}. Processing: {}", i + 1, url);

        // First, detect what type of URL this is
        match detect_url_type(url) {
            Ok(url_type) => {
                println!("   Type: {url_type}");
            }
            Err(e) => {
                eprintln!("   ❌ Failed to detect URL type: {e}");
                continue;
            }
        }

        // Convert the URL to markdown
        match convert_url(url).await {
            Ok(markdown) => {
                let content_length = markdown.as_str().len();
                let line_count = markdown.as_str().lines().count();

                println!("   ✅ Successfully converted!");
                println!("   📊 Content: {content_length} characters, {line_count} lines");

                // Show a preview of the content (first 200 chars)
                let preview = if content_length > 200 {
                    format!("{}...", &markdown.as_str()[..200])
                } else {
                    markdown.as_str().to_string()
                };

                println!("   📝 Preview:");
                for line in preview.lines().take(3) {
                    println!("      {line}");
                }

                // Check if it has frontmatter
                if let Some(frontmatter) = markdown.frontmatter() {
                    println!("   📋 Has YAML frontmatter ({} chars)", frontmatter.len());
                } else {
                    println!("   📋 No frontmatter");
                }
            }
            Err(e) => {
                eprintln!("   ❌ Failed to convert: {e}");

                // Show error suggestions if available
                let suggestions = e.suggestions();
                if !suggestions.is_empty() {
                    eprintln!("   💡 Suggestions:");
                    for suggestion in suggestions.iter().take(2) {
                        eprintln!("      - {suggestion}");
                    }
                }
            }
        }

        println!(); // Empty line for readability
    }

    println!("🎉 Basic usage examples completed!");
    Ok(())
}
