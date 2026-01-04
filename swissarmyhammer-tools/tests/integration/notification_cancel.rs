//! Tests for cancel notifications
//!
//! This test suite verifies that progress operations can be cancelled
//! and that cancellation notifications are properly sent and handled.

use swissarmyhammer_tools::mcp::progress_notifications::{
    generate_progress_token, ProgressNotification, ProgressSender,
};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout, Duration};

/// Create a cancellation notification for an operation
fn cancel_notification(token: &str, reason: impl Into<String>) -> ProgressNotification {
    let reason_str = reason.into();
    ProgressNotification {
        progress_token: token.to_string(),
        progress: None, // Indeterminate - operation was cancelled
        message: format!("Cancelled: {}", reason_str),
        metadata: Some(serde_json::json!({
            "cancelled": true,
            "reason": reason_str
        })),
    }
}

/// Test that cancel notifications can be sent
#[tokio::test]
async fn test_send_cancel_notification() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Send start notification
    sender.send_progress(&token, Some(0), "Starting").unwrap();

    // Send cancel notification
    let cancel = cancel_notification(&token, "User cancelled operation");
    sender.send(cancel).unwrap();

    // Verify start notification
    let notif1 = rx.recv().await.unwrap();
    assert_eq!(notif1.progress_token, token);
    assert_eq!(notif1.progress, Some(0));

    // Verify cancel notification
    let notif2 = rx.recv().await.unwrap();
    assert_eq!(notif2.progress_token, token);
    assert!(notif2.progress.is_none());
    assert!(notif2.message.contains("Cancelled"));
    assert!(notif2.metadata.is_some());

    // Verify metadata contains cancellation info
    let metadata = notif2.metadata.unwrap();
    assert_eq!(metadata["cancelled"], true);
    assert_eq!(metadata["reason"], "User cancelled operation");
}

/// Test cancel notification with different reasons
#[tokio::test]
async fn test_cancel_notification_with_reasons() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    let reasons = vec![
        "User cancelled",
        "Timeout exceeded",
        "Resource unavailable",
        "System shutdown",
    ];

    for reason in &reasons {
        let token = generate_progress_token();
        let cancel = cancel_notification(&token, *reason);
        sender.send(cancel).unwrap();
    }

    // Verify all cancel notifications
    for expected_reason in &reasons {
        let notif = rx.recv().await.unwrap();
        assert!(notif.message.contains("Cancelled"));
        let metadata = notif.metadata.unwrap();
        assert_eq!(metadata["cancelled"], true);
        assert_eq!(metadata["reason"], *expected_reason);
    }
}

/// Test cancel notification during operation
#[tokio::test]
async fn test_cancel_notification_during_operation() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Start operation
    sender.send_progress(&token, Some(0), "Starting").unwrap();

    // Send some progress updates
    sender.send_progress(&token, Some(25), "Working").unwrap();
    sender.send_progress(&token, Some(50), "Halfway").unwrap();

    // Cancel operation
    let cancel = cancel_notification(&token, "Operation cancelled by user");
    sender.send(cancel).unwrap();

    // Verify sequence
    let notif1 = rx.recv().await.unwrap();
    assert_eq!(notif1.progress, Some(0));

    let notif2 = rx.recv().await.unwrap();
    assert_eq!(notif2.progress, Some(25));

    let notif3 = rx.recv().await.unwrap();
    assert_eq!(notif3.progress, Some(50));

    let notif4 = rx.recv().await.unwrap();
    assert!(notif4.progress.is_none());
    assert!(notif4.message.contains("Cancelled"));
}

