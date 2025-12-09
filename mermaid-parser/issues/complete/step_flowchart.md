# Implementation Plan: Flowchart Diagrams

## Overview
Flowchart diagrams are the most complex Mermaid diagram type with 631 lines of grammar, supporting 15+ node shapes, complex connections, subgraphs, styling, and advanced features.

## Grammar Analysis

### Key Features
- Headers: `graph`, `flowchart` with directions (TB, TD, BT, RL, LR)
- Node shapes: 15+ different shapes including rectangles, circles, rhombus, hexagon, etc.
- Connections: Various arrow types, dotted lines, thick lines, invisible links
- Subgraphs: Nested graph structures with titles
- Styling: Classes, inline styles, style definitions
- Click events: Interactive elements with callbacks and links
- Comments: `%%` for line comments

### Example Input
```
flowchart TD
    A[Christmas] -->|Get money| B(Go shopping)
    B --> C{Let me think}
    C -->|One| D[Laptop]
    C -->|Two| E[iPhone]
    C -->|Three| F[fa:fa-car Car]
    
    subgraph ide1 [Process]
        G{{Mission}} -.-> H>Result]
        H ==> I[(Database)]
    end
    
    I --> J[/Parallelogram/]
    J --> K[\Trapezoid alt\]
    K --> L[/Trapezoid\]
    
    style A fill:#f9f,stroke:#333,stroke-width:4px
    style B fill:#bbf,stroke:#f66,stroke-width:2px,color:#fff,stroke-dasharray: 5 5
    
    classDef green fill:#9f6,stroke:#333,stroke-width:2px;
    classDef orange fill:#f96,stroke:#333,stroke-width:4px;
    class C green
    class D,E orange
    
    click A "https://www.github.com" _blank
    click B callback "Tooltip text"
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct FlowchartDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub direction: FlowDirection,
    pub nodes: HashMap<String, FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<Subgraph>,
    pub styles: Vec<StyleDefinition>,
    pub class_defs: HashMap<String, ClassDef>,
    pub clicks: Vec<ClickEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowDirection {
    TB, // Top to Bottom (same as TD)
    TD, // Top Down
    BT, // Bottom to Top
    RL, // Right to Left
    LR, // Left to Right
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowNode {
    pub id: String,
    pub text: Option<String>,
    pub shape: NodeShape,
    pub classes: Vec<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeShape {
    Rectangle,           // [text]
    RoundedRectangle,   // (text)
    Stadium,            // ([text])
    Subroutine,         // [[text]]
    Cylinder,           // [(text)]
    Circle,             // ((text))
    Asymmetric,         // >text]
    Rhombus,            // {text}
    Hexagon,            // {{text}}
    Parallelogram,      // [/text/]
    ParallelogramAlt,   // [\text\]
    Trapezoid,          // [/text\]
    TrapezoidAlt,       // [\text/]
    DoubleCircle,       // (((text)))
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
    pub min_length: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeType {
    Arrow,              // -->
    DottedArrow,        // -.->
    ThickArrow,         // ==>
    OpenLink,           // ---
    DottedLink,         // -.-
    ThickLink,          // ===
    Invisible,          // ~~~
    CircleEdge,         // --o
    CrossEdge,          // --x
    MultiDirectional,   // <-->
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subgraph {
    pub id: String,
    pub title: Option<String>,
    pub nodes: Vec<String>,     // Node IDs
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<Subgraph>, // Nested subgraphs
    pub direction: Option<FlowDirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleDefinition {
    pub target: StyleTarget,
    pub styles: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StyleTarget {
    Node(String),
    Edge(String, String),
    Subgraph(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: String,
    pub styles: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClickEvent {
    pub node_id: String,
    pub action: ClickAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClickAction {
    Href(String, Option<String>), // URL, target
    Callback(String),             // Function name
    Both(String, String, Option<String>), // Callback, URL, target
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowToken {
    // Keywords
    Graph,
    Flowchart,
    Subgraph,
    End,
    Direction,
    Style,
    ClassDef,
    Class,
    Click,
    
    // Directions
    TB, TD, BT, RL, LR,
    
    // Node brackets
    LeftSquare,         // [
    RightSquare,        // ]
    LeftParen,          // (
    RightParen,         // )
    LeftBrace,          // {
    RightBrace,         // }
    LeftAngle,          // <
    RightAngle,         // >
    DoubleLeftSquare,   // [[
    DoubleRightSquare,  // ]]
    DoubleLeftParen,    // ((
    DoubleRightParen,   // ))
    TripleLeftParen,    // (((
    TripleRightParen,   // )))
    DoubleLeftBrace,    // {{
    DoubleRightBrace,   // }}
    
    // Edge components
    Dash,               // -
    DashDash,           // --
    Dot,                // .
    Equal,              // =
    Tilde,              // ~
    Pipe,               // |
    Arrow,              // >
    Circle,             // o
    Cross,              // x
    
    // Special symbols
    Slash,              // /
    Backslash,          // \
    Colon,              // :
    Semicolon,          // ;
    Comma,              // ,
    Ampersand,          // &
    Quote,              // "
    
    // Values
    NodeId(String),
    Text(String),
    QuotedString(String),
    StyleProperty(String, String), // property:value
    Icon(String),       // fa:fa-icon
    Number(i32),
    Url(String),
    Target(String),     // _blank, _self, etc.
    
    Comment(String),
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Complex Token Recognition
```rust
use chumsky::prelude::*;

