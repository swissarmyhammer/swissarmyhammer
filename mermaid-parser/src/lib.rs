//! # Mermaid Parser
//!
//! A high-performance Rust parser for [Mermaid](https://mermaid.js.org/) diagrams using the
//! [Chumsky](https://github.com/zesterer/chumsky) parser combinator library.
//! This crate focuses on parsing Mermaid syntax into Abstract Syntax Trees ([`DiagramType`])
//! without rendering graphics.
//!
//! ## Features
//!
//! - 🚀 **Fast parsing** using advanced parser combinators
//! - 📊 **Comprehensive diagram support** for 20+ diagram types
//! - 🔍 **Detailed error reporting** with [`ParseError`] and source location information
//! - 🧪 **Visitor pattern** support via [`AstVisitor`] for AST analysis
//! - 📖 **Type-safe API** with full rustdoc documentation
//! - ⚡ **Memory efficient** with zero-copy parsing where possible
//!
//! ## Supported Diagram Types
//!
//! The parser supports all major Mermaid diagram types through the [`DiagramType`] enum:
//!
//! - **[`DiagramType::Sankey`]**: Flow data visualization with weighted connections
//! - **[`DiagramType::Timeline`]**: Chronological event sequences  
//! - **[`DiagramType::Journey`]**: User experience mapping with satisfaction scores
//! - **[`DiagramType::Sequence`]**: Message passing between actors over time
//! - **[`DiagramType::Class`]**: Object-oriented class relationships
//! - **[`DiagramType::State`]**: State machine representations
//! - **[`DiagramType::Flowchart`]**: General-purpose flow diagrams
//! - **[`DiagramType::Gantt`]**: Project timeline charts
//! - **[`DiagramType::Pie`]**: Data distribution visualization
//! - And many more specialized types...
//!
//! ## Quick Start
//!
//! ```rust
//! use mermaid_parser::{parse_diagram, DiagramType};
//!
//! let input = r#"
//! sankey-beta
//! A,B,10
//! B,C,5
//! "#;
//!
//! match parse_diagram(input) {
//!     Ok(DiagramType::Sankey(diagram)) => {
//!         println!("Found {} nodes and {} links",
//!                  diagram.nodes.len(), diagram.links.len());
//!     }
//!     Ok(other) => println!("Parsed diagram: {:?}", other),
//!     Err(e) => eprintln!("Parse error: {}", e),
//! }
//! ```
//!
//! ## Error Handling
//!
//! The parser provides comprehensive error handling through [`ParseError`]:
//!
//! ```rust
//! use mermaid_parser::{parse_diagram, ParseError};
//!
//! let invalid_input = "flowchart TD\n    A => B";  // Invalid arrow syntax
//!
//! match parse_diagram(invalid_input) {
//!     Ok(diagram) => println!("Success: {:?}", diagram),
//!     Err(ParseError::SyntaxError { message, line, column, .. }) => {
//!         eprintln!("Syntax error at {}:{}: {}", line, column, message);
//!     }
//!     Err(e) => eprintln!("Other error: {}", e),
//! }
//! ```
//!
//! ## Advanced Usage
//!
//! ### AST Analysis with Visitor Pattern
//!
//! ```rust
//! use mermaid_parser::{parse_diagram, NodeCounter};
//!
//! if let Ok(diagram) = parse_diagram("flowchart TD\n    A --> B\n    B --> C") {
//!     let mut counter = NodeCounter::new();
//!     diagram.accept(&mut counter);
//!     println!("Found {} nodes", counter.nodes());
//! }
//! ```
//!
//! ### Diagram Metrics and Analysis
//!
//! ```rust
//! use mermaid_parser::{parse_diagram, ComplexityAnalyzer};
//!
//! if let Ok(diagram) = parse_diagram("flowchart TD\n    A --> B") {
//!     let mut analyzer = ComplexityAnalyzer::new();
//!     diagram.accept(&mut analyzer);
//!     println!("Cyclomatic complexity: {}", analyzer.cyclomatic_complexity());
//! }
//! ```

pub mod common;
pub mod error;
pub mod parsers;

pub use common::ast::{CardinalityValue, DiagramType, KeyType};
pub use common::metrics::{
    BasicMetrics, ComplexityMetrics, DiagramMetrics, MetricsReport, QualityMetrics, SeverityLevel,
    Suggestion, SuggestionCategory,
};
pub use common::pretty_print::{MermaidPrinter, PrintOptions};
pub use common::visitor::{
    AstVisitor, AstVisitorMut, ComplexityAnalyzer, NodeCounter, ReferenceValidator, TitleSetter,
};
pub use error::{ParseError, Result};

