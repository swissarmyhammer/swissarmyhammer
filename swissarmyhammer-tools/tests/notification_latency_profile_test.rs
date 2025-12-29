//! Latency profiling tests for notification streaming
//!
//! This test suite profiles the performance characteristics of the notification
//! streaming system, measuring latency, throughput, and overhead.

use serde_json::json;
use std::time::{Duration, Instant};
use swissarmyhammer_tools::mcp::progress_notifications::{
    generate_progress_token, ProgressNotification, ProgressSender,
};
use tokio::sync::mpsc;

/// Profile single notification end-to-end latency
#[tokio::test]
async fn profile_single_notification_latency() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    let mut latencies = Vec::new();

    // Measure 1000 individual notifications
    for i in 0..1000 {
        let start = Instant::now();

        sender
            .send_progress(&token, Some(i % 100), format!("Progress: {}", i))
            .unwrap();

        let notif = rx.recv().await.unwrap();
        let latency = start.elapsed();

        latencies.push(latency);

        // Verify notification
        assert_eq!(notif.progress_token, token);
        assert_eq!(notif.progress, Some(i % 100));
    }

    // Calculate statistics
    let total: Duration = latencies.iter().sum();
    let avg = total / latencies.len() as u32;

    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];
    let max = latencies.last().unwrap();

    println!("\n=== Single Notification Latency Profile ===");
    println!("Samples: {}", latencies.len());
    println!("Average: {:?}", avg);
    println!("P50: {:?}", p50);
    println!("P95: {:?}", p95);
    println!("P99: {:?}", p99);
    println!("Max: {:?}", max);

    // Assert reasonable performance (adjust thresholds as needed)
    assert!(
        avg < Duration::from_micros(100),
        "Average latency too high: {:?}",
        avg
    );
    assert!(
        p99 < Duration::from_micros(500),
        "P99 latency too high: {:?}",
        p99
    );
}

/// Profile notification throughput under load
#[tokio::test]
async fn profile_notification_throughput() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    let notification_count = 10_000;

    // Send burst of notifications
    let start = Instant::now();
    for i in 0..notification_count {
        sender
            .send_progress(&token, Some(i % 100), format!("Progress: {}", i))
            .unwrap();
    }
    let send_duration = start.elapsed();

    // Receive all notifications
    let mut received_count = 0;
    let recv_start = Instant::now();
    while received_count < notification_count {
        rx.recv().await.unwrap();
        received_count += 1;
    }
    let recv_duration = recv_start.elapsed();
    let total_duration = start.elapsed();

    let send_throughput = notification_count as f64 / send_duration.as_secs_f64();
    let recv_throughput = notification_count as f64 / recv_duration.as_secs_f64();
    let total_throughput = notification_count as f64 / total_duration.as_secs_f64();

    println!("\n=== Notification Throughput Profile ===");
    println!("Total notifications: {}", notification_count);
    println!("Send duration: {:?}", send_duration);
    println!("Receive duration: {:?}", recv_duration);
    println!("Total duration: {:?}", total_duration);
    println!("Send throughput: {:.0} notifications/sec", send_throughput);
    println!(
        "Receive throughput: {:.0} notifications/sec",
        recv_throughput
    );
    println!(
        "End-to-end throughput: {:.0} notifications/sec",
        total_throughput
    );

    // Assert reasonable throughput (adjust as needed)
    assert!(
        total_throughput > 10_000.0,
        "Throughput too low: {:.0} notifications/sec",
        total_throughput
    );
}

/// Profile concurrent notification latency
#[tokio::test]
async fn profile_concurrent_notification_latency() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    let task_count = 10;
    let notifications_per_task = 100;

    // Spawn concurrent tasks
    let start = Instant::now();
    let mut handles = vec![];

    for task_id in 0..task_count {
        let sender_clone = sender.clone();
        let handle = tokio::spawn(async move {
            let token = format!("task_{}", task_id);
            let mut task_latencies = Vec::new();

            for i in 0..notifications_per_task {
                let send_start = Instant::now();
                sender_clone
                    .send_progress(&token, Some(i), format!("Task {} progress: {}", task_id, i))
                    .unwrap();
                task_latencies.push(send_start.elapsed());
            }

            task_latencies
        });
        handles.push(handle);
    }

    // Wait for all tasks
    let mut all_latencies = Vec::new();
    for handle in handles {
        let task_latencies = handle.await.unwrap();
        all_latencies.extend(task_latencies);
    }

    let total_duration = start.elapsed();

    // Receive all notifications
    let mut received = 0;
    let total_notifications = task_count * notifications_per_task;
    while received < total_notifications {
        rx.recv().await.unwrap();
        received += 1;
    }

    // Calculate statistics
    all_latencies.sort();
    let avg: Duration = all_latencies.iter().sum::<Duration>() / all_latencies.len() as u32;
    let p50 = all_latencies[all_latencies.len() / 2];
    let p95 = all_latencies[all_latencies.len() * 95 / 100];
    let p99 = all_latencies[all_latencies.len() * 99 / 100];
    let max = all_latencies.last().unwrap();

    println!("\n=== Concurrent Notification Latency Profile ===");
    println!("Concurrent tasks: {}", task_count);
    println!("Notifications per task: {}", notifications_per_task);
    println!("Total notifications: {}", total_notifications);
    println!("Total duration: {:?}", total_duration);
    println!("Average send latency: {:?}", avg);
    println!("P50: {:?}", p50);
    println!("P95: {:?}", p95);
    println!("P99: {:?}", p99);
    println!("Max: {:?}", max);

    assert!(
        avg < Duration::from_micros(200),
        "Average concurrent latency too high: {:?}",
        avg
    );
}

