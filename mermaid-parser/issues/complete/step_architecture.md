# Implementation Plan: Architecture Diagrams

## Overview
Architecture diagrams represent system architecture with services, groups, junctions, and connections.
TypeScript-based parser requiring analysis of implementation patterns rather than formal grammar.

## TypeScript Parser Analysis

### Key Features (from architectureParser.ts)
- Services: Nodes in the architecture
- Groups: Logical grouping of services
- Junctions: Connection points for complex routing
- Edges: Various connection types between services
- Icons: Support for service icons
- Directions: TB, BT, LR, RL
- Comments: `%%` for line comments

### Example Input
```
architecture-beta
    group api(cloud)[API]

    service db(database)[Database] in api
    service disk1(disk)[Storage] in api
    service disk2(disk)[Storage] in api
    service server(server)[Server] in api

    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ArchitectureDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub direction: ArchDirection,
    pub services: HashMap<String, Service>,
    pub groups: HashMap<String, Group>,
    pub junctions: HashMap<String, Junction>,
    pub edges: Vec<ArchEdge>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArchDirection {
    TB, // Top to Bottom
    BT, // Bottom to Top
    LR, // Left to Right
    RL, // Right to Left
}

#[derive(Debug, Clone, PartialEq)]
pub struct Service {
    pub id: String,
    pub icon: Option<String>,
    pub title: String,
    pub in_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub id: String,
    pub icon: Option<String>,
    pub title: String,
    pub in_group: Option<String>, // For nested groups
}

#[derive(Debug, Clone, PartialEq)]
pub struct Junction {
    pub id: String,
    pub in_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchEdge {
    pub from: EdgeEndpoint,
    pub to: EdgeEndpoint,
    pub label: Option<String>,
    pub edge_type: ArchEdgeType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeEndpoint {
    pub id: String,
    pub port: Option<Port>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Port {
    Left,   // L
    Right,  // R
    Top,    // T
    Bottom, // B
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArchEdgeType {
    Solid,      // --
    Dotted,     // ..
    Arrow,      // ->
    BiArrow,    // <->
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArchToken {
    ArchitectureBeta,        // "architecture-beta"
    Group,                   // "group"
    Service,                 // "service"
    Junction,                // "junction"
    In,                      // "in"
    LeftParen,               // (
    RightParen,              // )
    LeftSquare,              // [
    RightSquare,             // ]
    Colon,                   // :
    Dash,                    // -
    DashDash,                // --
    Dot,                     // .
    DotDot,                  // ..
    Arrow,                   // ->
    BiArrow,                 // <->
    PortL,                   // L
    PortR,                   // R
    PortT,                   // T
    PortB,                   // B
    Identifier(String),      // IDs and names
    Icon(String),            // Icon names
    Title(String),           // Quoted titles
    Comment(String),
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn architecture_lexer() -> impl Parser<char, Vec<ArchToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| ArchToken::Comment(text.into_iter().collect()));
    
    // Keywords
    let keywords = choice((
        text::keyword("architecture-beta").map(|_| ArchToken::ArchitectureBeta),
        text::keyword("group").map(|_| ArchToken::Group),
        text::keyword("service").map(|_| ArchToken::Service),
        text::keyword("junction").map(|_| ArchToken::Junction),
        text::keyword("in").map(|_| ArchToken::In),
    ));
    
    // Port specifiers
    let ports = choice((
        just('L').map(|_| ArchToken::PortL),
        just('R').map(|_| ArchToken::PortR),
        just('T').map(|_| ArchToken::PortT),
        just('B').map(|_| ArchToken::PortB),
    ));
    
    // Edge types
    let edges = choice((
        text::string("<->").map(|_| ArchToken::BiArrow),
        text::string("->").map(|_| ArchToken::Arrow),
        text::string("--").map(|_| ArchToken::DashDash),
        text::string("..").map(|_| ArchToken::DotDot),
        just('-').map(|_| ArchToken::Dash),
        just('.').map(|_| ArchToken::Dot),
    ));
    
    // Icon in parentheses
    let icon = just('(')
        .ignore_then(
            filter(|c: &char| c.is_alphanumeric() || *c == '-' || *c == '_')
                .repeated()
                .at_least(1)
                .collect::<String>()
        )
        .then_ignore(just(')'))
        .map(ArchToken::Icon);
    
    // Title in square brackets
    let title = just('[')
        .ignore_then(
            none_of("]")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just(']'))
        .map(ArchToken::Title);
    
    // Identifier
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_' || *c == '-'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(ArchToken::Identifier);
    
    let left_paren = just('(').map(|_| ArchToken::LeftParen);
    let right_paren = just(')').map(|_| ArchToken::RightParen);
    let left_square = just('[').map(|_| ArchToken::LeftSquare);
    let right_square = just(']').map(|_| ArchToken::RightSquare);
    let colon = just(':').map(|_| ArchToken::Colon);
    
    let newline = just('\n').map(|_| ArchToken::NewLine);
    
    let token = choice((
        comment,
        keywords,
        edges,
        ports,
        icon,
        title,
        left_paren,
        right_paren,
        left_square,
        right_square,
        colon,
        identifier,
    ));
    
    whitespace
        .ignore_then(token)
        .or(newline)
        .repeated()
        .then_ignore(end())
}
```