pub fn flowchart_lexer() -> impl Parser<char, Vec<FlowToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| FlowToken::Comment(text.into_iter().collect()));
    
    // Keywords
    let keywords = choice((
        text::keyword("flowchart").map(|_| FlowToken::Flowchart),
        text::keyword("graph").map(|_| FlowToken::Graph),
        text::keyword("subgraph").map(|_| FlowToken::Subgraph),
        text::keyword("end").map(|_| FlowToken::End),
        text::keyword("direction").map(|_| FlowToken::Direction),
        text::keyword("style").map(|_| FlowToken::Style),
        text::keyword("classDef").map(|_| FlowToken::ClassDef),
        text::keyword("class").map(|_| FlowToken::Class),
        text::keyword("click").map(|_| FlowToken::Click),
    ));
    
    // Directions
    let directions = choice((
        text::keyword("TB").map(|_| FlowToken::TB),
        text::keyword("TD").map(|_| FlowToken::TD),
        text::keyword("BT").map(|_| FlowToken::BT),
        text::keyword("RL").map(|_| FlowToken::RL),
        text::keyword("LR").map(|_| FlowToken::LR),
    ));
    
    // Node shape brackets (order matters for overlapping patterns)
    let node_brackets = choice((
        text::string("(((").map(|_| FlowToken::TripleLeftParen),
        text::string(")))").map(|_| FlowToken::TripleRightParen),
        text::string("((").map(|_| FlowToken::DoubleLeftParen),
        text::string("))").map(|_| FlowToken::DoubleRightParen),
        text::string("[[").map(|_| FlowToken::DoubleLeftSquare),
        text::string("]]").map(|_| FlowToken::DoubleRightSquare),
        text::string("{{").map(|_| FlowToken::DoubleLeftBrace),
        text::string("}}").map(|_| FlowToken::DoubleRightBrace),
        just('[').map(|_| FlowToken::LeftSquare),
        just(']').map(|_| FlowToken::RightSquare),
        just('(').map(|_| FlowToken::LeftParen),
        just(')').map(|_| FlowToken::RightParen),
        just('{').map(|_| FlowToken::LeftBrace),
        just('}').map(|_| FlowToken::RightBrace),
        just('<').map(|_| FlowToken::LeftAngle),
        just('>').map(|_| FlowToken::RightAngle),
    ));
    
    // Edge patterns (complex due to many combinations)
    let edge_patterns = choice((
        text::string("~~~").map(|_| FlowToken::Tilde),
        text::string("===").map(|_| FlowToken::Equal),
        text::string("---").map(|_| FlowToken::DashDash),
        text::string("-.-").map(|_| FlowToken::Dot),
        text::string("-->").map(|_| FlowToken::Arrow),
        text::string("--o").map(|_| FlowToken::Circle),
        text::string("--x").map(|_| FlowToken::Cross),
        text::string("--").map(|_| FlowToken::DashDash),
        just('-').map(|_| FlowToken::Dash),
        just('=').map(|_| FlowToken::Equal),
        just('.').map(|_| FlowToken::Dot),
        just('|').map(|_| FlowToken::Pipe),
    ));
    
    // Quoted string
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(FlowToken::QuotedString);
    
    // Node ID (alphanumeric with underscores)
    let node_id = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(FlowToken::NodeId);
    
    // Icon (fa:fa-icon format)
    let icon = text::keyword("fa:")
        .then(
            filter(|c: &char| c.is_alphanumeric() || *c == '-')
                .repeated()
                .at_least(1)
        )
        .map(|(_, chars)| FlowToken::Icon(format!("fa:{}", chars.into_iter().collect::<String>())));
    
    // Style property (property:value)
    let style_property = filter(|c: &char| {
        c.is_alphanumeric() || *c == '-'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .then_ignore(just(':'))
    .then(
        filter(|c: &char| {
            !matches!(*c, ',' | ';' | '\n')
        })
        .repeated()
        .at_least(1)
        .collect::<String>()
    )
    .map(|(prop, val)| FlowToken::StyleProperty(prop, val.trim().to_string()));
    
    // URL
    let url = choice((
        just('"')
            .ignore_then(
                filter(|c| *c != '"')
                    .repeated()
                    .collect::<String>()
            )
            .then_ignore(just('"')),
        filter(|c: &char| {
            c.is_alphanumeric() || matches!(*c, ':' | '/' | '.' | '-' | '_' | '#' | '?' | '&' | '=')
        })
        .repeated()
        .at_least(1)
        .collect::<String>()
    ))
    .map(FlowToken::Url);
    
    let slash = just('/').map(|_| FlowToken::Slash);
    let backslash = just('\\').map(|_| FlowToken::Backslash);
    let colon = just(':').map(|_| FlowToken::Colon);
    let semicolon = just(';').map(|_| FlowToken::Semicolon);
    let comma = just(',').map(|_| FlowToken::Comma);
    let ampersand = just('&').map(|_| FlowToken::Ampersand);
    
    let newline = just('\n').map(|_| FlowToken::NewLine);
    
    // Complex ordering to handle overlapping patterns
    let token = choice((
        comment,
        keywords,
        directions,
        edge_patterns,
        node_brackets,
        icon,
        style_property,
        quoted_string,
        url,
        slash,
        backslash,
        colon,
        semicolon,
        comma,
        ampersand,
        node_id,
    ));
    
    whitespace
        .ignore_then(token)
        .or(newline)
        .repeated()
        .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Highly Complex Flowchart Parser
```rust
pub fn flowchart_parser() -> impl Parser<FlowToken, FlowchartDiagram, Error = Simple<FlowToken>> {
    enum ParseContext {
        TopLevel,
        InSubgraph(String),
    }
    
    let header = choice((
        just(FlowToken::Flowchart),
        just(FlowToken::Graph),
    ))
    .then(
        choice((
            just(FlowToken::TB).map(|_| FlowDirection::TB),
            just(FlowToken::TD).map(|_| FlowDirection::TD),
            just(FlowToken::BT).map(|_| FlowDirection::BT),
            just(FlowToken::RL).map(|_| FlowDirection::RL),
            just(FlowToken::LR).map(|_| FlowDirection::LR),
        ))
        .or_not()
    )
    .map(|(_, dir)| dir.unwrap_or(FlowDirection::TD));
    
    header
        .then_ignore(
            filter(|t| matches!(t, FlowToken::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(FlowToken::Eof).or_not())
        .map(|(direction, tokens)| {
            let mut nodes = HashMap::new();
            let mut edges = Vec::new();
            let mut subgraphs = Vec::new();
            let mut styles = Vec::new();
            let mut class_defs = HashMap::new();
            let mut clicks = Vec::new();
            let mut context_stack = vec![ParseContext::TopLevel];
            let mut i = 0;
            
            while i < tokens.len() {
                match &tokens[i] {
                    FlowToken::Comment(_) | FlowToken::NewLine => {
                        i += 1;
                    }
                    FlowToken::NodeId(id) => {
                        // Could be node definition or edge start
                        if let Some((result, consumed)) = parse_node_or_edge(&tokens[i..], &mut nodes) {
                            match result {
                                ParseResult::Node(node) => {
                                    nodes.insert(node.id.clone(), node);
                                }
                                ParseResult::Edge(edge) => {
                                    edges.push(edge);
                                }
                                ParseResult::NodeAndEdge(node, edge) => {
                                    nodes.insert(node.id.clone(), node);
                                    edges.push(edge);
                                }
                            }
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    FlowToken::Subgraph => {
                        if let Some((subgraph, consumed)) = parse_subgraph(&tokens[i..], &mut nodes, &mut edges) {
                            subgraphs.push(subgraph);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    FlowToken::Style => {
                        if let Some((style, consumed)) = parse_style(&tokens[i..]) {
                            styles.push(style);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    FlowToken::ClassDef => {
                        if let Some((class_def, consumed)) = parse_class_def(&tokens[i..]) {
                            class_defs.insert(class_def.name.clone(), class_def);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    FlowToken::Class => {
                        if let Some((assignments, consumed)) = parse_class_assignment(&tokens[i..]) {
                            for (node_id, class_name) in assignments {
                                if let Some(node) = nodes.get_mut(&node_id) {
                                    node.classes.push(class_name);
                                }
                            }
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    FlowToken::Click => {
                        if let Some((click, consumed)) = parse_click(&tokens[i..]) {
                            clicks.push(click);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            
            FlowchartDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                direction,
                nodes,
                edges,
                subgraphs,
                styles,
                class_defs,
                clicks,
            }
        })
}

enum ParseResult {
    Node(FlowNode),
    Edge(FlowEdge),
    NodeAndEdge(FlowNode, FlowEdge),
}

fn parse_node_or_edge(tokens: &[FlowToken], nodes: &mut HashMap<String, FlowNode>) -> Option<(ParseResult, usize)> {
    if tokens.is_empty() {
        return None;
    }
    
    let mut i = 0;
    
    let node_id = match &tokens[i] {
        FlowToken::NodeId(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    if i >= tokens.len() {
        return None;
    }
    
    // Check what follows the node ID
    match &tokens[i] {
        // Node shape definition
        FlowToken::LeftSquare | FlowToken::LeftParen | FlowToken::LeftBrace
        | FlowToken::DoubleLeftSquare | FlowToken::DoubleLeftParen
        | FlowToken::TripleLeftParen | FlowToken::DoubleLeftBrace => {
            if let Some((node, consumed)) = parse_node_shape(&tokens[i..], &node_id) {
                i += consumed;
                
                // Check if followed by edge
                if i < tokens.len() && is_edge_start(&tokens[i]) {
                    if let Some((edge, edge_consumed)) = parse_edge(&tokens[i..], &node_id) {
                        i += edge_consumed;
                        return Some((ParseResult::NodeAndEdge(node, edge), i));
                    }
                }
                
                return Some((ParseResult::Node(node), i));
            }
        }
        // Direct edge
        _ if is_edge_start(&tokens[i]) => {
            // Ensure source node exists
            if !nodes.contains_key(&node_id) {
                nodes.insert(node_id.clone(), FlowNode {
                    id: node_id.clone(),
                    text: None,
                    shape: NodeShape::Rectangle,
                    classes: Vec::new(),
                    icon: None,
                });
            }
            
            if let Some((edge, consumed)) = parse_edge(&tokens[i..], &node_id) {
                i += consumed;
                return Some((ParseResult::Edge(edge), i));
            }
        }
        _ => {}
    }
    
    None
}

fn parse_node_shape(tokens: &[FlowToken], node_id: &str) -> Option<(FlowNode, usize)> {
    if tokens.is_empty() {
        return None;
    }
    
    let mut i = 0;
    
    let (shape, start_pattern, end_pattern) = match &tokens[i] {
        FlowToken::TripleLeftParen => (NodeShape::DoubleCircle, FlowToken::TripleLeftParen, FlowToken::TripleRightParen),
        FlowToken::DoubleLeftParen => (NodeShape::Circle, FlowToken::DoubleLeftParen, FlowToken::DoubleRightParen),
        FlowToken::DoubleLeftSquare => (NodeShape::Subroutine, FlowToken::DoubleLeftSquare, FlowToken::DoubleRightSquare),
        FlowToken::DoubleLeftBrace => (NodeShape::Hexagon, FlowToken::DoubleLeftBrace, FlowToken::DoubleRightBrace),
        FlowToken::LeftParen => {
            // Could be rounded rectangle or other shapes
            if i + 1 < tokens.len() && matches!(&tokens[i + 1], FlowToken::LeftSquare) {
                i += 1;
                (NodeShape::Stadium, FlowToken::LeftParen, FlowToken::RightParen)
            } else if i + 1 < tokens.len() && matches!(&tokens[i + 1], FlowToken::LeftBrace) {
                i += 1;
                (NodeShape::Cylinder, FlowToken::LeftParen, FlowToken::RightParen)
            } else {
                (NodeShape::RoundedRectangle, FlowToken::LeftParen, FlowToken::RightParen)
            }
        }
        FlowToken::LeftSquare => {
            // Check for special shapes with slashes
            if i + 1 < tokens.len() && matches!(&tokens[i + 1], FlowToken::Slash) {
                i += 1;
                (NodeShape::Parallelogram, FlowToken::LeftSquare, FlowToken::RightSquare)
            } else if i + 1 < tokens.len() && matches!(&tokens[i + 1], FlowToken::Backslash) {
                i += 1;
                (NodeShape::ParallelogramAlt, FlowToken::LeftSquare, FlowToken::RightSquare)
            } else {
                (NodeShape::Rectangle, FlowToken::LeftSquare, FlowToken::RightSquare)
            }
        }
        FlowToken::LeftBrace => (NodeShape::Rhombus, FlowToken::LeftBrace, FlowToken::RightBrace),
        FlowToken::LeftAngle => (NodeShape::Asymmetric, FlowToken::LeftAngle, FlowToken::RightSquare),
        _ => return None,
    };
    i += 1;
    
    // Parse node text
    let mut text_parts = Vec::new();
    let mut icon = None;
    
    while i < tokens.len() {
        match &tokens[i] {
            tok if tok == &end_pattern => {
                i += 1;
                break;
            }
            FlowToken::Text(t) | FlowToken::NodeId(t) | FlowToken::QuotedString(t) => {
                text_parts.push(t.clone());
                i += 1;
            }
            FlowToken::Icon(ic) => {
                icon = Some(ic.clone());
                i += 1;
            }
            FlowToken::Slash | FlowToken::Backslash => {
                // Part of shape syntax
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    
    let text = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(" "))
    };
    
    Some((
        FlowNode {
            id: node_id.to_string(),
            text,
            shape,
            classes: Vec::new(),
            icon,
        },
        i,
    ))
}

fn parse_edge(tokens: &[FlowToken], from_id: &str) -> Option<(FlowEdge, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 0;
    
    // Parse edge type
    let (edge_type, label_start, label_end) = match &tokens[i] {
        FlowToken::Arrow => (EdgeType::Arrow, None, None),
        FlowToken::DashDash => {
            i += 1;
            if i < tokens.len() {
                match &tokens[i] {
                    FlowToken::RightAngle => {
                        i += 1;
                        (EdgeType::Arrow, None, None)
                    }
                    FlowToken::Circle => {
                        i += 1;
                        (EdgeType::CircleEdge, None, None)
                    }
                    FlowToken::Cross => {
                        i += 1;
                        (EdgeType::CrossEdge, None, None)
                    }
                    FlowToken::Pipe => {
                        // Label start
                        (EdgeType::Arrow, Some(FlowToken::Pipe), Some(FlowToken::Pipe))
                    }
                    _ => (EdgeType::OpenLink, None, None),
                }
            } else {
                (EdgeType::OpenLink, None, None)
            }
        }
        FlowToken::Dot => {
            i += 1;
            if i < tokens.len() && matches!(&tokens[i], FlowToken::Dash) {
                i += 1;
                if i < tokens.len() && matches!(&tokens[i], FlowToken::RightAngle) {
                    i += 1;
                    (EdgeType::DottedArrow, None, None)
                } else {
                    (EdgeType::DottedLink, None, None)
                }
            } else {
                return None;
            }
        }
        FlowToken::Equal => {
            i += 1;
            if i < tokens.len() && matches!(&tokens[i], FlowToken::Equal) {
                i += 1;
                if i < tokens.len() && matches!(&tokens[i], FlowToken::RightAngle) {
                    i += 1;
                    (EdgeType::ThickArrow, None, None)
                } else {
                    (EdgeType::ThickLink, None, None)
                }
            } else {
                return None;
            }
        }
        FlowToken::Tilde => {
            i += 1;
            (EdgeType::Invisible, None, None)
        }
        _ => return None,
    };
    
    // Parse optional label
    let mut label = None;
    if let Some(start) = label_start {
        i += 1; // Skip label start
        let mut label_parts = Vec::new();
        while i < tokens.len() {
            if matches!(&tokens[i], label_end.unwrap()) {
                i += 1;
                break;
            }
            match &tokens[i] {
                FlowToken::Text(t) | FlowToken::NodeId(t) | FlowToken::QuotedString(t) => {
                    label_parts.push(t.clone());
                }
                _ => {}
            }
            i += 1;
        }
        if !label_parts.is_empty() {
            label = Some(label_parts.join(" "));
        }
    }
    
    // Parse target node
    let to_id = match &tokens[i] {
        FlowToken::NodeId(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    Some((
        FlowEdge {
            from: from_id.to_string(),
            to: to_id,
            edge_type,
            label,
            min_length: None,
        },
        i,
    ))
}

fn parse_subgraph(
    tokens: &[FlowToken],
    nodes: &mut HashMap<String, FlowNode>,
    edges: &mut Vec<FlowEdge>
) -> Option<(Subgraph, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip "subgraph"
    
    let id = match &tokens[i] {
        FlowToken::NodeId(id) => id.clone(),
        _ => format!("subgraph_{}", uuid::Uuid::new_v4()),
    };
    i += 1;
    
    let title = if i < tokens.len() {
        match &tokens[i] {
            FlowToken::LeftSquare => {
                i += 1;
                let mut title_parts = Vec::new();
                while i < tokens.len() && !matches!(&tokens[i], FlowToken::RightSquare) {
                    if let FlowToken::Text(t) | FlowToken::NodeId(t) = &tokens[i] {
                        title_parts.push(t.clone());
                    }
                    i += 1;
                }
                if matches!(&tokens[i], FlowToken::RightSquare) {
                    i += 1;
                }
                Some(title_parts.join(" "))
            }
            _ => None,
        }
    } else {
        None
    };
    
    // Skip to content
    while i < tokens.len() && !matches!(&tokens[i], FlowToken::NewLine) {
        i += 1;
    }
    
    let mut subgraph_nodes = Vec::new();
    let mut subgraph_edges = Vec::new();
    let mut nested_subgraphs = Vec::new();
    
    while i < tokens.len() {
        match &tokens[i] {
            FlowToken::End => {
                i += 1;
                break;
            }
            FlowToken::NodeId(node_id) => {
                if let Some((result, consumed)) = parse_node_or_edge(&tokens[i..], nodes) {
                    match result {
                        ParseResult::Node(node) => {
                            subgraph_nodes.push(node.id.clone());
                            nodes.insert(node.id.clone(), node);
                        }
                        ParseResult::Edge(edge) => {
                            subgraph_edges.push(edge.clone());
                            edges.push(edge);
                        }
                        ParseResult::NodeAndEdge(node, edge) => {
                            subgraph_nodes.push(node.id.clone());
                            nodes.insert(node.id.clone(), node);
                            subgraph_edges.push(edge.clone());
                            edges.push(edge);
                        }
                    }
                    i += consumed;
                } else {
                    i += 1;
                }
            }
            FlowToken::Subgraph => {
                if let Some((nested, consumed)) = parse_subgraph(&tokens[i..], nodes, edges) {
                    nested_subgraphs.push(nested);
                    i += consumed;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    
    Some((
        Subgraph {
            id,
            title,
            nodes: subgraph_nodes,
            edges: subgraph_edges,
            subgraphs: nested_subgraphs,
            direction: None,
        },
        i,
    ))
}

fn parse_style(tokens: &[FlowToken]) -> Option<(StyleDefinition, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip "style"
    
    let target = match &tokens[i] {
        FlowToken::NodeId(id) => StyleTarget::Node(id.clone()),
        _ => return None,
    };
    i += 1;
    
    let mut styles = HashMap::new();
    
    while i < tokens.len() {
        match &tokens[i] {
            FlowToken::StyleProperty(prop, val) => {
                styles.insert(prop.clone(), val.clone());
                i += 1;
                if i < tokens.len() && matches!(&tokens[i], FlowToken::Comma) {
                    i += 1;
                }
            }
            FlowToken::NewLine => break,
            _ => i += 1,
        }
    }
    
    Some((
        StyleDefinition {
            target,
            styles,
        },
        i,
    ))
}

fn parse_class_def(tokens: &[FlowToken]) -> Option<(ClassDef, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip "classDef"
    
    let name = match &tokens[i] {
        FlowToken::NodeId(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    let mut styles = HashMap::new();
    
    while i < tokens.len() {
        match &tokens[i] {
            FlowToken::StyleProperty(prop, val) => {
                styles.insert(prop.clone(), val.clone());
                i += 1;
                if i < tokens.len() && matches!(&tokens[i], FlowToken::Comma) {
                    i += 1;
                }
            }
            FlowToken::NewLine | FlowToken::Semicolon => break,
            _ => i += 1,
        }
    }
    
    Some((
        ClassDef {
            name,
            styles,
        },
        i,
    ))
}

fn parse_class_assignment(tokens: &[FlowToken]) -> Option<(Vec<(String, String)>, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip "class"
    
    let mut node_ids = Vec::new();
    
    // Parse node IDs
    while i < tokens.len() {
        match &tokens[i] {
            FlowToken::NodeId(id) => {
                node_ids.push(id.clone());
                i += 1;
                if i < tokens.len() && matches!(&tokens[i], FlowToken::Comma) {
                    i += 1;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    
    let class_name = match &tokens[i] {
        FlowToken::NodeId(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    let assignments = node_ids.into_iter()
        .map(|node_id| (node_id, class_name.clone()))
        .collect();
    
    Some((assignments, i))
}

fn parse_click(tokens: &[FlowToken]) -> Option<(ClickEvent, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip "click"
    
    let node_id = match &tokens[i] {
        FlowToken::NodeId(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    let action = if i < tokens.len() {
        match &tokens[i] {
            FlowToken::Url(url) => {
                i += 1;
                let target = if i < tokens.len() {
                    match &tokens[i] {
                        FlowToken::Target(t) => {
                            i += 1;
                            Some(t.clone())
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                ClickAction::Href(url.clone(), target)
            }
            FlowToken::NodeId(callback) if callback == "callback" => {
                i += 1;
                if let FlowToken::QuotedString(tooltip) = &tokens[i] {
                    i += 1;
                    ClickAction::Callback(tooltip.clone())
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    } else {
        return None;
    };
    
    Some((
        ClickEvent {
            node_id,
            action,
        },
        i,
    ))
}

fn is_edge_start(token: &FlowToken) -> bool {
    matches!(token, 
        FlowToken::Arrow | FlowToken::DashDash | FlowToken::Dot 
        | FlowToken::Equal | FlowToken::Tilde | FlowToken::Dash
    )
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/flowchart/`
- Expected count: 576 files (largest collection)
- Copy to: `mermaid-parser/test/flowchart/`

### Command
```bash
cp -r ../mermaid-samples/flowchart/* ./test/flowchart/
```

## Step 5: Unit Testing

### Comprehensive Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_flowchart_files(#[files("test/flowchart/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = flowchart_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = flowchart_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.nodes.is_empty() || !diagram.subgraphs.is_empty(), 
            "Should have at least one node or subgraph");
}

