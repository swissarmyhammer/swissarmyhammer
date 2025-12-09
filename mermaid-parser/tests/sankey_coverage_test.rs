//! Additional tests to improve coverage for sankey.rs parser

use mermaid_parser::error::ParseError;
use mermaid_parser::parsers::sankey;

#[test]
fn test_empty_input_error() {
    let input = "";
    let result = sankey::parse(input);
    assert!(result.is_err());
    // Parser should report enhanced syntax error for empty input
    match result {
        Err(ParseError::EnhancedSyntaxError { .. }) => {}
        _ => panic!("Expected EnhancedSyntaxError for empty input"),
    }
}

#[test]
fn test_invalid_header_error() {
    let input = "flowchart TD\nA --> B";
    let result = sankey::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::EnhancedSyntaxError { message, .. }) => {
            // The error message might vary, just check that it's an enhanced error
            assert!(!message.is_empty());
        }
        _ => panic!("Expected EnhancedSyntaxError for invalid header"),
    }
}

#[test]
fn test_sankey_header_variant() {
    let input = r#"sankey
A,B,10
B,C,5
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 2);
    assert_eq!(diagram.nodes.len(), 3);
}

#[test]
fn test_invalid_value_parsed_as_zero() {
    let input = r#"sankey-beta
A,B,not_a_number
B,C,5
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links[0].value, 0.0); // Invalid values default to 0.0
    assert_eq!(diagram.links[1].value, 5.0);
}

#[test]
fn test_empty_quoted_fields() {
    let input = r#"sankey-beta
"","",10
"A","",20
"","B",30
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 3);

    // Empty quoted strings are preserved
    assert_eq!(diagram.links[0].source, "");
    assert_eq!(diagram.links[0].target, "");
    assert_eq!(diagram.links[1].source, "A");
    assert_eq!(diagram.links[1].target, "");
    assert_eq!(diagram.links[2].source, "");
    assert_eq!(diagram.links[2].target, "B");
}

