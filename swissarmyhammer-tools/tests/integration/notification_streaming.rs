//! Integration tests for notification streaming
//!
//! This test suite verifies that progress notifications are properly streamed
//! through the channel-based delivery system and can be received by MCP clients.

use serde_json::json;
use swissarmyhammer_tools::mcp::progress_notifications::{
    complete_notification, generate_progress_token, start_notification, ProgressNotification,
    ProgressSender,
};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

/// Test that basic notification streaming works
#[tokio::test]
async fn test_basic_notification_streaming() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    let token = generate_progress_token();

    // Send a series of notifications
    sender.send_progress(&token, Some(0), "Starting").unwrap();
    sender.send_progress(&token, Some(50), "Halfway").unwrap();
    sender.send_progress(&token, Some(100), "Complete").unwrap();

    // Verify all notifications are received in order
    let notif1 = rx.recv().await.unwrap();
    assert_eq!(notif1.progress_token, token);
    assert_eq!(notif1.progress, Some(0));
    assert_eq!(notif1.message, "Starting");

    let notif2 = rx.recv().await.unwrap();
    assert_eq!(notif2.progress_token, token);
    assert_eq!(notif2.progress, Some(50));
    assert_eq!(notif2.message, "Halfway");

    let notif3 = rx.recv().await.unwrap();
    assert_eq!(notif3.progress_token, token);
    assert_eq!(notif3.progress, Some(100));
    assert_eq!(notif3.message, "Complete");
}

/// Test that notifications can be sent concurrently from multiple tasks
#[tokio::test]
async fn test_concurrent_notification_streaming() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    // Spawn multiple tasks sending notifications
    let mut handles = vec![];
    for i in 0..5 {
        let sender_clone = sender.clone();
        let handle = tokio::spawn(async move {
            let token = generate_progress_token();
            sender_clone
                .send_progress(&token, Some(0), format!("Task {} starting", i))
                .unwrap();
            sender_clone
                .send_progress(&token, Some(100), format!("Task {} done", i))
                .unwrap();
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Should have received 10 notifications total (2 per task)
    let mut notifications = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        notifications.push(notif);
    }
    assert_eq!(notifications.len(), 10);
}

/// Test that rapid notification streaming doesn't lose messages
#[tokio::test]
async fn test_rapid_notification_streaming() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Send 100 notifications rapidly
    for i in 0..100 {
        sender
            .send_progress(&token, Some(i), format!("Progress: {}", i))
            .unwrap();
    }

    // Verify all 100 notifications are received
    let mut notifications = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        notifications.push(notif);
    }
    assert_eq!(notifications.len(), 100);

    // Verify they're in order
    for (i, notif) in notifications.iter().enumerate() {
        assert_eq!(notif.progress, Some(i as u32));
        assert_eq!(notif.message, format!("Progress: {}", i));
    }
}

/// Test notification streaming with metadata
#[tokio::test]
async fn test_notification_streaming_with_metadata() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    let metadata = json!({
        "files_processed": 42,
        "total_files": 100,
        "current_file": "src/main.rs"
    });

    sender
        .send_progress_with_metadata(&token, Some(42), "Processing files", metadata.clone())
        .unwrap();

    let notif = rx.recv().await.unwrap();
    assert_eq!(notif.progress_token, token);
    assert_eq!(notif.progress, Some(42));
    assert_eq!(notif.message, "Processing files");
    assert_eq!(notif.metadata, Some(metadata));
}

/// Test that indeterminate progress notifications work
#[tokio::test]
async fn test_indeterminate_progress_streaming() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Send notifications with indeterminate progress
    sender.send_progress(&token, None, "Processing...").unwrap();
    sender
        .send_progress(&token, None, "Still working...")
        .unwrap();
    sender.send_progress(&token, Some(100), "Done!").unwrap();

    let notif1 = rx.recv().await.unwrap();
    assert!(notif1.progress.is_none());

    let notif2 = rx.recv().await.unwrap();
    assert!(notif2.progress.is_none());

    let notif3 = rx.recv().await.unwrap();
    assert_eq!(notif3.progress, Some(100));
}

