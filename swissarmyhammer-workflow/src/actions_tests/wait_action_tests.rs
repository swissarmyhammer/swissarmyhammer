//! Tests for WaitAction

use crate::workflow::actions::*;
use crate::workflow::actions_tests::common::*;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_wait_action_duration_creation() {
    let duration = Duration::from_secs(30);
    let action = WaitAction::new_duration(duration);

    assert_eq!(action.duration, Some(duration));
    assert!(action.message.is_none());
}

#[test]
fn test_wait_action_user_input_creation() {
    let action = WaitAction::new_user_input();

    assert!(action.duration.is_none());
    assert!(action.message.is_none());
}

#[test]
fn test_wait_action_with_message() {
    let action = WaitAction::new_duration(Duration::from_secs(10))
        .with_message("Please wait...".to_string());

    assert_eq!(action.message, Some("Please wait...".to_string()));
}

#[test]
fn test_wait_action_description() {
    let action = WaitAction::new_duration(Duration::from_secs(30));
    assert!(action.description().contains("30s"));

    let action = WaitAction::new_user_input();
    assert_eq!(action.description(), "Wait for user input");
}

#[test]
fn test_wait_action_type() {
    let action = WaitAction::new_duration(Duration::from_secs(10));
    assert_eq!(action.action_type(), "wait");
}

#[tokio::test]
async fn test_wait_action_duration_execution() {
    let action = WaitAction::new_duration(Duration::from_millis(100));
    let mut context = HashMap::new();

    let start = std::time::Instant::now();
    let result = action.execute(&mut context).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Null);
    assert!(elapsed >= Duration::from_millis(90)); // Allow some tolerance
    assert_eq!(context.get("last_action_result"), Some(&Value::Bool(true)));
}

#[tokio::test]
async fn test_wait_action_duration_with_message() {
    let action = WaitAction::new_duration(Duration::from_millis(10))
        .with_message("Processing...".to_string());
    let mut context = HashMap::new();

    let result = action.execute(&mut context).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Null);
}

// Note: Testing user input wait is complex in automated tests
// We'll focus on the timeout behavior and structure
#[tokio::test]
async fn test_wait_action_user_input_timeout_setup() {
    let action = WaitAction::new_user_input();
    // We can't actually test stdin reading in unit tests easily,
    // but we can verify the action structure
    assert!(action.duration.is_none());
    assert_eq!(action.action_type(), "wait");
}