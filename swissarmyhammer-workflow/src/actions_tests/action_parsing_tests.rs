//! Tests for action parsing functionality

use crate::parse_action_from_description;

#[test]
fn test_parse_action_from_description_prompt() {
    let description = r#"Execute prompt "test-prompt" with arg1="value1" arg2="value2""#;
    let action = parse_action_from_description(description).unwrap().unwrap();

    assert_eq!(action.action_type(), "prompt");
    assert!(action.description().contains("test-prompt"));
}

#[test]
fn test_parse_action_from_description_wait() {
    let description = "Wait 30 seconds";
    let action = parse_action_from_description(description).unwrap().unwrap();

    assert_eq!(action.action_type(), "wait");
    assert!(action.description().contains("30s"));
}

#[test]
fn test_parse_action_from_description_log() {
    let description = r#"Log "Test message""#;
    let action = parse_action_from_description(description).unwrap().unwrap();

    assert_eq!(action.action_type(), "log");
    assert!(action.description().contains("Test message"));
}

#[test]
fn test_parse_action_from_description_set_variable() {
    let description = r#"Set variable_name="value""#;
    let action = parse_action_from_description(description).unwrap().unwrap();

    assert_eq!(action.action_type(), "set_variable");
    assert!(action.description().contains("variable_name"));
}

#[test]
fn test_parse_action_from_description_set_variable_format() {
    let description = r#"set_variable result="success""#;
    let action = parse_action_from_description(description).unwrap().unwrap();

    assert_eq!(action.action_type(), "set_variable");
    assert!(action.description().contains("result"));
}

#[test]
fn test_parse_action_from_description_sub_workflow() {
    let description = r#"Run workflow "test-workflow" with input="value""#;
    let action = parse_action_from_description(description).unwrap().unwrap();

    assert_eq!(action.action_type(), "sub_workflow");
    assert!(action.description().contains("test-workflow"));
}

#[test]
fn test_parse_action_from_description_no_match() {
    let description = "This doesn't match any action pattern";
    let action = parse_action_from_description(description).unwrap();

    assert!(action.is_none());
}

#[test]
fn test_parse_action_from_description_empty() {
    let description = "";
    let action = parse_action_from_description(description).unwrap();

    assert!(action.is_none());
}

#[test]
fn test_parse_action_from_description_whitespace() {
    let description = "   \n\n   ";
    let action = parse_action_from_description(description).unwrap();

    assert!(action.is_none());
}
