//! Centralized constants for magic strings used throughout the parser
//!
//! This module provides a central location for all string constants used across
//! the mermaid-parser crate. By centralizing these constants, we improve
//! maintainability, consistency, and reduce the risk of typos in magic strings.

/// Diagram type identifiers and headers
pub mod diagram_headers {
    // Architecture diagrams
    pub const ARCHITECTURE: &str = "architecture";
    pub const ARCHITECTURE_BETA: &str = "architecture-beta";
    pub const ARCHITECTURE_HEADERS: &[&str] = &[ARCHITECTURE, ARCHITECTURE_BETA];

    // State diagrams
    pub const STATE_V1: &str = "stateDiagram";
    pub const STATE_V2: &str = "stateDiagram-v2";
    pub const STATE_HEADERS: &[&str] = &[STATE_V1, STATE_V2];

    // Flow diagrams
    pub const FLOWCHART: &str = "flowchart";
    pub const GRAPH: &str = "graph";
    pub const FLOW_HEADERS: &[&str] = &[FLOWCHART, GRAPH];

    // Sequence diagrams
    pub const SEQUENCE: &str = "sequenceDiagram";
    pub const SEQUENCE_HEADERS: &[&str] = &[SEQUENCE];

    // Sankey diagrams
    pub const SANKEY: &str = "sankey";
    pub const SANKEY_BETA: &str = "sankey-beta";
    pub const SANKEY_HEADERS: &[&str] = &[SANKEY, SANKEY_BETA];

    // Timeline diagrams
    pub const TIMELINE: &str = "timeline";
    pub const TIMELINE_HEADERS: &[&str] = &[TIMELINE];

    // Gantt diagrams
    pub const GANTT: &str = "gantt";
    pub const GANTT_TEST_CLICK: &str = "gantttestclick";
    pub const GANTT_HEADERS: &[&str] = &[GANTT, GANTT_TEST_CLICK];

    // Pie charts
    pub const PIE: &str = "pie";
    pub const PIE_HEADERS: &[&str] = &[PIE];

    // C4 diagrams
    pub const C4_CONTEXT: &str = "c4context";
    pub const C4_CONTAINER: &str = "c4container";
    pub const C4_COMPONENT: &str = "c4component";
    pub const C4_DYNAMIC: &str = "c4dynamic";
    pub const C4_DEPLOYMENT: &str = "c4deployment";
    pub const C4_HEADERS: &[&str] = &[
        C4_CONTEXT,
        C4_CONTAINER,
        C4_COMPONENT,
        C4_DYNAMIC,
        C4_DEPLOYMENT,
    ];

    // Block diagrams
    pub const BLOCK: &str = "block";
    pub const BLOCK_BETA: &str = "block-beta";
    pub const BLOCK_HEADERS: &[&str] = &[BLOCK, BLOCK_BETA];

    // Packet diagrams
    pub const PACKET: &str = "packet";
    pub const PACKET_BETA: &str = "packet-beta";
    pub const PACKET_HEADERS: &[&str] = &[PACKET, PACKET_BETA];

    // XY charts
    pub const XYCHART: &str = "xychart";
    pub const XYCHART_BETA: &str = "xychart-beta";
    pub const XYCHART_HEADERS: &[&str] = &[XYCHART, XYCHART_BETA];

    // Treemap diagrams
    pub const TREEMAP: &str = "treemap";
    pub const TREEMAP_BETA: &str = "treemap-beta";
    pub const TREEMAP_HEADERS: &[&str] = &[TREEMAP, TREEMAP_BETA];

    // Other diagram types
    pub const MINDMAP: &str = "mindmap";
    pub const QUADRANT: &str = "quadrant";
    pub const QUADRANT_CHART: &str = "quadrantchart";
    pub const JOURNEY: &str = "journey";
    pub const KANBAN: &str = "kanban";
    pub const RADAR: &str = "radar";
    pub const REQUIREMENT: &str = "requirement";
    pub const REQUIREMENT_DIAGRAM: &str = "requirementdiagram";
    pub const ER_DIAGRAM: &str = "erdiagram";
    pub const ER_DIAGRAM_TITLE: &str = "erdiagramtitletext";
    pub const CLASS_DIAGRAM: &str = "classdiagram";
    pub const GIT_GRAPH: &str = "gitgraph";
    pub const INFO: &str = "info";
}