#[test]
fn test_simple_flowchart() {
    let input = r#"flowchart TD
    A[Start] --> B{Is it?}
    B -->|Yes| C[OK]
    B -->|No| D[End]
"#;
    
    let tokens = flowchart_lexer().parse(input.chars()).unwrap();
    let diagram = flowchart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.direction, FlowDirection::TD);
    assert_eq!(diagram.nodes.len(), 4);
    assert_eq!(diagram.edges.len(), 3);
    
    assert_eq!(diagram.nodes["A"].shape, NodeShape::Rectangle);
    assert_eq!(diagram.nodes["B"].shape, NodeShape::Rhombus);
}

#[test]
fn test_node_shapes() {
    let input = r#"graph LR
    A[Rectangle]
    B(Rounded)
    C([Stadium])
    D[[Subroutine]]
    E[(Cylinder)]
    F((Circle))
    G>Asymmetric]
    H{Rhombus}
    I{{Hexagon}}
    J[/Parallelogram/]
    K[\Parallelogram alt\]
    L[/Trapezoid\]
    M[\Trapezoid alt/]
    N(((Double circle)))
"#;
    
    let tokens = flowchart_lexer().parse(input.chars()).unwrap();
    let diagram = flowchart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.nodes.len(), 14);
    assert_eq!(diagram.nodes["A"].shape, NodeShape::Rectangle);
    assert_eq!(diagram.nodes["F"].shape, NodeShape::Circle);
    assert_eq!(diagram.nodes["N"].shape, NodeShape::DoubleCircle);
}

