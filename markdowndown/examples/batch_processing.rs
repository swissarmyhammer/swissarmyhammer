//! Batch processing examples for the markdowndown library.
//!
//! This example demonstrates how to process multiple URLs efficiently
//! with proper error handling, parallel processing, and result aggregation.

use markdowndown::{types::Markdown, types::MarkdownError, Config, MarkdownDown};
use std::time::{Duration, Instant};
use tokio::time::timeout;

// Type aliases for complex result types
type ConversionResult = Result<(String, Markdown, Duration), (String, MarkdownError, Duration)>;
type ConversionResults = Vec<ConversionResult>;

// Configuration constants
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_MAX_RETRIES: u32 = 2;
const MAX_CONCURRENT_REQUESTS: usize = 3;
const PER_URL_TIMEOUT_SECONDS: u64 = 10;
const MAX_RETRY_ATTEMPTS: u32 = 3;
const RETRY_BASE_DELAY_MS: u64 = 1000;
const PERCENTAGE_MULTIPLIER: f32 = 100.0;

/// Log a processing result with consistent formatting
fn log_result(i: usize, total: usize, url: &str, result: &ConversionResult) {
    match result {
        Ok((_, markdown, duration)) => {
            println!(
                "   [{}/{}] ✅ {}: {} chars in {:?}",
                i + 1,
                total,
                url,
                markdown.as_str().len(),
                duration
            );
        }
        Err((_, e, duration)) => {
            println!(
                "   [{}/{}] ❌ {}: Failed in {:?} - {}",
                i + 1,
                total,
                url,
                duration,
                e
            );
        }
    }
}

/// Batch processing metrics
struct BatchMetrics {
    success_count: usize,
    total_chars: usize,
    success_rate: f32,
}

impl BatchMetrics {
    /// Create metrics from processing results
    fn from_results<T: AsRef<str>, E>(
        results: &[Result<(String, T, Duration), E>],
        total_urls: usize,
    ) -> Self {
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let total_chars: usize = results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .map(|(_, markdown, _)| markdown.as_ref().len())
            .sum();
        let success_rate = (success_count as f32 / total_urls as f32) * PERCENTAGE_MULTIPLIER;

        Self {
            success_count,
            total_chars,
            success_rate,
        }
    }
}

/// Get error category for a MarkdownError
fn error_category(error: &MarkdownError) -> &'static str {
    match error {
        MarkdownError::ValidationError { .. } => "Validation",
        MarkdownError::EnhancedNetworkError { .. } => "Network",
        MarkdownError::NetworkError { .. } => "Network (Legacy)",
        MarkdownError::AuthenticationError { .. } => "Authentication",
        MarkdownError::ContentError { .. } => "Content",
        MarkdownError::ConverterError { .. } => "Converter",
        MarkdownError::ConfigurationError { .. } => "Configuration",
        MarkdownError::ParseError { .. } => "Parse",
        MarkdownError::InvalidUrl { .. } => "Invalid URL",
        MarkdownError::AuthError { .. } => "Auth (Legacy)",
        MarkdownError::LegacyConfigurationError { .. } => "Config (Legacy)",
    }
}

/// Generic timing wrapper for async operations
async fn time_operation<F, T>(operation: F) -> Result<(T, Duration), (MarkdownError, Duration)>
where
    F: std::future::Future<Output = Result<T, MarkdownError>>,
{
    let start = Instant::now();
    match operation.await {
        Ok(result) => Ok((result, start.elapsed())),
        Err(e) => Err((e, start.elapsed())),
    }
}

/// Convert a URL with timing information
async fn convert_url_with_timing(md: &MarkdownDown, url: &str) -> ConversionResult {
    match time_operation(md.convert_url(url)).await {
        Ok((markdown, duration)) => Ok((url.to_string(), markdown, duration)),
        Err((e, duration)) => Err((url.to_string(), e, duration)),
    }
}

/// Convert a URL with timing and attempt tracking
async fn convert_url_with_timing_and_attempts(
    md: &MarkdownDown,
    url: &str,
    attempt: usize,
) -> Result<(String, Markdown, Duration, usize), (String, MarkdownError, Duration, usize)> {
    match time_operation(md.convert_url(url)).await {
        Ok((markdown, duration)) => Ok((url.to_string(), markdown, duration, attempt)),
        Err((e, duration)) => Err((url.to_string(), e, duration, attempt)),
    }
}

/// Generic retry helper with exponential backoff
async fn retry_with_backoff<F, Fut, T, E>(
    operation: F,
    max_attempts: usize,
    is_retryable: impl Fn(&E) -> bool,
    get_duration: impl Fn(&Result<T, E>) -> Duration,
) -> Result<T, E>
where
    F: Fn(usize) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut attempts = 0;

    loop {
        attempts += 1;
        let result = operation(attempts).await;
        let duration = get_duration(&result);

        match &result {
            Ok(_) => {
                println!("      ✅ Success on attempt {attempts} in {duration:?}");
                return result;
            }
            Err(e) => {
                if attempts < max_attempts && is_retryable(e) {
                    println!("      🔄 Attempt {attempts} failed in {duration:?}, retrying");
                    tokio::time::sleep(Duration::from_millis(
                        RETRY_BASE_DELAY_MS * attempts as u64,
                    ))
                    .await;
                    continue;
                } else {
                    println!("      ❌ Failed after {attempts} attempts in {duration:?}");
                    return result;
                }
            }
        }
    }
}

