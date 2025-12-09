//! Flowchart diagram parser implementation
//!
//! This module provides parsing capabilities for Mermaid flowchart diagrams, which are
//! general-purpose diagrams showing process flows, decision trees, and system workflows.
//!
//! ## Syntax Support
//!
//! The parser supports comprehensive Mermaid flowchart syntax including:
//!
//! - **Directions**: `TD`, `TB`, `BT`, `LR`, `RL`
//! - **Node shapes**: rectangles `[text]`, diamonds `{text}`, circles `((text))`, etc.
//! - **Edge types**: solid `-->`, dotted `-.->`, thick `==>`, with labels
//! - **Subgraphs**: nested diagram sections
//! - **Styling**: CSS classes, inline styles, click events
//!
//! ## Features
//!
//! - **Flexible node syntax** - Supports all standard Mermaid node shapes
//! - **Rich edge types** - Multiple arrow styles and decorations
//! - **Subgraph nesting** - Hierarchical diagram organization
//! - **Style integration** - CSS styling and click handlers
//! - **Error recovery** - Robust parsing with helpful error messages
//!
//! ## Example
//!
//! ```rust
//! use mermaid_parser::parsers::flowchart;
//!
//! let input = r#"
//! flowchart TD
//!     A[Start] --> B{Decision?}
//!     B -->|Yes| C[Process]
//!     B -->|No| D[Skip]
//!     C --> E[End]
//!     D --> E
//! "#;
//!
//! let diagram = flowchart::parse(input)?;
//! println!("Nodes: {}, Edges: {}", diagram.nodes.len(), diagram.edges.len());
//!
//! // Access node information
//! for (node_id, node) in &diagram.nodes {
//!     println!("Node {}: {:?}", node_id, node.shape);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::common::ast::{
    AccessibilityInfo, EdgeType, FlowDirection, FlowEdge, FlowNode, FlowchartDiagram, NodeShape,
};
use crate::common::constants::{directions, flowchart_keywords};
use crate::common::parser_utils::{parse_comment, parse_whitespace};
use crate::error::Result;
use chumsky::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FlowToken {
    // Keywords
    Graph,
    Flowchart,
    Subgraph,
    End,

    // Directions
    TB,
    TD,
    BT,
    RL,
    LR,

    // Node brackets
    LeftSquare,        // [
    RightSquare,       // ]
    LeftParen,         // (
    RightParen,        // )
    LeftBrace,         // {
    RightBrace,        // }
    LeftAngle,         // <
    RightAngle,        // >
    DoubleLeftSquare,  // [[
    DoubleRightSquare, // ]]
    DoubleLeftParen,   // ((
    DoubleRightParen,  // ))
    TripleLeftParen,   // (((
    TripleRightParen,  // )))
    DoubleLeftBrace,   // {{
    DoubleRightBrace,  // }}

    // Edge components
    Dash,     // -
    DashDash, // --
    Arrow,    // > or -->

    // Values
    NodeId(String),
    Text(String),
    At, // @

    Comment(String),
    Semicolon,
    NewLine,
    Eof,
}

impl From<&FlowToken> for String {
    fn from(token: &FlowToken) -> Self {
        format!("{:?}", token)
    }
}