/// Common directive prefixes
pub mod directives {
    pub const TITLE: &str = "title ";
    pub const ACC_TITLE: &str = "accTitle: ";
    pub const ACC_DESC: &str = "accDescr: ";
    pub const ACC_DESC_START: &str = "accDescr {";
    pub const ACC_DESC_END: &str = "}";
}

/// Sequence diagram specific keywords
pub mod sequence_keywords {
    pub const PARTICIPANT: &str = "participant ";
    pub const ACTOR: &str = "actor ";
    pub const LOOP: &str = "loop ";
    pub const ALT: &str = "alt ";
    pub const OPT: &str = "opt ";
    pub const NOTE: &str = "note ";
    pub const ACTIVATE: &str = "activate ";
    pub const DEACTIVATE: &str = "deactivate ";
    pub const AUTONUMBER: &str = "autonumber";
}

/// Flowchart diagram specific keywords
pub mod flowchart_keywords {
    pub const FLOWCHART: &str = "flowchart";
    pub const GRAPH: &str = "graph";
    pub const SUBGRAPH: &str = "subgraph";
    pub const END: &str = "end";

    // Edge patterns
    pub const DOUBLE_DASH: &str = "--";
}

/// State diagram specific keywords
pub mod state_keywords {
    pub const STATE: &str = "state ";
    pub const DIRECTION: &str = "direction ";
    pub const NOTE: &str = "note";

    // State types
    pub const CHOICE: &str = "choice";
    pub const FORK: &str = "fork";
    pub const JOIN: &str = "join";
    pub const END: &str = "end";
}

/// Comment patterns
pub mod comments {
    pub const DOUBLE_SLASH: &str = "//";
    pub const DOUBLE_PERCENT: &str = "%%";
    pub const COMMENT_PREFIXES: &[&str] = &[DOUBLE_SLASH, DOUBLE_PERCENT];
}

/// Arrow types and symbols used in diagrams
pub mod arrows {
    // Basic arrows
    pub const ARROW_RIGHT: &str = "-->";
    pub const ARROW_RIGHT_SHORT: &str = "->";
    pub const ARROW_LEFT: &str = "<--";
    pub const ARROW_LEFT_SHORT: &str = "<-";

    // Thick arrows
    pub const THICK_ARROW_RIGHT: &str = "==>";
    pub const THICK_ARROW_LEFT: &str = "<==";
    pub const THICK_ARROW_BIDIRECTIONAL: &str = "<=>";

    // Dotted arrows
    pub const DOTTED_ARROW_RIGHT: &str = ".->";
    pub const DOTTED_ARROW_LEFT: &str = "<-.";
    pub const DOTTED_ARROW_BIDIRECTIONAL: &str = "<-.>";

    // Special sequence diagram arrows
    pub const SEQUENCE_SYNC: &str = "->>";
    pub const SEQUENCE_ASYNC: &str = "->";
    pub const SEQUENCE_REPLY: &str = "-->>";

    // All arrow patterns for pattern matching
    pub const ALL_ARROWS: &[&str] = &[
        ARROW_RIGHT,
        ARROW_RIGHT_SHORT,
        ARROW_LEFT,
        ARROW_LEFT_SHORT,
        THICK_ARROW_RIGHT,
        THICK_ARROW_LEFT,
        THICK_ARROW_BIDIRECTIONAL,
        DOTTED_ARROW_RIGHT,
        DOTTED_ARROW_LEFT,
        DOTTED_ARROW_BIDIRECTIONAL,
        SEQUENCE_SYNC,
        SEQUENCE_ASYNC,
        SEQUENCE_REPLY,
    ];
}

