//! Configuration examples for the markdowndown library.
//!
//! This example demonstrates how to use custom configuration with the Config builder
//! pattern to customize authentication, timeouts, output format, and other options.

use markdowndown::{Config, MarkdownDown};

// Configuration constants for example scenarios
const BASIC_TIMEOUT_SECONDS: u64 = 45;
const BASIC_MAX_RETRIES: u32 = 2;
const GITHUB_TIMEOUT_SECONDS: u64 = 60;
const GITHUB_MAX_RETRIES: u32 = 3;
const OUTPUT_TIMEOUT_SECONDS: u64 = 30;
const OUTPUT_MAX_BLANK_LINES: usize = 1;
const FRONTMATTER_PREVIEW_LINES: usize = 8;
const CONTENT_PREVIEW_LINES: usize = 3;
const FAST_TIMEOUT_SECONDS: u64 = 10;
const FAST_MAX_RETRIES: u32 = 1;
const ROBUST_TIMEOUT_SECONDS: u64 = 120;
const ROBUST_MAX_RETRIES: u32 = 5;
const ROBUST_MAX_BLANK_LINES: usize = 3;

/// Helper function to test URL conversion with consistent error handling
async fn test_conversion(
    md: &MarkdownDown,
    url: &str,
    context: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    match md.convert_url(url).await {
        Ok(markdown) => {
            let len = markdown.as_str().len();
            println!("   ✅ {context} successful ({len} chars)");
            Ok(len)
        }
        Err(e) => {
            println!("   ⚠️  {context} failed: {e}");
            Err(e.into())
        }
    }
}

/// Helper function to display configuration details
fn print_config_summary(name: &str, config: &Config) {
    println!("   🔧 {name} Configuration:");
    println!("      Timeout: {:?}", config.http.timeout);
    println!("      Max Retries: {}", config.http.max_retries);
    println!(
        "      Include Frontmatter: {}",
        config.output.include_frontmatter
    );
    println!(
        "      Custom Fields: {}",
        config.output.custom_frontmatter_fields.len()
    );
}

/// Example 1: Basic custom configuration
async fn example_basic_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Basic Custom Configuration");
    println!("   Setting timeout, user agent, and retry options...");

    let basic_config = Config::builder()
        .timeout_seconds(BASIC_TIMEOUT_SECONDS)
        .user_agent("MarkdownDown-Example/1.0")
        .max_retries(BASIC_MAX_RETRIES)
        .build();

    let md_basic = MarkdownDown::with_config(basic_config);

    // Test with a simple URL
    let test_url = "https://httpbin.org/html";
    test_conversion(&md_basic, test_url, "Basic config")
        .await
        .ok();
    println!();

    Ok(())
}

/// Example 2: GitHub-specific configuration
async fn example_github_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("2. GitHub-Specific Configuration");
    println!(
        "   Note: Set GITHUB_TOKEN environment variable with your GitHub personal access token"
    );

    let github_config = Config::builder()
        .github_token(
            std::env::var("GITHUB_TOKEN").expect("Please set GITHUB_TOKEN environment variable"),
        )
        .timeout_seconds(GITHUB_TIMEOUT_SECONDS)
        .user_agent("MarkdownDown-GitHub-Example/1.0")
        .max_retries(GITHUB_MAX_RETRIES)
        .build();

    let md_github = MarkdownDown::with_config(github_config);

    // This would work with a real token
    let github_url = "https://github.com/rust-lang/rust/issues/1";
    test_conversion(
        &md_github,
        github_url,
        "GitHub config (expected without real token)",
    )
    .await
    .ok();
    println!();

    Ok(())
}

