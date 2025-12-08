//! Batch processing examples for the markdowndown library.
//!
//! This example demonstrates how to process multiple URLs efficiently
//! with proper error handling, parallel processing, and result aggregation.

use markdowndown::{types::MarkdownError, Config, MarkdownDown};
use std::time::{Duration, Instant};
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ markdowndown Batch Processing Examples\n");

    // Set up configuration optimized for batch processing
    let config = Config::builder()
        .timeout_seconds(30)
        .max_retries(2)
        .user_agent("MarkdownDown-BatchProcessor/1.0")
        .include_frontmatter(true)
        .custom_frontmatter_field("batch_id", "example_batch_001")
        .build();

    let md = MarkdownDown::with_config(config.clone());

    // Example URLs to process
    let urls = vec![
        "https://httpbin.org/html",
        "https://httpbin.org/json",
        "https://httpbin.org/xml",
        "https://blog.rust-lang.org/2024/01/15/Rust-1.75.0.html",
        "https://doc.rust-lang.org/book/ch01-00-getting-started.html",
        "https://invalid-url-that-will-fail.nonexistent",
        "https://httpbin.org/status/404", // This will fail
        "https://httpbin.org/delay/2",    // This will work but be slow
    ];

    println!("📋 Processing {} URLs in batch...\n", urls.len());

    // Example 1: Sequential processing with detailed logging
    println!("1. Sequential Processing");
    println!("   Processing URLs one by one with detailed logging...");

    let start_time = Instant::now();
    let mut sequential_results = Vec::new();

    for (i, url) in urls.iter().enumerate() {
        println!("   [{}/{}] Processing: {}", i + 1, urls.len(), url);

        let url_start = Instant::now();
        match md.convert_url(url).await {
            Ok(markdown) => {
                let duration = url_start.elapsed();
                let char_count = markdown.as_str().len();
                println!("      ✅ Success: {char_count} chars in {duration:?}");
                sequential_results.push(Ok((url.to_string(), markdown, duration)));
            }
            Err(e) => {
                let duration = url_start.elapsed();
                println!("      ❌ Failed in {duration:?}: {e}");
                sequential_results.push(Err((url.to_string(), e, duration)));
            }
        }
    }

    let sequential_duration = start_time.elapsed();
    println!("   📊 Sequential processing completed in {sequential_duration:?}\n");

    // Example 2: Parallel processing with concurrency control
    println!("2. Parallel Processing");
    println!("   Processing URLs concurrently with controlled parallelism...");

    let parallel_start = Instant::now();

    // Process in parallel with a semaphore to limit concurrent requests
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(3)); // Max 3 concurrent
    let mut parallel_tasks = Vec::new();

    for url in &urls {
        let md_for_task = MarkdownDown::with_config(config.clone());
        let url_owned = url.to_string();
        let sem_permit = semaphore.clone();

        let task = tokio::spawn(async move {
            let _permit = sem_permit.acquire().await.unwrap();
            let start = Instant::now();

            match md_for_task.convert_url(&url_owned).await {
                Ok(markdown) => {
                    let duration = start.elapsed();
                    Ok((url_owned, markdown, duration))
                }
                Err(e) => {
                    let duration = start.elapsed();
                    Err((url_owned, e, duration))
                }
            }
        });

        parallel_tasks.push(task);
    }

    // Collect results as they complete
    let mut parallel_results = Vec::new();
    for (i, task) in parallel_tasks.into_iter().enumerate() {
        match task.await {
            Ok(result) => match result {
                Ok((url, markdown, duration)) => {
                    println!(
                        "   [{}/{}] ✅ {}: {} chars in {:?}",
                        i + 1,
                        urls.len(),
                        url,
                        markdown.as_str().len(),
                        duration
                    );
                    parallel_results.push(Ok((url, markdown, duration)));
                }
                Err((url, e, duration)) => {
                    println!(
                        "   [{}/{}] ❌ {}: Failed in {:?} - {}",
                        i + 1,
                        urls.len(),
                        url,
                        duration,
                        e
                    );
                    parallel_results.push(Err((url, e, duration)));
                }
            },
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

    // Example 3: Batch processing with timeout and retry logic
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

        // Set a per-URL timeout of 10 seconds
        let result = timeout(Duration::from_secs(10), async {
            let mut attempts = 0;
            let max_attempts = 3;

            loop {
                attempts += 1;
                let attempt_start = Instant::now();

                match md.convert_url(url).await {
                    Ok(markdown) => {
                        let duration = attempt_start.elapsed();
                        println!("      ✅ Success on attempt {attempts} in {duration:?}");
                        return Ok((url.to_string(), markdown, duration, attempts));
                    }
                    Err(e) => {
                        let duration = attempt_start.elapsed();

                        // Check if this error is retryable
                        if attempts < max_attempts && e.is_retryable() {
                            println!(
                                "      🔄 Attempt {attempts} failed in {duration:?}, retrying: {e}"
                            );
                            tokio::time::sleep(Duration::from_millis(1000 * attempts as u64)).await;
                            continue;
                        } else {
                            println!(
                                "      ❌ Failed after {attempts} attempts in {duration:?}: {e}"
                            );
                            return Err((url.to_string(), e, duration, attempts));
                        }
                    }
                }
            }
        })
        .await;

        match result {
            Ok(inner_result) => {
                advanced_results.push(inner_result);
            }
            Err(_timeout_error) => {
                println!("      ⏰ Timeout after 10 seconds");
                let timeout_error = MarkdownError::NetworkError {
                    message: "Request timeout".to_string(),
                };
                advanced_results.push(Err((
                    url.to_string(),
                    timeout_error,
                    Duration::from_secs(10),
                    1,
                )));
            }
        }
    }

    let advanced_duration = advanced_start.elapsed();
    println!("   📊 Advanced processing completed in {advanced_duration:?}\n");

    // Example 4: Results analysis and reporting
    println!("4. Batch Results Analysis");
    println!("   Analyzing and reporting on batch processing results...");

    // Analyze sequential results
    let seq_success_count = sequential_results.iter().filter(|r| r.is_ok()).count();
    let seq_total_chars: usize = sequential_results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .map(|(_, markdown, _)| markdown.as_str().len())
        .sum();

    println!("   📈 Sequential Results:");
    println!(
        "      Success Rate: {}/{} ({:.1}%)",
        seq_success_count,
        urls.len(),
        (seq_success_count as f32 / urls.len() as f32) * 100.0
    );
    println!("      Total Content: {seq_total_chars} characters");
    println!("      Total Time: {sequential_duration:?}");

    // Analyze parallel results
    let par_success_count = parallel_results.iter().filter(|r| r.is_ok()).count();
    let par_total_chars: usize = parallel_results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .map(|(_, markdown, _)| markdown.as_str().len())
        .sum();

    println!("   📈 Parallel Results:");
    println!(
        "      Success Rate: {}/{} ({:.1}%)",
        par_success_count,
        urls.len(),
        (par_success_count as f32 / urls.len() as f32) * 100.0
    );
    println!("      Total Content: {par_total_chars} characters");
    println!("      Total Time: {parallel_duration:?}");
    println!(
        "      Speedup: {:.1}x",
        sequential_duration.as_secs_f32() / parallel_duration.as_secs_f32()
    );

    // Show error breakdown
    println!("   🔍 Error Analysis:");
    let mut error_types = std::collections::HashMap::new();
    for result in &sequential_results {
        if let Err((_, error, _)) = result {
            let error_type = match error {
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
            };
            *error_types.entry(error_type).or_insert(0) += 1;
        }
    }

    for (error_type, count) in error_types {
        println!("      {error_type}: {count} occurrences");
    }

    println!("\n🎯 Batch Processing Summary:");
    println!("   • Sequential processing: Good for debugging and detailed logging");
    println!(
        "   • Parallel processing: {:.1}x faster for I/O bound operations",
        sequential_duration.as_secs_f32() / parallel_duration.as_secs_f32()
    );
    println!("   • Advanced error handling: Improves success rate with retries");
    println!("   • Use semaphores to control concurrency and avoid overwhelming servers");

    println!("\n🚀 Batch processing examples completed!");
    Ok(())
}
