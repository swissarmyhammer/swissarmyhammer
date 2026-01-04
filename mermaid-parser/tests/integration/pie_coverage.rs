//! Additional tests to improve coverage for pie.rs parser

use mermaid_parser::error::ParseError;
use mermaid_parser::parsers::pie;

#[test]
fn test_empty_input_error() {
    let input = "";
    let result = pie::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::EmptyInput) => {}
        _ => panic!("Expected EmptyInput error"),
    }
}

#[test]
fn test_missing_header_error() {
    let input = r#"title My Chart
    "Label" : 42"#;

    let result = pie::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::SyntaxError { message, .. }) => {
            assert!(message.contains("Expected pie header"));
        }
        _ => panic!("Expected SyntaxError for missing header"),
    }
}

#[test]
fn test_pie_header_with_acc_title() {
    let input = r#"pie accTitle: Chart Accessibility Title
    "Data A" : 30
    "Data B" : 70"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(
        diagram.accessibility.title,
        Some("Chart Accessibility Title".to_string())
    );
    assert_eq!(diagram.data.len(), 2);
}

#[test]
fn test_pie_header_with_acc_descr() {
    let input = r#"pie accDescr: This chart shows data distribution
    "Category 1" : 25
    "Category 2" : 75"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(
        diagram.accessibility.description,
        Some("This chart shows data distribution".to_string())
    );
    assert_eq!(diagram.data.len(), 2);
}

#[test]
fn test_pie_header_with_multiline_acc_descr() {
    let input = r#"pie accDescr {
This is a multiline
accessibility description
that spans multiple lines
}
    "Item A" : 40
    "Item B" : 60"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(
        diagram.accessibility.description,
        Some("This is a multiline accessibility description that spans multiple lines".to_string())
    );
    assert_eq!(diagram.data.len(), 2);
}

#[test]
fn test_pie_header_with_text_after_pie() {
    let input = r#"pie chart data visualization
    "First Item" : 10
    "Second Item" : 20"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.title, Some("chart data visualization".to_string()));
    assert_eq!(diagram.data.len(), 2);
}

#[test]
fn test_pie_header_just_pie_keyword() {
    let input = r#"pie
    "Data Point" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.title, None);
    assert_eq!(diagram.data.len(), 1);
}

#[test]
fn test_standalone_directives_after_header() {
    let input = r#"pie
title Standalone Title
showData
accTitle: Standalone Accessibility Title
accDescr: Standalone accessibility description
    "Value 1" : 50
    "Value 2" : 50"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.title, Some("Standalone Title".to_string()));
    assert!(diagram.show_data);
    assert_eq!(
        diagram.accessibility.title,
        Some("Standalone Accessibility Title".to_string())
    );
    assert_eq!(
        diagram.accessibility.description,
        Some("Standalone accessibility description".to_string())
    );
    assert_eq!(diagram.data.len(), 2);
}

#[test]
fn test_standalone_multiline_acc_descr() {
    let input = r#"pie
accDescr {
This is a standalone
multiline accessibility
description block
}
    "Test" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(
        diagram.accessibility.description,
        Some("This is a standalone multiline accessibility description block".to_string())
    );
    assert_eq!(diagram.data.len(), 1);
}

#[test]
fn test_labels_without_quotes() {
    let input = r#"pie
    Label1 : 25
    Label2 : 75"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.data.len(), 2);
    assert_eq!(diagram.data[0].label, "Label1");
    assert_eq!(diagram.data[1].label, "Label2");
}

#[test]
fn test_labels_with_quotes() {
    let input = r#"pie
    "Label with spaces" : 30
    "Another label" : 70"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.data.len(), 2);
    assert_eq!(diagram.data[0].label, "Label with spaces");
    assert_eq!(diagram.data[1].label, "Another label");
}

#[test]
fn test_invalid_numeric_value() {
    let input = r#"pie
    "Valid" : 42
    "Invalid" : not_a_number"#;

    let result = pie::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::SyntaxError { message, .. }) => {
            assert!(message.contains("Invalid numeric value"));
        }
        _ => panic!("Expected SyntaxError for invalid numeric value"),
    }
}