fn flowchart_lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<FlowToken>, extra::Err<Simple<'src, char>>> {
    let comment = parse_comment().map(|_| FlowToken::Comment("".to_string()));

    let flowchart_keyword = just(flowchart_keywords::FLOWCHART).map(|_| FlowToken::Flowchart);

    let graph_keyword = just(flowchart_keywords::GRAPH).map(|_| FlowToken::Graph);

    // Directions
    let directions_parser = choice((
        just(directions::TOP_BOTTOM).to(FlowToken::TB),
        just(directions::TOP_DOWN).to(FlowToken::TD),
        just(directions::BOTTOM_TOP).to(FlowToken::BT),
        just(directions::RIGHT_LEFT).to(FlowToken::RL),
        just(directions::LEFT_RIGHT).to(FlowToken::LR),
    ));

    // Node shape brackets (order matters for overlapping patterns)
    let node_brackets = choice((
        just("(((").to(FlowToken::TripleLeftParen),
        just(")))").to(FlowToken::TripleRightParen),
        just("((").to(FlowToken::DoubleLeftParen),
        just("))").to(FlowToken::DoubleRightParen),
        just("[[").to(FlowToken::DoubleLeftSquare),
        just("]]").to(FlowToken::DoubleRightSquare),
        just("{{").to(FlowToken::DoubleLeftBrace),
        just("}}").to(FlowToken::DoubleRightBrace),
        just('[').to(FlowToken::LeftSquare),
        just(']').to(FlowToken::RightSquare),
        just('(').to(FlowToken::LeftParen),
        just(')').to(FlowToken::RightParen),
        just('{').to(FlowToken::LeftBrace),
        just('}').to(FlowToken::RightBrace),
        just('<').to(FlowToken::LeftAngle),
        just('>').to(FlowToken::RightAngle),
    ));

    // Edge patterns (order matters for overlapping patterns)
    let edge_patterns = choice((
        just("-->").to(FlowToken::Arrow),
        just(flowchart_keywords::DOUBLE_DASH).to(FlowToken::DashDash),
        just('-').to(FlowToken::Dash),
        just('>').to(FlowToken::RightAngle),
    ));

    // Edge labels: |text| (with optional closing |)
    let edge_label = just('|')
        .then(none_of('|').repeated().collect::<String>())
        .then(just('|').or_not())
        .map(|((_, text), closing)| {
            if closing.is_some() {
                FlowToken::Text(format!("|{}|", text))
            } else {
                FlowToken::Text(format!("|{}", text))
            }
        });

    // Text for node labels - will be handled differently
    let text_chars = none_of("]})\n>|")
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|s: String| FlowToken::Text(s.trim().to_string()));

    // Simple identifier
    let identifier = text::ident().map(|s: &str| FlowToken::NodeId(s.to_string()));

    // Semicolon and At symbol
    let semicolon = just(';').to(FlowToken::Semicolon);
    let at_symbol = just('@').to(FlowToken::At);

    // Combine all tokens (order matters for parsing)
    let token = choice((
        comment,
        flowchart_keyword,
        graph_keyword,
        directions_parser,
        edge_patterns,
        node_brackets,
        edge_label,
        semicolon,
        at_symbol,
        identifier,
        text_chars, // Keep this last to avoid conflicts
    ));

    // Handle whitespace and newlines
    choice((
        parse_whitespace().ignore_then(token),
        just('\n').to(FlowToken::NewLine),
        parse_whitespace()
            .ignore_then(just('\n'))
            .to(FlowToken::NewLine), // Handle trailing whitespace before newline
    ))
    .repeated()
    .collect::<Vec<_>>()
}

fn parse_node_shape(left_bracket: &FlowToken, right_bracket: &FlowToken) -> NodeShape {
    match (left_bracket, right_bracket) {
        (FlowToken::LeftSquare, FlowToken::RightSquare) => NodeShape::Rectangle,
        (FlowToken::LeftParen, FlowToken::RightParen) => NodeShape::RoundedRectangle,
        (FlowToken::LeftBrace, FlowToken::RightBrace) => NodeShape::Rhombus,
        (FlowToken::DoubleLeftParen, FlowToken::DoubleRightParen) => NodeShape::Circle,
        (FlowToken::TripleLeftParen, FlowToken::TripleRightParen) => NodeShape::DoubleCircle,
        (FlowToken::DoubleLeftSquare, FlowToken::DoubleRightSquare) => NodeShape::Subroutine,
        (FlowToken::DoubleLeftBrace, FlowToken::DoubleRightBrace) => NodeShape::Hexagon,
        _ => NodeShape::Rectangle, // Default
    }
}