#[test]
fn test_edge_types() {
    let input = r#"flowchart LR
    A --> B
    C -.-> D
    E ==> F
    G --- H
    I -.- J
    K === L
    M ~~~ N
    O --o P
    Q --x R
"#;
    
    let tokens = flowchart_lexer().parse(input.chars()).unwrap();
    let diagram = flowchart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.edges.len(), 9);
    // Verify edge types
}

#[test]
fn test_subgraphs() {
    let input = r#"flowchart TB
    subgraph one [One]
        A --> B
    end
    
    subgraph two [Two]
        C --> D
        subgraph three [Three]
            E --> F
        end
    end
    
    B --> C
"#;
    
    let tokens = flowchart_lexer().parse(input.chars()).unwrap();
    let diagram = flowchart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.subgraphs.len(), 2);
    assert_eq!(diagram.subgraphs[0].title, Some("One".to_string()));
    assert_eq!(diagram.subgraphs[1].subgraphs.len(), 1);
}

#[test]
fn test_styling() {
    let input = r#"flowchart LR
    A:::orange
    
    style A fill:#f9f,stroke:#333,stroke-width:4px
    style B fill:#bbf,stroke:#f66,stroke-width:2px,color:#fff,stroke-dasharray: 5 5
    
    classDef green fill:#9f6,stroke:#333,stroke-width:2px;
    classDef orange fill:#f96,stroke:#333,stroke-width:4px;
    
    class B,C green
"#;
    
    let tokens = flowchart_lexer().parse(input.chars()).unwrap();
    let diagram = flowchart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.styles.len(), 2);
    assert_eq!(diagram.class_defs.len(), 2);
    assert!(diagram.nodes["B"].classes.contains(&"green".to_string()));
}

