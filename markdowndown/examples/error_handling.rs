//! Comprehensive error handling examples for the markdowndown library.
//!
//! This example demonstrates all types of errors that can occur, how to handle them,
//! recovery strategies, and best practices for robust error handling.

use markdowndown::types::{
    AuthErrorKind, ContentErrorKind, MarkdownError, NetworkErrorKind, ValidationErrorKind,
};
use markdowndown::{convert_url, detect_url_type, Config, MarkdownDown};
use std::time::Duration;

// HTTP Status Code Constants
const HTTP_STATUS_UNAUTHORIZED: u16 = 401;
const HTTP_STATUS_FORBIDDEN: u16 = 403;
const HTTP_STATUS_NOT_FOUND: u16 = 404;
const HTTP_SERVER_ERROR_MIN: u16 = 500;
const HTTP_SERVER_ERROR_MAX: u16 = 503;

// Retry and Backoff Constants
const BACKOFF_BASE: u32 = 2;
const DEFAULT_MAX_RETRY_ATTEMPTS: usize = 3;
const DEFAULT_RETRY_BASE_DELAY_MS: u64 = 500;

// Configuration Constants
const PRIMARY_TIMEOUT_SECONDS: u64 = 10;
const PRIMARY_MAX_RETRIES: u32 = 2;
const FALLBACK_TIMEOUT_SECONDS: u64 = 30;

// Performance and Content Threshold Constants
const SLOW_RESPONSE_THRESHOLD_SECONDS: u64 = 5;
const LARGE_CONTENT_THRESHOLD_CHARS: usize = 100_000;

// Display Constants
const MAX_DISPLAYED_SUGGESTIONS: usize = 2;
const SUCCESS_PREFIX: &str = "✅ Success:";

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

/// Helper function to format success messages consistently
fn format_success_message(markdown: &str, prefix: &str) -> String {
    format!("{} {} characters", prefix, markdown.len())
}

/// Test case structure for URL testing
struct TestCase {
    description: &'static str,
    url: &'static str,
}

impl TestCase {
    const fn new(description: &'static str, url: &'static str) -> Self {
        Self { description, url }
    }
}

/// Error report structure for structured error handling
struct ErrorReport {
    category: String,
    details: Vec<String>,
    suggestions: Vec<String>,
}

impl ErrorReport {
    fn new(category: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            details: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }

    fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    fn with_timestamp(mut self, context: &markdowndown::types::ErrorContext) -> Self {
        self.details
            .push(format!("🕐 Error occurred at: {}", context.timestamp));
        self
    }
}

/// Classify server error status code into suggestion message
fn classify_server_error(status: u16) -> &'static str {
    match status {
        HTTP_SERVER_ERROR_MIN..=HTTP_SERVER_ERROR_MAX => "💡 Server issue, retry later",
        HTTP_STATUS_UNAUTHORIZED => "🔐 Authentication required",
        HTTP_STATUS_FORBIDDEN => "🚫 Access forbidden",
        HTTP_STATUS_NOT_FOUND => "📭 Resource not found",
        _ => "❓ Check server documentation",
    }
}

/// Build error report from category, optional context, and error-specific details
fn build_error_report<F>(
    category: String,
    context: Option<&markdowndown::types::ErrorContext>,
    build_fn: F,
) -> ErrorReport
where
    F: FnOnce(ErrorReport) -> ErrorReport,
{
    let report = ErrorReport::new(category);
    let report = if let Some(ctx) = context {
        report.with_timestamp(ctx)
    } else {
        report
    };
    build_fn(report)
}

/// Generic error handler function that uses a mapping function to extract details and suggestions
fn handle_error_with_mapping<K>(
    kind: &K,
    context: Option<&markdowndown::types::ErrorContext>,
    mapper: impl Fn(&K) -> (String, Vec<String>, Vec<String>),
) -> ErrorReport {
    let (category, details, suggestions) = mapper(kind);
    let mut report = build_error_report(category, context, |r| r);
    for detail in details {
        report = report.with_detail(detail);
    }
    for suggestion in suggestions {
        report = report.with_suggestion(suggestion);
    }
    report
}