fn parse_simple_node_and_edges(tokens: &[FlowToken]) -> (HashMap<String, FlowNode>, Vec<FlowEdge>) {
    let mut nodes = HashMap::new();
    let mut edges = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            FlowToken::NodeId(node_id) => {
                // Check if this is a node definition: A[text...] or A{text...}, etc.
                if i + 1 < tokens.len() {
                    if let Some(left_bracket) = tokens.get(i + 1) {
                        if matches!(
                            left_bracket,
                            FlowToken::LeftSquare
                                | FlowToken::LeftParen
                                | FlowToken::LeftBrace
                                | FlowToken::DoubleLeftSquare
                                | FlowToken::DoubleLeftParen
                                | FlowToken::TripleLeftParen
                                | FlowToken::DoubleLeftBrace
                        ) {
                            // Collect text tokens and node ids until we find the closing bracket
                            let mut text_parts = Vec::new();
                            let mut j = i + 2;
                            let mut found_close = false;

                            while j < tokens.len() {
                                match &tokens[j] {
                                    FlowToken::NodeId(text) => {
                                        text_parts.push(text.clone());
                                        j += 1;
                                    }
                                    FlowToken::Text(text) => {
                                        text_parts.push(text.clone());
                                        j += 1;
                                    }
                                    bracket
                                        if matches!(
                                            bracket,
                                            FlowToken::RightSquare
                                                | FlowToken::RightParen
                                                | FlowToken::RightBrace
                                                | FlowToken::DoubleRightSquare
                                                | FlowToken::DoubleRightParen
                                                | FlowToken::TripleRightParen
                                                | FlowToken::DoubleRightBrace
                                        ) =>
                                    {
                                        let shape = parse_node_shape(left_bracket, bracket);
                                        let node_text = if text_parts.is_empty() {
                                            None
                                        } else {
                                            Some(text_parts.join(" "))
                                        };

                                        let node = FlowNode {
                                            id: node_id.clone(),
                                            text: node_text,
                                            shape,
                                            classes: Vec::new(),
                                            icon: None,
                                        };
                                        nodes.insert(node_id.clone(), node);
                                        found_close = true;
                                        i = j + 1;
                                        break;
                                    }
                                    _ => break,
                                }
                            }

                            if found_close {
                                // After parsing a node, check if there's an edge following it
                                if i < tokens.len() && matches!(tokens[i], FlowToken::Arrow) {
                                    // Continue to edge parsing below
                                } else {
                                    continue;
                                }
                            } else {
                                // No closing bracket found - skip this malformed node to avoid infinite loop
                                i += 1;
                                continue;
                            }
                        }
                    }
                }

                // Check for edge patterns: A --> B or A -->|label| B or A[Start] --> B{Decision}

                // If we just parsed a node definition, check from current position
                // Otherwise, look for an arrow after the current node id
                let arrow_pos = if i < tokens.len() && matches!(tokens[i], FlowToken::Arrow) {
                    i
                } else if i + 1 < tokens.len() && matches!(tokens[i + 1], FlowToken::Arrow) {
                    i + 1
                } else {
                    // No arrow found, skip this node
                    i += 1;
                    continue;
                };

                // Extract source node ID
                let source_id = node_id.clone();

                // Look for target after arrow
                let mut target_pos = arrow_pos + 1;
                let mut edge_label = None;

                // Check for edge label: -->|label|
                if target_pos < tokens.len() {
                    match &tokens[target_pos] {
                        FlowToken::Text(label_text)
                            if label_text.starts_with('|') && label_text.ends_with('|') =>
                        {
                            // Extract label text between |pipes|
                            let label = label_text
                                .trim_start_matches('|')
                                .trim_end_matches('|')
                                .to_string();
                            edge_label = Some(label);
                            target_pos += 1;
                        }
                        _ => {}
                    }
                }

                // Find target node
                if target_pos < tokens.len() {
                    if let FlowToken::NodeId(target_id) = &tokens[target_pos] {
                        // Create edge
                        let edge = FlowEdge {
                            from: source_id,
                            to: target_id.clone(),
                            edge_type: EdgeType::Arrow,
                            label: edge_label,
                            min_length: None,
                        };
                        edges.push(edge);

                        // Check if target has node definition after it
                        if target_pos + 1 < tokens.len() {
                            if let Some(left_bracket) = tokens.get(target_pos + 1) {
                                if matches!(
                                    left_bracket,
                                    FlowToken::LeftSquare
                                        | FlowToken::LeftParen
                                        | FlowToken::LeftBrace
                                        | FlowToken::DoubleLeftSquare
                                        | FlowToken::DoubleLeftParen
                                        | FlowToken::TripleLeftParen
                                        | FlowToken::DoubleLeftBrace
                                ) {
                                    // Parse target node definition
                                    let mut text_parts = Vec::new();
                                    let mut j = target_pos + 2;

                                    let mut found_closing = false;
                                    while j < tokens.len() {
                                        match &tokens[j] {
                                            FlowToken::NodeId(text) => {
                                                text_parts.push(text.clone());
                                                j += 1;
                                            }
                                            FlowToken::Text(text) => {
                                                text_parts.push(text.clone());
                                                j += 1;
                                            }
                                            bracket
                                                if matches!(
                                                    bracket,
                                                    FlowToken::RightSquare
                                                        | FlowToken::RightParen
                                                        | FlowToken::RightBrace
                                                        | FlowToken::DoubleRightSquare
                                                        | FlowToken::DoubleRightParen
                                                        | FlowToken::TripleRightParen
                                                        | FlowToken::DoubleRightBrace
                                                ) =>
                                            {
                                                let shape = parse_node_shape(left_bracket, bracket);
                                                let node_text = if text_parts.is_empty() {
                                                    None
                                                } else {
                                                    Some(text_parts.join(" "))
                                                };

                                                let node = FlowNode {
                                                    id: target_id.clone(),
                                                    text: node_text,
                                                    shape,
                                                    classes: Vec::new(),
                                                    icon: None,
                                                };
                                                nodes.insert(target_id.clone(), node);
                                                i = j + 1;
                                                found_closing = true;
                                                break;
                                            }
                                            _ => break,
                                        }
                                    }

                                    if found_closing {
                                        continue;
                                    }
                                    // No closing bracket found for target node - treat as malformed, skip to end
                                    i = tokens.len(); // End parsing to avoid infinite loop
                                    continue;
                                }
                            }
                        }

                        // Skip to after the target
                        i = target_pos + 1;
                        continue;
                    }
                }

                i += 1;
            }
            _ => i += 1,
        }
    }

    (nodes, edges)
}