/// Test concurrent cancellation of multiple operations
#[tokio::test]
async fn test_concurrent_cancel_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    // Start multiple operations
    let mut tokens = Vec::new();
    for i in 0..5 {
        let token = generate_progress_token();
        tokens.push(token.clone());

        let sender_clone = sender.clone();
        tokio::spawn(async move {
            sender_clone
                .send_progress(&token, Some(0), format!("Operation {} starting", i))
                .unwrap();
            sleep(Duration::from_millis(10)).await;
            sender_clone
                .send_progress(&token, Some(50), format!("Operation {} working", i))
                .unwrap();
            sleep(Duration::from_millis(10)).await;
        });
    }

    // Wait a bit for operations to start
    sleep(Duration::from_millis(5)).await;

    // Cancel all operations
    for (i, token) in tokens.iter().enumerate() {
        let cancel = cancel_notification(token, format!("Operation {} cancelled", i));
        sender.send(cancel).unwrap();
    }

    // Collect notifications
    let mut notifications = Vec::new();
    while let Ok(notif) = timeout(Duration::from_millis(100), rx.recv()).await {
        if let Some(n) = notif {
            notifications.push(n);
        }
    }

    // Verify we got cancel notifications (at least 5)
    let cancel_count = notifications
        .iter()
        .filter(|n| n.message.contains("Cancelled") || n.message.contains("cancelled"))
        .count();
    assert!(
        cancel_count >= 5,
        "Should have at least 5 cancel notifications, got {}",
        cancel_count
    );
}

/// Test cancel notification with additional metadata
#[tokio::test]
async fn test_cancel_notification_with_metadata() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Create cancel notification with rich metadata
    let cancel = ProgressNotification {
        progress_token: token.clone(),
        progress: None,
        message: "Cancelled: Timeout exceeded".to_string(),
        metadata: Some(serde_json::json!({
            "cancelled": true,
            "reason": "Timeout exceeded",
            "duration_ms": 30000,
            "progress_at_cancel": 75,
            "partial_results": true
        })),
    };

    sender.send(cancel).unwrap();

    let notif = rx.recv().await.unwrap();
    assert_eq!(notif.progress_token, token);
    assert!(notif.message.contains("Cancelled"));

    let metadata = notif.metadata.unwrap();
    assert_eq!(metadata["cancelled"], true);
    assert_eq!(metadata["reason"], "Timeout exceeded");
    assert_eq!(metadata["duration_ms"], 30000);
    assert_eq!(metadata["progress_at_cancel"], 75);
    assert_eq!(metadata["partial_results"], true);
}

/// Test that cancel notifications don't interfere with other operations
#[tokio::test]
async fn test_cancel_notification_isolation() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    let token1 = generate_progress_token();
    let token2 = generate_progress_token();

    // Start two operations
    sender
        .send_progress(&token1, Some(0), "Operation 1 starting")
        .unwrap();
    sender
        .send_progress(&token2, Some(0), "Operation 2 starting")
        .unwrap();

    // Progress on both
    sender
        .send_progress(&token1, Some(50), "Operation 1 working")
        .unwrap();
    sender
        .send_progress(&token2, Some(50), "Operation 2 working")
        .unwrap();

    // Cancel only operation 1
    let cancel = cancel_notification(&token1, "User cancelled operation 1");
    sender.send(cancel).unwrap();

    // Operation 2 continues
    sender
        .send_progress(&token2, Some(100), "Operation 2 complete")
        .unwrap();

    // Verify notifications
    let mut notifications = Vec::new();
    while let Ok(notif) = timeout(Duration::from_millis(50), rx.recv()).await {
        if let Some(n) = notif {
            notifications.push(n);
        }
    }

    // Verify operation 1 was cancelled
    let op1_cancel = notifications
        .iter()
        .find(|n| n.progress_token == token1 && n.message.contains("Cancelled"));
    assert!(op1_cancel.is_some(), "Operation 1 should be cancelled");

    // Verify operation 2 completed
    let op2_complete = notifications
        .iter()
        .find(|n| n.progress_token == token2 && n.progress == Some(100));
    assert!(op2_complete.is_some(), "Operation 2 should complete");
}

/// Test cancel notification after channel closure
#[tokio::test]
async fn test_cancel_notification_channel_closed() {
    let (tx, rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Close receiver
    drop(rx);

    // Attempt to send cancel notification
    let cancel = cancel_notification(&token, "Operation cancelled");
    let result = sender.send(cancel);

    // Should fail gracefully
    assert!(result.is_err());
}

/// Test cancel notification serialization
#[tokio::test]
async fn test_cancel_notification_serialization() {
    let token = "test_token_123";
    let cancel = cancel_notification(token, "Test cancellation");

    // Serialize
    let json_str = serde_json::to_string(&cancel).unwrap();

    // Deserialize
    let deserialized: ProgressNotification = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.progress_token, token);
    assert!(deserialized.progress.is_none());
    assert!(deserialized.message.contains("Cancelled"));
    assert!(deserialized.metadata.is_some());

    let metadata = deserialized.metadata.unwrap();
    assert_eq!(metadata["cancelled"], true);
}