/// Handle validation errors
fn handle_validation_error(
    kind: &ValidationErrorKind,
    context: &markdowndown::types::ErrorContext,
) -> ErrorReport {
    handle_error_with_mapping(kind, Some(context), |k| match k {
        ValidationErrorKind::InvalidUrl => (
            "🔗 Invalid URL detected".to_string(),
            vec![format!("📍 URL: {}", context.url)],
            vec!["🔧 Fix: Ensure URL starts with http:// or https://".to_string()],
        ),
        ValidationErrorKind::InvalidFormat => {
            ("📄 Invalid format detected".to_string(), vec![], vec![])
        }
        ValidationErrorKind::MissingParameter => {
            ("📝 Missing required parameter".to_string(), vec![], vec![])
        }
    })
}

/// Handle network errors
fn handle_network_error(
    kind: &NetworkErrorKind,
    context: &markdowndown::types::ErrorContext,
) -> ErrorReport {
    handle_error_with_mapping(kind, Some(context), |k| match k {
        NetworkErrorKind::Timeout => (
            "⏰ Network timeout".to_string(),
            vec![],
            vec!["💡 Consider increasing timeout or checking connection".to_string()],
        ),
        NetworkErrorKind::ConnectionFailed => (
            "🔌 Connection failed".to_string(),
            vec![],
            vec!["💡 Check network connectivity and firewall settings".to_string()],
        ),
        NetworkErrorKind::RateLimited => (
            "🐌 Rate limited (HTTP 429)".to_string(),
            vec![],
            vec!["💡 Wait before retrying or authenticate for higher limits".to_string()],
        ),
        NetworkErrorKind::ServerError(status) => (
            format!("🖥️  Server error: HTTP {status}"),
            vec![],
            vec![classify_server_error(*status).to_string()],
        ),
        NetworkErrorKind::DnsResolution => (
            "🌐 DNS resolution failed".to_string(),
            vec![],
            vec!["💡 Check domain name and DNS settings".to_string()],
        ),
    })
}

/// Handle authentication errors
fn handle_authentication_error(
    kind: &AuthErrorKind,
    context: &markdowndown::types::ErrorContext,
) -> ErrorReport {
    handle_error_with_mapping(kind, Some(context), |k| match k {
        AuthErrorKind::MissingToken => (
            "🔑 Missing authentication token".to_string(),
            vec![],
            vec![format!("💡 Set up API token for {}", context.url)],
        ),
        AuthErrorKind::InvalidToken => (
            "❌ Invalid authentication token".to_string(),
            vec![],
            vec!["💡 Check token format and regenerate if needed".to_string()],
        ),
        AuthErrorKind::PermissionDenied => (
            "🚫 Permission denied".to_string(),
            vec![],
            vec!["💡 Check token permissions and resource access".to_string()],
        ),
        AuthErrorKind::TokenExpired => (
            "⏰ Token expired".to_string(),
            vec![],
            vec!["💡 Refresh or regenerate authentication token".to_string()],
        ),
    })
}

/// Handle content errors
fn handle_content_error(kind: &ContentErrorKind) -> ErrorReport {
    handle_error_with_mapping(kind, None, |k| match k {
        ContentErrorKind::EmptyContent => (
            "📄 Empty content received".to_string(),
            vec![],
            vec!["💡 Verify URL contains actual content".to_string()],
        ),
        ContentErrorKind::UnsupportedFormat => (
            "📝 Unsupported content format".to_string(),
            vec![],
            vec!["💡 Try different converter or check content type".to_string()],
        ),
        ContentErrorKind::ParsingFailed => (
            "🔧 Content parsing failed".to_string(),
            vec![],
            vec!["💡 Content may be corrupted or malformed".to_string()],
        ),
    })
}

/// Handle legacy error types
fn handle_legacy_error(error: &MarkdownError) -> Option<ErrorReport> {
    match error {
        MarkdownError::NetworkError { message } => Some(ErrorReport::new(format!(
            "🌐 Network error (legacy): {message}"
        ))),
        MarkdownError::ParseError { message } => Some(ErrorReport::new(format!(
            "📄 Parse error (legacy): {message}"
        ))),
        MarkdownError::InvalidUrl { url } => {
            Some(ErrorReport::new(format!("🔗 Invalid URL (legacy): {url}")))
        }
        MarkdownError::AuthError { message } => Some(ErrorReport::new(format!(
            "🔐 Auth error (legacy): {message}"
        ))),
        _ => None,
    }
}

/// Handle enhanced error types
fn handle_enhanced_error(error: &MarkdownError) -> Option<ErrorReport> {
    match error {
        MarkdownError::ValidationError { kind, context } => {
            Some(handle_validation_error(kind, context))
        }
        MarkdownError::EnhancedNetworkError { kind, context } => {
            Some(handle_network_error(kind, context))
        }
        MarkdownError::AuthenticationError { kind, context } => {
            Some(handle_authentication_error(kind, context))
        }
        MarkdownError::ContentError { kind, context: _ } => Some(handle_content_error(kind)),
        _ => None,
    }
}