/// Parse a Mermaid diagram from text input
///
/// This function automatically detects the diagram type using [`detect_diagram_type`] and
/// routes to the appropriate specialized parser to build a [`DiagramType`] AST.
///
/// The parser supports all major Mermaid diagram types with intelligent error recovery
/// and detailed error reporting via [`ParseError`].
///
/// # Arguments
///
/// * `input` - The Mermaid diagram text to parse
///
/// # Returns
///
/// Returns a [`Result`]`<`[`DiagramType`], [`ParseError`]`>` containing the parsed
/// diagram or a detailed parse error with location information.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use mermaid_parser::{parse_diagram, DiagramType};
///
/// let result = parse_diagram("sankey-beta\nA,B,10");
/// assert!(result.is_ok());
///
/// if let Ok(DiagramType::Sankey(sankey)) = result {
///     assert_eq!(sankey.links.len(), 1);
/// }
/// ```
///
/// ## Error Handling
///
/// ```rust
/// use mermaid_parser::{parse_diagram, ParseError};
///
/// match parse_diagram("invalid diagram") {
///     Ok(diagram) => println!("Parsed: {:?}", diagram),
///     Err(ParseError::EmptyInput) => println!("Input was empty"),
///     Err(e) => println!("Parse error: {}", e),
/// }
/// ```
///
/// ## Supported Diagram Types
///
/// All major Mermaid diagram types are supported:
///
/// ```rust
/// use mermaid_parser::parse_diagram;
///
/// // Flowcharts
/// let flowchart = parse_diagram("flowchart TD\n    A --> B");
///
/// // Sequence diagrams  
/// let sequence = parse_diagram("sequenceDiagram\n    A->>B: Hello");
///
/// // And many more...
/// ```
///
/// # Errors
///
/// Returns [`ParseError`] variants for different error conditions:
///
/// - [`ParseError::EmptyInput`] - No valid diagram content found
/// - [`ParseError::SyntaxError`] - Invalid syntax according to grammar rules
/// - [`ParseError::SemanticError`] - Valid syntax but semantically incorrect
/// - See [`ParseError`] for complete error type documentation
pub fn parse_diagram(input: &str) -> Result<DiagramType> {
    // Detect diagram type from input
    let diagram_type = detect_diagram_type(input)?;

    // Parse based on detected type
    match diagram_type {
        "sankey" => parsers::sankey::parse(input).map(DiagramType::Sankey),
        "architecture" => parsers::architecture::parse(input).map(DiagramType::Architecture),
        "block" => parsers::block::parse(input).map(DiagramType::Block),
        "c4" => parsers::c4::parse(input).map(DiagramType::C4),
        "class" => parsers::class::parse(input).map(DiagramType::Class),
        "er" => parsers::er::parse(input).map(DiagramType::Er),
        "flowchart" => parsers::flowchart::parse(input).map(DiagramType::Flowchart),
        "gantt" => parsers::gantt::parse(input).map(DiagramType::Gantt),
        "git" => parsers::git::parse(input).map(DiagramType::Git),
        "kanban" => parsers::kanban::parse(input).map(DiagramType::Kanban),
        "mindmap" => parsers::mindmap::parse(input).map(DiagramType::Mindmap),
        "packet" => parsers::packet::parse(input).map(DiagramType::Packet),
        "pie" => parsers::pie::parse(input).map(DiagramType::Pie),
        "quadrant" => parsers::quadrant::parse(input).map(DiagramType::Quadrant),
        "radar" => parsers::radar::parse(input).map(DiagramType::Radar),
        "requirement" => parsers::requirement::parse(input).map(DiagramType::Requirement),
        "timeline" => parsers::timeline::parse(input).map(DiagramType::Timeline),
        "treemap" => parsers::treemap::parse(input).map(DiagramType::Treemap),
        "journey" => parsers::journey::parse(input).map(DiagramType::Journey),
        "sequence" => parsers::sequence::parse(input).map(DiagramType::Sequence),
        "state" => parsers::state::parse(input).map(DiagramType::State),
        "xychart" => parsers::xy::parse(input).map(DiagramType::XyChart),
        // Add other parsers as they're implemented
        _ => {
            // Try misc parser as a fallback for unknown diagram types
            parsers::misc::parse(input).map(DiagramType::Misc)
        }
    }
}