/// Profile notification latency with metadata
#[tokio::test]
async fn profile_notification_with_metadata_latency() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    let mut small_metadata_latencies = Vec::new();
    let mut large_metadata_latencies = Vec::new();

    // Profile small metadata
    for i in 0..500 {
        let metadata = json!({
            "iteration": i,
            "status": "active"
        });

        let start = Instant::now();
        sender
            .send_progress_with_metadata(&token, Some(i % 100), format!("Small: {}", i), metadata)
            .unwrap();
        rx.recv().await.unwrap();
        small_metadata_latencies.push(start.elapsed());
    }

    // Profile large metadata
    for i in 0..500 {
        let files: Vec<String> = (0..100).map(|j| format!("file_{}_{}.rs", i, j)).collect();
        let metadata = json!({
            "iteration": i,
            "files": files,
            "stats": {
                "processed": i * 100,
                "total": 50000,
                "errors": i % 10
            }
        });

        let start = Instant::now();
        sender
            .send_progress_with_metadata(&token, Some(i % 100), format!("Large: {}", i), metadata)
            .unwrap();
        rx.recv().await.unwrap();
        large_metadata_latencies.push(start.elapsed());
    }

    // Calculate statistics for small metadata
    small_metadata_latencies.sort();
    let small_avg: Duration =
        small_metadata_latencies.iter().sum::<Duration>() / small_metadata_latencies.len() as u32;
    let small_p50 = small_metadata_latencies[small_metadata_latencies.len() / 2];
    let small_p95 = small_metadata_latencies[small_metadata_latencies.len() * 95 / 100];
    let small_p99 = small_metadata_latencies[small_metadata_latencies.len() * 99 / 100];

    // Calculate statistics for large metadata
    large_metadata_latencies.sort();
    let large_avg: Duration =
        large_metadata_latencies.iter().sum::<Duration>() / large_metadata_latencies.len() as u32;
    let large_p50 = large_metadata_latencies[large_metadata_latencies.len() / 2];
    let large_p95 = large_metadata_latencies[large_metadata_latencies.len() * 95 / 100];
    let large_p99 = large_metadata_latencies[large_metadata_latencies.len() * 99 / 100];

    println!("\n=== Notification Metadata Size Impact Profile ===");
    println!("\nSmall Metadata (~50 bytes):");
    println!("  Average: {:?}", small_avg);
    println!("  P50: {:?}", small_p50);
    println!("  P95: {:?}", small_p95);
    println!("  P99: {:?}", small_p99);

    println!("\nLarge Metadata (~5KB):");
    println!("  Average: {:?}", large_avg);
    println!("  P50: {:?}", large_p50);
    println!("  P95: {:?}", large_p95);
    println!("  P99: {:?}", large_p99);

    println!(
        "\nMetadata overhead: {:?} ({}%)",
        large_avg - small_avg,
        ((large_avg.as_nanos() as f64 / small_avg.as_nanos() as f64) - 1.0) * 100.0
    );

    // Large metadata should still be reasonably fast
    assert!(
        large_avg < Duration::from_micros(500),
        "Large metadata latency too high: {:?}",
        large_avg
    );
}

/// Profile notification token generation overhead
#[tokio::test]
async fn profile_token_generation() {
    let mut latencies = Vec::new();

    // Generate 10,000 tokens
    for _ in 0..10_000 {
        let start = Instant::now();
        let _token = generate_progress_token();
        latencies.push(start.elapsed());
    }

    latencies.sort();
    let avg: Duration = latencies.iter().sum::<Duration>() / latencies.len() as u32;
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];
    let max = latencies.last().unwrap();

    println!("\n=== Token Generation Latency Profile ===");
    println!("Samples: {}", latencies.len());
    println!("Average: {:?}", avg);
    println!("P50: {:?}", p50);
    println!("P95: {:?}", p95);
    println!("P99: {:?}", p99);
    println!("Max: {:?}", max);

    assert!(
        avg < Duration::from_micros(10),
        "Token generation too slow: {:?}",
        avg
    );
}

