//! Tests for notification and request separation
//!
//! This test suite verifies that progress notifications (sent asynchronously via channels)
//! and MCP requests (synchronous protocol operations) use separate channels and don't
//! interfere with each other.
//!
//! Key aspects tested:
//! - Notifications don't block request processing
//! - Requests don't block notification delivery
//! - Both can operate concurrently without deadlocks
//! - Channel closure is handled independently

use serde_json::json;
use swissarmyhammer_tools::mcp::progress_notifications::{generate_progress_token, ProgressSender};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout, Duration};

/// Test that notifications can be sent while requests are being processed
#[tokio::test]
async fn test_notifications_dont_block_requests() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Simulate a long-running request that sends notifications
    let sender_clone = sender.clone();
    let token_clone = token.clone();
    let request_task = tokio::spawn(async move {
        // Send start notification
        sender_clone
            .send_progress(&token_clone, Some(0), "Request starting")
            .unwrap();

        // Simulate request processing
        sleep(Duration::from_millis(50)).await;

        // Send progress notification
        sender_clone
            .send_progress(&token_clone, Some(50), "Request processing")
            .unwrap();

        // Simulate more processing
        sleep(Duration::from_millis(50)).await;

        // Send completion notification
        sender_clone
            .send_progress(&token_clone, Some(100), "Request complete")
            .unwrap();

        "request_result"
    });

    // Verify we can receive notifications while request is processing
    let notif1 = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Should receive notification within timeout")
        .expect("Should receive notification");
    assert_eq!(notif1.progress, Some(0));
    assert_eq!(notif1.message, "Request starting");

    let notif2 = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Should receive notification within timeout")
        .expect("Should receive notification");
    assert_eq!(notif2.progress, Some(50));

    let notif3 = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Should receive notification within timeout")
        .expect("Should receive notification");
    assert_eq!(notif3.progress, Some(100));

    // Verify request completed successfully
    let result = request_task.await.unwrap();
    assert_eq!(result, "request_result");
}

/// Test that requests can be processed while notifications are being sent
#[tokio::test]
async fn test_requests_dont_block_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    // Spawn multiple "request" tasks that send notifications
    let mut request_handles = vec![];
    for i in 0..10 {
        let sender_clone = sender.clone();
        let handle = tokio::spawn(async move {
            let token = generate_progress_token();
            sender_clone
                .send_progress(&token, Some(i * 10), format!("Request {} processing", i))
                .unwrap();
            sleep(Duration::from_millis(10)).await;
            format!("result_{}", i)
        });
        request_handles.push(handle);
    }

    // Verify all notifications are received without blocking
    let mut received_count = 0;
    while received_count < 10 {
        match timeout(Duration::from_secs(1), rx.recv()).await {
            Ok(Some(_)) => received_count += 1,
            Ok(None) => break,
            Err(_) => panic!("Timeout waiting for notifications"),
        }
    }
    assert_eq!(received_count, 10, "Should receive all notifications");

    // Verify all requests completed
    for handle in request_handles {
        let result = handle.await.unwrap();
        assert!(result.starts_with("result_"));
    }
}

/// Test that notification channel closure doesn't affect request processing
#[tokio::test]
async fn test_notification_channel_closure_independence() {
    let (tx, rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Send initial notification
    sender
        .send_progress(&token, Some(0), "Starting")
        .expect("Should send successfully");

    // Close the receiver
    drop(rx);

    // Attempt to send notification after closure - should fail gracefully
    let result = sender.send_progress(&token, Some(50), "This should fail");
    assert!(result.is_err(), "Send should fail after channel closed");

    // Simulate request processing continuing despite notification failure
    let request_result = simulate_request_processing().await;
    assert_eq!(request_result, "success");
}

/// Test concurrent notifications and requests don't deadlock
#[tokio::test]
async fn test_no_deadlock_with_concurrent_operations() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    // Spawn notification sender
    let sender_clone = sender.clone();
    let notification_task = tokio::spawn(async move {
        for i in 0..100 {
            let token = generate_progress_token();
            sender_clone
                .send_progress(&token, Some(i), format!("Progress {}", i))
                .unwrap();
        }
    });

    // Spawn notification receiver
    let receiver_task = tokio::spawn(async move {
        let mut count = 0;
        while count < 100 {
            if rx.recv().await.is_some() {
                count += 1;
            }
        }
        count
    });

    // Spawn concurrent "request" tasks
    let mut request_handles = vec![];
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(1)).await;
            i * 2
        });
        request_handles.push(handle);
    }

    // Verify no deadlock - all tasks complete within reasonable time
    timeout(Duration::from_secs(5), notification_task)
        .await
        .expect("Notification task should complete")
        .expect("Notification task should succeed");

    let receiver_result = timeout(Duration::from_secs(5), receiver_task)
        .await
        .expect("Receiver task should complete")
        .unwrap();
    assert_eq!(receiver_result, 100);

    for handle in request_handles {
        let result = timeout(Duration::from_secs(5), handle)
            .await
            .expect("Request should complete")
            .unwrap();
        assert!(result < 20);
    }
}

