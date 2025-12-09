use mermaid_parser::parsers::pie;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_pie_files(#[files("test/pie/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let _diagram = pie::parse(&content).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });

    // Pie charts can be empty placeholders - no additional validation needed
}

#[test]
fn test_simple_pie() {
    let input = r#"pie title NETFLIX
    "Time spent looking for movie" : 90
    "Time spent watching it" : 10
"#;

    let diagram = pie::parse(input).unwrap();

    assert_eq!(diagram.title, Some("NETFLIX".to_string()));
    assert_eq!(diagram.data.len(), 2);
    assert_eq!(diagram.data[0].label, "Time spent looking for movie");
    assert_eq!(diagram.data[0].value, 90.0);
    assert_eq!(diagram.data[1].value, 10.0);
}

#[test]
fn test_pie_with_show_data() {
    let input = r#"pie showData
    "A" : 386
    "B" : 85.5
    "C" : 15
"#;

    let diagram = pie::parse(input).unwrap();

    assert!(diagram.show_data);
    assert_eq!(diagram.data.len(), 3);
    assert_eq!(diagram.data[1].value, 85.5); // Test decimal values
}

#[test]
fn test_pie_separate_title() {
    let input = r#"pie
title My Chart Title
    "Category A" : 40
    "Category B" : 60
"#;

    let diagram = pie::parse(input).unwrap();

    assert_eq!(diagram.title, Some("My Chart Title".to_string()));
    assert_eq!(diagram.data.len(), 2);
}

#[test]
fn test_accessibility() {
    let input = r#"pie
accTitle: Pie Chart Accessibility Title
accDescr: This chart shows distribution
    "A" : 50
    "B" : 50
"#;

    let diagram = pie::parse(input).unwrap();

    assert_eq!(
        diagram.accessibility.title,
        Some("Pie Chart Accessibility Title".to_string())
    );
    assert_eq!(
        diagram.accessibility.description,
        Some("This chart shows distribution".to_string())
    );
}

#[test]
fn test_value_parsing() {
    #[allow(clippy::approx_constant)]
    let values = [("42", 42.0), ("3.14", 3.14), ("100", 100.0), ("0.5", 0.5)];

    for (input, expected) in values {
        let parsed: f64 = input.parse().unwrap();
        assert!((parsed - expected).abs() < 0.001);
    }
}

#[test]
fn test_basic_pie() {
    let input = r#"pie
    "Dogs" : 386
    "Cats" : 85
    "Rats" : 15
"#;

    let diagram = pie::parse(input).unwrap();

    assert_eq!(diagram.data.len(), 3);
    assert_eq!(diagram.data[0].label, "Dogs");
    assert_eq!(diagram.data[0].value, 386.0);
    assert_eq!(diagram.data[1].label, "Cats");
    assert_eq!(diagram.data[1].value, 85.0);
    assert_eq!(diagram.data[2].label, "Rats");
    assert_eq!(diagram.data[2].value, 15.0);
    assert!(!diagram.show_data);
    assert_eq!(diagram.title, None);
}

#[test]
fn test_pie_with_integer_values() {
    let input = r#"pie title Test Chart
    "First" : 100
    "Second" : 200
    "Third" : 300
"#;

    let diagram = pie::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Test Chart".to_string()));
    assert_eq!(diagram.data.len(), 3);

    for slice in &diagram.data {
        assert!(slice.value > 0.0);
        assert!(slice.value == slice.value.floor()); // All should be integers
    }
}

#[test]
fn test_pie_mixed_values() {
    let input = r#"pie showData
    "Integer" : 42
    "Float" : 3.141592653589793
    "Zero" : 0
"#;

    let diagram = pie::parse(input).unwrap();

    assert!(diagram.show_data);
    assert_eq!(diagram.data.len(), 3);

    assert_eq!(diagram.data[0].value, 42.0);
    assert!((diagram.data[1].value - std::f64::consts::PI).abs() < 0.001);
    assert_eq!(diagram.data[2].value, 0.0);
}