/// Helper function to handle error patterns and return structured report
fn handle_error_pattern(error: &MarkdownError) -> ErrorReport {
    if let Some(report) = handle_enhanced_error(error) {
        return report;
    }

    if let Some(report) = handle_legacy_error(error) {
        return report;
    }

    ErrorReport::new(format!("❓ Other error: {error}"))
}

/// Helper function to print error report
fn print_error_report(report: &ErrorReport) {
    println!("      {}", report.category);
    for detail in &report.details {
        println!("         {detail}");
    }
    for suggestion in &report.suggestions {
        println!("         {suggestion}");
    }
}

/// Helper function to convert with config
async fn try_conversion_with_config(url: &str, config: Config) -> Result<String, MarkdownError> {
    let md = MarkdownDown::with_config(config);
    let markdown = md.convert_url(url).await?;
    Ok(markdown.into())
}

/// Metrics logging structure
struct ConversionMetrics {
    request_id: String,
    duration: Duration,
    result: Result<usize, String>,
}

/// Check performance thresholds and log warnings
fn check_performance_thresholds(request_id: &str, duration: Duration, char_count: usize) {
    if duration > Duration::from_secs(SLOW_RESPONSE_THRESHOLD_SECONDS) {
        println!(
            "   ⚠️  [{}] SLOW_RESPONSE: {:?} exceeds {}s threshold",
            request_id, duration, SLOW_RESPONSE_THRESHOLD_SECONDS
        );
    }

    if char_count > LARGE_CONTENT_THRESHOLD_CHARS {
        println!(
            "   📈 [{}] LARGE_CONTENT: {} chars exceeds {} threshold",
            request_id, char_count, LARGE_CONTENT_THRESHOLD_CHARS
        );
    }
}

/// Helper function to log conversion metrics
fn log_metrics(metrics: &ConversionMetrics) {
    match &metrics.result {
        Ok(char_count) => {
            println!(
                "   ✅ [{}] SUCCESS in {:?}: {} chars",
                metrics.request_id, metrics.duration, char_count
            );
            check_performance_thresholds(&metrics.request_id, metrics.duration, *char_count);
        }
        Err(error_msg) => {
            println!(
                "   ❌ [{}] {} in {:?}",
                metrics.request_id, error_msg, metrics.duration
            );
        }
    }
}

/// Helper function to log conversion result with metrics
fn log_conversion_result(
    request_id: &str,
    duration: Duration,
    result: Result<&str, &MarkdownError>,
) {
    let metrics = match result {
        Ok(markdown) => ConversionMetrics {
            request_id: request_id.to_string(),
            duration,
            result: Ok(markdown.len()),
        },
        Err(e) => ConversionMetrics {
            request_id: request_id.to_string(),
            duration,
            result: Err(classify_error(e).to_string()),
        },
    };
    log_metrics(&metrics);
}

/// Generic result processor that handles success and error cases
fn process_result<F>(result: Result<markdowndown::types::Markdown, MarkdownError>, error_handler: F)
where
    F: FnOnce(&MarkdownError),
{
    match result {
        Ok(markdown) => {
            println!(
                "      {}",
                format_success_message(markdown.as_str(), SUCCESS_PREFIX)
            );
        }
        Err(e) => error_handler(&e),
    }
}

/// Process a single URL test result with basic error analysis
fn process_test_result(result: Result<markdowndown::types::Markdown, MarkdownError>) {
    process_result(result, |e| {
        let analysis = analyze_error(e);
        println!("      ❌ Failed: {e} ({analysis})");

        let suggestions = e.suggestions();
        if !suggestions.is_empty() {
            println!("      💡 Suggestions:");
            for suggestion in suggestions.iter().take(MAX_DISPLAYED_SUGGESTIONS) {
                println!("         - {suggestion}");
            }
        }
    });
}

/// Generic test case executor
async fn execute_test_cases<'a, F, Fut>(
    test_emoji: &str,
    test_cases: Vec<(&'a str, &'a str)>,
    processor: F,
) where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    for (description, url) in test_cases {
        println!("   {test_emoji} Testing {description}: {url}");
        processor(url.to_string()).await;
        println!();
    }
}