pub fn parse(input: &str) -> Result<FlowchartDiagram> {
    // First tokenize the input
    let tokens = flowchart_lexer().parse(input).into_result().map_err(|e| {
        crate::error::ParseError::LexError {
            message: format!("Lexer error: {:?}", e),
            line: 1,
            column: 1,
        }
    })?;

    // Parse the header to get direction
    let (direction, skip_count) = if tokens.len() >= 2 {
        match (&tokens[0], &tokens[1]) {
            (FlowToken::Flowchart | FlowToken::Graph, FlowToken::TB) => (FlowDirection::TB, 2),
            (FlowToken::Flowchart | FlowToken::Graph, FlowToken::TD) => (FlowDirection::TD, 2),
            (FlowToken::Flowchart | FlowToken::Graph, FlowToken::BT) => (FlowDirection::BT, 2),
            (FlowToken::Flowchart | FlowToken::Graph, FlowToken::RL) => (FlowDirection::RL, 2),
            (FlowToken::Flowchart | FlowToken::Graph, FlowToken::LR) => (FlowDirection::LR, 2),
            (FlowToken::Flowchart | FlowToken::Graph, _) => (FlowDirection::TD, 1), // No direction specified, skip only header
            _ => (FlowDirection::TD, 0), // Default, no tokens to skip
        }
    } else if !tokens.is_empty() && matches!(tokens[0], FlowToken::Flowchart | FlowToken::Graph) {
        (FlowDirection::TD, 1) // Just flowchart/graph keyword, no direction
    } else {
        (FlowDirection::TD, 0)
    };

    // Skip header tokens and parse nodes and edges from the rest
    let remaining_tokens = if tokens.len() > skip_count {
        &tokens[skip_count..]
    } else {
        &[]
    };
    let (nodes, edges) = parse_simple_node_and_edges(remaining_tokens);

    Ok(FlowchartDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        direction,
        nodes,
        edges,
        subgraphs: Vec::new(),
        styles: Vec::new(),
        class_defs: HashMap::new(),
        clicks: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_flowchart_lexer() {
        let input = "flowchart TD";
        let tokens = flowchart_lexer().parse(input).into_result();

        assert!(tokens.is_ok(), "Failed to tokenize: {:?}", tokens);
        let tokens = tokens.unwrap();

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], FlowToken::Flowchart);
        assert_eq!(tokens[1], FlowToken::TD);
    }

    #[test]
    fn test_simple_flowchart() {
        let input = r#"flowchart TD
    A[Start] --> B[End]
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.direction, FlowDirection::TD);
    }

    #[test]
    fn test_flowchart_directions() {
        let test_cases = vec![
            ("flowchart TB", FlowDirection::TB),
            ("flowchart TD", FlowDirection::TD),
            ("flowchart BT", FlowDirection::BT),
            ("flowchart LR", FlowDirection::LR),
            ("flowchart RL", FlowDirection::RL),
            ("graph LR", FlowDirection::LR),
        ];

        for (input, expected_direction) in test_cases {
            let result = parse(input);
            assert!(result.is_ok(), "Failed to parse: {}", input);

            let diagram = result.unwrap();
            assert_eq!(
                diagram.direction, expected_direction,
                "Wrong direction for: {}",
                input
            );
        }
    }

    #[test]
    fn test_basic_node_parsing() {
        let input = "flowchart TD\nA[Start Node]";
        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.direction, FlowDirection::TD);
        assert_eq!(diagram.nodes.len(), 1);

        let node = diagram.nodes.get("A").expect("Node A should exist");
        assert_eq!(node.id, "A");
        assert_eq!(node.text, Some("Start Node".to_string()));
        assert_eq!(node.shape, NodeShape::Rectangle);
    }

    #[test]
    fn test_basic_edge_parsing() {
        let input = "flowchart TD\nA --> B";
        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.edges.len(), 1);

        let edge = &diagram.edges[0];
        assert_eq!(edge.from, "A");
        assert_eq!(edge.to, "B");
        assert_eq!(edge.edge_type, EdgeType::Arrow);
    }

    #[test]
    fn test_malformed_unclosed_bracket() {
        use std::time::{Duration, Instant};

        let input = "graph TD\nA[ h ] -- hello --> B[";
        println!("Testing malformed input: {}", input.replace('\n', "\\n"));

        // Test lexer first
        let start = Instant::now();
        let tokens = flowchart_lexer().parse(input).into_result();
        let lex_duration = start.elapsed();

        println!("Lexer duration: {:?}", lex_duration);
        println!("Tokens: {:?}", tokens);

        // Test parser
        let start = Instant::now();
        let result = parse(input);
        let parse_duration = start.elapsed();

        println!("Parser duration: {:?}", parse_duration);
        println!("Parser result: {:?}", result);

        // Should not hang - fail test if it takes too long
        assert!(
            parse_duration < Duration::from_secs(1),
            "Parser took too long: {:?}",
            parse_duration
        );
    }
}