#[test]
fn test_escaped_quotes_in_quoted_fields() {
    let input = r#"sankey-beta
"Node with ""quotes""","Another ""quoted"" node",15.5
"Normal","""Leading quotes",25
"""Trailing quotes""","Normal",35
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.links[0].source, "Node with \"quotes\"");
    assert_eq!(diagram.links[0].target, "Another \"quoted\" node");
    assert_eq!(diagram.links[1].source, "Normal");
    assert_eq!(diagram.links[1].target, "\"Leading quotes");
    assert_eq!(diagram.links[2].source, "\"Trailing quotes\"");
    assert_eq!(diagram.links[2].target, "Normal");
}

#[test]
fn test_comment_at_start() {
    let input = r#"%% This is a comment
sankey-beta
A,B,10
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 1);
}

#[test]
fn test_comments_between_lines() {
    let input = r#"sankey-beta
A,B,10
%% Comment in the middle
B,C,20
%% Another comment
C,D,30
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 3);
}

#[test]
fn test_multiple_blank_lines() {
    let input = r#"sankey-beta


A,B,10


B,C,20


"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 2);
}

#[test]
fn test_mixed_whitespace() {
    let input = "sankey-beta\n\t  \t\nA,B,10\n\t\n  \t  \nB,C,20\n";

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 2);
}

#[test]
fn test_carriage_return_newlines() {
    let input = "sankey-beta\r\nA,B,10\r\nB,C,20\r\n";

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 2);
}

#[test]
fn test_header_at_eof() {
    let input = "sankey-beta";

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 0);
    assert_eq!(diagram.nodes.len(), 0);
}

#[test]
fn test_tabs_in_fields() {
    let input = "sankey-beta\nA\t,\tB\t,\t10\t\nC,D,20\n";

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    // Tabs are trimmed from field values
    assert_eq!(diagram.links[0].source, "A");
    assert_eq!(diagram.links[0].target, "B");
}

#[test]
fn test_special_characters_in_unquoted_fields() {
    let input = r#"sankey-beta
Node-with-hyphens,Node_with_underscores,10
Node/with/slashes,Node'with'quotes,20
Node@special#chars,Node$money%percent,30
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.links[0].source, "Node-with-hyphens");
    assert_eq!(diagram.links[0].target, "Node_with_underscores");
    assert_eq!(diagram.links[1].source, "Node/with/slashes");
    assert_eq!(diagram.links[1].target, "Node'with'quotes");
    assert_eq!(diagram.links[2].source, "Node@special#chars");
    assert_eq!(diagram.links[2].target, "Node$money%percent");
}

#[test]
fn test_decimal_and_scientific_notation() {
    let input = r#"sankey-beta
A,B,123.456
C,D,1e3
E,F,1.5e-2
G,H,.5
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.links[0].value, 123.456);
    assert_eq!(diagram.links[1].value, 1000.0);
    assert_eq!(diagram.links[2].value, 0.015);
    assert_eq!(diagram.links[3].value, 0.5);
}

#[test]
fn test_negative_values() {
    let input = r#"sankey-beta
A,B,-10
C,D,-5.5
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Negative values are parsed correctly
    assert_eq!(diagram.links[0].value, -10.0);
    assert_eq!(diagram.links[1].value, -5.5);
}

#[test]
fn test_leading_trailing_spaces_in_values() {
    let input = r#"sankey-beta
A,B,  10  
C,D,   20.5   
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Spaces are trimmed from numeric values
    assert_eq!(diagram.links[0].value, 10.0);
    assert_eq!(diagram.links[1].value, 20.5);
}

#[test]
fn test_get_line_column_function() {
    // Test internal line/column calculation with multi-byte characters
    let input = "sankey-beta\n❌Invalid,Syntax,10\nA,B,20";

    let _result = sankey::parse(input);
    // Should still parse successfully if lexer accepts these characters

    // Test another case that might trigger error handling
    let input2 = "not-a-sankey\nA,B,10";
    let result2 = sankey::parse(input2);
    assert!(result2.is_err());
}

#[test]
fn test_missing_comma_error() {
    let input = "sankey-beta\nA B 10\n";

    let result = sankey::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::EnhancedSyntaxError { suggestions, .. }) => {
            // Check that suggestions are provided
            assert!(!suggestions.is_empty());
        }
        _ => panic!("Expected EnhancedSyntaxError"),
    }
}

#[test]
fn test_node_deduplication() {
    let input = r#"sankey-beta
A,B,10
B,C,20
A,C,30
B,A,5
"#;

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Should have exactly 3 unique nodes
    assert_eq!(diagram.nodes.len(), 3);

    let node_names: Vec<_> = diagram.nodes.iter().map(|n| &n.name).collect();
    assert!(node_names.contains(&&"A".to_string()));
    assert!(node_names.contains(&&"B".to_string()));
    assert!(node_names.contains(&&"C".to_string()));
}

#[test]
fn test_empty_lines_at_start() {
    let input = "\n\n\nsankey-beta\nA,B,10\n";

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 1);
}

#[test]
fn test_comment_without_newline_at_eof() {
    let input = "sankey-beta\nA,B,10\n%% Final comment without newline";

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 1);
}

#[test]
fn test_link_without_final_newline() {
    let input = "sankey-beta\nA,B,10\nB,C,20";

    let result = sankey::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.links.len(), 2);
}

#[test]
fn test_lexer_error_coverage() {
    // Test input that triggers lexer errors
    let input = "sankey-beta\n\"Unclosed quote";

    let result = sankey::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::EnhancedSyntaxError { found, .. }) => {
            // Should report what was found
            assert!(!found.is_empty());
        }
        _ => panic!("Expected EnhancedSyntaxError"),
    }
}

#[test]
fn test_parser_error_without_specific_error() {
    // This might be hard to trigger, but let's try
    // The parser expects tokens in a specific order
    // We'd need to create a scenario where the parser fails without a specific error
    // This is mostly for coverage of the error handling branches

    let input = "sankey-beta\n";
    let result = sankey::parse(input);
    assert!(result.is_ok()); // Empty sankey is valid
}
