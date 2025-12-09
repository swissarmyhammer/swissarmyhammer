# Implementation Plan: ER (Entity Relationship) Diagrams

## Overview
ER diagrams represent database schemas with entities, attributes, and relationships including cardinality.
Medium-high complexity grammar (293 lines) with entity definitions, relationships, and attribute specifications.

## Grammar Analysis

### Key Features
- Header: `erDiagram`
- Entities: Named entities with attributes
- Attributes: Type specifications, primary/foreign keys, comments
- Relationships: Various cardinality types (||--||, ||--o{, etc.)
- Relationship labels: Descriptive text for relationships
- Comments: `%%` for line comments

### Example Input
```
erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER {
        string name PK
        string customerId PK
        string address
        string phoneNumber
    }
    ORDER ||--|{ LINE-ITEM : contains
    ORDER {
        int orderId PK
        string customerId FK
        date orderDate
        string status
    }
    LINE-ITEM {
        string productId FK
        int quantity
        float pricePerUnit
    }
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ERDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub entities: HashMap<String, Entity>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub name: String,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub attr_type: String,
    pub key_type: Option<KeyType>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyType {
    PK,  // Primary Key
    FK,  // Foreign Key
    UK,  // Unique Key
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub left_entity: String,
    pub right_entity: String,
    pub left_cardinality: Cardinality,
    pub right_cardinality: Cardinality,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cardinality {
    pub min: CardinalityValue,
    pub max: CardinalityValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CardinalityValue {
    Zero,
    One,
    Many,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ERToken {
    ERDiagram,                // "erDiagram"
    EntityName(String),       // Entity identifier
    RelSymbol(String),        // ||--||, ||--o{, etc.
    Label(String),            // : "relationship label"
    LeftBrace,                // {
    RightBrace,               // }
    AttributeType(String),    // string, int, float, etc.
    AttributeName(String),    // Attribute identifier
    KeyType(KeyType),         // PK, FK, UK
    QuotedString(String),     // "comment"
    Comment(String),          // %% comment
    Colon,                    // :
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn er_lexer() -> impl Parser<char, Vec<ERToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| ERToken::Comment(text.into_iter().collect()));
    
    let er_keyword = text::keyword("erDiagram")
        .map(|_| ERToken::ERDiagram);
    
    // Relationship symbols
    let rel_symbols = choice((
        text::string("}o--o{").map(|_| "many-to-many"),
        text::string("}o--||").map(|_| "many-to-one"),
        text::string("||--o{").map(|_| "one-to-many"),
        text::string("||--||").map(|_| "one-to-one"),
        text::string("}o..o{").map(|_| "many-to-many-optional"),
        text::string("}o..||").map(|_| "many-to-one-optional"),
        text::string("||..o{").map(|_| "one-to-many-optional"),
        text::string("||..||").map(|_| "one-to-one-optional"),
        text::string("}|--||").map(|_| "one-or-more-to-one"),
        text::string("||--|{").map(|_| "one-to-one-or-more"),
        text::string("}|--|{").map(|_| "one-or-more-to-one-or-more"),
        // Add more relationship patterns as needed
    ))
    .map(|s| ERToken::RelSymbol(s.to_string()));
    
    // Key types
    let key_type = choice((
        text::keyword("PK").map(|_| ERToken::KeyType(KeyType::PK)),
        text::keyword("FK").map(|_| ERToken::KeyType(KeyType::FK)),
        text::keyword("UK").map(|_| ERToken::KeyType(KeyType::UK)),
    ));
    
    // Attribute types
    let attr_types = choice((
        text::keyword("string"),
        text::keyword("int"),
        text::keyword("integer"),
        text::keyword("float"),
        text::keyword("double"),
        text::keyword("decimal"),
        text::keyword("boolean"),
        text::keyword("bool"),
        text::keyword("date"),
        text::keyword("datetime"),
        text::keyword("timestamp"),
        text::keyword("time"),
        text::keyword("blob"),
        text::keyword("text"),
        text::keyword("varchar"),
        text::keyword("char"),
    ))
    .map(|t| ERToken::AttributeType(t.to_string()));
    
    // Quoted string
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(ERToken::QuotedString);
    
    // Entity/Attribute names
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_' || *c == '-'
    })
    .repeated()
    .at_least(1)
    .collect::<String>();
    
    let left_brace = just('{').map(|_| ERToken::LeftBrace);
    let right_brace = just('}').map(|_| ERToken::RightBrace);
    let colon = just(':').map(|_| ERToken::Colon);
    let newline = just('\n').map(|_| ERToken::NewLine);
    
    // Context-sensitive parsing
    let token = choice((
        comment,
        er_keyword,
        key_type,
        attr_types,
        rel_symbols,
        left_brace,
        right_brace,
        colon,
        quoted_string,
        identifier.map(|name| {
            // This is simplified - in practice, we'd need context
            // to distinguish between entity names and attribute names
            ERToken::EntityName(name)
        }),
    ));
    
    whitespace
        .ignore_then(token)
        .or(newline)
        .repeated()
        .then_ignore(end())
}
```

## Step 3: Parser Implementation

### State-Based Parser
```rust
pub fn er_parser() -> impl Parser<ERToken, ERDiagram, Error = Simple<ERToken>> {
    enum ParseContext {
        TopLevel,
        InEntity(String),
    }
    
    just(ERToken::ERDiagram)
        .then_ignore(
            filter(|t| matches!(t, ERToken::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(ERToken::Eof).or_not())
        .map(|(_, tokens)| {
            let mut entities = HashMap::new();
            let mut relationships = Vec::new();
            let mut context = ParseContext::TopLevel;
            let mut current_attributes = Vec::new();
            let mut i = 0;
            
            while i < tokens.len() {
                match (&context, &tokens[i]) {
                    (_, ERToken::Comment(_)) => {
                        i += 1;
                    }
                    (_, ERToken::NewLine) => {
                        i += 1;
                    }
                    (ParseContext::TopLevel, ERToken::EntityName(name)) => {
                        // Check if this is a relationship or entity definition
                        if i + 1 < tokens.len() {
                            match &tokens[i + 1] {
                                ERToken::RelSymbol(symbol) => {
                                    // Parse relationship
                                    let relationship = parse_relationship(&tokens[i..]);
                                    if let Some((rel, consumed)) = relationship {
                                        relationships.push(rel);
                                        i += consumed;
                                    } else {
                                        i += 1;
                                    }
                                }
                                ERToken::LeftBrace => {
                                    // Start entity definition
                                    context = ParseContext::InEntity(name.clone());
                                    current_attributes.clear();
                                    i += 2; // Skip name and brace
                                }
                                _ => {
                                    i += 1;
                                }
                            }
                        } else {
                            i += 1;
                        }
                    }
                    (ParseContext::InEntity(entity_name), ERToken::RightBrace) => {
                        // End entity definition
                        entities.insert(
                            entity_name.clone(),
                            Entity {
                                name: entity_name.clone(),
                                attributes: current_attributes.clone(),
                            }
                        );
                        context = ParseContext::TopLevel;
                        i += 1;
                    }
                    (ParseContext::InEntity(_), ERToken::AttributeType(attr_type)) => {
                        // Parse attribute
                        if let Some((attr, consumed)) = parse_attribute(&tokens[i..], attr_type) {
                            current_attributes.push(attr);
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
            
            ERDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                entities,
                relationships,
            }
        })
}

fn parse_relationship(tokens: &[ERToken]) -> Option<(Relationship, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 0;
    let left_entity = match &tokens[i] {
        ERToken::EntityName(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    let (left_card, right_card) = match &tokens[i] {
        ERToken::RelSymbol(symbol) => parse_cardinality(symbol),
        _ => return None,
    };
    i += 1;
    
    let right_entity = match &tokens[i] {
        ERToken::EntityName(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    let label = if i < tokens.len() && matches!(&tokens[i], ERToken::Colon) {
        i += 1;
        match &tokens[i] {
            ERToken::EntityName(text) | ERToken::QuotedString(text) => {
                i += 1;
                Some(text.clone())
            }
            _ => None,
        }
    } else {
        None
    };
    
    Some((
        Relationship {
            left_entity,
            right_entity,
            left_cardinality: left_card,
            right_cardinality: right_card,
            label,
        },
        i,
    ))
}

fn parse_attribute(tokens: &[ERToken], attr_type: &str) -> Option<(Attribute, usize)> {
    if tokens.len() < 2 {
        return None;
    }
    
    let mut i = 1;
    let name = match &tokens[i] {
        ERToken::EntityName(name) | ERToken::AttributeName(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    let mut key_type = None;
    let mut comment = None;
    
    while i < tokens.len() {
        match &tokens[i] {
            ERToken::KeyType(kt) => {
                key_type = Some(kt.clone());
                i += 1;
            }
            ERToken::QuotedString(text) => {
                comment = Some(text.clone());
                i += 1;
            }
            ERToken::NewLine => {
                break;
            }
            _ => {
                i += 1;
            }
        }
    }
    
    Some((
        Attribute {
            name,
            attr_type: attr_type.to_string(),
            key_type,
            comment,
        },
        i,
    ))
}

fn parse_cardinality(symbol: &str) -> (Cardinality, Cardinality) {
    match symbol {
        "one-to-one" | "||--||" => (
            Cardinality { min: CardinalityValue::One, max: CardinalityValue::One },
            Cardinality { min: CardinalityValue::One, max: CardinalityValue::One },
        ),
        "one-to-many" | "||--o{" => (
            Cardinality { min: CardinalityValue::One, max: CardinalityValue::One },
            Cardinality { min: CardinalityValue::Zero, max: CardinalityValue::Many },
        ),
        "many-to-one" | "}o--||" => (
            Cardinality { min: CardinalityValue::Zero, max: CardinalityValue::Many },
            Cardinality { min: CardinalityValue::One, max: CardinalityValue::One },
        ),
        "many-to-many" | "}o--o{" => (
            Cardinality { min: CardinalityValue::Zero, max: CardinalityValue::Many },
            Cardinality { min: CardinalityValue::Zero, max: CardinalityValue::Many },
        ),
        "one-to-one-or-more" | "||--|{" => (
            Cardinality { min: CardinalityValue::One, max: CardinalityValue::One },
            Cardinality { min: CardinalityValue::One, max: CardinalityValue::Many },
        ),
        _ => (
            Cardinality { min: CardinalityValue::Zero, max: CardinalityValue::Many },
            Cardinality { min: CardinalityValue::Zero, max: CardinalityValue::Many },
        ),
    }
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/er/`
- Expected count: 113 files
- Copy to: `mermaid-parser/test/er/`

### Command
```bash
cp -r ../mermaid-samples/er/* ./test/er/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_er_files(#[files("test/er/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = er_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = er_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.entities.is_empty() || !diagram.relationships.is_empty(), 
            "Should have at least one entity or relationship");
}

#[test]
fn test_simple_er_diagram() {
    let input = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER {
        string name PK
        int customerId
    }
    ORDER {
        int orderId PK
        int customerId FK
    }
"#;
    
    let tokens = er_lexer().parse(input.chars()).unwrap();
    let diagram = er_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.entities.len(), 2);
    assert_eq!(diagram.relationships.len(), 1);
    
    let customer = &diagram.entities["CUSTOMER"];
    assert_eq!(customer.attributes.len(), 2);
    assert_eq!(customer.attributes[0].name, "name");
    assert_eq!(customer.attributes[0].key_type, Some(KeyType::PK));
    
    let rel = &diagram.relationships[0];
    assert_eq!(rel.left_entity, "CUSTOMER");
    assert_eq!(rel.right_entity, "ORDER");
    assert_eq!(rel.label, Some("places".to_string()));
}

#[test]
fn test_attribute_types() {
    let input = r#"erDiagram
    PRODUCT {
        string productId PK
        string name
        float price
        boolean inStock
        date lastUpdated
        text description "Product description"
    }
"#;
    
    let tokens = er_lexer().parse(input.chars()).unwrap();
    let diagram = er_parser().parse(tokens).unwrap();
    
    let product = &diagram.entities["PRODUCT"];
    assert_eq!(product.attributes.len(), 6);
    
    let price_attr = product.attributes.iter()
        .find(|a| a.name == "price")
        .unwrap();
    assert_eq!(price_attr.attr_type, "float");
    
    let desc_attr = product.attributes.iter()
        .find(|a| a.name == "description")
        .unwrap();
    assert_eq!(desc_attr.comment, Some("Product description".to_string()));
}

#[test]
fn test_multiple_relationships() {
    let input = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
    PRODUCT ||--o{ LINE-ITEM : "ordered in"
"#;
    
    let tokens = er_lexer().parse(input.chars()).unwrap();
    let diagram = er_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.relationships.len(), 3);
    
    let product_rel = diagram.relationships.iter()
        .find(|r| r.left_entity == "PRODUCT")
        .unwrap();
    assert_eq!(product_rel.label, Some("ordered in".to_string()));
}

#[test]
fn test_cardinality_types() {
    let input = r#"erDiagram
    A ||--|| B : "one-to-one"
    C ||--o{ D : "one-to-many"
    E }o--|| F : "many-to-one"
    G }o--o{ H : "many-to-many"
"#;
    
    let tokens = er_lexer().parse(input.chars()).unwrap();
    let diagram = er_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.relationships.len(), 4);
    // Verify cardinalities are parsed correctly
}
```

## Success Criteria
1. ✅ Parse all 113 ER diagram sample files successfully
2. ✅ Handle entity definitions with attributes
3. ✅ Support all attribute types (string, int, float, date, etc.)
4. ✅ Parse key type specifications (PK, FK, UK)
5. ✅ Handle relationship definitions with cardinality
6. ✅ Support relationship labels
7. ✅ Parse attribute comments
8. ✅ Handle all cardinality patterns

## Implementation Priority
**Priority 14** - Implement in Phase 3 after block diagrams. ER diagrams introduce relationship modeling concepts that are foundational for class and C4 diagrams. The cardinality patterns and entity-attribute structures provide important patterns for more complex relationship diagrams.