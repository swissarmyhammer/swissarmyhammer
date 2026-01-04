//! Comprehensive tests targeting missing coverage areas in packet.rs parser

use mermaid_parser::error::ParseError;
use mermaid_parser::parsers::packet;

#[cfg(test)]
mod packet_coverage_tests {
    use super::*;

    // Test error cases - empty input
    #[test]
    fn test_empty_input_error() {
        let result = packet::parse("");
        assert!(matches!(result, Err(ParseError::EmptyInput)));
    }

    // Test error cases - missing packet header
    #[test]
    fn test_missing_packet_header() {
        let input = "0-15: \"Source Port\"";
        let result = packet::parse(input);

        assert!(result.is_err());
        if let Err(ParseError::SyntaxError {
            message,
            expected,
            found,
            line,
            column,
        }) = result
        {
            assert_eq!(message, "Expected packet-beta or packet header");
            assert!(expected.contains(&"packet-beta".to_string()));
            assert!(expected.contains(&"packet".to_string()));
            assert_eq!(found, "0-15: \"Source Port\"");
            assert_eq!(line, 1);
            assert_eq!(column, 1);
        } else {
            panic!("Expected SyntaxError");
        }
    }

    // Test title directive
    #[test]
    fn test_title_directive() {
        let input = r#"packet-beta
title Network Packet Structure
0-7: "Type"
8-15: "Length"
"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.title, Some("Network Packet Structure".to_string()));
        assert_eq!(diagram.fields.len(), 2);
    }

    // Test title with extra whitespace
    #[test]
    fn test_title_with_whitespace() {
        let input = r#"packet-beta
title   TCP Header Format   
0-15: "Source Port"
"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.title, Some("TCP Header Format".to_string()));
    }

    // Test packet header without "beta"
    #[test]
    fn test_packet_header_without_beta() {
        let input = r#"packet
0-7: "Type"
8-15: "Length"
"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.fields.len(), 2);
        assert_eq!(diagram.fields[0].name, "Type");
    }

    // Test additive format (+16)
    #[test]
    fn test_additive_format() {
        let input = r#"packet-beta
0-7: "First"
+8: "Second"
+16: "Third"
"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.fields.len(), 3);

        // First field: 0-7
        assert_eq!(diagram.fields[0].start_bit, 0);
        assert_eq!(diagram.fields[0].end_bit, 7);
        assert_eq!(diagram.fields[0].name, "First");

        // Second field: 8-15 (8 bits starting from bit 8)
        assert_eq!(diagram.fields[1].start_bit, 8);
        assert_eq!(diagram.fields[1].end_bit, 15);
        assert_eq!(diagram.fields[1].name, "Second");

        // Third field: 16-31 (16 bits starting from bit 16)
        assert_eq!(diagram.fields[2].start_bit, 16);
        assert_eq!(diagram.fields[2].end_bit, 31);
        assert_eq!(diagram.fields[2].name, "Third");
    }

    // Test additive format zero width (+0)
    #[test]
    fn test_additive_format_zero_width() {
        let input = r#"packet-beta
0-7: "Header"
+0: "Marker"
+8: "Data"
"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.fields.len(), 3);

        // Zero width field - starts at next available bit (8) and ends at same bit
        assert_eq!(diagram.fields[1].start_bit, 8);
        assert_eq!(diagram.fields[1].end_bit, 8);
        assert_eq!(diagram.fields[1].name, "Marker");

        // Next field starts at the bit after the zero-width field
        assert_eq!(diagram.fields[2].start_bit, 9);
        assert_eq!(diagram.fields[2].end_bit, 16);
        assert_eq!(diagram.fields[2].name, "Data");
    }

    // Test invalid additive format
    #[test]
    fn test_invalid_additive_format() {
        let input = r#"packet-beta
+abc: "Invalid"
"#;

        let result = packet::parse(input);
        assert!(result.is_err());
        if let Err(ParseError::SyntaxError {
            message,
            expected,
            found,
            ..
        }) = result
        {
            assert_eq!(message, "Invalid bit width");
            assert!(expected.contains(&"number".to_string()));
            assert_eq!(found, "abc");
        } else {
            panic!("Expected SyntaxError for invalid additive format");
        }
    }

    // Test invalid start bit number
    #[test]
    fn test_invalid_start_bit() {
        let input = r#"packet-beta
abc-15: "Invalid"
"#;

        let result = packet::parse(input);
        assert!(result.is_err());
        if let Err(ParseError::SyntaxError {
            message,
            expected,
            found,
            ..
        }) = result
        {
            assert_eq!(message, "Invalid start bit number");
            assert!(expected.contains(&"number".to_string()));
            assert_eq!(found, "abc");
        } else {
            panic!("Expected SyntaxError for invalid start bit");
        }
    }

    // Test invalid end bit number
    #[test]
    fn test_invalid_end_bit() {
        let input = r#"packet-beta
0-xyz: "Invalid"
"#;

        let result = packet::parse(input);
        assert!(result.is_err());
        if let Err(ParseError::SyntaxError {
            message,
            expected,
            found,
            ..
        }) = result
        {
            assert_eq!(message, "Invalid end bit number");
            assert!(expected.contains(&"number".to_string()));
            assert_eq!(found, "xyz");
        } else {
            panic!("Expected SyntaxError for invalid end bit");
        }
    }

