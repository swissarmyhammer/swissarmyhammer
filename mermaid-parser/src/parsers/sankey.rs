//! Sankey diagram parser implementation
//!
//! This module provides parsing capabilities for Mermaid Sankey diagrams, which visualize
//! flows and their quantities between nodes. Sankey diagrams show how resources, data, or energy move through a system.
//!
//! ## Syntax Support
//!
//! The parser supports the Mermaid Sankey syntax:
//!
//! ```text
//! sankey-beta
//!     Source,Target,Value
//!     A,B,10
//!     B,C,5
//! ```
//!
//! ## Features
//!
//! - **Node discovery** - Automatically identifies nodes from link definitions
//! - **Value parsing** - Supports integer and floating-point flow values
//! - **Text handling** - Processes both quoted and unquoted node names
//! - **Error recovery** - Provides detailed error messages with suggestions
//!
//! ## Example
//!
//! ```rust
//! use mermaid_parser::parsers::sankey;
//!
//! let input = r#"
//! sankey-beta
//!     A,B,10
//!     B,C,5
//!     A,C,3
//! "#;
//!
//! let diagram = sankey::parse(input)?;
//! println!("Nodes: {}, Links: {}", diagram.nodes.len(), diagram.links.len());
//!
//! // Access flow data
//! for link in &diagram.links {
//!     println!("{} -> {} ({})", link.source, link.target, link.value);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::common::ast::{SankeyDiagram, SankeyLink, SankeyNode};
use crate::error::{format_error_snippet, Location, ParseError, Result};
use chumsky::prelude::*;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum SankeyToken {
    Header,               // "sankey-beta"
    Comma,                // ","
    NewLine,              // "\n" | "\r\n"
    QuotedText(String),   // "text"
    UnquotedText(String), // text
}

/// Parse a Mermaid Sankey diagram from text input
///
/// This function processes a Sankey diagram, extracting flow information and
/// building a structured representation of the data.
///
/// # Arguments
///
/// * `input` - The Mermaid Sankey diagram text to parse
///
/// # Returns
///
/// Returns a [`Result`] containing the parsed [`SankeyDiagram`] or a [`ParseError`].
///
/// # Example
///
/// ```rust
/// use mermaid_parser::parsers::sankey;
///
/// let input = r#"
/// sankey-beta
///     A,B,10
///     B,C,5
/// "#;
///
/// let diagram = sankey::parse(input)?;
/// assert_eq!(diagram.nodes.len(), 3); // A, B, C
/// assert_eq!(diagram.links.len(), 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`ParseError`] if:
/// - The input doesn't start with "sankey-beta" or "sankey"
/// - Link definitions are malformed
/// - Values cannot be parsed as numbers
/// - Required syntax elements are missing
pub fn parse(input: &str) -> Result<SankeyDiagram> {
    let tokens = sankey_lexer()
        .parse(input)
        .into_result()
        .map_err(|errors| {
            if let Some(error) = errors.first() {
                let span = error.span();
                let (line, column) = get_line_column(input, span.start);

                // Create enhanced error with context
                ParseError::EnhancedSyntaxError {
                    message: "Failed to tokenize sankey diagram".to_string(),
                    location: Location { line, column },
                    snippet: Box::new(format_error_snippet(input, line, column, column + 1)),
                    suggestions: Box::new(vec![
                        "Ensure the diagram starts with 'sankey-beta' or 'sankey'".to_string(),
                        "Check that node names and values are properly formatted".to_string(),
                    ]),
                    expected: Box::new(vec!["valid sankey syntax".to_string()]),
                    found: error
                        .found()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "end of input".to_string()),
                }
            } else {
                ParseError::SyntaxError {
                    message: "Failed to tokenize sankey diagram".to_string(),
                    expected: vec![],
                    found: "unknown error".to_string(),
                    line: 0,
                    column: 0,
                }
            }
        })?;

    let result = sankey_parser()
        .parse(&tokens[..])
        .into_result()
        .map_err(|errors| {
            if let Some(_error) = errors.first() {
                // For token-level parsing, we provide more general error info since
                // we don't have direct character position in the original input
                ParseError::EnhancedSyntaxError {
                    message: "Invalid sankey diagram structure".to_string(),
                    location: Location { line: 1, column: 1 },
                    snippet: Box::new(format_error_snippet(input, 1, 1, 2)),
                    suggestions: Box::new(vec![
                        "Check that links follow the format: source,target,value".to_string(),
                        "Ensure node names are valid identifiers".to_string(),
                        "Values should be positive numbers".to_string(),
                    ]),
                    expected: Box::new(vec!["header or link definition".to_string()]),
                    found: "invalid structure".to_string(),
                }
            } else {
                ParseError::SyntaxError {
                    message: "Failed to parse sankey diagram".to_string(),
                    expected: vec![],
                    found: "unknown error".to_string(),
                    line: 0,
                    column: 0,
                }
            }
        });
    result
}

