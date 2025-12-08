//! Comprehensive error handling examples for the markdowndown library.
//!
//! This example demonstrates all types of errors that can occur, how to handle them,
//! recovery strategies, and best practices for robust error handling.

use markdowndown::types::{
    AuthErrorKind, ContentErrorKind, MarkdownError, NetworkErrorKind, ValidationErrorKind,
};
use markdowndown::{convert_url, detect_url_type, Config, MarkdownDown};
use std::time::Duration;

/// Helper function to demonstrate error analysis
fn analyze_error(error: &MarkdownError) -> String {
    let mut analysis = Vec::new();

    // Check error characteristics
    if error.is_retryable() {
        analysis.push("retryable".to_string());
    }
    if error.is_recoverable() {
        analysis.push("recoverable".to_string());
    }

    // Add context if available
    if let Some(context) = error.context() {
        analysis.push(format!("context: {}", context.operation));
    }

    if analysis.is_empty() {
        "permanent failure".to_string()
    } else {
        analysis.join(", ")
    }
}

/// Helper function to demonstrate retry logic
async fn retry_with_backoff<F, Fut, T>(
    operation: F,
    max_attempts: usize,
    base_delay: Duration,
) -> Result<T, MarkdownError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, MarkdownError>>,
{
    let mut last_error = None;

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                println!("      🔄 Attempt {attempt} failed: {e}");

                if attempt < max_attempts && e.is_retryable() {
                    let delay = base_delay * (2_u32.pow(attempt as u32 - 1)); // Exponential backoff
                    println!("      ⏳ Waiting {delay:?} before retry...");
                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(last_error.unwrap())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚨 markdowndown Error Handling Examples\n");

    // Example 1: Basic error types and classification
    println!("1. Error Types and Classification");
    println!("   Demonstrating different error types and their characteristics...");

    let test_cases = vec![
        ("Invalid URL", "not-a-valid-url"),
        (
            "Non-existent domain",
            "https://this-domain-definitely-does-not-exist-12345.invalid",
        ),
        ("HTTP 404", "https://httpbin.org/status/404"),
        ("HTTP 500", "https://httpbin.org/status/500"),
        ("Slow response", "https://httpbin.org/delay/5"),
        ("Valid URL", "https://httpbin.org/html"),
    ];

    for (description, url) in test_cases {
        println!("   🧪 Testing {description}: {url}");

        match convert_url(url).await {
            Ok(markdown) => {
                println!("      ✅ Success: {} characters", markdown.as_str().len());
            }
            Err(e) => {
                let analysis = analyze_error(&e);
                println!("      ❌ Failed: {e} ({analysis})");

                // Show error suggestions
                let suggestions = e.suggestions();
                if !suggestions.is_empty() {
                    println!("      💡 Suggestions:");
                    for suggestion in suggestions.iter().take(2) {
                        println!("         - {suggestion}");
                    }
                }
            }
        }
        println!();
    }

    // Example 2: Enhanced error handling with pattern matching
    println!("2. Pattern Matching Error Handling");
    println!("   Demonstrating specific error handling strategies...");

    let error_test_urls = vec![
        "invalid-url",
        "https://httpbin.org/status/401",
        "https://httpbin.org/status/429",
        "https://httpbin.org/status/503",
    ];

    for url in error_test_urls {
        println!("   🎯 Testing error patterns for: {url}");

        match convert_url(url).await {
            Ok(markdown) => {
                println!(
                    "      ✅ Unexpected success: {} chars",
                    markdown.as_str().len()
                );
            }
            Err(error) => {
                match error {
                    MarkdownError::ValidationError { kind, context } => match kind {
                        ValidationErrorKind::InvalidUrl => {
                            println!("      🔗 Invalid URL detected");
                            println!("         📍 URL: {}", context.url);
                            println!("         🔧 Fix: Ensure URL starts with http:// or https://");
                        }
                        ValidationErrorKind::InvalidFormat => {
                            println!("      📄 Invalid format detected");
                        }
                        ValidationErrorKind::MissingParameter => {
                            println!("      📝 Missing required parameter");
                        }
                    },
                    MarkdownError::EnhancedNetworkError { kind, context } => {
                        match kind {
                            NetworkErrorKind::Timeout => {
                                println!("      ⏰ Network timeout");
                                println!("         💡 Consider increasing timeout or checking connection");
                            }
                            NetworkErrorKind::ConnectionFailed => {
                                println!("      🔌 Connection failed");
                                println!(
                                    "         💡 Check network connectivity and firewall settings"
                                );
                            }
                            NetworkErrorKind::RateLimited => {
                                println!("      🐌 Rate limited (HTTP 429)");
                                println!("         💡 Wait before retrying or authenticate for higher limits");
                            }
                            NetworkErrorKind::ServerError(status) => {
                                println!("      🖥️  Server error: HTTP {status}");
                                match status {
                                    500..=503 => println!("         💡 Server issue, retry later"),
                                    401 => println!("         🔐 Authentication required"),
                                    403 => println!("         🚫 Access forbidden"),
                                    404 => println!("         📭 Resource not found"),
                                    _ => println!("         ❓ Check server documentation"),
                                }
                            }
                            NetworkErrorKind::DnsResolution => {
                                println!("      🌐 DNS resolution failed");
                                println!("         💡 Check domain name and DNS settings");
                            }
                        }
                        println!("         🕐 Error occurred at: {}", context.timestamp);
                    }
                    MarkdownError::AuthenticationError { kind, context } => match kind {
                        AuthErrorKind::MissingToken => {
                            println!("      🔑 Missing authentication token");
                            println!("         💡 Set up API token for {}", context.url);
                        }
                        AuthErrorKind::InvalidToken => {
                            println!("      ❌ Invalid authentication token");
                            println!("         💡 Check token format and regenerate if needed");
                        }
                        AuthErrorKind::PermissionDenied => {
                            println!("      🚫 Permission denied");
                            println!("         💡 Check token permissions and resource access");
                        }
                        AuthErrorKind::TokenExpired => {
                            println!("      ⏰ Token expired");
                            println!("         💡 Refresh or regenerate authentication token");
                        }
                    },
                    MarkdownError::ContentError { kind, context: _ } => match kind {
                        ContentErrorKind::EmptyContent => {
                            println!("      📄 Empty content received");
                            println!("         💡 Verify URL contains actual content");
                        }
                        ContentErrorKind::UnsupportedFormat => {
                            println!("      📝 Unsupported content format");
                            println!("         💡 Try different converter or check content type");
                        }
                        ContentErrorKind::ParsingFailed => {
                            println!("      🔧 Content parsing failed");
                            println!("         💡 Content may be corrupted or malformed");
                        }
                    },
                    // Legacy error types
                    MarkdownError::NetworkError { message } => {
                        println!("      🌐 Network error (legacy): {message}");
                    }
                    MarkdownError::ParseError { message } => {
                        println!("      📄 Parse error (legacy): {message}");
                    }
                    MarkdownError::InvalidUrl { url } => {
                        println!("      🔗 Invalid URL (legacy): {url}");
                    }
                    MarkdownError::AuthError { message } => {
                        println!("      🔐 Auth error (legacy): {message}");
                    }
                    _ => {
                        println!("      ❓ Other error: {error}");
                    }
                }
            }
        }
        println!();
    }

    // Example 3: Retry strategies and recovery
    println!("3. Retry Strategies and Recovery");
    println!("   Demonstrating intelligent retry logic...");

    // Test retry with different types of failures
    let retry_urls = vec![
        ("Timeout simulation", "https://httpbin.org/delay/2"),
        ("Server error simulation", "https://httpbin.org/status/503"),
        (
            "Non-retryable error",
            "https://invalid-domain-for-testing.invalid",
        ),
    ];

    for (description, url) in retry_urls {
        println!("   🔄 Testing retry strategy for {description}: {url}");

        let result = retry_with_backoff(
            || convert_url(url),
            3,                          // max attempts
            Duration::from_millis(500), // base delay
        )
        .await;

        match result {
            Ok(markdown) => {
                println!(
                    "      ✅ Succeeded after retries: {} chars",
                    markdown.as_str().len()
                );
            }
            Err(e) => {
                println!("      ❌ Failed after all retries: {e}");
                if e.is_recoverable() {
                    println!("      🔄 Error is recoverable - could try alternative approach");
                } else {
                    println!("      🛑 Error is not recoverable - permanent failure");
                }
            }
        }
        println!();
    }

    // Example 4: Graceful degradation and fallbacks
    println!("4. Graceful Degradation and Fallbacks");
    println!("   Implementing fallback strategies for robust applications...");

    async fn convert_with_fallbacks(url: &str) -> Result<String, String> {
        // Primary: Try with custom configuration
        let primary_config = Config::builder().timeout_seconds(10).max_retries(2).build();
        let md_primary = MarkdownDown::with_config(primary_config);

        match md_primary.convert_url(url).await {
            Ok(markdown) => Ok(format!(
                "✅ Primary conversion successful: {} chars",
                markdown.as_str().len()
            )),
            Err(e) => {
                println!("      🔸 Primary conversion failed: {e}");

                // Fallback 1: Try with more lenient configuration
                if e.is_recoverable() {
                    println!("      🔄 Trying fallback configuration...");
                    let fallback_config =
                        Config::builder().timeout_seconds(30).max_retries(1).build();
                    let md_fallback = MarkdownDown::with_config(fallback_config);

                    match md_fallback.convert_url(url).await {
                        Ok(markdown) => {
                            return Ok(format!(
                                "⚡ Fallback conversion successful: {} chars",
                                markdown.as_str().len()
                            ));
                        }
                        Err(fallback_error) => {
                            println!("      🔸 Fallback also failed: {fallback_error}");
                        }
                    }
                }

                // Fallback 2: Try URL type detection only
                println!("      🔍 Trying URL detection as last resort...");
                match detect_url_type(url) {
                    Ok(url_type) => Ok(format!("📋 Could only detect URL type: {url_type}")),
                    Err(detection_error) => Err(format!(
                        "❌ All fallbacks failed. Last error: {detection_error}"
                    )),
                }
            }
        }
    }

    let fallback_test_urls = vec![
        "https://httpbin.org/html",
        "https://httpbin.org/status/503",
        "https://invalid-url-for-fallback-test.invalid",
    ];

    for url in fallback_test_urls {
        println!("   🛡️  Testing fallback strategy for: {url}");
        match convert_with_fallbacks(url).await {
            Ok(result) => println!("      {result}"),
            Err(error) => println!("      {error}"),
        }
        println!();
    }

    // Example 5: Error logging and monitoring patterns
    println!("5. Error Logging and Monitoring");
    println!("   Best practices for error logging and monitoring...");

    // Custom error handler with detailed logging
    async fn convert_with_monitoring(url: &str, request_id: &str) -> Result<(), MarkdownError> {
        let start_time = std::time::Instant::now();

        println!("   📊 [{request_id}] Starting conversion for: {url}");

        match convert_url(url).await {
            Ok(markdown) => {
                let duration = start_time.elapsed();
                let char_count = markdown.as_str().len();

                // Success metrics
                println!("   ✅ [{request_id}] SUCCESS in {duration:?}: {char_count} chars");

                // Log performance metrics
                if duration > Duration::from_secs(5) {
                    println!(
                        "   ⚠️  [{request_id}] SLOW_RESPONSE: {duration:?} exceeds 5s threshold"
                    );
                }

                if char_count > 100_000 {
                    println!("   📈 [{request_id}] LARGE_CONTENT: {char_count} chars exceeds 100k threshold");
                }

                Ok(())
            }
            Err(e) => {
                let duration = start_time.elapsed();

                // Error classification for monitoring
                let error_category = match &e {
                    MarkdownError::ValidationError { .. } => "VALIDATION_ERROR",
                    MarkdownError::EnhancedNetworkError { kind, .. } => match kind {
                        NetworkErrorKind::Timeout => "NETWORK_TIMEOUT",
                        NetworkErrorKind::ConnectionFailed => "CONNECTION_ERROR",
                        NetworkErrorKind::RateLimited => "RATE_LIMITED",
                        NetworkErrorKind::ServerError(status) => {
                            if *status >= 500 {
                                "SERVER_ERROR"
                            } else {
                                "CLIENT_ERROR"
                            }
                        }
                        NetworkErrorKind::DnsResolution => "DNS_ERROR",
                    },
                    MarkdownError::AuthenticationError { .. } => "AUTH_ERROR",
                    MarkdownError::ContentError { .. } => "CONTENT_ERROR",
                    _ => "OTHER_ERROR",
                };

                println!("   ❌ [{request_id}] {error_category} in {duration:?}: {e}");

                // Log error context for debugging
                if let Some(context) = e.context() {
                    println!(
                        "   🔍 [{}] CONTEXT: operation={}, converter={}",
                        request_id, context.operation, context.converter_type
                    );
                    if let Some(info) = &context.additional_info {
                        println!("   📝 [{request_id}] ADDITIONAL_INFO: {info}");
                    }
                }

                // Determine if this should trigger alerts
                let should_alert = match &e {
                    MarkdownError::EnhancedNetworkError { kind, .. } => {
                        matches!(kind, NetworkErrorKind::ServerError(500..=503))
                    }
                    MarkdownError::ContentError { kind, .. } => {
                        matches!(kind, ContentErrorKind::ParsingFailed)
                    }
                    _ => false,
                };

                if should_alert {
                    println!("   🚨 [{request_id}] ALERT_WORTHY: This error type should trigger monitoring alerts");
                }

                Err(e)
            }
        }
    }

    // Test monitoring with different URLs
    let monitoring_urls = [
        "https://httpbin.org/html",
        "https://httpbin.org/status/500",
        "invalid-url-for-monitoring",
    ];

    for (i, url) in monitoring_urls.iter().enumerate() {
        let request_id = format!("REQ_{:03}", i + 1);
        let _ = convert_with_monitoring(url, &request_id).await;
        println!();
    }

    println!("🎯 Error Handling Summary:");
    println!("   • Always check if errors are retryable or recoverable");
    println!("   • Use pattern matching for specific error handling");
    println!("   • Implement exponential backoff for retries");
    println!("   • Design fallback strategies for critical applications");
    println!("   • Log errors with context for debugging and monitoring");
    println!("   • Use error characteristics to determine alert priorities");

    println!("\n🚀 Error handling examples completed!");
    Ok(())
}
