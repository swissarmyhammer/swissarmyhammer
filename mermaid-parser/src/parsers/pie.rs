use crate::common::ast::{AccessibilityInfo, PieDiagram, PieSlice};
use crate::common::parser_utils::{parse_common_directives, CommonDirectiveParser};
use crate::error::{ParseError, Result};

/// Simple string-based parser for pie chart diagrams
pub fn parse(input: &str) -> Result<PieDiagram> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut diagram = PieDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        show_data: false,
        data: Vec::new(),
    };

    let mut first_line_processed = false;
    let mut common_parser = CommonDirectiveParser::new();

    for (line_num, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Handle first line validation manually for compound headers
        if !first_line_processed {
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("%%") {
                continue; // Skip empty lines and comments before header
            }

            if !trimmed.starts_with("pie") {
                return Err(ParseError::SyntaxError {
                    message: "Expected pie header".to_string(),
                    expected: vec!["pie".to_string()],
                    found: trimmed.to_string(),
                    line: line_num + 1,
                    column: 1,
                });
            }

            first_line_processed = true;
            // Continue processing this line as it may have additional content
        }

        // Handle common directives with multiline support
        if common_parser.parse_line(line, &mut diagram.title, &mut diagram.accessibility) {
            continue;
        }

        // Skip lines that are just whitespace or escape sequences like \t
        if trimmed.is_empty() || trimmed.chars().all(|c| c.is_whitespace()) || trimmed == "\\t" {
            continue;
        }

        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with("%%") {
            continue;
        }

        // Handle lines starting with \t followed by directives
        let effective_trimmed = if trimmed.starts_with("\\t") {
            trimmed.strip_prefix("\\t").unwrap_or(trimmed)
        } else {
            trimmed
        };

        // Handle pie-specific header variations
        if trimmed.starts_with("pie title ") {
            // Inline title: "pie title Chart Name"
            let title = trimmed.strip_prefix("pie title ").unwrap().trim();
            diagram.title = Some(title.to_string());
            continue;
        } else if trimmed == "pie showData" {
            // pie with showData
            diagram.show_data = true;
            continue;
        } else if trimmed.starts_with("pie accTitle:") {
            // pie accTitle: content - convert to standard format
            let acc_line = trimmed.strip_prefix("pie ").unwrap();
            parse_common_directives(acc_line, &mut diagram.title, &mut diagram.accessibility);
            continue;
        } else if trimmed.starts_with("pie accDescr:") {
            // pie accDescr: content - convert to standard format
            let acc_line = trimmed.strip_prefix("pie ").unwrap();
            parse_common_directives(acc_line, &mut diagram.title, &mut diagram.accessibility);
            continue;
        } else if trimmed.starts_with("pie accDescr {") {
            // pie accDescr { ... } - convert to standard format and start multiline block
            let acc_line = trimmed.strip_prefix("pie ").unwrap();
            common_parser.parse_line(acc_line, &mut diagram.title, &mut diagram.accessibility);
            continue;
        } else if trimmed.starts_with("pie ") {
            // "pie chart" or other text after pie - treat as title
            let title = trimmed.strip_prefix("pie ").unwrap().trim();
            if !title.is_empty() {
                diagram.title = Some(title.to_string());
            }
            continue;
        } else if trimmed == "pie" {
            // Just "pie" keyword alone
            continue;
        }

        // Handle directives - both direct and with \t prefix
        if parse_common_directives(line, &mut diagram.title, &mut diagram.accessibility) {
            // Directive was handled
        } else if effective_trimmed == "showData" {
            diagram.show_data = true;
        } else if effective_trimmed.starts_with("pie") {
            // Skip renderer-specific directives that start with "pie"
            // These are not part of standard Mermaid pie syntax but appear in test files
            continue;
        } else if let Some(colon_pos) = effective_trimmed.find(':') {
            // Parse data entry: "Label" : value
            let label_part = effective_trimmed[..colon_pos].trim();
            let value_part = effective_trimmed[colon_pos + 1..].trim();

            // Extract label (remove quotes if present)
            let label = if label_part.starts_with('"') && label_part.ends_with('"') {
                label_part[1..label_part.len() - 1].to_string()
            } else {
                label_part.to_string()
            };

            // Parse value
            match value_part.parse::<f64>() {
                Ok(value) => {
                    diagram.data.push(PieSlice { label, value });
                }
                Err(_) => {
                    return Err(ParseError::SyntaxError {
                        message: "Invalid numeric value".to_string(),
                        expected: vec!["number".to_string()],
                        found: value_part.to_string(),
                        line: line_num + 1,
                        column: colon_pos + 2,
                    });
                }
            }
        } else {
            return Err(ParseError::SyntaxError {
                message: "Expected data entry or directive".to_string(),
                expected: vec![
                    "\"label\" : value".to_string(),
                    "title".to_string(),
                    "showData".to_string(),
                ],
                found: effective_trimmed.to_string(),
                line: line_num + 1,
                column: 1,
            });
        }
    }

    // Accept pie charts with just the header - they might be placeholders or incomplete

    Ok(diagram)
}