#[test]
fn test_unrecognized_directive() {
    let input = r#"pie
    unknownDirective something
    "Data" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::SyntaxError { message, .. }) => {
            assert!(message.contains("Expected data entry or directive"));
        }
        _ => panic!("Expected SyntaxError for unrecognized directive"),
    }
}

#[test]
fn test_comments_and_empty_lines() {
    let input = r#"pie title Test Chart
    // This is a comment
    
    %% This is also a comment
    
    "Data A" : 40
    // Another comment
    "Data B" : 60
    
    %% Final comment"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.title, Some("Test Chart".to_string()));
    assert_eq!(diagram.data.len(), 2);
}

#[test]
fn test_backslash_tab_escapes() {
    let input = r#"pie
\ttitle Escaped Tab Title
\tshowData
\t"Category" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.title, Some("Escaped Tab Title".to_string()));
    assert!(diagram.show_data);
    assert_eq!(diagram.data.len(), 1);
}

#[test]
fn test_whitespace_only_lines() {
    let input = r#"pie
    
\t
    
    "Data" : 50"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.data.len(), 1);
}

#[test]
fn test_empty_pie_chart() {
    let input = r#"pie title Empty Chart"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.title, Some("Empty Chart".to_string()));
    assert_eq!(diagram.data.len(), 0);
}

#[test]
fn test_multiline_acc_descr_with_comments() {
    let input = r#"pie accDescr {
This is content
// This comment should be ignored in multiline block
More content here
%% Another comment to ignore
Final content
}
    "Test" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(
        diagram.accessibility.description,
        Some("This is content More content here Final content".to_string())
    );
}

#[test]
fn test_multiline_acc_descr_empty() {
    let input = r#"pie accDescr {
// Only comments
%% Nothing else
}
    "Test" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.accessibility.description, None);
}

#[test]
fn test_complex_numeric_values() {
    let input = r#"pie
    "Zero" : 0
    "Negative" : -5.5
    "Large" : 999999.123456
    "Scientific" : 1.23e10"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.data.len(), 4);
    assert_eq!(diagram.data[0].value, 0.0);
    assert_eq!(diagram.data[1].value, -5.5);
    assert_eq!(diagram.data[2].value, 999999.123456);
    assert_eq!(diagram.data[3].value, 1.23e10);
}

#[test]
fn test_mixed_quoted_unquoted_labels() {
    let input = r#"pie
    UnquotedLabel : 25
    "Quoted Label" : 35
    AnotherUnquoted : 40"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.data.len(), 3);
    assert_eq!(diagram.data[0].label, "UnquotedLabel");
    assert_eq!(diagram.data[1].label, "Quoted Label");
    assert_eq!(diagram.data[2].label, "AnotherUnquoted");
}

#[test]
fn test_pie_showdata_with_data() {
    let input = r#"pie showData
    "Visible Data" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert!(diagram.show_data);
    assert_eq!(diagram.data.len(), 1);
    assert_eq!(diagram.data[0].label, "Visible Data");
}

#[test]
fn test_overwriting_title() {
    let input = r#"pie title First Title
title Second Title
    "Data" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    // The standalone title directive should overwrite the inline title
    assert_eq!(diagram.title, Some("Second Title".to_string()));
}

#[test]
fn test_overwriting_accessibility() {
    let input = r#"pie accTitle: First Title
accTitle: Second Title
accDescr: First Description
accDescr: Second Description
    "Data" : 100"#;

    let result = pie::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(
        diagram.accessibility.title,
        Some("Second Title".to_string())
    );
    assert_eq!(
        diagram.accessibility.description,
        Some("Second Description".to_string())
    );
}

#[test]
fn test_all_pie_header_variations() {
    // Test each header variation separately
    let variations = vec![
        ("pie title Test", Some("Test".to_string()), false),
        ("pie showData", None, true),
        ("pie", None, false),
        ("pie custom text", Some("custom text".to_string()), false),
    ];

    for (header, expected_title, expected_show_data) in variations {
        let input = format!("{}\n    \"Data\" : 42", header);
        let result = pie::parse(&input);
        assert!(result.is_ok(), "Failed to parse header: {}", header);
        let diagram = result.unwrap();
        assert_eq!(diagram.title, expected_title);
        assert_eq!(diagram.show_data, expected_show_data);
        assert_eq!(diagram.data.len(), 1);
    }
}