/// Flow directions commonly used in diagrams
pub mod directions {
    pub const TOP_DOWN: &str = "TD";
    pub const TOP_BOTTOM: &str = "TB";
    pub const BOTTOM_TOP: &str = "BT";
    pub const LEFT_RIGHT: &str = "LR";
    pub const RIGHT_LEFT: &str = "RL";

    pub const ALL_DIRECTIONS: &[&str] = &[TOP_DOWN, TOP_BOTTOM, BOTTOM_TOP, LEFT_RIGHT, RIGHT_LEFT];
}

/// Common test patterns
pub mod test_patterns {
    pub const FLOWCHART_TD: &str = "flowchart TD";
    pub const FLOWCHART_LR: &str = "flowchart LR";
    pub const SIMPLE_SEQUENCE: &str = "sequenceDiagram\n    A->>B: Hello";
    pub const SIMPLE_STATE: &str = "stateDiagram-v2\n    [*] --> A";
}

/// Error message templates
pub mod error_messages {
    pub const EXPECTED_HEADER: &str = "Expected {} header";
    pub const INVALID_SYNTAX: &str = "Invalid syntax";
    pub const UNEXPECTED_TOKEN: &str = "Unexpected token";
    pub const MISSING_CLOSING: &str = "Missing closing bracket";
    pub const INVALID_ARROW: &str = "Invalid arrow syntax";
}

/// Helper functions for working with constants
///
/// Check if a line starts with any comment prefix
pub fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    comments::COMMENT_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

/// Check if a string matches any arrow pattern
pub fn is_arrow(text: &str) -> bool {
    arrows::ALL_ARROWS.iter().any(|arrow| text.contains(arrow))
}

/// Get the diagram type from a header line
pub fn detect_diagram_type(line: &str) -> Option<&'static str> {
    let trimmed = line.trim().to_lowercase();

    if diagram_headers::ARCHITECTURE_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("architecture");
    }

    if diagram_headers::FLOW_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("flowchart");
    }

    if diagram_headers::SEQUENCE_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("sequence");
    }

    if diagram_headers::STATE_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("state");
    }

    if diagram_headers::SANKEY_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("sankey");
    }

    if diagram_headers::TIMELINE_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("timeline");
    }

    if diagram_headers::GANTT_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("gantt");
    }

    if diagram_headers::PIE_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("pie");
    }

    if diagram_headers::C4_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("c4");
    }

    if diagram_headers::BLOCK_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("block");
    }

    if diagram_headers::PACKET_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("packet");
    }

    if diagram_headers::XYCHART_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("xychart");
    }

    if diagram_headers::TREEMAP_HEADERS
        .iter()
        .any(|h| trimmed.starts_with(h))
    {
        return Some("treemap");
    }

    // Check individual diagram types
    if trimmed.starts_with(diagram_headers::MINDMAP) {
        return Some("mindmap");
    }

    if trimmed.starts_with(diagram_headers::QUADRANT)
        || trimmed.starts_with(diagram_headers::QUADRANT_CHART)
    {
        return Some("quadrant");
    }

    if trimmed.starts_with(diagram_headers::JOURNEY) {
        return Some("journey");
    }

    if trimmed.starts_with(diagram_headers::KANBAN) {
        return Some("kanban");
    }

    if trimmed.starts_with(diagram_headers::RADAR) {
        return Some("radar");
    }

    if trimmed.starts_with(diagram_headers::REQUIREMENT)
        || trimmed.starts_with(diagram_headers::REQUIREMENT_DIAGRAM)
    {
        return Some("requirement");
    }

    if trimmed.starts_with(diagram_headers::ER_DIAGRAM) {
        return Some("er");
    }

    if trimmed.starts_with(diagram_headers::CLASS_DIAGRAM) {
        return Some("class");
    }

    if trimmed.starts_with(diagram_headers::GIT_GRAPH) {
        return Some("git");
    }

    if trimmed.starts_with(diagram_headers::INFO) {
        return Some("info");
    }

    None
}
