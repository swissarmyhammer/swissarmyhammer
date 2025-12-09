# Implementation Plan: C4 Diagrams

## Overview
C4 diagrams represent software architecture at multiple levels: Context, Container, Component, and Dynamic.
Medium-high complexity grammar (322 lines) supporting multiple diagram contexts and enterprise-scale system modeling.

## Grammar Analysis

### Key Features
- Headers: `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, `C4Deployment`
- Elements: Person, System, Container, Component, Node, Deployment Node
- Boundaries: System_Boundary, Container_Boundary, Boundary, Enterprise_Boundary
- Relationships: Rel, BiRel, Rel_Up, Rel_Down, Rel_Left, Rel_Right
- Properties: External suffix (Person_Ext, System_Ext)
- Attributes: Technology stack, descriptions, tags
- Comments: `%%` for line comments

### Example Input
```
C4Context
    title System Context diagram for Internet Banking System

    Person(customerA, "Banking Customer A", "A customer of the bank, with personal bank accounts.")
    Person(customerB, "Banking Customer B")
    Person_Ext(customerC, "Banking Customer C")
    System(SystemAA, "Internet Banking System", "Allows customers to view information about their bank accounts, and make payments.")
    Person(customerD, "Banking Customer D", "A customer of the bank, <br/> with personal bank accounts.")

    Enterprise_Boundary(b1, "BankBoundary") {
        SystemDb_Ext(SystemE, "Mainframe Banking System", "Stores all of the core banking information about customers, accounts, transactions, etc.")
        System_Boundary(b2, "BankBoundary2") {
            System(SystemA, "Banking System A")
            System(SystemB, "Banking System B", "A system of the bank, with personal bank accounts.")
        }
        System_Ext(SystemC, "E-mail system", "The internal Microsoft Exchange e-mail system.")
        SystemDb(SystemD, "Banking System D Database", "A system of the bank, with personal bank accounts.")
        Boundary(b3, "BankBoundary3", "boundary") {
            SystemQueue(SystemF, "Banking System F Queue", "A system of the bank, with personal bank accounts.")
            SystemQueue_Ext(SystemG, "Banking System G Queue", "A system of the bank, with personal bank accounts.")
        }
    }

    BiRel(customerA, SystemAA, "Uses")
    BiRel(SystemAA, SystemE, "Uses")
    Rel(SystemAA, SystemC, "Sends e-mails", "SMTP")
    Rel(SystemC, customerA, "Sends e-mails to")
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct C4Diagram {
    pub diagram_type: C4DiagramType,
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub elements: HashMap<String, C4Element>,
    pub boundaries: Vec<C4Boundary>,
    pub relationships: Vec<C4Relationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum C4DiagramType {
    Context,
    Container,
    Component,
    Dynamic,
    Deployment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Element {
    pub id: String,
    pub element_type: C4ElementType,
    pub name: String,
    pub description: Option<String>,
    pub technology: Option<String>,
    pub tags: Vec<String>,
    pub is_external: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum C4ElementType {
    Person,
    System,
    SystemDb,
    SystemQueue,
    Container,
    ContainerDb,
    ContainerQueue,
    Component,
    ComponentDb,
    ComponentQueue,
    Node,
    DeploymentNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Boundary {
    pub id: String,
    pub boundary_type: C4BoundaryType,
    pub label: String,
    pub tags: Vec<String>,
    pub elements: Vec<String>,  // Element IDs
    pub boundaries: Vec<C4Boundary>,  // Nested boundaries
}

#[derive(Debug, Clone, PartialEq)]
pub enum C4BoundaryType {
    System,
    Container,
    Enterprise,
    Generic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Relationship {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub technology: Option<String>,
    pub direction: RelationshipDirection,
    pub is_bidirectional: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationshipDirection {
    Default,
    Up,
    Down,
    Left,
    Right,
    Back,
}

#[derive(Debug, Clone, PartialEq)]
pub enum C4Token {
    // Diagram types
    C4Context,
    C4Container,
    C4Component,
    C4Dynamic,
    C4Deployment,
    
    // Keywords
    Title,
    UpdateElementStyle,
    UpdateRelStyle,
    UpdateBoundaryStyle,
    UpdateLayoutConfig,
    
    // Element types
    Person,
    PersonExt,
    System,
    SystemExt,
    SystemDb,
    SystemDbExt,
    SystemQueue,
    SystemQueueExt,
    Container,
    ContainerExt,
    ContainerDb,
    ContainerDbExt,
    ContainerQueue,
    ContainerQueueExt,
    Component,
    ComponentExt,
    ComponentDb,
    ComponentDbExt,
    ComponentQueue,
    ComponentQueueExt,
    Node,
    NodeExt,
    DeploymentNode,
    DeploymentNodeExt,
    
    // Boundary types
    SystemBoundary,
    ContainerBoundary,
    EnterpriseBoundary,
    Boundary,
    
    // Relationship types
    Rel,
    BiRel,
    RelUp,
    RelDown,
    RelLeft,
    RelRight,
    RelBack,
    
    // Symbols
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    DollarSign,
    
    // Values
    Identifier(String),
    QuotedString(String),
    Variable(String),     // $variable
    Comment(String),
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn c4_lexer() -> impl Parser<char, Vec<C4Token>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| C4Token::Comment(text.into_iter().collect()));
    
    // Diagram type keywords
    let diagram_types = choice((
        text::keyword("C4Context").map(|_| C4Token::C4Context),
        text::keyword("C4Container").map(|_| C4Token::C4Container),
        text::keyword("C4Component").map(|_| C4Token::C4Component),
        text::keyword("C4Dynamic").map(|_| C4Token::C4Dynamic),
        text::keyword("C4Deployment").map(|_| C4Token::C4Deployment),
    ));
    
    // Other keywords
    let keywords = choice((
        text::keyword("title").map(|_| C4Token::Title),
        text::keyword("UpdateElementStyle").map(|_| C4Token::UpdateElementStyle),
        text::keyword("UpdateRelStyle").map(|_| C4Token::UpdateRelStyle),
        text::keyword("UpdateBoundaryStyle").map(|_| C4Token::UpdateBoundaryStyle),
        text::keyword("UpdateLayoutConfig").map(|_| C4Token::UpdateLayoutConfig),
    ));
    
    // Element types (order matters for _Ext suffixes)
    let element_types = choice((
        text::keyword("Person_Ext").map(|_| C4Token::PersonExt),
        text::keyword("Person").map(|_| C4Token::Person),
        text::keyword("System_Ext").map(|_| C4Token::SystemExt),
        text::keyword("SystemDb_Ext").map(|_| C4Token::SystemDbExt),
        text::keyword("SystemDb").map(|_| C4Token::SystemDb),
        text::keyword("SystemQueue_Ext").map(|_| C4Token::SystemQueueExt),
        text::keyword("SystemQueue").map(|_| C4Token::SystemQueue),
        text::keyword("System").map(|_| C4Token::System),
        text::keyword("Container_Ext").map(|_| C4Token::ContainerExt),
        text::keyword("ContainerDb_Ext").map(|_| C4Token::ContainerDbExt),
        text::keyword("ContainerDb").map(|_| C4Token::ContainerDb),
        text::keyword("ContainerQueue_Ext").map(|_| C4Token::ContainerQueueExt),
        text::keyword("ContainerQueue").map(|_| C4Token::ContainerQueue),
        text::keyword("Container").map(|_| C4Token::Container),
        text::keyword("Component_Ext").map(|_| C4Token::ComponentExt),
        text::keyword("ComponentDb_Ext").map(|_| C4Token::ComponentDbExt),
        text::keyword("ComponentDb").map(|_| C4Token::ComponentDb),
        text::keyword("ComponentQueue_Ext").map(|_| C4Token::ComponentQueueExt),
        text::keyword("ComponentQueue").map(|_| C4Token::ComponentQueue),
        text::keyword("Component").map(|_| C4Token::Component),
        text::keyword("Node_Ext").map(|_| C4Token::NodeExt),
        text::keyword("Node").map(|_| C4Token::Node),
        text::keyword("DeploymentNode_Ext").map(|_| C4Token::DeploymentNodeExt),
        text::keyword("DeploymentNode").map(|_| C4Token::DeploymentNode),
    ));
    
    // Boundary types
    let boundary_types = choice((
        text::keyword("System_Boundary").map(|_| C4Token::SystemBoundary),
        text::keyword("Container_Boundary").map(|_| C4Token::ContainerBoundary),
        text::keyword("Enterprise_Boundary").map(|_| C4Token::EnterpriseBoundary),
        text::keyword("Boundary").map(|_| C4Token::Boundary),
    ));
    
    // Relationship types
    let relationship_types = choice((
        text::keyword("BiRel").map(|_| C4Token::BiRel),
        text::keyword("Rel_Up").map(|_| C4Token::RelUp),
        text::keyword("Rel_Down").map(|_| C4Token::RelDown),
        text::keyword("Rel_Left").map(|_| C4Token::RelLeft),
        text::keyword("Rel_Right").map(|_| C4Token::RelRight),
        text::keyword("Rel_Back").map(|_| C4Token::RelBack),
        text::keyword("Rel").map(|_| C4Token::Rel),
    ));
    
    // Quoted string (supports multi-line with <br/>)
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(C4Token::QuotedString);
    
    // Variable ($variable)
    let variable = just('$')
        .then(
            filter(|c: &char| c.is_alphanumeric() || *c == '_')
                .repeated()
                .at_least(1)
        )
        .map(|(_, chars)| C4Token::Variable(chars.into_iter().collect()));
    
    // Identifier
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(C4Token::Identifier);
    
    let left_paren = just('(').map(|_| C4Token::LeftParen);
    let right_paren = just(')').map(|_| C4Token::RightParen);
    let left_brace = just('{').map(|_| C4Token::LeftBrace);
    let right_brace = just('}').map(|_| C4Token::RightBrace);
    let comma = just(',').map(|_| C4Token::Comma);
    let dollar = just('$').map(|_| C4Token::DollarSign);
    
    let newline = just('\n').map(|_| C4Token::NewLine);
    
    let token = choice((
        comment,
        diagram_types,
        keywords,
        element_types,
        boundary_types,
        relationship_types,
        quoted_string,
        variable,
        left_paren,
        right_paren,
        left_brace,
        right_brace,
        comma,
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

### Hierarchical C4 Parser
```rust
pub fn c4_parser() -> impl Parser<C4Token, C4Diagram, Error = Simple<C4Token>> {
    let diagram_type = choice((
        just(C4Token::C4Context).map(|_| C4DiagramType::Context),
        just(C4Token::C4Container).map(|_| C4DiagramType::Container),
        just(C4Token::C4Component).map(|_| C4DiagramType::Component),
        just(C4Token::C4Dynamic).map(|_| C4DiagramType::Dynamic),
        just(C4Token::C4Deployment).map(|_| C4DiagramType::Deployment),
    ));
    
    diagram_type
        .then_ignore(
            filter(|t| matches!(t, C4Token::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(C4Token::Eof).or_not())
        .map(|(diagram_type, tokens)| {
            let mut elements = HashMap::new();
            let mut boundaries = Vec::new();
            let mut relationships = Vec::new();
            let mut title = None;
            let mut i = 0;
            
            while i < tokens.len() {
                match &tokens[i] {
                    C4Token::Comment(_) | C4Token::NewLine => {
                        i += 1;
                    }
                    C4Token::Title => {
                        if let Some((t, consumed)) = parse_title(&tokens[i..]) {
                            title = Some(t);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    C4Token::Person | C4Token::PersonExt
                    | C4Token::System | C4Token::SystemExt
                    | C4Token::SystemDb | C4Token::SystemDbExt
                    | C4Token::SystemQueue | C4Token::SystemQueueExt
                    | C4Token::Container | C4Token::ContainerExt
                    | C4Token::ContainerDb | C4Token::ContainerDbExt
                    | C4Token::ContainerQueue | C4Token::ContainerQueueExt
                    | C4Token::Component | C4Token::ComponentExt
                    | C4Token::ComponentDb | C4Token::ComponentDbExt
                    | C4Token::ComponentQueue | C4Token::ComponentQueueExt
                    | C4Token::Node | C4Token::NodeExt
                    | C4Token::DeploymentNode | C4Token::DeploymentNodeExt => {
                        if let Some((elem, consumed)) = parse_element(&tokens[i..]) {
                            elements.insert(elem.id.clone(), elem);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    C4Token::SystemBoundary | C4Token::ContainerBoundary
                    | C4Token::EnterpriseBoundary | C4Token::Boundary => {
                        if let Some((boundary, consumed)) = parse_boundary(&tokens[i..], &mut elements) {
                            boundaries.push(boundary);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    C4Token::Rel | C4Token::BiRel
                    | C4Token::RelUp | C4Token::RelDown
                    | C4Token::RelLeft | C4Token::RelRight
                    | C4Token::RelBack => {
                        if let Some((rel, consumed)) = parse_relationship(&tokens[i..]) {
                            relationships.push(rel);
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
            
            C4Diagram {
                diagram_type,
                title,
                accessibility: AccessibilityInfo::default(),
                elements,
                boundaries,
                relationships,
            }
        })
}

fn parse_title(tokens: &[C4Token]) -> Option<(String, usize)> {
    if tokens.len() < 2 {
        return None;
    }
    
    let mut i = 1; // Skip "title"
    
    match &tokens[i] {
        C4Token::QuotedString(s) | C4Token::Identifier(s) => {
            Some((s.clone(), 2))
        }
        _ => None,
    }
}

fn parse_element(tokens: &[C4Token]) -> Option<(C4Element, usize)> {
    if tokens.len() < 4 {
        return None;
    }
    
    let mut i = 0;
    
    let (element_type, is_external) = match &tokens[i] {
        C4Token::Person => (C4ElementType::Person, false),
        C4Token::PersonExt => (C4ElementType::Person, true),
        C4Token::System => (C4ElementType::System, false),
        C4Token::SystemExt => (C4ElementType::System, true),
        C4Token::SystemDb => (C4ElementType::SystemDb, false),
        C4Token::SystemDbExt => (C4ElementType::SystemDb, true),
        C4Token::SystemQueue => (C4ElementType::SystemQueue, false),
        C4Token::SystemQueueExt => (C4ElementType::SystemQueue, true),
        C4Token::Container => (C4ElementType::Container, false),
        C4Token::ContainerExt => (C4ElementType::Container, true),
        C4Token::ContainerDb => (C4ElementType::ContainerDb, false),
        C4Token::ContainerDbExt => (C4ElementType::ContainerDb, true),
        C4Token::ContainerQueue => (C4ElementType::ContainerQueue, false),
        C4Token::ContainerQueueExt => (C4ElementType::ContainerQueue, true),
        C4Token::Component => (C4ElementType::Component, false),
        C4Token::ComponentExt => (C4ElementType::Component, true),
        C4Token::ComponentDb => (C4ElementType::ComponentDb, false),
        C4Token::ComponentDbExt => (C4ElementType::ComponentDb, true),
        C4Token::ComponentQueue => (C4ElementType::ComponentQueue, false),
        C4Token::ComponentQueueExt => (C4ElementType::ComponentQueue, true),
        C4Token::Node => (C4ElementType::Node, false),
        C4Token::NodeExt => (C4ElementType::Node, true),
        C4Token::DeploymentNode => (C4ElementType::DeploymentNode, false),
        C4Token::DeploymentNodeExt => (C4ElementType::DeploymentNode, true),
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], C4Token::LeftParen) {
        return None;
    }
    i += 1;
    
    // Parse element parameters: (id, name, description?, technology?)
    let id = match &tokens[i] {
        C4Token::Identifier(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], C4Token::Comma) {
        return None;
    }
    i += 1;
    
    let name = match &tokens[i] {
        C4Token::QuotedString(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    let mut description = None;
    let mut technology = None;
    
    // Optional description
    if i < tokens.len() && matches!(&tokens[i], C4Token::Comma) {
        i += 1;
        if let C4Token::QuotedString(desc) = &tokens[i] {
            description = Some(desc.clone());
            i += 1;
            
            // Optional technology
            if i < tokens.len() && matches!(&tokens[i], C4Token::Comma) {
                i += 1;
                if let C4Token::QuotedString(tech) = &tokens[i] {
                    technology = Some(tech.clone());
                    i += 1;
                }
            }
        }
    }
    
    if !matches!(&tokens[i], C4Token::RightParen) {
        return None;
    }
    i += 1;
    
    Some((
        C4Element {
            id,
            element_type,
            name,
            description,
            technology,
            tags: Vec::new(),
            is_external,
        },
        i,
    ))
}

fn parse_boundary(
    tokens: &[C4Token],
    elements: &mut HashMap<String, C4Element>
) -> Option<(C4Boundary, usize)> {
    if tokens.len() < 6 {
        return None;
    }
    
    let mut i = 0;
    
    let boundary_type = match &tokens[i] {
        C4Token::SystemBoundary => C4BoundaryType::System,
        C4Token::ContainerBoundary => C4BoundaryType::Container,
        C4Token::EnterpriseBoundary => C4BoundaryType::Enterprise,
        C4Token::Boundary => C4BoundaryType::Generic,
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], C4Token::LeftParen) {
        return None;
    }
    i += 1;
    
    let id = match &tokens[i] {
        C4Token::Identifier(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], C4Token::Comma) {
        return None;
    }
    i += 1;
    
    let label = match &tokens[i] {
        C4Token::QuotedString(label) => label.clone(),
        _ => return None,
    };
    i += 1;
    
    // Optional tags parameter for generic Boundary
    let mut tags = Vec::new();
    if boundary_type == C4BoundaryType::Generic {
        if i < tokens.len() && matches!(&tokens[i], C4Token::Comma) {
            i += 1;
            if let C4Token::QuotedString(tag) = &tokens[i] {
                tags.push(tag.clone());
                i += 1;
            }
        }
    }
    
    if !matches!(&tokens[i], C4Token::RightParen) {
        return None;
    }
    i += 1;
    
    if !matches!(&tokens[i], C4Token::LeftBrace) {
        return None;
    }
    i += 1;
    
    // Parse boundary contents
    let mut boundary_elements = Vec::new();
    let mut nested_boundaries = Vec::new();
    
    while i < tokens.len() {
        match &tokens[i] {
            C4Token::RightBrace => {
                i += 1;
                break;
            }
            C4Token::Person | C4Token::PersonExt
            | C4Token::System | C4Token::SystemExt
            | C4Token::SystemDb | C4Token::SystemDbExt
            | C4Token::SystemQueue | C4Token::SystemQueueExt
            | C4Token::Container | C4Token::ContainerExt
            | C4Token::ContainerDb | C4Token::ContainerDbExt
            | C4Token::ContainerQueue | C4Token::ContainerQueueExt
            | C4Token::Component | C4Token::ComponentExt
            | C4Token::ComponentDb | C4Token::ComponentDbExt
            | C4Token::ComponentQueue | C4Token::ComponentQueueExt => {
                if let Some((elem, consumed)) = parse_element(&tokens[i..]) {
                    boundary_elements.push(elem.id.clone());
                    elements.insert(elem.id.clone(), elem);
                    i += consumed;
                } else {
                    i += 1;
                }
            }
            C4Token::SystemBoundary | C4Token::ContainerBoundary
            | C4Token::Boundary => {
                if let Some((nested, consumed)) = parse_boundary(&tokens[i..], elements) {
                    nested_boundaries.push(nested);
                    i += consumed;
                } else {
                    i += 1;
                }
            }
            C4Token::NewLine => {
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    
    Some((
        C4Boundary {
            id,
            boundary_type,
            label,
            tags,
            elements: boundary_elements,
            boundaries: nested_boundaries,
        },
        i,
    ))
}

fn parse_relationship(tokens: &[C4Token]) -> Option<(C4Relationship, usize)> {
    if tokens.len() < 6 {
        return None;
    }
    
    let mut i = 0;
    
    let (rel_type, direction) = match &tokens[i] {
        C4Token::Rel => (false, RelationshipDirection::Default),
        C4Token::BiRel => (true, RelationshipDirection::Default),
        C4Token::RelUp => (false, RelationshipDirection::Up),
        C4Token::RelDown => (false, RelationshipDirection::Down),
        C4Token::RelLeft => (false, RelationshipDirection::Left),
        C4Token::RelRight => (false, RelationshipDirection::Right),
        C4Token::RelBack => (false, RelationshipDirection::Back),
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], C4Token::LeftParen) {
        return None;
    }
    i += 1;
    
    let from = match &tokens[i] {
        C4Token::Identifier(id) | C4Token::Variable(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], C4Token::Comma) {
        return None;
    }
    i += 1;
    
    let to = match &tokens[i] {
        C4Token::Identifier(id) | C4Token::Variable(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    let mut label = None;
    let mut technology = None;
    
    // Optional label
    if i < tokens.len() && matches!(&tokens[i], C4Token::Comma) {
        i += 1;
        if let C4Token::QuotedString(l) = &tokens[i] {
            label = Some(l.clone());
            i += 1;
            
            // Optional technology
            if i < tokens.len() && matches!(&tokens[i], C4Token::Comma) {
                i += 1;
                if let C4Token::QuotedString(tech) = &tokens[i] {
                    technology = Some(tech.clone());
                    i += 1;
                }
            }
        }
    }
    
    if !matches!(&tokens[i], C4Token::RightParen) {
        return None;
    }
    i += 1;
    
    Some((
        C4Relationship {
            from,
            to,
            label,
            technology,
            direction,
            is_bidirectional: rel_type,
            tags: Vec::new(),
        },
        i,
    ))
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/c4/`
- Expected count: 50 files
- Copy to: `mermaid-parser/test/c4/`

### Command
```bash
cp -r ../mermaid-samples/c4/* ./test/c4/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_c4_files(#[files("test/c4/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = c4_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = c4_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.elements.is_empty() || !diagram.boundaries.is_empty(), 
            "Should have at least one element or boundary");
}

#[test]
fn test_c4_context_diagram() {
    let input = r#"C4Context
    title System Context diagram for Internet Banking System
    
    Person(customer, "Banking Customer", "A customer of the bank.")
    System(banking, "Internet Banking System", "Allows customers to manage accounts.")
    System_Ext(email, "E-mail System", "Microsoft Exchange")
    
    Rel(customer, banking, "Uses")
    Rel(banking, email, "Sends e-mails", "SMTP")
"#;
    
    let tokens = c4_lexer().parse(input.chars()).unwrap();
    let diagram = c4_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.diagram_type, C4DiagramType::Context);
    assert_eq!(diagram.title, Some("System Context diagram for Internet Banking System".to_string()));
    assert_eq!(diagram.elements.len(), 3);
    assert_eq!(diagram.relationships.len(), 2);
    
    let customer = &diagram.elements["customer"];
    assert_eq!(customer.element_type, C4ElementType::Person);
    assert!(!customer.is_external);
    
    let email = &diagram.elements["email"];
    assert!(email.is_external);
}

#[test]
fn test_boundaries() {
    let input = r#"C4Container
    Enterprise_Boundary(b1, "Company") {
        System_Boundary(b2, "Banking System") {
            Container(web, "Web Application", "React")
            Container(api, "API Application", "Java")
        }
    }
"#;
    
    let tokens = c4_lexer().parse(input.chars()).unwrap();
    let diagram = c4_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.boundaries.len(), 1);
    let enterprise = &diagram.boundaries[0];
    assert_eq!(enterprise.boundary_type, C4BoundaryType::Enterprise);
    assert_eq!(enterprise.boundaries.len(), 1);
    
    let system = &enterprise.boundaries[0];
    assert_eq!(system.boundary_type, C4BoundaryType::System);
    assert_eq!(system.elements.len(), 2);
}

#[test]
fn test_bidirectional_relationships() {
    let input = r#"C4Context
    Person(user, "User")
    System(system, "System")
    
    BiRel(user, system, "Interacts with")
"#;
    
    let tokens = c4_lexer().parse(input.chars()).unwrap();
    let diagram = c4_parser().parse(tokens).unwrap();
    
    let rel = &diagram.relationships[0];
    assert!(rel.is_bidirectional);
    assert_eq!(rel.label, Some("Interacts with".to_string()));
}

#[test]
fn test_directional_relationships() {
    let input = r#"C4Container
    Container(a, "Service A")
    Container(b, "Service B")
    Container(c, "Service C")
    Container(d, "Service D")
    
    Rel_Up(a, b, "calls")
    Rel_Down(b, c, "queries")
    Rel_Left(c, d, "notifies")
"#;
    
    let tokens = c4_lexer().parse(input.chars()).unwrap();
    let diagram = c4_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.relationships[0].direction, RelationshipDirection::Up);
    assert_eq!(diagram.relationships[1].direction, RelationshipDirection::Down);
    assert_eq!(diagram.relationships[2].direction, RelationshipDirection::Left);
}

#[test]
fn test_deployment_nodes() {
    let input = r#"C4Deployment
    DeploymentNode(aws, "AWS") {
        DeploymentNode(ecs, "ECS") {
            Container(app, "Application", "Docker")
        }
    }
"#;
    
    let tokens = c4_lexer().parse(input.chars()).unwrap();
    let diagram = c4_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.diagram_type, C4DiagramType::Deployment);
    assert!(diagram.elements.contains_key("aws"));
    assert!(diagram.elements.contains_key("app"));
}
```

## Success Criteria
1. ✅ Parse all 50 C4 diagram sample files successfully
2. ✅ Support all diagram types (Context, Container, Component, Dynamic, Deployment)
3. ✅ Handle all element types with _Ext variants
4. ✅ Parse nested boundaries correctly
5. ✅ Support all relationship types and directions
6. ✅ Handle optional parameters (description, technology)
7. ✅ Parse bidirectional relationships
8. ✅ Support title and other metadata
9. ✅ Handle multi-line descriptions with <br/>

## Implementation Priority
**Priority 21** - Implement in Phase 4 after class diagrams. C4 diagrams build upon the structural concepts from class diagrams and boundary concepts from earlier diagrams. The multi-context approach (Context/Container/Component) requires understanding of hierarchical modeling patterns established in previous implementations.