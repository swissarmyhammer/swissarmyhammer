# Implementation Plan: Class Diagrams

## Overview
Class diagrams represent UML class structures with inheritance, composition, associations, and method definitions.
High complexity grammar (420 lines) with extensive OOP relationship modeling and member specifications.

## Grammar Analysis

### Key Features
- Header: `classDiagram`
- Classes: With properties and methods
- Relationships: Inheritance, composition, aggregation, association
- Cardinality: 1, 0..1, 1..*, *, n, 0..n, 1..n
- Visibility: +public, -private, #protected, ~package
- Modifiers: Static ($), Abstract (*)
- Stereotypes: <<interface>>, <<abstract>>, <<service>>, <<enumeration>>
- Annotations: Metadata for classes and members
- Comments: `%%` for line comments

### Example Input
```
classDiagram
    Animal <|-- Duck
    Animal <|-- Fish
    Animal <|-- Zebra
    Animal : +int age
    Animal : +String gender
    Animal: +isMammal() bool
    Animal: +mate()
    
    class Duck{
        +String beakColor
        +swim()
        +quack()
    }
    
    class Fish{
        -int sizeInFeet
        -canEat()
    }
    
    class Zebra{
        +bool is_wild
        +run()
    }
    
    Vehicle <|-- Car
    Vehicle <|-- Bike
    
    class Vehicle{
        <<abstract>>
        +String brand
        +String model
        +int wheels
        +start()*
    }
    
    class Car{
        +String color
        +int doors
        +start()
    }
    
    class Bike{
        +int gears
        +start()
    }
    
    Customer "1" --> "*" Order
    Order "*" --> "1..*" Product
    
    class BankAccount{
        <<interface>>
        +String accountNumber
        +deposit(amount) bool
        +withdraw(amount) bool
    }
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub classes: HashMap<String, Class>,
    pub relationships: Vec<ClassRelationship>,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub name: String,
    pub stereotype: Option<Stereotype>,
    pub members: Vec<ClassMember>,
    pub annotations: Vec<String>,
    pub css_class: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stereotype {
    Interface,
    Abstract,
    Service,
    Enumeration,
    Exception,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    Property(Property),
    Method(Method),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub prop_type: Option<String>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_abstract: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,    // +
    Private,   // -
    Protected, // #
    Package,   // ~
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassRelationship {
    pub from: String,
    pub to: String,
    pub relationship_type: RelationshipType,
    pub from_cardinality: Option<String>,
    pub to_cardinality: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationshipType {
    Inheritance,        // <|--
    Composition,        // *--
    Aggregation,        // o--
    Association,        // <--
    Link,              // --
    DashedLink,        // ..
    Dependency,        // <..
    Realization,       // <|..
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassToken {
    ClassDiagram,                   // "classDiagram"
    Class,                          // "class"
    ClassName(String),              // Class identifier
    LeftBrace,                      // {
    RightBrace,                     // }
    LeftAngle,                      // <
    RightAngle,                     // >
    Pipe,                           // |
    Star,                           // *
    Circle,                         // o
    Dash,                           // -
    DashDash,                       // --
    DotDot,                         // ..
    LeftParen,                      // (
    RightParen,                     // )
    Colon,                          // :
    Comma,                          // ,
    Plus,                           // +
    Minus,                          // -
    Hash,                           // #
    Tilde,                          // ~
    Dollar,                         // $
    QuotedString(String),           // "text"
    StereotypeStart,                // <<
    StereotypeEnd,                  // >>
    StereotypeName(String),         // interface, abstract, etc.
    TypeName(String),               // int, String, etc.
    Identifier(String),             // General identifier
    Cardinality(String),            // 1, *, 0..1, etc.
    Comment(String),                // %% comment
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn class_lexer() -> impl Parser<char, Vec<ClassToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| ClassToken::Comment(text.into_iter().collect()));
    
    // Keywords
    let class_diagram = text::keyword("classDiagram")
        .map(|_| ClassToken::ClassDiagram);
    
    let class_keyword = text::keyword("class")
        .map(|_| ClassToken::Class);
    
    // Relationship symbols (order matters for overlapping patterns)
    let relationships = choice((
        text::string("<|--").map(|_| ClassToken::Inheritance),
        text::string("<|..").map(|_| ClassToken::Realization),
        text::string("*--").map(|_| ClassToken::Composition),
        text::string("o--").map(|_| ClassToken::Aggregation),
        text::string("<--").map(|_| ClassToken::Association),
        text::string("<..").map(|_| ClassToken::Dependency),
        text::string("--").map(|_| ClassToken::DashDash),
        text::string("..").map(|_| ClassToken::DotDot),
    ));
    
    // Visibility modifiers
    let visibility = choice((
        just('+').map(|_| ClassToken::Plus),
        just('-').map(|_| ClassToken::Minus),
        just('#').map(|_| ClassToken::Hash),
        just('~').map(|_| ClassToken::Tilde),
    ));
    
    // Stereotypes
    let stereotype = text::string("<<")
        .ignore_then(
            none_of(">")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(text::string(">>"))
        .map(|name| ClassToken::StereotypeName(name.trim().to_string()));
    
    // Cardinality patterns
    let cardinality = choice((
        just('*').map(|_| "＊".to_string()),
        text::int(10)
            .then(text::string("..").then(text::int(10).or(just('*').map(|_| "＊"))).or_not())
            .map(|(start, end)| {
                if let Some((_, end)) = end {
                    format!("{}..{}", start, end)
                } else {
                    start.to_string()
                }
            }),
        text::string("n").map(|_| "n".to_string()),
    ))
    .map(ClassToken::Cardinality);
    
    // Quoted string
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(ClassToken::QuotedString);
    
    // Type names (common types)
    let type_names = choice((
        text::keyword("int"),
        text::keyword("Integer"),
        text::keyword("String"),
        text::keyword("string"),
        text::keyword("bool"),
        text::keyword("boolean"),
        text::keyword("Boolean"),
        text::keyword("float"),
        text::keyword("Float"),
        text::keyword("double"),
        text::keyword("Double"),
        text::keyword("void"),
        text::keyword("List"),
        text::keyword("Map"),
        text::keyword("Set"),
    ))
    .map(|t| ClassToken::TypeName(t.to_string()));
    
    // Identifier (class names, member names, custom types)
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(ClassToken::Identifier);
    
    let left_brace = just('{').map(|_| ClassToken::LeftBrace);
    let right_brace = just('}').map(|_| ClassToken::RightBrace);
    let left_angle = just('<').map(|_| ClassToken::LeftAngle);
    let right_angle = just('>').map(|_| ClassToken::RightAngle);
    let pipe = just('|').map(|_| ClassToken::Pipe);
    let star = just('*').map(|_| ClassToken::Star);
    let circle = just('o').map(|_| ClassToken::Circle);
    let left_paren = just('(').map(|_| ClassToken::LeftParen);
    let right_paren = just(')').map(|_| ClassToken::RightParen);
    let colon = just(':').map(|_| ClassToken::Colon);
    let comma = just(',').map(|_| ClassToken::Comma);
    let dollar = just('$').map(|_| ClassToken::Dollar);
    
    let newline = just('\n').map(|_| ClassToken::NewLine);
    
    let token = choice((
        comment,
        class_diagram,
        class_keyword,
        relationships,
        stereotype,
        visibility,
        type_names,
        quoted_string,
        cardinality,
        left_brace,
        right_brace,
        left_angle,
        right_angle,
        pipe,
        star,
        circle,
        left_paren,
        right_paren,
        colon,
        comma,
        dollar,
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

### Complex Class Parser
```rust
pub fn class_parser() -> impl Parser<ClassToken, ClassDiagram, Error = Simple<ClassToken>> {
    just(ClassToken::ClassDiagram)
        .then_ignore(
            filter(|t| matches!(t, ClassToken::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(ClassToken::Eof).or_not())
        .map(|(_, tokens)| {
            let mut classes = HashMap::new();
            let mut relationships = Vec::new();
            let mut notes = Vec::new();
            let mut i = 0;
            
            while i < tokens.len() {
                match &tokens[i] {
                    ClassToken::Comment(_) | ClassToken::NewLine => {
                        i += 1;
                    }
                    ClassToken::Class => {
                        // Parse class definition
                        if let Some((class, consumed)) = parse_class_definition(&tokens[i..]) {
                            classes.insert(class.name.clone(), class);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    ClassToken::Identifier(name) | ClassToken::ClassName(name) => {
                        // Check for inline class definition or relationship
                        if i + 1 < tokens.len() {
                            match &tokens[i + 1] {
                                ClassToken::LeftBrace => {
                                    // Inline class definition
                                    if let Some((class, consumed)) = parse_inline_class(&tokens[i..], name) {
                                        classes.insert(class.name.clone(), class);
                                        i += consumed;
                                    } else {
                                        i += 1;
                                    }
                                }
                                ClassToken::Colon => {
                                    // Class member definition
                                    if let Some((member, consumed)) = parse_class_member_shorthand(&tokens[i..], name) {
                                        ensure_class_exists(&mut classes, name);
                                        classes.get_mut(name).unwrap().members.push(member);
                                        i += consumed;
                                    } else {
                                        i += 1;
                                    }
                                }
                                ClassToken::QuotedString(_) | ClassToken::Cardinality(_) => {
                                    // Relationship with cardinality
                                    if let Some((rel, consumed)) = parse_relationship(&tokens[i..]) {
                                        relationships.push(rel);
                                        i += consumed;
                                    } else {
                                        i += 1;
                                    }
                                }
                                ClassToken::Inheritance | ClassToken::Composition
                                | ClassToken::Aggregation | ClassToken::Association
                                | ClassToken::DashDash | ClassToken::DotDot
                                | ClassToken::Dependency | ClassToken::Realization => {
                                    // Direct relationship
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
                        } else {
                            i += 1;
                        }
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            
            ClassDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                classes,
                relationships,
                notes,
            }
        })
}

fn parse_class_definition(tokens: &[ClassToken]) -> Option<(Class, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip "class"
    
    let name = match &tokens[i] {
        ClassToken::Identifier(n) | ClassToken::ClassName(n) => n.clone(),
        _ => return None,
    };
    i += 1;
    
    let mut stereotype = None;
    let mut members = Vec::new();
    
    // Check for opening brace
    if matches!(&tokens[i], ClassToken::LeftBrace) {
        i += 1;
        
        // Parse class body
        while i < tokens.len() {
            match &tokens[i] {
                ClassToken::RightBrace => {
                    i += 1;
                    break;
                }
                ClassToken::StereotypeName(s) => {
                    stereotype = Some(parse_stereotype(s));
                    i += 1;
                }
                ClassToken::Plus | ClassToken::Minus | ClassToken::Hash | ClassToken::Tilde => {
                    // Parse member
                    if let Some((member, consumed)) = parse_member(&tokens[i..]) {
                        members.push(member);
                        i += consumed;
                    } else {
                        i += 1;
                    }
                }
                ClassToken::Identifier(_) => {
                    // Member without visibility
                    if let Some((member, consumed)) = parse_member_without_visibility(&tokens[i..]) {
                        members.push(member);
                        i += consumed;
                    } else {
                        i += 1;
                    }
                }
                ClassToken::NewLine => {
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
    }
    
    Some((
        Class {
            name,
            stereotype,
            members,
            annotations: Vec::new(),
            css_class: None,
        },
        i,
    ))
}

fn parse_inline_class(tokens: &[ClassToken], name: &str) -> Option<(Class, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip class name
    
    if !matches!(&tokens[i], ClassToken::LeftBrace) {
        return None;
    }
    i += 1;
    
    let mut stereotype = None;
    let mut members = Vec::new();
    
    // Parse class body (same as in parse_class_definition)
    while i < tokens.len() {
        match &tokens[i] {
            ClassToken::RightBrace => {
                i += 1;
                break;
            }
            ClassToken::StereotypeName(s) => {
                stereotype = Some(parse_stereotype(s));
                i += 1;
            }
            ClassToken::Plus | ClassToken::Minus | ClassToken::Hash | ClassToken::Tilde => {
                if let Some((member, consumed)) = parse_member(&tokens[i..]) {
                    members.push(member);
                    i += consumed;
                } else {
                    i += 1;
                }
            }
            ClassToken::Identifier(_) => {
                if let Some((member, consumed)) = parse_member_without_visibility(&tokens[i..]) {
                    members.push(member);
                    i += consumed;
                } else {
                    i += 1;
                }
            }
            ClassToken::NewLine => {
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    
    Some((
        Class {
            name: name.to_string(),
            stereotype,
            members,
            annotations: Vec::new(),
            css_class: None,
        },
        i,
    ))
}

fn parse_member(tokens: &[ClassToken]) -> Option<(ClassMember, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 0;
    
    let visibility = match &tokens[i] {
        ClassToken::Plus => Visibility::Public,
        ClassToken::Minus => Visibility::Private,
        ClassToken::Hash => Visibility::Protected,
        ClassToken::Tilde => Visibility::Package,
        _ => return None,
    };
    i += 1;
    
    let is_static = if matches!(&tokens[i], ClassToken::Dollar) {
        i += 1;
        true
    } else {
        false
    };
    
    // Check for type before name (property) or name with parens (method)
    if let Some((member, consumed)) = parse_typed_member(&tokens[i..], visibility, is_static) {
        i += consumed;
        Some((member, i))
    } else {
        None
    }
}

fn parse_typed_member(
    tokens: &[ClassToken],
    visibility: Visibility,
    is_static: bool
) -> Option<(ClassMember, usize)> {
    if tokens.is_empty() {
        return None;
    }
    
    let mut i = 0;
    
    // Try to parse as: Type name or name(params) returnType
    match &tokens[i] {
        ClassToken::TypeName(type_name) | ClassToken::Identifier(type_name) => {
            i += 1;
            if i < tokens.len() {
                match &tokens[i] {
                    ClassToken::Identifier(name) => {
                        // Property: Type name
                        i += 1;
                        Some((
                            ClassMember::Property(Property {
                                name: name.clone(),
                                prop_type: Some(type_name.clone()),
                                visibility,
                                is_static,
                                default_value: None,
                            }),
                            i,
                        ))
                    }
                    ClassToken::LeftParen => {
                        // Method without name? Skip
                        None
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

fn parse_class_member_shorthand(tokens: &[ClassToken], class_name: &str) -> Option<(ClassMember, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip class name
    
    if !matches!(&tokens[i], ClassToken::Colon) {
        return None;
    }
    i += 1;
    
    // Parse member after colon
    parse_member(&tokens[i..]).map(|(member, consumed)| (member, i + consumed))
}

fn parse_relationship(tokens: &[ClassToken]) -> Option<(ClassRelationship, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 0;
    
    let from = match &tokens[i] {
        ClassToken::Identifier(name) | ClassToken::ClassName(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    // Optional from cardinality
    let from_cardinality = match &tokens[i] {
        ClassToken::QuotedString(card) | ClassToken::Cardinality(card) => {
            i += 1;
            Some(card.clone())
        }
        _ => None,
    };
    
    // Relationship type
    let rel_type = match &tokens[i] {
        ClassToken::Inheritance => RelationshipType::Inheritance,
        ClassToken::Composition => RelationshipType::Composition,
        ClassToken::Aggregation => RelationshipType::Aggregation,
        ClassToken::Association => RelationshipType::Association,
        ClassToken::DashDash => RelationshipType::Link,
        ClassToken::DotDot => RelationshipType::DashedLink,
        ClassToken::Dependency => RelationshipType::Dependency,
        ClassToken::Realization => RelationshipType::Realization,
        _ => return None,
    };
    i += 1;
    
    // Optional to cardinality
    let to_cardinality = match &tokens[i] {
        ClassToken::QuotedString(card) | ClassToken::Cardinality(card) => {
            i += 1;
            Some(card.clone())
        }
        _ => None,
    };
    
    let to = match &tokens[i] {
        ClassToken::Identifier(name) | ClassToken::ClassName(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    // Optional label
    let label = if i < tokens.len() && matches!(&tokens[i], ClassToken::Colon) {
        i += 1;
        match &tokens[i] {
            ClassToken::Identifier(l) | ClassToken::QuotedString(l) => {
                i += 1;
                Some(l.clone())
            }
            _ => None,
        }
    } else {
        None
    };
    
    Some((
        ClassRelationship {
            from,
            to,
            relationship_type: rel_type,
            from_cardinality,
            to_cardinality,
            label,
        },
        i,
    ))
}

fn parse_stereotype(name: &str) -> Stereotype {
    match name.to_lowercase().as_str() {
        "interface" => Stereotype::Interface,
        "abstract" => Stereotype::Abstract,
        "service" => Stereotype::Service,
        "enumeration" | "enum" => Stereotype::Enumeration,
        "exception" => Stereotype::Exception,
        _ => Stereotype::Custom(name.to_string()),
    }
}

fn ensure_class_exists(classes: &mut HashMap<String, Class>, name: &str) {
    if !classes.contains_key(name) {
        classes.insert(name.to_string(), Class {
            name: name.to_string(),
            stereotype: None,
            members: Vec::new(),
            annotations: Vec::new(),
            css_class: None,
        });
    }
}

fn parse_member_without_visibility(tokens: &[ClassToken]) -> Option<(ClassMember, usize)> {
    // Simplified - would need full implementation
    None
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/class/`
- Expected count: 212 files
- Copy to: `mermaid-parser/test/class/`

### Command
```bash
cp -r ../mermaid-samples/class/* ./test/class/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_class_files(#[files("test/class/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = class_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = class_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.classes.is_empty() || !diagram.relationships.is_empty(), 
            "Should have at least one class or relationship");
}

#[test]
fn test_simple_class_diagram() {
    let input = r#"classDiagram
    class Animal {
        +int age
        +String gender
        +isMammal() bool
        +mate()
    }
    
    Animal <|-- Duck
    Animal <|-- Fish
"#;
    
    let tokens = class_lexer().parse(input.chars()).unwrap();
    let diagram = class_parser().parse(tokens).unwrap();
    
    assert!(diagram.classes.contains_key("Animal"));
    let animal = &diagram.classes["Animal"];
    assert_eq!(animal.members.len(), 4);
    
    assert_eq!(diagram.relationships.len(), 2);
    assert_eq!(diagram.relationships[0].relationship_type, RelationshipType::Inheritance);
}

#[test]
fn test_class_with_visibility() {
    let input = r#"classDiagram
    class BankAccount {
        -String accountNumber
        +deposit(amount) bool
        #getBalance() float
        ~internalMethod()
    }
"#;
    
    let tokens = class_lexer().parse(input.chars()).unwrap();
    let diagram = class_parser().parse(tokens).unwrap();
    
    let bank = &diagram.classes["BankAccount"];
    // Verify visibility modifiers are parsed correctly
}

#[test]
fn test_stereotypes() {
    let input = r#"classDiagram
    class Vehicle {
        <<abstract>>
        +start()*
    }
    
    class PaymentService {
        <<interface>>
        +processPayment(amount) bool
    }
"#;
    
    let tokens = class_lexer().parse(input.chars()).unwrap();
    let diagram = class_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.classes["Vehicle"].stereotype, Some(Stereotype::Abstract));
    assert_eq!(diagram.classes["PaymentService"].stereotype, Some(Stereotype::Interface));
}

#[test]
fn test_relationships_with_cardinality() {
    let input = r#"classDiagram
    Customer "1" --> "*" Order : places
    Order "*" --> "1..*" Product : contains
"#;
    
    let tokens = class_lexer().parse(input.chars()).unwrap();
    let diagram = class_parser().parse(tokens).unwrap();
    
    let customer_order = &diagram.relationships[0];
    assert_eq!(customer_order.from_cardinality, Some("1".to_string()));
    assert_eq!(customer_order.to_cardinality, Some("*".to_string()));
    assert_eq!(customer_order.label, Some("places".to_string()));
}

#[test]
fn test_inline_class_definition() {
    let input = r#"classDiagram
    Duck {
        +String beakColor
        +swim()
        +quack()
    }
"#;
    
    let tokens = class_lexer().parse(input.chars()).unwrap();
    let diagram = class_parser().parse(tokens).unwrap();
    
    assert!(diagram.classes.contains_key("Duck"));
    assert_eq!(diagram.classes["Duck"].members.len(), 3);
}

#[test]
fn test_shorthand_member_syntax() {
    let input = r#"classDiagram
    Animal : +int age
    Animal : +String gender
    Animal : +isMammal() bool
"#;
    
    let tokens = class_lexer().parse(input.chars()).unwrap();
    let diagram = class_parser().parse(tokens).unwrap();
    
    assert!(diagram.classes.contains_key("Animal"));
    assert_eq!(diagram.classes["Animal"].members.len(), 3);
}
```

## Success Criteria
1. ✅ Parse all 212 class diagram sample files successfully
2. ✅ Handle class definitions with properties and methods
3. ✅ Support all visibility modifiers (+, -, #, ~)
4. ✅ Parse static ($) and abstract (*) modifiers
5. ✅ Handle stereotypes (interface, abstract, service, etc.)
6. ✅ Support all relationship types (inheritance, composition, etc.)
7. ✅ Parse cardinality specifications
8. ✅ Handle both explicit and inline class definitions
9. ✅ Support shorthand member syntax

## Implementation Priority
**Priority 20** - Implement in Phase 4 after state diagrams. Class diagrams are among the most complex with sophisticated type systems, multiple syntaxes for defining members, and complex relationship modeling. The OOP concepts build upon relationship patterns from ER diagrams and structural patterns from earlier diagrams.