/// Test notification streaming with channel closure
#[tokio::test]
async fn test_notification_streaming_channel_closed() {
    let (tx, rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Close the receiver
    drop(rx);

    // Sending should fail gracefully
    let result = sender.send_progress(&token, Some(50), "This should fail");
    assert!(result.is_err());
}

/// Test notification streaming with cloned senders
#[tokio::test]
async fn test_notification_streaming_cloned_senders() {
    let (tx, mut rx) = mpsc::unbounded_channel::<ProgressNotification>();
    let sender1 = ProgressSender::new(tx);
    let sender2 = sender1.clone();

    let token1 = generate_progress_token();
    let token2 = generate_progress_token();

    // Send from first sender
    sender1
        .send_progress(&token1, Some(50), "From sender 1")
        .unwrap();

    // Send from cloned sender
    sender2
        .send_progress(&token2, Some(75), "From sender 2")
        .unwrap();

    // Both notifications should be received
    let notif1 = rx.recv().await.unwrap();
    let notif2 = rx.recv().await.unwrap();

    assert!(
        (notif1.progress_token == token1 && notif2.progress_token == token2)
            || (notif1.progress_token == token2 && notif2.progress_token == token1)
    );
}

/// Test start and complete notification helpers
#[tokio::test]
async fn test_start_and_complete_notification_helpers() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    let start = start_notification(&token, "Test operation");
    sender.send(start).unwrap();

    let complete = complete_notification(&token, "Test operation");
    sender.send(complete).unwrap();

    let notif1 = rx.recv().await.unwrap();
    assert_eq!(notif1.progress, Some(0));
    assert!(notif1.message.contains("Starting"));

    let notif2 = rx.recv().await.unwrap();
    assert_eq!(notif2.progress, Some(100));
    assert!(notif2.message.contains("Completed"));
}

/// Test notification streaming in a realistic scenario
#[tokio::test]
async fn test_realistic_notification_streaming_scenario() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Simulate a file processing operation
    let total_files = 10;

    // Start notification
    sender
        .send_progress(
            &token,
            Some(0),
            format!("Starting to process {} files", total_files),
        )
        .unwrap();

    // Process files with progress updates
    for i in 1..=total_files {
        // Simulate processing time
        sleep(Duration::from_millis(1)).await;

        let progress = (i * 100 / total_files) as u32;
        let metadata = json!({
            "files_processed": i,
            "total_files": total_files,
            "current_file": format!("file_{}.rs", i)
        });

        sender
            .send_progress_with_metadata(
                &token,
                Some(progress),
                format!("Processing file {}/{}", i, total_files),
                metadata,
            )
            .unwrap();
    }

    // Complete notification
    sender
        .send_progress(
            &token,
            Some(100),
            format!("Completed processing {} files", total_files),
        )
        .unwrap();

    // Verify all notifications received
    let mut notifications = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        notifications.push(notif);
    }

    // Should have: start + 10 progress + complete = 12 notifications
    assert_eq!(notifications.len(), 12);

    // Verify start notification
    assert_eq!(notifications[0].progress, Some(0));

    // Verify progress increases monotonically
    for i in 1..notifications.len() - 1 {
        let prev_progress = notifications[i - 1].progress.unwrap_or(0);
        let curr_progress = notifications[i].progress.unwrap_or(0);
        assert!(curr_progress >= prev_progress);
    }

    // Verify complete notification
    let last = notifications.last().unwrap();
    assert_eq!(last.progress, Some(100));
    assert!(last.message.contains("Completed"));
}

