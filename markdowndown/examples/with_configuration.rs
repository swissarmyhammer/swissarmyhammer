//! Configuration examples for the markdowndown library.
//!
//! This example demonstrates how to use custom configuration with the Config builder
//! pattern to customize authentication, timeouts, output format, and other options.

use markdowndown::{Config, MarkdownDown};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 markdowndown Configuration Examples\n");

    // Example 1: Basic custom configuration
    println!("1. Basic Custom Configuration");
    println!("   Setting timeout, user agent, and retry options...");

    let basic_config = Config::builder()
        .timeout_seconds(45)
        .user_agent("MarkdownDown-Example/1.0")
        .max_retries(2)
        .build();

    let md_basic = MarkdownDown::with_config(basic_config);

    // Test with a simple URL
    let test_url = "https://httpbin.org/html";
    match md_basic.convert_url(test_url).await {
        Ok(markdown) => {
            println!(
                "   ✅ Basic config successful ({} chars)",
                markdown.as_str().len()
            );
        }
        Err(e) => {
            println!("   ⚠️  Basic config failed: {e}");
        }
    }
    println!();

    // Example 2: GitHub-specific configuration
    println!("2. GitHub-Specific Configuration");
    println!(
        "   Note: Set GITHUB_TOKEN environment variable with your GitHub personal access token"
    );

    let github_config = Config::builder()
        .github_token(
            std::env::var("GITHUB_TOKEN").expect("Please set GITHUB_TOKEN environment variable"),
        )
        .timeout_seconds(60)
        .user_agent("MarkdownDown-GitHub-Example/1.0")
        .max_retries(3)
        .build();

    let md_github = MarkdownDown::with_config(github_config);

    // This would work with a real token
    let github_url = "https://github.com/rust-lang/rust/issues/1";
    match md_github.convert_url(github_url).await {
        Ok(markdown) => {
            println!(
                "   ✅ GitHub config successful ({} chars)",
                markdown.as_str().len()
            );
        }
        Err(e) => {
            println!("   ⚠️  GitHub config failed (expected without real token): {e}");
        }
    }
    println!();

    // Example 3: Output formatting configuration
    println!("3. Output Formatting Configuration");
    println!("   Customizing frontmatter and output options...");

    let output_config = Config::builder()
        .include_frontmatter(true)
        .custom_frontmatter_field("project", "markdown-examples")
        .custom_frontmatter_field("version", "1.0.0")
        .custom_frontmatter_field("processor", "markdowndown")
        .normalize_whitespace(true)
        .max_consecutive_blank_lines(1)
        .timeout_seconds(30)
        .build();

    let md_output = MarkdownDown::with_config(output_config);

    match md_output.convert_url("https://httpbin.org/html").await {
        Ok(markdown) => {
            println!("   ✅ Output config successful");

            // Show the frontmatter
            if let Some(frontmatter) = markdown.frontmatter() {
                println!("   📋 Generated frontmatter:");
                for line in frontmatter.lines().take(8) {
                    println!("      {line}");
                }
                if frontmatter.lines().count() > 8 {
                    println!("      ...");
                }
            }

            // Show content without frontmatter
            let content_only = markdown.content_only();
            println!("   📝 Content preview (without frontmatter):");
            for line in content_only.lines().take(3) {
                println!("      {line}");
            }
        }
        Err(e) => {
            println!("   ❌ Output config failed: {e}");
        }
    }
    println!();

    // Example 4: Environment-based configuration
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

    match md_env.convert_url("https://httpbin.org/html").await {
        Ok(markdown) => {
            println!(
                "   ✅ Environment config successful ({} chars)",
                markdown.as_str().len()
            );
        }
        Err(e) => {
            println!("   ⚠️  Environment config failed: {e}");
        }
    }
    println!();

    // Example 5: Configuration comparison
    println!("5. Configuration Feature Demonstration");
    println!("   Comparing different configurations side by side...");

    let configs = vec![
        ("Default", Config::default()),
        (
            "Fast & Minimal",
            Config::builder()
                .timeout_seconds(10)
                .max_retries(1)
                .include_frontmatter(false)
                .normalize_whitespace(false)
                .build(),
        ),
        (
            "Robust & Detailed",
            Config::builder()
                .timeout_seconds(120)
                .max_retries(5)
                .include_frontmatter(true)
                .custom_frontmatter_field("conversion_type", "robust")
                .normalize_whitespace(true)
                .max_consecutive_blank_lines(3)
                .build(),
        ),
    ];

    for (name, config) in configs {
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
        println!();
    }

    println!("✨ Configuration examples completed!");
    println!("\n💡 Tips:");
    println!("   - Use Config::builder() for fluent configuration");
    println!("   - Use Config::from_env() to load from environment variables");
    println!("   - Adjust timeout and retries based on your use case");
    println!("   - Add authentication tokens for better API access");

    Ok(())
}