/// Fetch a URL with retry logic
async fn fetch_with_retry(
    md: &MarkdownDown,
    url: &str,
    max_attempts: usize,
) -> Result<(String, Markdown, Duration, usize), (String, MarkdownError, Duration, usize)> {
    retry_with_backoff(
        |attempt| convert_url_with_timing_and_attempts(md, url, attempt),
        max_attempts,
        |e: &(String, MarkdownError, Duration, usize)| e.1.is_retryable(),
        |result| match result {
            Ok((_, _, duration, _)) => *duration,
            Err((_, _, duration, _)) => *duration,
        },
    )
    .await
}

/// Process URLs sequentially with detailed logging
async fn process_urls_sequentially(
    md: &MarkdownDown,
    urls: &[&str],
) -> (ConversionResults, Duration) {
    println!("1. Sequential Processing");
    println!("   Processing URLs one by one with detailed logging...");

    let start_time = Instant::now();
    let mut sequential_results = Vec::new();

    for (i, url) in urls.iter().enumerate() {
        let result = convert_url_with_timing(md, url).await;
        log_result(i, urls.len(), url, &result);
        sequential_results.push(result);
    }

    let sequential_duration = start_time.elapsed();
    println!("   📊 Sequential processing completed in {sequential_duration:?}\n");

    (sequential_results, sequential_duration)
}

/// Process URLs in parallel with concurrency control
async fn process_urls_parallel(config: &Config, urls: &[&str]) -> (ConversionResults, Duration) {
    println!("2. Parallel Processing");
    println!("   Processing URLs concurrently with controlled parallelism...");

    let parallel_start = Instant::now();

    // Process in parallel with a semaphore to limit concurrent requests
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut parallel_tasks = Vec::new();

    for url in urls {
        let md_for_task = MarkdownDown::with_config(config.clone());
        let url_owned = url.to_string();
        let sem_permit = semaphore.clone();

        let task = tokio::spawn(async move {
            let _permit = sem_permit.acquire().await.unwrap();
            convert_url_with_timing(&md_for_task, &url_owned).await
        });

        parallel_tasks.push(task);
    }

    // Collect results as they complete
    let mut parallel_results = Vec::new();
    for (i, task) in parallel_tasks.into_iter().enumerate() {
        match task.await {
            Ok(result) => {
                log_result(i, urls.len(), urls[i], &result);
                parallel_results.push(result);
            }
            Err(join_error) => {
                println!(
                    "   [{}/{}] 💥 Task failed: {}",
                    i + 1,
                    urls.len(),
                    join_error
                );
            }
        }
    }

    let parallel_duration = parallel_start.elapsed();
    println!("   📊 Parallel processing completed in {parallel_duration:?}\n");

    (parallel_results, parallel_duration)
}

/// Process URLs with timeout and retry logic
async fn process_urls_with_retry(
    md: &MarkdownDown,
    urls: &[&str],
) -> Vec<Result<(String, Markdown, Duration, usize), (String, MarkdownError, Duration, usize)>> {
    println!("3. Batch Processing with Advanced Error Handling");
    println!("   Processing with per-URL timeouts and smart retry logic...");

    let advanced_start = Instant::now();
    let mut advanced_results = Vec::new();

    for (i, url) in urls.iter().enumerate() {
        println!(
            "   [{}/{}] Processing with timeout: {}",
            i + 1,
            urls.len(),
            url
        );

        // Set a per-URL timeout
        let result = timeout(
            Duration::from_secs(PER_URL_TIMEOUT_SECONDS),
            fetch_with_retry(md, url, MAX_RETRY_ATTEMPTS as usize),
        )
        .await;

        match result {
            Ok(inner_result) => {
                advanced_results.push(inner_result);
            }
            Err(_timeout_error) => {
                println!("      ⏰ Timeout after {PER_URL_TIMEOUT_SECONDS} seconds");
                let timeout_error = MarkdownError::NetworkError {
                    message: "Request timeout".to_string(),
                };
                advanced_results.push(Err((
                    url.to_string(),
                    timeout_error,
                    Duration::from_secs(PER_URL_TIMEOUT_SECONDS),
                    1,
                )));
            }
        }
    }

    let advanced_duration = advanced_start.elapsed();
    println!("   📊 Advanced processing completed in {advanced_duration:?}\n");

    advanced_results
}

