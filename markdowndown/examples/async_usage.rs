//! Async usage examples for the markdowndown library.
//!
//! This example demonstrates various async patterns, proper error handling in async context,
//! streaming results, and integration with async ecosystems.

use futures::stream::{self, StreamExt};
use markdowndown::{convert_url, Config, MarkdownDown};
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};

/// Simulated async workload that processes markdown content
async fn process_markdown_content(markdown: &str, delay_ms: u64) -> String {
    // Simulate some async processing
    sleep(Duration::from_millis(delay_ms)).await;

    // Return some processing results
    format!(
        "Processed {} chars, {} lines, {} words",
        markdown.len(),
        markdown.lines().count(),
        markdown.split_whitespace().count()
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 markdowndown Async Usage Examples\n");

    // Setup configuration for async examples
    let config = Config::builder()
        .timeout_seconds(30)
        .max_retries(2)
        .user_agent("MarkdownDown-AsyncExample/1.0")
        .build();

    let md = MarkdownDown::with_config(config);

    // Example URLs for testing
    let test_urls = vec![
        "https://httpbin.org/html",
        "https://httpbin.org/json",
        "https://httpbin.org/xml",
    ];

    // Example 1: Basic async/await patterns
    println!("1. Basic Async/Await Patterns");
    println!("   Demonstrating fundamental async usage...");

    // Simple async conversion
    println!("   📥 Simple async conversion:");
    let start = Instant::now();
    match convert_url("https://httpbin.org/html").await {
        Ok(markdown) => {
            println!(
                "      ✅ Converted in {:?}: {} chars",
                start.elapsed(),
                markdown.as_str().len()
            );
        }
        Err(e) => {
            println!("      ❌ Failed in {:?}: {}", start.elapsed(), e);
        }
    }

    // Async conversion with custom configuration
    println!("   🔧 Async with custom configuration:");
    let start = Instant::now();
    match md.convert_url("https://httpbin.org/json").await {
        Ok(markdown) => {
            println!(
                "      ✅ Converted in {:?}: {} chars",
                start.elapsed(),
                markdown.as_str().len()
            );
        }
        Err(e) => {
            println!("      ❌ Failed in {:?}: {}", start.elapsed(), e);
        }
    }
    println!();

    // Example 2: Async error handling patterns
    println!("2. Async Error Handling Patterns");
    println!("   Demonstrating proper async error handling...");

    // Using Result chaining with async
    println!("   🔗 Result chaining:");
    let result = async {
        let markdown = convert_url("https://httpbin.org/html").await?;
        let processed = process_markdown_content(markdown.as_str(), 100).await;
        Ok::<String, Box<dyn std::error::Error>>(processed)
    }
    .await;

    match result {
        Ok(processed) => println!("      ✅ Chained processing: {processed}"),
        Err(e) => println!("      ❌ Chained processing failed: {e}"),
    }

    // Using match with async
    println!("   🎯 Match-based error handling:");
    match convert_url("https://invalid-url-for-testing.invalid").await {
        Ok(markdown) => {
            println!(
                "      ✅ Unexpected success: {} chars",
                markdown.as_str().len()
            );
        }
        Err(e) => {
            println!("      ❌ Expected failure: {e}");
            let suggestions = e.suggestions();
            if !suggestions.is_empty() {
                println!("      💡 Suggestion: {}", suggestions[0]);
            }
        }
    }
    println!();

    // Example 3: Concurrent async operations
    println!("3. Concurrent Async Operations");
    println!("   Running multiple async operations concurrently...");

    // Using join! for concurrent execution
    println!("   ⚡ Concurrent with join!:");
    let start = Instant::now();

    let (result1, result2, result3) = tokio::join!(
        convert_url(test_urls[0]),
        convert_url(test_urls[1]),
        convert_url(test_urls[2])
    );

    let duration = start.elapsed();
    println!("      ⏱️  All three completed in {duration:?}");

    let results = vec![result1, result2, result3];
    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(markdown) => println!("      ✅ URL {}: {} chars", i + 1, markdown.as_str().len()),
            Err(e) => println!("      ❌ URL {}: {}", i + 1, e),
        }
    }

    // Using try_join! for fail-fast behavior
    println!("   🚨 Fail-fast with try_join!:");
    let start = Instant::now();

    match tokio::try_join!(
        convert_url("https://httpbin.org/html"),
        convert_url("https://httpbin.org/json"),
        convert_url("https://invalid-url-that-will-fail.invalid")
    ) {
        Ok((md1, md2, md3)) => {
            println!(
                "      ✅ All succeeded: {}, {}, {} chars",
                md1.as_str().len(),
                md2.as_str().len(),
                md3.as_str().len()
            );
        }
        Err(e) => {
            println!(
                "      ❌ One failed (as expected) in {:?}: {}",
                start.elapsed(),
                e
            );
        }
    }
    println!();

    // Example 4: Async streams and iterators
    println!("4. Async Streams and Processing");
    println!("   Using streams for async data processing...");

    // Create a stream of URLs
    let url_stream = stream::iter(&test_urls);

    // Process URLs as a stream with concurrency limit
    println!("   🌊 Stream processing with concurrency limit:");
    let start = Instant::now();

    let results: Vec<_> = url_stream
        .map(|url| async move {
            let start = Instant::now();
            match convert_url(url).await {
                Ok(markdown) => {
                    let processing_result = process_markdown_content(markdown.as_str(), 200).await;
                    Ok((url, processing_result, start.elapsed()))
                }
                Err(e) => Err((url, e, start.elapsed())),
            }
        })
        .buffer_unordered(2) // Process up to 2 URLs concurrently
        .collect()
        .await;

    let total_duration = start.elapsed();
    println!("      ⏱️  Stream processing completed in {total_duration:?}");

    for result in results {
        match result {
            Ok((url, processing, duration)) => {
                println!("      ✅ {url} in {duration:?}: {processing}");
            }
            Err((url, e, duration)) => {
                println!("      ❌ {url} in {duration:?}: {e}");
            }
        }
    }
    println!();

    // Example 5: Async with timeouts and cancellation
    println!("5. Async Timeouts and Cancellation");
    println!("   Demonstrating timeout handling and cancellation...");

    // Using timeout wrapper
    println!("   ⏰ Individual operation timeout:");
    let timeout_duration = Duration::from_secs(5);

    match timeout(timeout_duration, convert_url("https://httpbin.org/delay/2")).await {
        Ok(Ok(markdown)) => {
            println!(
                "      ✅ Completed within timeout: {} chars",
                markdown.as_str().len()
            );
        }
        Ok(Err(e)) => {
            println!("      ❌ Failed within timeout: {e}");
        }
        Err(_) => {
            println!("      ⏰ Operation timed out after {timeout_duration:?}");
        }
    }

    // Cancellation with select!
    println!("   🛑 Cancellation with select!:");
    let start = Instant::now();

    tokio::select! {
        result = convert_url("https://httpbin.org/delay/3") => {
            match result {
                Ok(markdown) => println!("      ✅ Conversion completed: {} chars", markdown.as_str().len()),
                Err(e) => println!("      ❌ Conversion failed: {e}"),
            }
        }
        _ = sleep(Duration::from_secs(2)) => {
            println!("      🛑 Cancelled after 2 seconds (simulated user cancellation)");
        }
    }

    println!("      ⏱️  Select completed in {:?}", start.elapsed());
    println!();

    // Example 6: Async integration patterns
    println!("6. Async Integration Patterns");
    println!("   Common patterns for integrating with async applications...");

    // Background task pattern
    println!("   🔄 Background task pattern:");
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(10);

    // Spawn a background worker
    let worker_handle = tokio::spawn(async move {
        let md = MarkdownDown::new();
        let mut processed_count = 0;

        while let Some(url) = rx.recv().await {
            match md.convert_url(&url).await {
                Ok(markdown) => {
                    processed_count += 1;
                    println!(
                        "      📄 Worker processed {}: {} chars",
                        url,
                        markdown.as_str().len()
                    );
                }
                Err(e) => {
                    println!("      ❌ Worker failed on {url}: {e}");
                }
            }
        }

        println!("      🏁 Worker completed {processed_count} conversions");
        processed_count
    });

    // Send some work to the background worker
    for url in &test_urls {
        tx.send(url.to_string()).await?;
    }
    drop(tx); // Close the channel

    // Wait for worker to complete
    let processed_count = worker_handle.await?;
    println!("      ✅ Background worker processed {processed_count} URLs");

    // Rate-limited processing pattern
    println!("   🐌 Rate-limited processing:");
    let rate_limit = Duration::from_millis(500); // 2 requests per second

    for (i, url) in test_urls.iter().enumerate() {
        if i > 0 {
            sleep(rate_limit).await; // Rate limiting delay
        }

        let start = Instant::now();
        match convert_url(url).await {
            Ok(markdown) => {
                println!(
                    "      ✅ Rate-limited conversion {}: {} chars in {:?}",
                    i + 1,
                    markdown.as_str().len(),
                    start.elapsed()
                );
            }
            Err(e) => {
                println!(
                    "      ❌ Rate-limited conversion {} failed in {:?}: {}",
                    i + 1,
                    start.elapsed(),
                    e
                );
            }
        }
    }

    println!("\n🎉 Async usage examples completed!");
    println!("\n💡 Key Async Patterns:");
    println!("   • Use join! for concurrent independent operations");
    println!("   • Use try_join! when you need fail-fast behavior");
    println!("   • Use streams with buffer_unordered for controlled concurrency");
    println!("   • Use timeouts to prevent hanging operations");
    println!("   • Use select! for cancellation and racing operations");
    println!("   • Use background tasks for fire-and-forget processing");
    println!("   • Implement rate limiting to be respectful of servers");

    Ok(())
}