/// Test cancel notification in realistic scenario
#[tokio::test]
async fn test_realistic_cancel_scenario() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Simulate a long-running operation that gets cancelled
    let sender_clone = sender.clone();
    let token_clone = token.clone();
    let operation_task = tokio::spawn(async move {
        // Start
        sender_clone
            .send_progress(&token_clone, Some(0), "Starting file processing")
            .unwrap();

        // Simulate processing files
        for i in 1..=5 {
            sleep(Duration::from_millis(5)).await;
            sender_clone
                .send_progress(
                    &token_clone,
                    Some(i * 20),
                    format!("Processing file {}/5", i),
                )
                .unwrap();
        }

        // Check if cancelled (in real scenario, this would check a cancellation token)
        // For test purposes, we'll just cancel after some progress
        if true {
            // Simulating cancellation condition
            let cancel = ProgressNotification {
                progress_token: token_clone.clone(),
                progress: None,
                message: "Cancelled: User requested stop".to_string(),
                metadata: Some(serde_json::json!({
                    "cancelled": true,
                    "reason": "User requested stop",
                    "files_processed": 3,
                    "files_remaining": 2,
                    "progress_at_cancel": 60
                })),
            };
            sender_clone.send(cancel).unwrap();
            return "cancelled";
        }

        "completed"
    });

    // Collect notifications
    let mut notifications = Vec::new();
    while let Ok(notif) = timeout(Duration::from_millis(200), rx.recv()).await {
        if let Some(n) = notif {
            let is_cancel = n.message.contains("Cancelled");
            notifications.push(n);
            if is_cancel {
                break;
            }
        }
    }

    // Verify we got progress and then cancel
    assert!(!notifications.is_empty());

    let cancel_notif = notifications
        .iter()
        .find(|n| n.message.contains("Cancelled"));
    assert!(cancel_notif.is_some(), "Should have cancel notification");

    let cancel_notif = cancel_notif.unwrap();
    let metadata = cancel_notif.metadata.as_ref().unwrap();
    assert_eq!(metadata["cancelled"], true);
    assert_eq!(metadata["files_processed"], 3);
    assert_eq!(metadata["files_remaining"], 2);

    // Verify operation was cancelled
    let result = operation_task.await.unwrap();
    assert_eq!(result, "cancelled");
}

/// Test cancel notification token uniqueness
#[tokio::test]
async fn test_cancel_notification_token_uniqueness() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);

    let mut tokens = std::collections::HashSet::new();

    // Create multiple cancel notifications with unique tokens
    for i in 0..10 {
        let token = generate_progress_token();
        assert!(tokens.insert(token.clone()));

        let cancel = cancel_notification(&token, format!("Cancellation {}", i));
        sender.send(cancel).unwrap();
    }

    // Verify all cancel notifications have unique tokens
    let mut received_tokens = std::collections::HashSet::new();
    while let Ok(notif) = timeout(Duration::from_millis(50), rx.recv()).await {
        if let Some(n) = notif {
            received_tokens.insert(n.progress_token);
        }
    }

    assert_eq!(received_tokens.len(), 10);
}

/// Test cancel notification with empty reason
#[tokio::test]
async fn test_cancel_notification_empty_reason() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    let cancel = cancel_notification(&token, "");
    sender.send(cancel).unwrap();

    let notif = rx.recv().await.unwrap();
    assert!(notif.message.contains("Cancelled"));
}

/// Test cancel notification helpers work with sender
#[tokio::test]
async fn test_cancel_notification_helper_integration() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = ProgressSender::new(tx);
    let token = generate_progress_token();

    // Use helper to create and send cancel notification
    let cancel = cancel_notification(&token, "Test cancellation");
    sender.send(cancel).unwrap();

    // Verify notification received
    let notif = rx.recv().await.unwrap();
    assert_eq!(notif.progress_token, token);
    assert!(notif.progress.is_none());
    assert!(notif.message.contains("Cancelled"));

    let metadata = notif.metadata.unwrap();
    assert_eq!(metadata["cancelled"], true);
    assert_eq!(metadata["reason"], "Test cancellation");
}
