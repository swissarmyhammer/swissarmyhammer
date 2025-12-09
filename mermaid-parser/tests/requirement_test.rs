use chumsky::Parser;
use mermaid_parser::common::ast::{
    RelationshipType, RequirementType, RiskLevel, VerificationMethod,
};
use mermaid_parser::parsers::requirement;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_requirement_files(#[files("test/requirement/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Skip files with non-standard headers (test artifacts)
    let clean_content = content
        .lines()
        .filter(|line| !line.trim().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let first_line = clean_content
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");

    let known_invalid_headers = [
        "requirement_arrow",
        "requirement_contains",
        "requirementBox",
        "requirementDb",
        "requirementDiagramTitleText",
    ];

    // Skip files with template variables (test artifacts)
    if clean_content.contains("${") {
        return;
    }

    if known_invalid_headers
        .iter()
        .any(|&h| first_line.trim() == h)
    {
        // Skip known invalid test cases
        return;
    }

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let result = requirement::parse(&content);

    if let Err(e) = &result {
        panic!("Parser failed for {:?}: {:?}", path, e);
    }

    let _diagram = result.unwrap();

    // Empty diagrams are valid - just validate parsing succeeded
}

#[test]
fn test_underscore_identifier() {
    let input = r#"requirementDiagram

requirement "__test_req__" {
    id: 1
}"#;

    // Test lexer
    let tokens = requirement::requirement_lexer().parse(input).into_result();

    match tokens {
        Ok(tokens) => {
            println!("Tokens: {:?}", tokens);
        }
        Err(e) => {
            panic!("Lexer error: {:?}", e);
        }
    }
}

#[test]
fn test_requirement_only() {
    let input = "requirement\n";

    // Test lexer
    let tokens = requirement::requirement_lexer().parse(input).into_result();

    match tokens {
        Ok(tokens) => {
            println!("Tokens: {:?}", tokens);
            assert!(!tokens.is_empty());
        }
        Err(e) => {
            panic!("Lexer error: {:?}", e);
        }
    }

    // Test parser
    match requirement::parse(input) {
        Ok(diagram) => {
            println!("Parse successful!");
            assert_eq!(diagram.requirements.len(), 0);
            assert_eq!(diagram.elements.len(), 0);
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}

#[test]
fn test_debug_lexer() {
    let input = "requirementDiagram\n\nrequirement test_req {\n    id: 1\n    text: the test text.\n    risk: high\n    verifymethod: test\n}\n";

    // Test just the lexer first
    let tokens = requirement::requirement_lexer().parse(input).into_result();

    match tokens {
        Ok(tokens) => {
            println!("Tokens: {:?}", tokens);
            assert!(!tokens.is_empty());
        }
        Err(e) => {
            panic!("Lexer failed: {:?}", e);
        }
    }
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

    let diagram = requirement::parse(input).unwrap();

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

    let diagram = requirement::parse(input).unwrap();

    assert_eq!(diagram.requirements.len(), 4);
    assert_eq!(
        diagram.requirements["func_req"].req_type,
        RequirementType::FunctionalRequirement
    );
    assert_eq!(
        diagram.requirements["perf_req"].req_type,
        RequirementType::PerformanceRequirement
    );
    assert_eq!(
        diagram.requirements["int_req"].req_type,
        RequirementType::InterfaceRequirement
    );
    assert_eq!(
        diagram.requirements["design_req"].req_type,
        RequirementType::DesignConstraint
    );
}

#[test]
fn test_element_with_docref() {
    let input = r#"requirementDiagram

element test_entity {
    type: word doc
    docRef: reqs/test_entity
}
"#;

    let diagram = requirement::parse(input).unwrap();

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

    let diagram = requirement::parse(input).unwrap();

    let rel = &diagram.relationships[0];
    assert_eq!(rel.source, "req1");
    assert_eq!(rel.target, "req2");
    assert_eq!(rel.relationship_type, RelationshipType::Copies);
}