## Step 3: Parser Implementation

### TypeScript-Style Parser
```rust
pub fn architecture_parser() -> impl Parser<ArchToken, ArchitectureDiagram, Error = Simple<ArchToken>> {
    just(ArchToken::ArchitectureBeta)
        .then_ignore(
            filter(|t| matches!(t, ArchToken::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(ArchToken::Eof).or_not())
        .map(|(_, tokens)| {
            let mut services = HashMap::new();
            let mut groups = HashMap::new();
            let mut junctions = HashMap::new();
            let mut edges = Vec::new();
            let mut i = 0;
            
            while i < tokens.len() {
                match &tokens[i] {
                    ArchToken::Comment(_) | ArchToken::NewLine => {
                        i += 1;
                    }
                    ArchToken::Group => {
                        if let Some((group, consumed)) = parse_group(&tokens[i..]) {
                            groups.insert(group.id.clone(), group);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    ArchToken::Service => {
                        if let Some((service, consumed)) = parse_service(&tokens[i..]) {
                            services.insert(service.id.clone(), service);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    ArchToken::Junction => {
                        if let Some((junction, consumed)) = parse_junction(&tokens[i..]) {
                            junctions.insert(junction.id.clone(), junction);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    ArchToken::Identifier(id) => {
                        // Try to parse edge
                        if let Some((edge, consumed)) = parse_edge(&tokens[i..], id) {
                            edges.push(edge);
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
            
            ArchitectureDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                direction: ArchDirection::TB, // Default
                services,
                groups,
                junctions,
                edges,
            }
        })
}

fn parse_group(tokens: &[ArchToken]) -> Option<(Group, usize)> {
    if tokens.len() < 4 {
        return None;
    }
    
    let mut i = 1; // Skip "group"
    
    let id = match &tokens[i] {
        ArchToken::Identifier(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    let icon = match &tokens[i] {
        ArchToken::Icon(icon) => {
            i += 1;
            Some(icon.clone())
        }
        _ => None,
    };
    
    let title = match &tokens[i] {
        ArchToken::Title(title) => {
            i += 1;
            title.clone()
        }
        _ => id.clone(),
    };
    
    // Check for nested group
    let in_group = if i + 1 < tokens.len() && matches!(&tokens[i], ArchToken::In) {
        i += 1;
        match &tokens[i] {
            ArchToken::Identifier(parent) => {
                i += 1;
                Some(parent.clone())
            }
            _ => None,
        }
    } else {
        None
    };
    
    Some((
        Group {
            id,
            icon,
            title,
            in_group,
        },
        i,
    ))
}

fn parse_service(tokens: &[ArchToken]) -> Option<(Service, usize)> {
    if tokens.len() < 4 {
        return None;
    }
    
    let mut i = 1; // Skip "service"
    
    let id = match &tokens[i] {
        ArchToken::Identifier(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    let icon = match &tokens[i] {
        ArchToken::Icon(icon) => {
            i += 1;
            Some(icon.clone())
        }
        _ => None,
    };
    
    let title = match &tokens[i] {
        ArchToken::Title(title) => {
            i += 1;
            title.clone()
        }
        _ => id.clone(),
    };
    
    // Check for group membership
    let in_group = if i + 1 < tokens.len() && matches!(&tokens[i], ArchToken::In) {
        i += 1;
        match &tokens[i] {
            ArchToken::Identifier(group) => {
                i += 1;
                Some(group.clone())
            }
            _ => None,
        }
    } else {
        None
    };
    
    Some((
        Service {
            id,
            icon,
            title,
            in_group,
        },
        i,
    ))
}

fn parse_junction(tokens: &[ArchToken]) -> Option<(Junction, usize)> {
    if tokens.len() < 2 {
        return None;
    }
    
    let mut i = 1; // Skip "junction"
    
    let id = match &tokens[i] {
        ArchToken::Identifier(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    // Check for group membership
    let in_group = if i + 1 < tokens.len() && matches!(&tokens[i], ArchToken::In) {
        i += 1;
        match &tokens[i] {
            ArchToken::Identifier(group) => {
                i += 1;
                Some(group.clone())
            }
            _ => None,
        }
    } else {
        None
    };
    
    Some((
        Junction {
            id,
            in_group,
        },
        i,
    ))
}

fn parse_edge(tokens: &[ArchToken], from_id: &str) -> Option<(ArchEdge, usize)> {
    if tokens.len() < 5 {
        return None;
    }
    
    let mut i = 1; // Skip from_id
    
    // Parse from port if present
    let from_port = if matches!(&tokens[i], ArchToken::Colon) {
        i += 1;
        match &tokens[i] {
            ArchToken::PortL => {
                i += 1;
                Some(Port::Left)
            }
            ArchToken::PortR => {
                i += 1;
                Some(Port::Right)
            }
            ArchToken::PortT => {
                i += 1;
                Some(Port::Top)
            }
            ArchToken::PortB => {
                i += 1;
                Some(Port::Bottom)
            }
            _ => None,
        }
    } else {
        None
    };
    
    // Parse edge type
    let edge_type = match &tokens[i] {
        ArchToken::DashDash => {
            i += 1;
            ArchEdgeType::Solid
        }
        ArchToken::DotDot => {
            i += 1;
            ArchEdgeType::Dotted
        }
        ArchToken::Arrow => {
            i += 1;
            ArchEdgeType::Arrow
        }
        ArchToken::BiArrow => {
            i += 1;
            ArchEdgeType::BiArrow
        }
        _ => return None,
    };
    
    // Parse to port if present
    let (to_port, to_id) = if matches!(&tokens[i], ArchToken::PortL | ArchToken::PortR | ArchToken::PortT | ArchToken::PortB) {
        let port = match &tokens[i] {
            ArchToken::PortL => Port::Left,
            ArchToken::PortR => Port::Right,
            ArchToken::PortT => Port::Top,
            ArchToken::PortB => Port::Bottom,
            _ => unreachable!(),
        };
        i += 1;
        
        if matches!(&tokens[i], ArchToken::Colon) {
            i += 1;
            match &tokens[i] {
                ArchToken::Identifier(id) => {
                    i += 1;
                    (Some(port), id.clone())
                }
                _ => return None,
            }
        } else {
            return None;
        }
    } else {
        match &tokens[i] {
            ArchToken::Identifier(id) => {
                i += 1;
                (None, id.clone())
            }
            _ => return None,
        }
    };
    
    Some((
        ArchEdge {
            from: EdgeEndpoint {
                id: from_id.to_string(),
                port: from_port,
            },
            to: EdgeEndpoint {
                id: to_id,
                port: to_port,
            },
            label: None,
            edge_type,
        },
        i,
    ))
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/architecture/`
- Expected count: 38 files
- Copy to: `mermaid-parser/test/architecture/`

