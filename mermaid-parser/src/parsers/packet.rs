use crate::common::ast::{AccessibilityInfo, PacketDiagram, PacketField};
use crate::common::parser_utils::{parse_common_directives, validate_diagram_header};
use crate::common::parsing::{brackets, key_value, quoted_strings};
use crate::error::{ParseError, Result};

/// Simple string-based parser for packet diagrams
pub fn parse(input: &str) -> Result<PacketDiagram> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut diagram = PacketDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        fields: Vec::new(),
    };

    let mut first_line_processed = false;

    for (line_num, line) in lines.iter().enumerate() {
        // Use shared header validation utility
        let (should_skip, trimmed) = validate_diagram_header(
            line,
            line_num,
            &["packet-beta", "packet"],
            &mut first_line_processed,
        )?;
        if should_skip {
            continue;
        }

        // Handle common directives (title, accTitle, accDescr)
        if parse_common_directives(line, &mut diagram.title, &mut diagram.accessibility) {
            continue;
        }

        // Parse field definition: "0-15: \"Source Port\"" or "+16: \"Source Port\""
        if let Some((range_part, field_part)) = key_value::parse_colon_separated(trimmed) {
            // Parse bit range - handle different formats
            let (start_bit, end_bit) = if range_part.starts_with('+') {
                // Additive format: "+16" means next 16 bits from current position
                let width_str = range_part.strip_prefix('+').unwrap().trim();
                let width = width_str
                    .parse::<u32>()
                    .map_err(|_| ParseError::SyntaxError {
                        message: "Invalid bit width".to_string(),
                        expected: vec!["number".to_string()],
                        found: width_str.to_string(),
                        line: line_num + 1,
                        column: 1,
                    })?;

                // Calculate start bit based on previous fields
                let current_bit = diagram
                    .fields
                    .iter()
                    .map(|f| f.end_bit + 1)
                    .max()
                    .unwrap_or(0);

                // Handle zero width (special case)
                if width == 0 {
                    (current_bit, current_bit)
                } else {
                    (current_bit, current_bit + width - 1)
                }
            } else if let Some(dash_pos) = range_part.find('-') {
                // Range: "0-15"
                let start_str = range_part[..dash_pos].trim();
                let end_str = range_part[dash_pos + 1..].trim();

                let start = start_str
                    .parse::<u32>()
                    .map_err(|_| ParseError::SyntaxError {
                        message: "Invalid start bit number".to_string(),
                        expected: vec!["number".to_string()],
                        found: start_str.to_string(),
                        line: line_num + 1,
                        column: 1,
                    })?;

                let end = end_str
                    .parse::<u32>()
                    .map_err(|_| ParseError::SyntaxError {
                        message: "Invalid end bit number".to_string(),
                        expected: vec!["number".to_string()],
                        found: end_str.to_string(),
                        line: line_num + 1,
                        column: dash_pos + 2,
                    })?;

                (start, end)
            } else {
                // Single bit: "106"
                let bit = range_part
                    .parse::<u32>()
                    .map_err(|_| ParseError::SyntaxError {
                        message: "Invalid bit number".to_string(),
                        expected: vec!["number".to_string()],
                        found: range_part.to_string(),
                        line: line_num + 1,
                        column: 1,
                    })?;

                (bit, bit)
            };

            // Parse field name (handle quoted strings, parentheses, and identifiers)
            let (name, is_optional) = if field_part.is_empty() {
                // Empty field name - skip this field
                continue;
            } else if let Some(optional_name) = brackets::extract_paren_content(&field_part) {
                // Optional field: (Options and Padding)
                (optional_name, true)
            } else {
                // Quoted or unquoted field: "Source Port" or Source
                let (name, _was_quoted) = quoted_strings::parse_field(&field_part);
                (name, false)
            };

            diagram.fields.push(PacketField {
                start_bit,
                end_bit,
                name,
                is_optional,
            });
        }
        // Handle other packet directives or ignore unrecognized lines
        // Some test files have various directives like "packet structure", "packetTitle", etc.
        // We'll skip these for now to be more lenient
    }

    Ok(diagram)
}