/// Test URLs and display basic error analysis
async fn test_urls_with_analysis(urls: Vec<(&str, &str)>) {
    execute_test_cases("🧪", urls, |url| async move {
        let result = convert_url(&url).await;
        process_test_result(result);
    })
    .await;
}

/// Calculate exponential backoff delay for retry attempts
fn calculate_backoff_delay(base_delay: Duration, attempt: usize) -> Duration {
    base_delay * (BACKOFF_BASE.pow(attempt as u32 - 1))
}

/// Check if error should be retried
fn should_retry_error(error: &MarkdownError, attempt: usize, max_attempts: usize) -> bool {
    attempt < max_attempts && error.is_retryable()
}

/// Execute a single retry attempt and return the result
async fn execute_retry_attempt<F, Fut, T>(
    operation: &F,
    _attempt: usize,
) -> Result<T, MarkdownError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, MarkdownError>>,
{
    operation().await
}

/// Handle a retry failure by logging and determining if another retry should occur
async fn handle_retry_failure(
    error: MarkdownError,
    attempt: usize,
    max_attempts: usize,
    base_delay: Duration,
) -> Result<(), MarkdownError> {
    println!("      🔄 Attempt {attempt} failed: {error}");

    if !should_retry_error(&error, attempt, max_attempts) {
        return Err(error);
    }

    let delay = calculate_backoff_delay(base_delay, attempt);
    println!("      ⏳ Waiting {delay:?} before retry...");
    tokio::time::sleep(delay).await;
    Ok(())
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
    for attempt in 1..=max_attempts {
        match execute_retry_attempt(&operation, attempt).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                handle_retry_failure(e, attempt, max_attempts, base_delay).await?;
            }
        }
    }

    unreachable!("Loop should always return or error before reaching here")
}

/// Demonstrate basic error types and classification
async fn demonstrate_error_types() {
    println!("1. Error Types and Classification");
    println!("   Demonstrating different error types and their characteristics...");

    const ERROR_TYPE_TEST_CASES: &[TestCase] = &[
        TestCase::new("Invalid URL", "not-a-valid-url"),
        TestCase::new(
            "Non-existent domain",
            "https://this-domain-definitely-does-not-exist-12345.invalid",
        ),
        TestCase::new("HTTP 404", "https://httpbin.org/status/404"),
        TestCase::new("HTTP 500", "https://httpbin.org/status/500"),
        TestCase::new("Slow response", "https://httpbin.org/delay/5"),
        TestCase::new("Valid URL", "https://httpbin.org/html"),
    ];

    let test_cases: Vec<(&str, &str)> = ERROR_TYPE_TEST_CASES
        .iter()
        .map(|tc| (tc.description, tc.url))
        .collect();

    test_urls_with_analysis(test_cases).await;
}

/// Process URL result with pattern matching
fn process_pattern_result(result: Result<markdowndown::types::Markdown, MarkdownError>) {
    process_result(result, |error| {
        let report = handle_error_pattern(error);
        print_error_report(&report);
    });
}

/// Demonstrate enhanced error handling with pattern matching
async fn demonstrate_pattern_matching() {
    println!("2. Pattern Matching Error Handling");
    println!("   Demonstrating specific error handling strategies...");

    const PATTERN_TEST_CASES: &[TestCase] = &[
        TestCase::new("Invalid URL", "invalid-url"),
        TestCase::new("Unauthorized", "https://httpbin.org/status/401"),
        TestCase::new("Rate Limited", "https://httpbin.org/status/429"),
        TestCase::new("Server Error", "https://httpbin.org/status/503"),
    ];

    let error_test_urls: Vec<(&str, &str)> = PATTERN_TEST_CASES
        .iter()
        .map(|tc| (tc.description, tc.url))
        .collect();

    execute_test_cases("🎯", error_test_urls, |url| async move {
        let result = convert_url(&url).await;
        process_pattern_result(result);
    })
    .await;
}