#[test]
fn test_click_events() {
    let input = r#"flowchart LR
    A[Google]
    B[Callback]
    
    click A "https://www.google.com" _blank
    click B callback "Tooltip for B"
"#;
    
    let tokens = flowchart_lexer().parse(input.chars()).unwrap();
    let diagram = flowchart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.clicks.len(), 2);
    
    match &diagram.clicks[0].action {
        ClickAction::Href(url, target) => {
            assert_eq!(url, "https://www.google.com");
            assert_eq!(target, &Some("_blank".to_string()));
        }
        _ => panic!("Expected href action"),
    }
}

#[test]
fn test_complex_labels() {
    let input = r#"flowchart TD
    A -->|This is the text| B
    C ---|This is also text| D
    E -.-|Dotted link with text| F
"#;
    
    let tokens = flowchart_lexer().parse(input.chars()).unwrap();
    let diagram = flowchart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.edges[0].label, Some("This is the text".to_string()));
    assert_eq!(diagram.edges[1].label, Some("This is also text".to_string()));
    assert_eq!(diagram.edges[2].label, Some("Dotted link with text".to_string()));
}

#[test]
fn test_icons() {
    let input = r#"flowchart TD
    A[fa:fa-car Car]
    B[fa:fa-phone Phone icon]
"#;
    
    let tokens = flowchart_lexer().parse(input.chars()).unwrap();
    let diagram = flowchart_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.nodes["A"].icon, Some("fa:fa-car".to_string()));
    assert_eq!(diagram.nodes["A"].text, Some("Car".to_string()));
}
```

## Success Criteria
1. ✅ Parse all 576 flowchart sample files successfully
2. ✅ Support all 15+ node shapes
3. ✅ Handle all edge types and variations
4. ✅ Parse nested subgraphs correctly
5. ✅ Support inline and external styling
6. ✅ Handle class definitions and assignments
7. ✅ Parse click events with callbacks and URLs
8. ✅ Support icons in nodes
9. ✅ Handle complex edge labels
10. ✅ Parse all directional variants (TD, LR, etc.)
11. ✅ Support both `graph` and `flowchart` keywords

## Implementation Priority
**Priority 22** - Implement in Phase 4 as the most complex diagram type. Flowcharts require all the patterns learned from previous implementations: node shapes, edge variations, hierarchical structures (subgraphs), styling systems, and event handling. This is the culmination of all diagram parsing techniques and serves as the ultimate test of the parser architecture.

## Complexity Notes
- **Lexer Complexity**: Must handle overlapping patterns (e.g., `--`, `-->`, `--o`, etc.)
- **Parser Complexity**: Stateful parsing for subgraphs, complex node shape detection
- **Edge Cases**: Special shapes with internal syntax (e.g., `[/text/]`), multi-line text
- **Performance**: With 576 test files, optimization will be critical