/// Detect the type of Mermaid diagram from input text
///
/// This function examines the first non-comment, non-whitespace line
/// to determine the diagram type. It handles various diagram type keywords
/// including their beta versions and alternative names.
///
/// The detection process:
/// 1. Skips comment lines (starting with `//` or `#`)
/// 2. Finds the first meaningful line with content
/// 3. Extracts the first word (diagram type keyword)
/// 4. Normalizes and matches against known diagram types
///
/// # Arguments
///
/// * `input` - The Mermaid diagram text to analyze
///
/// # Returns
///
/// Returns a [`Result`]`<&'static str, `[`ParseError`]`>` containing the diagram
/// type identifier or [`ParseError::EmptyInput`] if no valid content is found.
///
/// # Examples
///
/// ```rust
/// # use mermaid_parser::*;
/// // This function is private, but shows the detection logic
/// let sankey_input = "sankey-beta\nA,B,10";
/// // Would detect as "sankey"
///
/// let flowchart_input = "flowchart TD\nA --> B";  
/// // Would detect as "flowchart"
///
/// let commented_input = "// Comment\n# Another comment\ntimeline\ntitle: Test";
/// // Would detect as "timeline" (skips comments)
/// ```
///
/// # Supported Keywords
///
/// - `sankey-beta`, `sankey` → "sankey"
/// - `flowchart`, `graph` → "flowchart"
/// - `sequenceDiagram` → "sequence"
/// - `classDiagram` → "class"
/// - `stateDiagram`, `stateDiagram-v2` → "state"
/// - And many more... (see source for complete list)
///
/// Unknown diagram types default to "misc" for fallback parsing.
///
/// # Errors
///
/// Returns [`ParseError::EmptyInput`] if the input contains no valid diagram content.
fn detect_diagram_type(input: &str) -> Result<&'static str> {
    let first_line = input
        .lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('#'))
        .ok_or(ParseError::EmptyInput)?;

    let first_word = first_line
        .split_whitespace()
        .next()
        .ok_or(ParseError::EmptyInput)?
        .to_lowercase()
        .trim_end_matches(':')
        .to_string();

    match first_word.as_str() {
        "sankey-beta" => Ok("sankey"),
        "timeline" => Ok("timeline"),
        "journey" => Ok("journey"),
        "sequencediagram" => Ok("sequence"),
        "classdiagram" => Ok("class"),
        "statediagram" | "statediagram-v2" => Ok("state"),
        "flowchart" | "graph" => Ok("flowchart"),
        "gantt" | "gantttestclick" => Ok("gantt"),
        "pie" => Ok("pie"),
        "gitgraph" => Ok("misc"), // Alternative gitGraph syntax handled by misc parser
        "info" => Ok("misc"),
        "erdiagram" | "erdiagramtitletext" => Ok("er"),
        "c4context" | "c4container" | "c4component" | "c4dynamic" | "c4deployment" => Ok("c4"),
        "mindmap" => Ok("mindmap"),
        "quadrant" => Ok("quadrant"),
        "quadrantchart" => Ok("quadrant"),
        "xychart" => Ok("xychart"),
        "xychart-beta" => Ok("xychart"),
        "kanban" => Ok("kanban"),
        "block" => Ok("block"),
        "block-beta" => Ok("block"),
        "architecture" => Ok("architecture"),
        "architecture-beta" => Ok("architecture"),
        "packet-beta" => Ok("packet"),
        "packet" => Ok("packet"),
        "requirement" | "requirementdiagram" => Ok("requirement"),
        "sankey" => Ok("sankey"),
        "treemap" | "treemap-beta" => Ok("treemap"),
        "radar" => Ok("radar"),
        _ => Ok("misc"), // Unknown diagram types are handled by misc parser
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagram_type_detection() {
        assert_eq!(detect_diagram_type("sankey-beta\nA,B,10"), Ok("sankey"));
        assert_eq!(
            detect_diagram_type("timeline\ntitle My Day"),
            Ok("timeline")
        );
        assert_eq!(
            detect_diagram_type("journey\ntitle My Journey"),
            Ok("journey")
        );
        assert_eq!(
            detect_diagram_type("sequenceDiagram\nAlice->Bob: Hi"),
            Ok("sequence")
        );
        assert_eq!(
            detect_diagram_type("flowchart TD\nA --> B"),
            Ok("flowchart")
        );
        assert_eq!(detect_diagram_type("graph LR\nA --> B"), Ok("flowchart"));
    }

    #[test]
    fn test_detection_with_comments() {
        let input = r#"
        // This is a comment
        # Another comment
        
        timeline
        title My Day
        "#;
        assert_eq!(detect_diagram_type(input), Ok("timeline"));
    }

    #[test]
    fn test_empty_input() {
        assert!(detect_diagram_type("").is_err());
        assert!(detect_diagram_type("   \n  \n  ").is_err());
        assert!(detect_diagram_type("// Only comments\n# More comments").is_err());
    }

    #[test]
    fn test_unknown_diagram_type() {
        assert_eq!(detect_diagram_type("unknown_diagram_type"), Ok("misc"));
    }
}