/// Demonstrate retry strategies and recovery
async fn demonstrate_retry_strategies() {
    println!("3. Retry Strategies and Recovery");
    println!("   Demonstrating intelligent retry logic...");

    const RETRY_TEST_CASES: &[TestCase] = &[
        TestCase::new("Timeout simulation", "https://httpbin.org/delay/2"),
        TestCase::new("Server error simulation", "https://httpbin.org/status/503"),
        TestCase::new(
            "Non-retryable error",
            "https://invalid-domain-for-testing.invalid",
        ),
    ];

    let retry_urls: Vec<(&str, &str)> = RETRY_TEST_CASES
        .iter()
        .map(|tc| (tc.description, tc.url))
        .collect();

    for (description, url) in retry_urls {
        println!("   🔄 Testing retry strategy for {description}: {url}");

        let result = retry_with_backoff(
            || convert_url(url),
            DEFAULT_MAX_RETRY_ATTEMPTS,
            Duration::from_millis(DEFAULT_RETRY_BASE_DELAY_MS),
        )
        .await;

        match result {
            Ok(markdown) => {
                println!(
                    "      {}",
                    format_success_message(markdown.as_str(), SUCCESS_PREFIX)
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
}

/// Configuration type for conversion strategies
enum ConfigType {
    Primary,
    Fallback,
}

/// Create configuration based on type
fn create_config(config_type: ConfigType) -> Config {
    match config_type {
        ConfigType::Primary => Config::builder()
            .timeout_seconds(PRIMARY_TIMEOUT_SECONDS)
            .max_retries(PRIMARY_MAX_RETRIES)
            .build(),
        ConfigType::Fallback => Config::builder()
            .timeout_seconds(FALLBACK_TIMEOUT_SECONDS)
            .max_retries(1)
            .build(),
    }
}

/// Try conversion with specified configuration type
async fn try_conversion_with_standard_config(
    url: &str,
    config_type: ConfigType,
) -> Result<String, MarkdownError> {
    if matches!(config_type, ConfigType::Fallback) {
        println!("      🔄 Trying fallback configuration...");
    }
    try_conversion_with_config(url, create_config(config_type)).await
}

/// Try URL detection as last resort
fn try_url_detection(url: &str) -> Result<String, String> {
    println!("      🔍 Trying URL detection as last resort...");
    match detect_url_type(url) {
        Ok(url_type) => Ok(format!("📋 Could only detect URL type: {url_type}")),
        Err(detection_error) => Err(format!(
            "❌ All fallbacks failed. Last error: {detection_error}"
        )),
    }
}

/// Try primary conversion with standard configuration
async fn try_primary_conversion(url: &str) -> Result<String, MarkdownError> {
    let markdown = try_conversion_with_standard_config(url, ConfigType::Primary).await?;
    Ok(format_success_message(
        &markdown,
        "✅ Primary conversion successful:",
    ))
}

/// Try fallback conversion with relaxed configuration
async fn try_fallback_conversion(url: &str) -> Result<String, String> {
    println!("      🔄 Trying fallback configuration...");
    match try_conversion_with_standard_config(url, ConfigType::Fallback).await {
        Ok(markdown) => Ok(format_success_message(
            &markdown,
            "⚡ Fallback conversion successful:",
        )),
        Err(_) => {
            println!("      🔸 Fallback also failed");
            try_url_detection(url)
        }
    }
}

/// Try detection fallback when conversions fail
fn try_detection_fallback(url: &str) -> Result<String, String> {
    println!("      🔸 Primary and fallback conversions failed");
    try_url_detection(url)
}

/// Convert with fallback strategies using chain of responsibility pattern
async fn convert_with_fallbacks(url: &str) -> Result<String, String> {
    match try_primary_conversion(url).await {
        Ok(result) => Ok(result),
        Err(e) => {
            println!("      🔸 Primary conversion failed: {e}");
            if !e.is_recoverable() {
                return try_detection_fallback(url);
            }
            try_fallback_conversion(url).await
        }
    }
}

/// Demonstrate graceful degradation and fallbacks
async fn demonstrate_fallback_strategies() {
    println!("4. Graceful Degradation and Fallbacks");
    println!("   Implementing fallback strategies for robust applications...");

    const FALLBACK_TEST_CASES: &[TestCase] = &[
        TestCase::new("Valid URL", "https://httpbin.org/html"),
        TestCase::new("Server error", "https://httpbin.org/status/503"),
        TestCase::new(
            "Invalid URL",
            "https://invalid-url-for-fallback-test.invalid",
        ),
    ];

    for test_case in FALLBACK_TEST_CASES {
        println!("   🛡️  Testing fallback strategy for: {}", test_case.url);
        match convert_with_fallbacks(test_case.url).await {
            Ok(result) => println!("      {result}"),
            Err(error) => println!("      {error}"),
        }
        println!();
    }
}

/// Classify server error status into monitoring category
fn classify_server_error_status(status: u16) -> &'static str {
    if status >= HTTP_SERVER_ERROR_MIN {
        "SERVER_ERROR"
    } else {
        "CLIENT_ERROR"
    }
}

/// Trait for error classification in monitoring systems
trait ErrorClassification {
    fn monitoring_category(&self) -> &'static str;
}

impl ErrorClassification for NetworkErrorKind {
    fn monitoring_category(&self) -> &'static str {
        match self {
            NetworkErrorKind::Timeout => "NETWORK_TIMEOUT",
            NetworkErrorKind::ConnectionFailed => "CONNECTION_ERROR",
            NetworkErrorKind::RateLimited => "RATE_LIMITED",
            NetworkErrorKind::ServerError(status) => classify_server_error_status(*status),
            NetworkErrorKind::DnsResolution => "DNS_ERROR",
        }
    }
}

impl ErrorClassification for MarkdownError {
    fn monitoring_category(&self) -> &'static str {
        match self {
            MarkdownError::ValidationError { .. } => "VALIDATION_ERROR",
            MarkdownError::EnhancedNetworkError { kind, .. } => kind.monitoring_category(),
            MarkdownError::AuthenticationError { .. } => "AUTH_ERROR",
            MarkdownError::ContentError { .. } => "CONTENT_ERROR",
            _ => "OTHER_ERROR",
        }
    }
}

/// Classify error type for monitoring
fn classify_error(error: &MarkdownError) -> &'static str {
    error.monitoring_category()
}

