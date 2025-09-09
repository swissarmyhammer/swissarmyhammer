//! Tests for error handling in actions

use super::*;

#[test]
fn test_action_error_display() {
    let error = ActionError::ClaudeError("Test error".to_string());
    assert!(error.to_string().contains("Claude execution failed"));
    assert!(error.to_string().contains("Test error"));

    let error = ActionError::VariableError("Variable error".to_string());
    assert!(error.to_string().contains("Variable operation failed"));

    let error = ActionError::ParseError("Parse error".to_string());
    assert!(error.to_string().contains("Action parsing failed"));

    let error = ActionError::Timeout {
        timeout: Duration::from_secs(30),
    };
    assert!(error.to_string().contains("timed out"));
    assert!(error.to_string().contains("30s"));

    let error = ActionError::ExecutionError("Execution error".to_string());
    assert!(error.to_string().contains("Action execution failed"));
}

#[test]
fn test_action_error_from_io_error() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let action_error = ActionError::from(io_error);

    match action_error {
        ActionError::IoError(_) => {
            assert!(action_error.to_string().contains("IO error"));
        }
        _ => panic!("Expected IoError"),
    }
}

#[test]
fn test_action_error_from_json_error() {
    let json_error = serde_json::from_str::<Value>("invalid json").unwrap_err();
    let action_error = ActionError::from(json_error);

    match action_error {
        ActionError::JsonError(_) => {
            assert!(action_error.to_string().contains("JSON parsing error"));
        }
        _ => panic!("Expected JsonError"),
    }
}