/// Test that progress sender can be used in optional scenarios
#[tokio::test]
async fn test_notification_streaming_optional_sender() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    // Store sender in Option to simulate ToolContext pattern
    let optional_sender: Option<ProgressSender> = Some(sender);

    let token = generate_progress_token();

    // Simulate a tool using optional sender
    if let Some(sender) = &optional_sender {
        sender
            .send_progress(&token, Some(0), "Tool starting")
            .unwrap();
        sender
            .send_progress(&token, Some(50), "Tool working")
            .unwrap();
        sender
            .send_progress(&token, Some(100), "Tool complete")
            .unwrap();
    }

    // Verify notifications received
    let notif1 = rx.recv().await.unwrap();
    assert_eq!(notif1.progress, Some(0));

    let notif2 = rx.recv().await.unwrap();
    assert_eq!(notif2.progress, Some(50));

    let notif3 = rx.recv().await.unwrap();
    assert_eq!(notif3.progress, Some(100));
}

/// Test notification token uniqueness
#[tokio::test]
async fn test_notification_token_uniqueness() {
    let mut tokens = std::collections::HashSet::new();

    // Generate 1000 tokens
    for _ in 0..1000 {
        let token = generate_progress_token();
        assert!(
            tokens.insert(token.clone()),
            "Duplicate token generated: {}",
            token
        );
    }

    assert_eq!(tokens.len(), 1000);
}

/// Test notification serialization and deserialization
#[tokio::test]
async fn test_notification_serialization() {
    let notification = ProgressNotification {
        progress_token: "token_123".to_string(),
        progress: Some(50),
        message: "Test message".to_string(),
        metadata: Some(json!({"key": "value"})),
    };

    // Serialize
    let json_str = serde_json::to_string(&notification).unwrap();

    // Deserialize
    let deserialized: ProgressNotification = serde_json::from_str(&json_str).unwrap();

    assert_eq!(notification.progress_token, deserialized.progress_token);
    assert_eq!(notification.progress, deserialized.progress);
    assert_eq!(notification.message, deserialized.message);
    assert_eq!(notification.metadata, deserialized.metadata);
}

/// Test that sender can be cloned and shared across threads
#[tokio::test]
async fn test_notification_sender_thread_safety() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    // Spawn tasks on different threads
    let mut handles = vec![];
    for i in 0..10 {
        let sender_clone = sender.clone();
        let handle = tokio::spawn(async move {
            let token = format!("thread_token_{}", i);
            sender_clone
                .send_progress(&token, Some(i * 10), format!("Thread {} progress", i))
                .unwrap();
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.await.unwrap();
    }

    // Collect notifications
    let mut notifications = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        notifications.push(notif);
    }

    assert_eq!(notifications.len(), 10);
}

/// Test notification streaming with large metadata
#[tokio::test]
async fn test_notification_streaming_with_large_metadata() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Create large metadata payload
    let large_data: Vec<String> = (0..100).map(|i| format!("file_{}.rs", i)).collect();
    let metadata = json!({
        "files": large_data,
        "stats": {
            "total": 100,
            "processed": 50,
            "errors": 2
        }
    });

    sender
        .send_progress_with_metadata(&token, Some(50), "Processing batch", metadata.clone())
        .unwrap();

    let notif = rx.recv().await.unwrap();
    assert_eq!(notif.metadata, Some(metadata));
}

/// Test notification batching scenario
#[tokio::test]
async fn test_notification_batching_scenario() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Simulate batched notifications (e.g., shell output)
    let batch_size = 5;
    let total_lines = 20;

    for batch in 0..(total_lines / batch_size) {
        let lines: Vec<String> = ((batch * batch_size)..((batch + 1) * batch_size))
            .map(|i| format!("Output line {}", i))
            .collect();

        let metadata = json!({
            "batch": batch,
            "lines": lines
        });

        sender
            .send_progress_with_metadata(
                &token,
                None, // Indeterminate for streaming output
                format!("Batch {}", batch),
                metadata,
            )
            .unwrap();
    }

    // Final completion
    sender
        .send_progress(&token, Some(100), "All output received")
        .unwrap();

    // Verify batches
    let mut notifications = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        notifications.push(notif);
    }

    // Should have 4 batches + 1 completion
    assert_eq!(notifications.len(), 5);

    // First 4 should be indeterminate
    for notif in &notifications[0..4] {
        assert!(notif.progress.is_none());
    }

    // Last should be complete
    assert_eq!(notifications.last().unwrap().progress, Some(100));
}