/// Example 3: Output formatting configuration
async fn example_output_formatting() -> Result<(), Box<dyn std::error::Error>> {
    println!("3. Output Formatting Configuration");
    println!("   Customizing frontmatter and output options...");

    let output_config = Config::builder()
        .include_frontmatter(true)
        .custom_frontmatter_field("project", "markdown-examples")
        .custom_frontmatter_field("version", "1.0.0")
        .custom_frontmatter_field("processor", "markdowndown")
        .normalize_whitespace(true)
        .max_consecutive_blank_lines(OUTPUT_MAX_BLANK_LINES)
        .timeout_seconds(OUTPUT_TIMEOUT_SECONDS)
        .build();

    let md_output = MarkdownDown::with_config(output_config);

    match md_output.convert_url("https://httpbin.org/html").await {
        Ok(markdown) => {
            println!("   ✅ Output config successful");

            // Show the frontmatter
            if let Some(frontmatter) = markdown.frontmatter() {
                println!("   📋 Generated frontmatter:");
                for line in frontmatter.lines().take(FRONTMATTER_PREVIEW_LINES) {
                    println!("      {line}");
                }
                if frontmatter.lines().count() > FRONTMATTER_PREVIEW_LINES {
                    println!("      ...");
                }
            }

            // Show content without frontmatter
            let content_only = markdown.content_only();
            println!("   📝 Content preview (without frontmatter):");
            for line in content_only.lines().take(CONTENT_PREVIEW_LINES) {
                println!("      {line}");
            }
        }
        Err(e) => {
            println!("   ❌ Output config failed: {e}");
        }
    }
    println!();

    Ok(())
}

/// Example 4: Environment-based configuration
async fn example_env_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("4. Environment-based Configuration");
    println!("   Loading configuration from environment variables...");
    println!("   Set these environment variables to test:");
    println!("   - GITHUB_TOKEN=your_token");
    println!("   - MARKDOWNDOWN_TIMEOUT=60");
    println!("   - MARKDOWNDOWN_USER_AGENT=MyApp/1.0");
    println!("   - MARKDOWNDOWN_MAX_RETRIES=5");

    let env_config = Config::from_env();
    let md_env = MarkdownDown::with_config(env_config);

    // Show what configuration was loaded
    let config = md_env.config();
    println!("   📊 Loaded configuration:");
    println!("      Timeout: {:?}", config.http.timeout);
    println!("      User Agent: {}", config.http.user_agent);
    println!("      Max Retries: {}", config.http.max_retries);
    println!(
        "      GitHub Token: {}",
        if config.auth.github_token.is_some() {
            "configured"
        } else {
            "not set"
        }
    );

    test_conversion(&md_env, "https://httpbin.org/html", "Environment config")
        .await
        .ok();
    println!();

    Ok(())
}

/// Example 5: Configuration comparison
async fn example_config_comparison() -> Result<(), Box<dyn std::error::Error>> {
    println!("5. Configuration Feature Demonstration");
    println!("   Comparing different configurations side by side...");

    let configs = vec![
        ("Default", Config::default()),
        (
            "Fast & Minimal",
            Config::builder()
                .timeout_seconds(FAST_TIMEOUT_SECONDS)
                .max_retries(FAST_MAX_RETRIES)
                .include_frontmatter(false)
                .normalize_whitespace(false)
                .build(),
        ),
        (
            "Robust & Detailed",
            Config::builder()
                .timeout_seconds(ROBUST_TIMEOUT_SECONDS)
                .max_retries(ROBUST_MAX_RETRIES)
                .include_frontmatter(true)
                .custom_frontmatter_field("conversion_type", "robust")
                .normalize_whitespace(true)
                .max_consecutive_blank_lines(ROBUST_MAX_BLANK_LINES)
                .build(),
        ),
    ];

    for (name, config) in configs {
        print_config_summary(name, &config);
        println!();
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 markdowndown Configuration Examples\n");

    example_basic_config().await?;
    example_github_config().await?;
    example_output_formatting().await?;
    example_env_config().await?;
    example_config_comparison().await?;

    println!("✨ Configuration examples completed!");
    println!("\n💡 Tips:");
    println!("   - Use Config::builder() for fluent configuration");
    println!("   - Use Config::from_env() to load from environment variables");
    println!("   - Adjust timeout and retries based on your use case");
    println!("   - Add authentication tokens for better API access");

    Ok(())
}
