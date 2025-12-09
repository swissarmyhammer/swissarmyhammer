//! Assertion helpers for tests

use mermaid_parser::{error::ParseError, DiagramType};
use std::path::PathBuf;

/// Asserts that parsing succeeds and returns the correct diagram type
#[allow(dead_code)]
pub fn assert_parse_success<T>(result: Result<DiagramType, ParseError>, path: &PathBuf) -> T
where
    T: TryFrom<DiagramType>,
    T::Error: std::fmt::Debug,
{
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);

    let diagram = result.unwrap();
    match T::try_from(diagram) {
        Ok(typed_diagram) => typed_diagram,
        Err(err) => panic!("Wrong diagram type for {:?}: {:?}", path, err),
    }
}

/// Asserts that parsing succeeds without checking the specific type
#[allow(dead_code)]
pub fn assert_parse_success_any(result: Result<DiagramType, ParseError>, path: &PathBuf) {
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);
}

/// Asserts accessibility information matches expected values
#[allow(dead_code)]
pub fn assert_accessibility(
    accessibility: &mermaid_parser::common::ast::AccessibilityInfo,
    expected_title: Option<&str>,
    expected_desc: Option<&str>,
) {
    assert_eq!(
        accessibility.title,
        expected_title.map(|s| s.to_string()),
        "Accessibility title mismatch"
    );
    assert_eq!(
        accessibility.description,
        expected_desc.map(|s| s.to_string()),
        "Accessibility description mismatch"
    );
}

/// Validates that a field is not empty
#[allow(dead_code)]
pub fn assert_non_empty_field(field: &str, field_name: &str, context: &str) {
    assert!(
        !field.is_empty(),
        "{} should not be empty in {}",
        field_name,
        context
    );
}

/// Validates that a collection is not empty  
#[allow(dead_code)]
pub fn assert_non_empty_collection<T>(collection: &[T], collection_name: &str, context: &str) {
    assert!(
        !collection.is_empty(),
        "{} should not be empty in {}",
        collection_name,
        context
    );
}

/// Helper to create accessibility test input
#[allow(dead_code)]
pub fn create_accessibility_test_input(
    diagram_type: &str,
    title: &str,
    desc: &str,
    content: &str,
) -> String {
    format!(
        "{}\naccTitle: {}\naccDescr: {}\n{}",
        diagram_type, title, desc, content
    )
}

/// Helper to create simple diagram test input
#[allow(dead_code)]
pub fn create_simple_diagram_input(diagram_type: &str, content: &str) -> String {
    format!("{}\n{}", diagram_type, content)
}