### Command
```bash
cp -r ../mermaid-samples/architecture/* ./test/architecture/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_architecture_files(#[files("test/architecture/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = architecture_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = architecture_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.services.is_empty() || !diagram.groups.is_empty(), 
            "Should have at least one service or group");
}

#[test]
fn test_simple_architecture() {
    let input = r#"architecture-beta
    service api[API Service]
    service db(database)[Database]
    
    api -- db
"#;
    
    let tokens = architecture_lexer().parse(input.chars()).unwrap();
    let diagram = architecture_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.services.len(), 2);
    assert_eq!(diagram.edges.len(), 1);
    
    let api = &diagram.services["api"];
    assert_eq!(api.title, "API Service");
    assert!(api.icon.is_none());
    
    let db = &diagram.services["db"];
    assert_eq!(db.icon, Some("database".to_string()));
}

#[test]
fn test_groups_and_services() {
    let input = r#"architecture-beta
    group backend[Backend Services]
    
    service api[API] in backend
    service db(database)[Database] in backend
    service cache(memory)[Cache] in backend
"#;
    
    let tokens = architecture_lexer().parse(input.chars()).unwrap();
    let diagram = architecture_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.groups.len(), 1);
    assert_eq!(diagram.services.len(), 3);
    
    for (_, service) in &diagram.services {
        assert_eq!(service.in_group, Some("backend".to_string()));
    }
}

#[test]
fn test_edges_with_ports() {
    let input = r#"architecture-beta
    service a[Service A]
    service b[Service B]
    service c[Service C]
    
    a:R -- L:b
    b:B -- T:c
    a:L <-> R:c
"#;
    
    let tokens = architecture_lexer().parse(input.chars()).unwrap();
    let diagram = architecture_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.edges.len(), 3);
    
    let edge1 = &diagram.edges[0];
    assert_eq!(edge1.from.port, Some(Port::Right));
    assert_eq!(edge1.to.port, Some(Port::Left));
    
    let edge3 = &diagram.edges[2];
    assert_eq!(edge3.edge_type, ArchEdgeType::BiArrow);
}

#[test]
fn test_junctions() {
    let input = r#"architecture-beta
    group network[Network]
    
    service server1[Server 1] in network
    service server2[Server 2] in network
    junction j1 in network
    
    server1 -- j1
    server2 -- j1
"#;
    
    let tokens = architecture_lexer().parse(input.chars()).unwrap();
    let diagram = architecture_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.junctions.len(), 1);
    assert_eq!(diagram.junctions["j1"].in_group, Some("network".to_string()));
}

#[test]
fn test_nested_groups() {
    let input = r#"architecture-beta
    group cloud[Cloud Infrastructure]
    group region[Region] in cloud
    
    service app[Application] in region
"#;
    
    let tokens = architecture_lexer().parse(input.chars()).unwrap();
    let diagram = architecture_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.groups.len(), 2);
    assert_eq!(diagram.groups["region"].in_group, Some("cloud".to_string()));
}
```

## Success Criteria
1. ✅ Parse all 38 architecture sample files successfully
2. ✅ Handle services with icons and titles
3. ✅ Support groups and nested groups
4. ✅ Parse junctions correctly
5. ✅ Handle edges with port specifications (L, R, T, B)
6. ✅ Support different edge types (solid, dotted, arrow, bi-arrow)
7. ✅ Manage group membership for services and junctions
8. ✅ Parse without formal grammar definition

## Implementation Priority
**Priority 17** - Implement in Phase 3 alongside Git diagrams. Architecture diagrams introduce service groupings and port-based connections that complement the workflow concepts from Git. The TypeScript parsing approach provides variety after implementing many jison-based parsers.