    // Test invalid single bit number
    #[test]
    fn test_invalid_single_bit() {
        let input = r#"packet-beta
def: "Invalid"
"#;

        let result = packet::parse(input);
        assert!(result.is_err());
        if let Err(ParseError::SyntaxError {
            message,
            expected,
            found,
            ..
        }) = result
        {
            assert_eq!(message, "Invalid bit number");
            assert!(expected.contains(&"number".to_string()));
            assert_eq!(found, "def");
        } else {
            panic!("Expected SyntaxError for invalid single bit");
        }
    }

    // Test comments and empty lines are ignored
    #[test]
    fn test_comments_and_empty_lines() {
        let input = r#"packet-beta

// This is a comment
%% This is also a comment

0-7: "Type"

// Another comment
8-15: "Length"

"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.fields.len(), 2);
        assert_eq!(diagram.fields[0].name, "Type");
        assert_eq!(diagram.fields[1].name, "Length");
    }

    // Test empty field names are skipped
    #[test]
    fn test_empty_field_names_skipped() {
        let input = r#"packet-beta
0-7: "Type"
8-15: 
16-23: "Data"
"#;

        let diagram = packet::parse(input);
        // Should parse successfully and skip the empty field
        assert!(diagram.is_ok());
        let diagram = diagram.unwrap();
        assert_eq!(diagram.fields.len(), 2); // Empty field should be skipped
        assert_eq!(diagram.fields[0].name, "Type");
        assert_eq!(diagram.fields[1].name, "Data");
    }

    // Test unrecognized lines are ignored (lines without colons)
    #[test]
    fn test_unrecognized_lines_ignored() {
        let input = r#"packet-beta
packet structure
some random text
0-7: "Type"
another random line
version 1.0
8-15: "Length"
diagram metadata
"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.fields.len(), 2);
        assert_eq!(diagram.fields[0].name, "Type");
        assert_eq!(diagram.fields[1].name, "Length");
    }

    // Test accessibility info is created with defaults
    #[test]
    fn test_accessibility_info_defaults() {
        let input = r#"packet-beta
0-7: "Type"
"#;

        let diagram = packet::parse(input).unwrap();
        assert!(diagram.accessibility.title.is_none());
        assert!(diagram.accessibility.description.is_none());
    }

    // Test mixed quoted and unquoted with optional fields
    #[test]
    fn test_complex_field_parsing() {
        let input = r#"packet-beta
title Complex Packet Format
0-3: Version
4-7: "Header Length"
8-15: (Options)
+16: "Payload"
32: Flag
"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.title, Some("Complex Packet Format".to_string()));
        assert_eq!(diagram.fields.len(), 5);

        // Unquoted
        assert_eq!(diagram.fields[0].name, "Version");
        assert!(!diagram.fields[0].is_optional);

        // Quoted
        assert_eq!(diagram.fields[1].name, "Header Length");
        assert!(!diagram.fields[1].is_optional);

        // Optional
        assert_eq!(diagram.fields[2].name, "Options");
        assert!(diagram.fields[2].is_optional);

        // Additive quoted
        assert_eq!(diagram.fields[3].name, "Payload");
        assert_eq!(diagram.fields[3].start_bit, 16);
        assert_eq!(diagram.fields[3].end_bit, 31);

        // Single bit
        assert_eq!(diagram.fields[4].name, "Flag");
        assert_eq!(diagram.fields[4].start_bit, 32);
        assert_eq!(diagram.fields[4].end_bit, 32);
    }

    // Test edge case: only comments and empty lines after header
    #[test]
    fn test_only_comments_after_header() {
        let input = r#"packet-beta
// Just comments
%% More comments

"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.fields.len(), 0);
        assert!(diagram.title.is_none());
    }

    // Test column position calculation for end bit error
    #[test]
    fn test_end_bit_error_column_position() {
        let input = r#"packet-beta
0-abc: "Test"
"#;

        let result = packet::parse(input);
        assert!(result.is_err());
        if let Err(ParseError::SyntaxError { column, .. }) = result {
            assert_eq!(column, 3); // Should point to start of 'abc' after '0-'
        } else {
            panic!("Expected SyntaxError with column position");
        }
    }

    // Test that field ordering is preserved
    #[test]
    fn test_field_ordering_preserved() {
        let input = r#"packet-beta
16-31: "Second"
0-7: "First"  
8-15: "Between"
32-39: "Last"
"#;

        let diagram = packet::parse(input).unwrap();
        assert_eq!(diagram.fields.len(), 4);

        // Should preserve order from input, not sort by bit position
        assert_eq!(diagram.fields[0].name, "Second");
        assert_eq!(diagram.fields[1].name, "First");
        assert_eq!(diagram.fields[2].name, "Between");
        assert_eq!(diagram.fields[3].name, "Last");
    }
}