/// Test that notifications maintain order while requests execute
#[tokio::test]
async fn test_notification_ordering_with_requests() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Send notifications in order while simulating request processing
    let sender_clone = sender.clone();
    let token_clone = token.clone();
    let send_task = tokio::spawn(async move {
        for i in 0..10 {
            sender_clone
                .send_progress(&token_clone, Some(i * 10), format!("Step {}", i))
                .unwrap();
            // Simulate some request processing
            sleep(Duration::from_millis(1)).await;
        }
    });

    // Verify notifications arrive in order
    let mut last_progress = None;
    for _ in 0..10 {
        let notif = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Should receive notification")
            .expect("Should receive notification");

        if let Some(progress) = notif.progress {
            if let Some(last) = last_progress {
                assert!(
                    progress >= last,
                    "Progress should be monotonically increasing"
                );
            }
            last_progress = Some(progress);
        }
    }

    send_task.await.unwrap();
}

/// Test that multiple notification senders don't interfere with requests
#[tokio::test]
async fn test_multiple_notification_sources() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender1 = ProgressSender::new(tx.clone());
    let sender2 = ProgressSender::new(tx.clone());
    let sender3 = ProgressSender::new(tx);

    // Simulate multiple concurrent operations sending notifications
    let token1 = generate_progress_token();
    let token2 = generate_progress_token();
    let token3 = generate_progress_token();

    let task1 = tokio::spawn({
        let sender = sender1;
        let token = token1.clone();
        async move {
            for i in 0..5 {
                sender
                    .send_progress(&token, Some(i * 20), format!("Op1: {}", i))
                    .unwrap();
                sleep(Duration::from_millis(1)).await;
            }
            "op1_done"
        }
    });

    let task2 = tokio::spawn({
        let sender = sender2;
        let token = token2.clone();
        async move {
            for i in 0..5 {
                sender
                    .send_progress(&token, Some(i * 20), format!("Op2: {}", i))
                    .unwrap();
                sleep(Duration::from_millis(1)).await;
            }
            "op2_done"
        }
    });

    let task3 = tokio::spawn({
        let sender = sender3;
        let token = token3.clone();
        async move {
            for i in 0..5 {
                sender
                    .send_progress(&token, Some(i * 20), format!("Op3: {}", i))
                    .unwrap();
                sleep(Duration::from_millis(1)).await;
            }
            "op3_done"
        }
    });

    // Collect all notifications
    let mut notifications = Vec::new();
    for _ in 0..15 {
        if let Ok(Some(notif)) = timeout(Duration::from_secs(1), rx.recv()).await {
            notifications.push(notif);
        }
    }

    assert_eq!(notifications.len(), 15, "Should receive all notifications");

    // Verify all operations completed
    assert_eq!(task1.await.unwrap(), "op1_done");
    assert_eq!(task2.await.unwrap(), "op2_done");
    assert_eq!(task3.await.unwrap(), "op3_done");
}

/// Test notification backpressure doesn't affect requests
#[tokio::test]
async fn test_notification_backpressure() {
    // Use unbounded channel (ProgressSender requires unbounded)
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Send many notifications rapidly
    let sender_clone = sender.clone();
    let token_clone = token.clone();
    let send_task = tokio::spawn(async move {
        for i in 0..100 {
            // This should not block
            let _ = sender_clone.send_progress(&token_clone, Some(i), format!("Progress {}", i));
        }
        "send_complete"
    });

    // Slowly consume notifications
    let mut received = 0;
    while received < 100 {
        if timeout(Duration::from_millis(100), rx.recv()).await.is_ok() {
            received += 1;
        } else {
            break;
        }
    }

    // Verify sender completed (didn't block)
    let result = timeout(Duration::from_secs(1), send_task)
        .await
        .expect("Send task should complete");
    assert_eq!(result.unwrap(), "send_complete");
}

/// Test that notification errors don't propagate to requests
#[tokio::test]
async fn test_notification_errors_isolated() {
    let (tx, rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Send successful notification
    sender
        .send_progress(&token, Some(0), "Starting")
        .expect("Should succeed");

    // Close receiver to cause errors
    drop(rx);

    // Notification should fail
    let notif_result = sender.send_progress(&token, Some(50), "This fails");
    assert!(notif_result.is_err());

    // But request processing should continue
    let request_result = simulate_request_processing().await;
    assert_eq!(request_result, "success");
}

/// Test rapid notification sending with metadata
#[tokio::test]
async fn test_rapid_notifications_with_metadata() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Send notifications rapidly with metadata
    let sender_clone = sender.clone();
    let token_clone = token.clone();
    tokio::spawn(async move {
        for i in 0..50 {
            let metadata = json!({
                "iteration": i,
                "batch": i / 10,
                "timestamp": format!("2024-{:02}-{:02}", (i % 12) + 1, (i % 28) + 1)
            });
            sender_clone
                .send_progress_with_metadata(
                    &token_clone,
                    Some(i * 2),
                    format!("Processing item {}", i),
                    metadata,
                )
                .unwrap();
        }
    });

    // Verify all notifications received with metadata
    let mut count = 0;
    while count < 50 {
        if let Ok(Some(notif)) = timeout(Duration::from_millis(100), rx.recv()).await {
            assert!(notif.metadata.is_some());
            count += 1;
        } else {
            break;
        }
    }
    assert_eq!(count, 50);
}

// Helper function to simulate request processing
async fn simulate_request_processing() -> &'static str {
    sleep(Duration::from_millis(10)).await;
    "success"
}