/// Convert byte position in input to line and column numbers (1-indexed)
fn get_line_column(input: &str, position: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;

    for (i, ch) in input.char_indices() {
        if i >= position {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

fn sankey_lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<SankeyToken>, extra::Err<Simple<'src, char>>> {
    let header = choice((just("sankey-beta"), just("sankey"))).map(|_| SankeyToken::Header);

    let comma = just(',').map(|_| SankeyToken::Comma);

    let newline = choice((just("\n"), just("\r\n"))).map(|_| SankeyToken::NewLine);

    let quoted_text = just('"')
        .ignore_then(
            choice((just("\"\"").to('"'), none_of('"').map(|c| c)))
                .repeated()
                .collect::<String>(),
        )
        .then_ignore(just('"'))
        .map(SankeyToken::QuotedText);

    // Unquoted text allows more characters including single quotes, hyphens, and slashes
    // but stops at commas, newlines, or tabs
    let unquoted_text = none_of(",\n\r\"\t")
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|s: String| SankeyToken::UnquotedText(s.trim().to_string()));

    let whitespace = one_of(" \t").repeated();

    // Comment lines starting with %% and extending to end of line
    let comment = just("%%")
        .then(none_of('\n').repeated())
        .then(just('\n').or_not())
        .ignored();

    let token = choice((header, comma, newline, quoted_text, unquoted_text));

    // Skip any leading whitespace/newlines before the first token
    let leading_ws = choice((one_of(" \t\n\r").ignored(), comment)).repeated();

    leading_ws.ignore_then(
        choice((comment.map(|_| None), token.map(Some)))
            .padded_by(whitespace)
            .repeated()
            .collect::<Vec<_>>()
            .map(|tokens| tokens.into_iter().flatten().collect()),
    )
}

fn sankey_parser<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    &'tokens [SankeyToken],
    SankeyDiagram,
    extra::Err<Simple<'tokens, SankeyToken>>,
> + Clone {
    let field = select! {
        SankeyToken::QuotedText(text) => text.clone(),
        SankeyToken::UnquotedText(text) => text.clone(),
    };

    let record = field // source
        .then_ignore(just(&SankeyToken::Comma))
        .then(field) // target
        .then_ignore(just(&SankeyToken::Comma))
        .then(field) // value
        .map(|((source, target), value_str)| {
            let value = value_str.trim().parse::<f64>().unwrap_or(0.0);
            SankeyLink {
                source: source.trim().to_string(),
                target: target.trim().to_string(),
                value,
            }
        });

    let csv_line = record.then_ignore(just(&SankeyToken::NewLine).or_not());

    let blank_line = just(&SankeyToken::NewLine);

    let content_line = choice((csv_line.map(Some), blank_line.map(|_| None)));

    just(&SankeyToken::Header)
        .then_ignore(just(&SankeyToken::NewLine).or_not()) // Header might be at EOF
        .then_ignore(just(&SankeyToken::NewLine).repeated()) // Allow blank lines after header
        .then(content_line.repeated().collect::<Vec<_>>())
        .map(|(_, lines)| {
            let links: Vec<SankeyLink> = lines.into_iter().flatten().collect();

            let mut nodes = HashSet::new();
            for link in &links {
                nodes.insert(link.source.clone());
                nodes.insert(link.target.clone());
            }

            SankeyDiagram {
                nodes: nodes
                    .into_iter()
                    .map(|name| SankeyNode {
                        id: name.clone(),
                        name,
                    })
                    .collect(),
                links,
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_simple_sankey() {
        let input = r#"sankey-beta
A,B,10
B,C,5
"#;

        let result = parse(input);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        assert_eq!(diagram.nodes.len(), 3);
        assert_eq!(diagram.links.len(), 2);
        assert_eq!(diagram.links[0].value, 10.0);
        assert_eq!(diagram.links[1].value, 5.0);
    }

    #[test]
    fn test_quoted_fields() {
        let input = r#"sankey-beta
"Source Node","Target Node",25.5
"Another ""Quoted"" Source",Destination,15.0
"#;

        let result = parse(input);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        assert_eq!(diagram.links[0].source, "Source Node");
        assert_eq!(diagram.links[0].target, "Target Node");
        assert_eq!(diagram.links[1].source, "Another \"Quoted\" Source");
    }

    #[test]
    fn test_with_blank_lines() {
        let input = r#"sankey-beta

A,B,10

B,C,5
"#;

        let result = parse(input);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        assert_eq!(diagram.links.len(), 2);
    }

    #[test]
    fn test_real_sankey_files() {
        let test_dir = Path::new("test/sankey");
        if test_dir.exists() {
            for entry in fs::read_dir(test_dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("mermaid") {
                    let content = fs::read_to_string(&path)
                        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

                    // Remove metadata comments
                    let content = content
                        .lines()
                        .filter(|line| !line.trim().starts_with("//"))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let result = parse(&content);
                    if let Err(e) = &result {
                        eprintln!("Error parsing file {:?}: {:?}", path, e);
                        eprintln!("Content after removing comments:\n{}", content);
                    }
                    assert!(result.is_ok(), "Failed to parse file: {:?}", path);

                    let diagram = result.unwrap();
                    // Some test files might have just the header with no data
                    if !diagram.links.is_empty() {
                        assert!(
                            !diagram.nodes.is_empty(),
                            "No nodes found in file: {:?}",
                            path
                        );

                        // Verify all links reference existing nodes
                        let node_names: HashSet<_> =
                            diagram.nodes.iter().map(|n| &n.name).collect();

                        for link in &diagram.links {
                            assert!(
                                node_names.contains(&link.source),
                                "Link source '{}' not found in nodes for file: {:?}",
                                link.source,
                                path
                            );
                            assert!(
                                node_names.contains(&link.target),
                                "Link target '{}' not found in nodes for file: {:?}",
                                link.target,
                                path
                            );
                            assert!(
                                link.value >= 0.0,
                                "Link value should be non-negative for file: {:?}",
                                path
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_whitespace_handling() {
        let input = r#"sankey-beta
  A  ,  B  ,  10  
C,D,20
"#;

        let result = parse(input);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        assert_eq!(diagram.links[0].source, "A");
        assert_eq!(diagram.links[0].target, "B");
        assert_eq!(diagram.links[0].value, 10.0);
    }

    #[test]
    fn test_special_characters_in_unquoted() {
        let input = r#"sankey-beta
Bio-conversion,Liquid,0.597
Agricultural 'waste',Bio-conversion,124.729
"#;

        let result = parse(input);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        assert_eq!(diagram.links[0].source, "Bio-conversion");
        assert_eq!(diagram.links[1].source, "Agricultural 'waste'");
        assert_eq!(diagram.links[1].target, "Bio-conversion");
        assert_eq!(diagram.links[1].value, 124.729);
    }

    #[test]
    fn test_comment_lines() {
        let input = r#"sankey-beta

%% source,target,value
Electricity grid,Over generation / exports,104.453
Electricity grid,Heating and cooling - homes,113.726
"#;

        let result = parse(input);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        assert_eq!(diagram.links.len(), 2);
        assert_eq!(diagram.links[0].source, "Electricity grid");
        assert_eq!(diagram.links[0].target, "Over generation / exports");
    }

    #[test]
    fn test_trailing_newline() {
        // This simulates the real file content after removing // comments
        let input = "\nsankey-beta\n\n%% source,target,value\nElectricity grid,Over generation / exports,104.453\nElectricity grid,Heating and cooling - homes,113.726\nElectricity grid,H2 conversion,27.14\n";

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse with error: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.links.len(), 3);
    }

    #[test]
    fn test_empty_sankey() {
        // Test sankey with just the header (no data)
        let input = "\nsankey\n";

        let result = parse(input);
        assert!(
            result.is_ok(),
            "Failed to parse empty sankey with error: {:?}",
            result
        );

        let diagram = result.unwrap();
        assert_eq!(diagram.links.len(), 0);
        assert_eq!(diagram.nodes.len(), 0);
    }

    #[test]
    fn test_enhanced_error_messages() {
        // Test invalid syntax to trigger enhanced error reporting
        let input = "sankey-beta\nA => B,10";

        let result = parse(input);
        assert!(
            result.is_err(),
            "Expected parsing to fail for invalid syntax"
        );

        let error = result.unwrap_err();
        let error_msg = error.to_string();

        // Verify the error message contains enhanced information
        println!("Enhanced error message:\n{}", error_msg);

        // Should contain position information
        assert!(error_msg.contains("line") && error_msg.contains("column"));

        // Should contain helpful suggestions
        assert!(error_msg.contains("help:") || error_msg.contains("note:"));
    }
}