/// Print result metrics with optional speedup calculation
fn print_result_metrics(
    label: &str,
    metrics: &BatchMetrics,
    duration: Duration,
    total_urls: usize,
    show_speedup: Option<f32>,
) {
    println!("   📈 {label} Results:");
    println!(
        "      Success Rate: {}/{} ({:.1}%)",
        metrics.success_count, total_urls, metrics.success_rate
    );
    println!("      Total Content: {} characters", metrics.total_chars);
    println!("      Total Time: {duration:?}");
    if let Some(speedup) = show_speedup {
        println!("      Speedup: {:.1}x", speedup);
    }
}

/// Print results analysis
fn print_results_analysis(
    sequential_results: &[ConversionResult],
    parallel_results: &[ConversionResult],
    sequential_duration: Duration,
    parallel_duration: Duration,
    total_urls: usize,
) {
    println!("4. Batch Results Analysis");
    println!("   Analyzing and reporting on batch processing results...");

    // Analyze sequential results
    let sequential_metrics = BatchMetrics::from_results(sequential_results, total_urls);
    print_result_metrics(
        "Sequential",
        &sequential_metrics,
        sequential_duration,
        total_urls,
        None,
    );

    // Analyze parallel results
    let parallel_metrics = BatchMetrics::from_results(parallel_results, total_urls);
    let speedup = sequential_duration.as_secs_f32() / parallel_duration.as_secs_f32();
    print_result_metrics(
        "Parallel",
        &parallel_metrics,
        parallel_duration,
        total_urls,
        Some(speedup),
    );

    // Show error breakdown
    println!("   🔍 Error Analysis:");
    let mut error_types = std::collections::HashMap::new();
    for result in sequential_results {
        if let Err((_, error, _)) = result {
            let error_type = error_category(error);
            *error_types.entry(error_type).or_insert(0) += 1;
        }
    }

    for (error_type, count) in error_types {
        println!("      {error_type}: {count} occurrences");
    }
}

/// Print batch processing summary
fn print_summary(sequential_duration: Duration, parallel_duration: Duration) {
    println!("\n🎯 Batch Processing Summary:");
    println!("   • Sequential processing: Good for debugging and detailed logging");
    println!(
        "   • Parallel processing: {:.1}x faster for I/O bound operations",
        sequential_duration.as_secs_f32() / parallel_duration.as_secs_f32()
    );
    println!("   • Advanced error handling: Improves success rate with retries");
    println!("   • Use semaphores to control concurrency and avoid overwhelming servers");
}

/// Set up configuration optimized for batch processing
fn setup_config() -> (Config, MarkdownDown) {
    let config = Config::builder()
        .timeout_seconds(DEFAULT_TIMEOUT_SECONDS)
        .max_retries(DEFAULT_MAX_RETRIES)
        .user_agent("MarkdownDown-BatchProcessor/1.0")
        .include_frontmatter(true)
        .custom_frontmatter_field("batch_id", "example_batch_001")
        .build();

    let md = MarkdownDown::with_config(config.clone());
    (config, md)
}

/// Get example URLs for batch processing
fn get_example_urls() -> Vec<&'static str> {
    vec![
        "https://httpbin.org/html",
        "https://httpbin.org/json",
        "https://httpbin.org/xml",
        "https://blog.rust-lang.org/2024/01/15/Rust-1.75.0.html",
        "https://doc.rust-lang.org/book/ch01-00-getting-started.html",
        "https://invalid-url-that-will-fail.nonexistent",
        "https://httpbin.org/status/404", // This will fail
        "https://httpbin.org/delay/2",    // This will work but be slow
    ]
}

/// Run all batch processing examples
async fn run_batch_examples(
    config: &Config,
    md: &MarkdownDown,
    urls: &[&str],
) -> (ConversionResults, ConversionResults, Duration, Duration) {
    let (sequential_results, sequential_duration) = process_urls_sequentially(md, urls).await;
    let (parallel_results, parallel_duration) = process_urls_parallel(config, urls).await;
    let _advanced_results = process_urls_with_retry(md, urls).await;

    (
        sequential_results,
        parallel_results,
        sequential_duration,
        parallel_duration,
    )
}

/// Display final summary of batch processing
fn display_final_summary(
    sequential_results: &[ConversionResult],
    parallel_results: &[ConversionResult],
    sequential_duration: Duration,
    parallel_duration: Duration,
    total_urls: usize,
) {
    print_results_analysis(
        sequential_results,
        parallel_results,
        sequential_duration,
        parallel_duration,
        total_urls,
    );

    print_summary(sequential_duration, parallel_duration);

    println!("\n🚀 Batch processing examples completed!");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ markdowndown Batch Processing Examples\n");

    let (config, md) = setup_config();
    let urls = get_example_urls();

    println!("📋 Processing {} URLs in batch...\n", urls.len());

    let (sequential_results, parallel_results, sequential_duration, parallel_duration) =
        run_batch_examples(&config, &md, &urls).await;

    display_final_summary(
        &sequential_results,
        &parallel_results,
        sequential_duration,
        parallel_duration,
        urls.len(),
    );

    Ok(())
}