/// Profile notification serialization overhead
#[tokio::test]
async fn profile_notification_serialization() {
    let notification = ProgressNotification {
        progress_token: generate_progress_token(),
        progress: Some(50),
        message: "Test notification with reasonable message length".to_string(),
        metadata: Some(json!({
            "files_processed": 50,
            "total_files": 100,
            "current_file": "src/main.rs",
            "errors": []
        })),
    };

    let mut latencies = Vec::new();

    // Serialize 10,000 times
    for _ in 0..10_000 {
        let start = Instant::now();
        let _json = serde_json::to_string(&notification).unwrap();
        latencies.push(start.elapsed());
    }

    latencies.sort();
    let avg: Duration = latencies.iter().sum::<Duration>() / latencies.len() as u32;
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];

    println!("\n=== Notification Serialization Latency Profile ===");
    println!("Samples: {}", latencies.len());
    println!("Average: {:?}", avg);
    println!("P50: {:?}", p50);
    println!("P95: {:?}", p95);
    println!("P99: {:?}", p99);

    assert!(
        avg < Duration::from_micros(50),
        "Serialization too slow: {:?}",
        avg
    );
}

/// Profile channel capacity impact on latency
#[tokio::test]
async fn profile_channel_capacity_impact() {
    let notification_count = 1000;

    // Test unbounded channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    let start = Instant::now();
    for i in 0..notification_count {
        sender
            .send_progress(&token, Some(i % 100), format!("Progress: {}", i))
            .unwrap();
    }
    let unbounded_send_duration = start.elapsed();

    let recv_start = Instant::now();
    for _ in 0..notification_count {
        rx.recv().await.unwrap();
    }
    let unbounded_recv_duration = recv_start.elapsed();
    let unbounded_total = start.elapsed();

    println!("\n=== Channel Capacity Impact Profile ===");
    println!("Notifications: {}", notification_count);
    println!("\nUnbounded channel:");
    println!("  Send duration: {:?}", unbounded_send_duration);
    println!("  Receive duration: {:?}", unbounded_recv_duration);
    println!("  Total duration: {:?}", unbounded_total);
    println!(
        "  Throughput: {:.0} notifications/sec",
        notification_count as f64 / unbounded_total.as_secs_f64()
    );
}

/// Profile memory allocation patterns
#[tokio::test]
async fn profile_memory_allocation_patterns() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    let iterations = 1000;
    let mut string_allocation_latencies = Vec::new();
    let mut metadata_allocation_latencies = Vec::new();

    for i in 0..iterations {
        // Profile string allocation
        let start = Instant::now();
        let token = generate_progress_token();
        let message = format!("Progress update iteration {}", i);
        string_allocation_latencies.push(start.elapsed());

        // Profile metadata allocation
        let start = Instant::now();
        let metadata = json!({
            "iteration": i,
            "files": vec!["file1.rs", "file2.rs", "file3.rs"],
            "stats": {
                "processed": i,
                "total": iterations
            }
        });
        metadata_allocation_latencies.push(start.elapsed());

        // Send notification
        sender
            .send_progress_with_metadata(&token, Some(i % 100), message, metadata)
            .unwrap();
    }

    // Receive all
    for _ in 0..iterations {
        rx.recv().await.unwrap();
    }

    string_allocation_latencies.sort();
    metadata_allocation_latencies.sort();

    let string_avg: Duration = string_allocation_latencies.iter().sum::<Duration>()
        / string_allocation_latencies.len() as u32;
    let metadata_avg: Duration = metadata_allocation_latencies.iter().sum::<Duration>()
        / metadata_allocation_latencies.len() as u32;

    println!("\n=== Memory Allocation Profile ===");
    println!("String allocation average: {:?}", string_avg);
    println!("Metadata allocation average: {:?}", metadata_avg);
    println!("Total allocation overhead: {:?}", string_avg + metadata_avg);
}

/// Profile realistic workflow scenario
#[tokio::test]
async fn profile_realistic_workflow_scenario() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    let workflow_steps = vec![
        ("Analyzing files", 100),
        ("Building index", 250),
        ("Processing dependencies", 500),
        ("Running checks", 300),
        ("Generating report", 150),
    ];

    let total_operations: u32 = workflow_steps.iter().map(|(_, count)| count).sum();

    let start = Instant::now();
    let token = generate_progress_token();
    let mut completed = 0u32;

    // Simulate workflow
    for (step_name, operation_count) in &workflow_steps {
        for i in 0..*operation_count {
            let progress = ((completed + i) * 100 / total_operations) as u32;
            let metadata = json!({
                "step": step_name,
                "step_progress": i,
                "step_total": operation_count,
                "overall_progress": completed + i,
                "overall_total": total_operations
            });

            sender
                .send_progress_with_metadata(
                    &token,
                    Some(progress),
                    format!("{}: {}/{}", step_name, i + 1, operation_count),
                    metadata,
                )
                .unwrap();
        }
        completed += operation_count;
    }

    let send_duration = start.elapsed();

    // Receive all notifications
    let mut notifications = Vec::new();
    while notifications.len() < total_operations as usize {
        notifications.push(rx.recv().await.unwrap());
    }

    let total_duration = start.elapsed();

    println!("\n=== Realistic Workflow Scenario Profile ===");
    println!("Total operations: {}", total_operations);
    println!("Workflow steps: {}", workflow_steps.len());
    println!("Send duration: {:?}", send_duration);
    println!("Total duration: {:?}", total_duration);
    println!(
        "Average latency per notification: {:?}",
        total_duration / total_operations
    );
    println!(
        "Throughput: {:.0} notifications/sec",
        total_operations as f64 / total_duration.as_secs_f64()
    );
}
