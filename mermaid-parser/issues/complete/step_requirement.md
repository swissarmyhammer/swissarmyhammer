# Implementation Plan: Requirement Diagrams

## Overview
Requirement diagrams represent requirements engineering with traceability, verification methods, and risk levels.
Medium complexity grammar (267 lines) with requirements, elements, and various relationship types.

## Grammar Analysis

### Key Features
- Header: `requirementDiagram`
- Requirements: Functional, performance, interface, design requirements
- Elements: Various element types with docRef
- Relationships: Contains, copies, derives, satisfies, verifies, refines, traces
- Risk levels: Low, Medium, High
- Verification methods: Analysis, Inspection, Test, Demonstration

### Example Input
```
requirementDiagram

requirement test_req {
    id: 1
    text: the test text.
    risk: high
    verifymethod: test
}

functionalRequirement test_req2 {
    id: 1.1
    text: the second test text.
    risk: low
    verifymethod: inspection
}

performanceRequirement test_req3 {
    id: 1.2
    text: the third test text.
    risk: medium
    verifymethod: demonstration
}

element test_entity {
    type: simulation
}

element test_entity2 {
    type: word doc
    docRef: reqs/test_entity
}

test_entity - satisfies -> test_req2
test_req - traces -> test_req2
test_req - contains -> test_req3
test_req <- copies - test_entity2
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RequirementDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub requirements: HashMap<String, Requirement>,
    pub elements: HashMap<String, Element>,
    pub relationships: Vec<RequirementRelationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Requirement {
    pub name: String,
    pub req_type: RequirementType,
    pub id: String,
    pub text: String,
    pub risk: Option<RiskLevel>,
    pub verify_method: Option<VerificationMethod>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequirementType {
    Requirement,
    FunctionalRequirement,
    PerformanceRequirement,
    InterfaceRequirement,
    DesignConstraint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VerificationMethod {
    Analysis,
    Inspection,
    Test,
    Demonstration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    pub element_type: String,
    pub doc_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequirementRelationship {
    pub source: String,
    pub target: String,
    pub relationship_type: RelationshipType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationshipType {
    Contains,
    Copies,
    Derives,
    Satisfies,
    Verifies,
    Refines,
    Traces,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequirementToken {
    RequirementDiagram,           // "requirementDiagram"
    Requirement,                  // "requirement"
    FunctionalRequirement,        // "functionalRequirement"
    PerformanceRequirement,       // "performanceRequirement"
    InterfaceRequirement,         // "interfaceRequirement"
    DesignConstraint,             // "designConstraint"
    Element,                      // "element"
    Id,                          // "id:"
    Text,                        // "text:"
    Risk,                        // "risk:"
    VerifyMethod,                // "verifymethod:"
    Type,                        // "type:"
    DocRef,                      // "docRef:"
    LeftBrace,                   // {
    RightBrace,                  // }
    Arrow,                       // ->
    BackArrow,                   // <-
    Dash,                        // -
    Identifier(String),          // Names and values
    QuotedString(String),        // Quoted text
    RelationshipType(String),    // satisfies, traces, etc.
    Comment(String),             // %% comment
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn requirement_lexer() -> impl Parser<char, Vec<RequirementToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| RequirementToken::Comment(text.into_iter().collect()));
    
    // Keywords
    let requirement_diagram = text::keyword("requirementDiagram")
        .map(|_| RequirementToken::RequirementDiagram);
    
    let requirement_types = choice((
        text::keyword("functionalRequirement")
            .map(|_| RequirementToken::FunctionalRequirement),
        text::keyword("performanceRequirement")
            .map(|_| RequirementToken::PerformanceRequirement),
        text::keyword("interfaceRequirement")
            .map(|_| RequirementToken::InterfaceRequirement),
        text::keyword("designConstraint")
            .map(|_| RequirementToken::DesignConstraint),
        text::keyword("requirement")
            .map(|_| RequirementToken::Requirement),
    ));
    
    let element_keyword = text::keyword("element")
        .map(|_| RequirementToken::Element);
    
    // Property keywords
    let properties = choice((
        text::keyword("id:").map(|_| RequirementToken::Id),
        text::keyword("text:").map(|_| RequirementToken::Text),
        text::keyword("risk:").map(|_| RequirementToken::Risk),
        text::keyword("verifymethod:").map(|_| RequirementToken::VerifyMethod),
        text::keyword("type:").map(|_| RequirementToken::Type),
        text::keyword("docRef:").map(|_| RequirementToken::DocRef),
    ));
    
    // Relationship types
    let relationship_types = choice((
        text::keyword("contains"),
        text::keyword("copies"),
        text::keyword("derives"),
        text::keyword("satisfies"),
        text::keyword("verifies"),
        text::keyword("refines"),
        text::keyword("traces"),
    ))
    .map(|rel| RequirementToken::RelationshipType(rel.to_string()));
    
    // Arrows and symbols
    let arrow = text::string("->").map(|_| RequirementToken::Arrow);
    let back_arrow = text::string("<-").map(|_| RequirementToken::BackArrow);
    let dash = just('-').map(|_| RequirementToken::Dash);
    
    let left_brace = just('{').map(|_| RequirementToken::LeftBrace);
    let right_brace = just('}').map(|_| RequirementToken::RightBrace);
    
    // Quoted string (can span multiple lines in requirements)
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(RequirementToken::QuotedString);
    
    // Identifier (alphanumeric with underscores, dots, slashes for paths)
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_' || *c == '.' || *c == '/' || *c == ' '
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(|s| RequirementToken::Identifier(s.trim().to_string()));
    
    let newline = just('\n').map(|_| RequirementToken::NewLine);
    
    let token = choice((
        comment,
        requirement_diagram,
        requirement_types,
        element_keyword,
        properties,
        relationship_types,
        arrow,
        back_arrow,
        dash,
        left_brace,
        right_brace,
        quoted_string,
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

### Structured Parser
```rust
pub fn requirement_parser() -> impl Parser<RequirementToken, RequirementDiagram, Error = Simple<RequirementToken>> {
    just(RequirementToken::RequirementDiagram)
        .then_ignore(
            filter(|t| matches!(t, RequirementToken::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(RequirementToken::Eof).or_not())
        .map(|(_, tokens)| {
            let mut requirements = HashMap::new();
            let mut elements = HashMap::new();
            let mut relationships = Vec::new();
            let mut i = 0;
            
            while i < tokens.len() {
                match &tokens[i] {
                    RequirementToken::Comment(_) | RequirementToken::NewLine => {
                        i += 1;
                    }
                    RequirementToken::Requirement
                    | RequirementToken::FunctionalRequirement
                    | RequirementToken::PerformanceRequirement
                    | RequirementToken::InterfaceRequirement
                    | RequirementToken::DesignConstraint => {
                        if let Some((req, consumed)) = parse_requirement(&tokens[i..]) {
                            requirements.insert(req.name.clone(), req);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    RequirementToken::Element => {
                        if let Some((elem, consumed)) = parse_element(&tokens[i..]) {
                            elements.insert(elem.name.clone(), elem);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    RequirementToken::Identifier(source) => {
                        // Try to parse relationship
                        if let Some((rel, consumed)) = parse_relationship(&tokens[i..], source) {
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
            
            RequirementDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                requirements,
                elements,
                relationships,
            }
        })
}

fn parse_requirement(tokens: &[RequirementToken]) -> Option<(Requirement, usize)> {
    if tokens.len() < 4 {
        return None;
    }
    
    let mut i = 0;
    let req_type = match &tokens[i] {
        RequirementToken::Requirement => RequirementType::Requirement,
        RequirementToken::FunctionalRequirement => RequirementType::FunctionalRequirement,
        RequirementToken::PerformanceRequirement => RequirementType::PerformanceRequirement,
        RequirementToken::InterfaceRequirement => RequirementType::InterfaceRequirement,
        RequirementToken::DesignConstraint => RequirementType::DesignConstraint,
        _ => return None,
    };
    i += 1;
    
    let name = match &tokens[i] {
        RequirementToken::Identifier(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], RequirementToken::LeftBrace) {
        return None;
    }
    i += 1;
    
    let mut id = String::new();
    let mut text = String::new();
    let mut risk = None;
    let mut verify_method = None;
    
    while i < tokens.len() {
        match &tokens[i] {
            RequirementToken::RightBrace => {
                i += 1;
                break;
            }
            RequirementToken::Id => {
                i += 1;
                if let RequirementToken::Identifier(val) = &tokens[i] {
                    id = val.clone();
                    i += 1;
                }
            }
            RequirementToken::Text => {
                i += 1;
                if let RequirementToken::Identifier(val) | RequirementToken::QuotedString(val) = &tokens[i] {
                    text = val.clone();
                    i += 1;
                }
            }
            RequirementToken::Risk => {
                i += 1;
                if let RequirementToken::Identifier(val) = &tokens[i] {
                    risk = match val.to_lowercase().as_str() {
                        "low" => Some(RiskLevel::Low),
                        "medium" => Some(RiskLevel::Medium),
                        "high" => Some(RiskLevel::High),
                        _ => None,
                    };
                    i += 1;
                }
            }
            RequirementToken::VerifyMethod => {
                i += 1;
                if let RequirementToken::Identifier(val) = &tokens[i] {
                    verify_method = match val.to_lowercase().as_str() {
                        "analysis" => Some(VerificationMethod::Analysis),
                        "inspection" => Some(VerificationMethod::Inspection),
                        "test" => Some(VerificationMethod::Test),
                        "demonstration" => Some(VerificationMethod::Demonstration),
                        _ => None,
                    };
                    i += 1;
                }
            }
            RequirementToken::NewLine => {
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    
    Some((
        Requirement {
            name,
            req_type,
            id,
            text,
            risk,
            verify_method,
        },
        i,
    ))
}

fn parse_element(tokens: &[RequirementToken]) -> Option<(Element, usize)> {
    if tokens.len() < 4 {
        return None;
    }
    
    let mut i = 1; // Skip "element"
    
    let name = match &tokens[i] {
        RequirementToken::Identifier(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], RequirementToken::LeftBrace) {
        return None;
    }
    i += 1;
    
    let mut element_type = String::new();
    let mut doc_ref = None;
    
    while i < tokens.len() {
        match &tokens[i] {
            RequirementToken::RightBrace => {
                i += 1;
                break;
            }
            RequirementToken::Type => {
                i += 1;
                if let RequirementToken::Identifier(val) = &tokens[i] {
                    element_type = val.clone();
                    i += 1;
                }
            }
            RequirementToken::DocRef => {
                i += 1;
                if let RequirementToken::Identifier(val) = &tokens[i] {
                    doc_ref = Some(val.clone());
                    i += 1;
                }
            }
            RequirementToken::NewLine => {
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    
    Some((
        Element {
            name,
            element_type,
            doc_ref,
        },
        i,
    ))
}

fn parse_relationship(tokens: &[RequirementToken], source: &str) -> Option<(RequirementRelationship, usize)> {
    if tokens.len() < 5 {
        return None;
    }
    
    let mut i = 1; // Skip source identifier
    
    // Parse relationship pattern: source - rel_type -> target
    // or: source <- rel_type - target
    
    let is_reverse = if matches!(&tokens[i], RequirementToken::BackArrow) {
        i += 1;
        true
    } else if matches!(&tokens[i], RequirementToken::Dash) {
        i += 1;
        false
    } else {
        return None;
    };
    
    let rel_type = match &tokens[i] {
        RequirementToken::RelationshipType(rel) => {
            match rel.as_str() {
                "contains" => RelationshipType::Contains,
                "copies" => RelationshipType::Copies,
                "derives" => RelationshipType::Derives,
                "satisfies" => RelationshipType::Satisfies,
                "verifies" => RelationshipType::Verifies,
                "refines" => RelationshipType::Refines,
                "traces" => RelationshipType::Traces,
                _ => return None,
            }
        }
        _ => return None,
    };
    i += 1;
    
    if is_reverse {
        if !matches!(&tokens[i], RequirementToken::Dash) {
            return None;
        }
        i += 1;
    } else {
        if !matches!(&tokens[i], RequirementToken::Arrow) {
            return None;
        }
        i += 1;
    }
    
    let target = match &tokens[i] {
        RequirementToken::Identifier(name) => name.clone(),
        _ => return None,
    };
    i += 1;
    
    let relationship = if is_reverse {
        RequirementRelationship {
            source: target,
            target: source.to_string(),
            relationship_type: rel_type,
        }
    } else {
        RequirementRelationship {
            source: source.to_string(),
            target,
            relationship_type: rel_type,
        }
    };
    
    Some((relationship, i))
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/requirement/`
- Expected count: 83 files
- Copy to: `mermaid-parser/test/requirement/`

### Command
```bash
cp -r ../mermaid-samples/requirement/* ./test/requirement/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_requirement_files(#[files("test/requirement/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = requirement_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = requirement_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.requirements.is_empty() || !diagram.elements.is_empty(), 
            "Should have at least one requirement or element");
}

#[test]
fn test_simple_requirement() {
    let input = r#"requirementDiagram

requirement test_req {
    id: 1
    text: the test text.
    risk: high
    verifymethod: test
}

element test_entity {
    type: simulation
}

test_entity - satisfies -> test_req
"#;
    
    let tokens = requirement_lexer().parse(input.chars()).unwrap();
    let diagram = requirement_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.requirements.len(), 1);
    assert_eq!(diagram.elements.len(), 1);
    assert_eq!(diagram.relationships.len(), 1);
    
    let req = &diagram.requirements["test_req"];
    assert_eq!(req.id, "1");
    assert_eq!(req.text, "the test text.");
    assert_eq!(req.risk, Some(RiskLevel::High));
    assert_eq!(req.verify_method, Some(VerificationMethod::Test));
    
    let rel = &diagram.relationships[0];
    assert_eq!(rel.source, "test_entity");
    assert_eq!(rel.target, "test_req");
    assert_eq!(rel.relationship_type, RelationshipType::Satisfies);
}

#[test]
fn test_requirement_types() {
    let input = r#"requirementDiagram

functionalRequirement func_req {
    id: 1.1
    text: functional requirement
}

performanceRequirement perf_req {
    id: 1.2
    text: performance requirement
}

interfaceRequirement int_req {
    id: 1.3
    text: interface requirement
}

designConstraint design_req {
    id: 1.4
    text: design constraint
}
"#;
    
    let tokens = requirement_lexer().parse(input.chars()).unwrap();
    let diagram = requirement_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.requirements.len(), 4);
    assert_eq!(diagram.requirements["func_req"].req_type, RequirementType::FunctionalRequirement);
    assert_eq!(diagram.requirements["perf_req"].req_type, RequirementType::PerformanceRequirement);
    assert_eq!(diagram.requirements["int_req"].req_type, RequirementType::InterfaceRequirement);
    assert_eq!(diagram.requirements["design_req"].req_type, RequirementType::DesignConstraint);
}

#[test]
fn test_element_with_docref() {
    let input = r#"requirementDiagram

element test_entity {
    type: word doc
    docRef: reqs/test_entity
}
"#;
    
    let tokens = requirement_lexer().parse(input.chars()).unwrap();
    let diagram = requirement_parser().parse(tokens).unwrap();
    
    let elem = &diagram.elements["test_entity"];
    assert_eq!(elem.element_type, "word doc");
    assert_eq!(elem.doc_ref, Some("reqs/test_entity".to_string()));
}

#[test]
fn test_reverse_relationship() {
    let input = r#"requirementDiagram

requirement req1 {
    id: 1
    text: first
}

requirement req2 {
    id: 2
    text: second
}

req2 <- copies - req1
"#;
    
    let tokens = requirement_lexer().parse(input.chars()).unwrap();
    let diagram = requirement_parser().parse(tokens).unwrap();
    
    let rel = &diagram.relationships[0];
    assert_eq!(rel.source, "req1");
    assert_eq!(rel.target, "req2");
    assert_eq!(rel.relationship_type, RelationshipType::Copies);
}
```

## Success Criteria
1. ✅ Parse all 83 requirement sample files successfully
2. ✅ Handle all requirement types (functional, performance, interface, design)
3. ✅ Parse requirement properties (id, text, risk, verifymethod)
4. ✅ Support elements with type and docRef
5. ✅ Handle all relationship types (contains, copies, derives, etc.)
6. ✅ Support forward and reverse relationship syntax
7. ✅ Parse risk levels correctly
8. ✅ Handle verification methods

## Implementation Priority
**Priority 15** - Implement after ER diagrams. Requirement diagrams share the entity-relationship pattern but add traceability and verification concepts. The relationship modeling from ER diagrams provides a foundation for the more specialized requirement relationships.