/// Log error context for debugging
fn log_error_context(error: &MarkdownError, request_id: &str) {
    if let Some(context) = error.context() {
        println!(
            "   🔍 [{}] CONTEXT: operation={}, converter={}",
            request_id, context.operation, context.converter_type
        );
        if let Some(info) = &context.additional_info {
            println!("   📝 [{request_id}] ADDITIONAL_INFO: {info}");
        }
    }
}

/// Check if status code is in server error range
fn is_server_error_status(status: u16) -> bool {
    (HTTP_SERVER_ERROR_MIN..=HTTP_SERVER_ERROR_MAX).contains(&status)
}

/// Check if network error should trigger alert
fn is_alertable_network_error(kind: &NetworkErrorKind) -> bool {
    matches!(kind, NetworkErrorKind::ServerError(status) if is_server_error_status(*status))
}

/// Check if content error should trigger alert
fn is_alertable_content_error(kind: &ContentErrorKind) -> bool {
    matches!(kind, ContentErrorKind::ParsingFailed)
}

/// Determine if error should trigger monitoring alert
fn should_trigger_alert(error: &MarkdownError) -> bool {
    match error {
        MarkdownError::EnhancedNetworkError { kind, .. } => is_alertable_network_error(kind),
        MarkdownError::ContentError { kind, .. } => is_alertable_content_error(kind),
        _ => false,
    }
}

/// Log conversion error with classification and context
fn log_conversion_error(request_id: &str, error: &MarkdownError, duration: Duration) {
    let error_category = classify_error(error);

    let metrics = ConversionMetrics {
        request_id: request_id.to_string(),
        duration,
        result: Err(format!("{error_category} in {:?}: {error}", duration)),
    };
    log_metrics(&metrics);

    log_error_context(error, request_id);

    if should_trigger_alert(error) {
        println!(
            "   🚨 [{request_id}] ALERT_WORTHY: This error type should trigger monitoring alerts"
        );
    }
}

/// Convert URL with comprehensive monitoring and logging
async fn convert_with_monitoring(url: &str, request_id: &str) -> Result<(), MarkdownError> {
    let start_time = std::time::Instant::now();

    println!("   📊 [{request_id}] Starting conversion for: {url}");

    let result = convert_url(url).await;
    let duration = start_time.elapsed();

    match &result {
        Ok(markdown) => {
            log_conversion_result(request_id, duration, Ok(markdown.as_str()));
            Ok(())
        }
        Err(e) => {
            log_conversion_error(request_id, e, duration);
            result.map(|_| ())
        }
    }
}

/// Demonstrate error logging and monitoring patterns
async fn demonstrate_error_monitoring() {
    println!("5. Error Logging and Monitoring");
    println!("   Best practices for error logging and monitoring...");

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚨 markdowndown Error Handling Examples\n");

    demonstrate_error_types().await;
    demonstrate_pattern_matching().await;
    demonstrate_retry_strategies().await;
    demonstrate_fallback_strategies().await;
    demonstrate_error_monitoring().await;

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
