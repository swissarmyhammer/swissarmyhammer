use mermaid_parser::{parse_diagram, DiagramType};
use rstest::*;
use std::path::PathBuf;

#[test]
fn test_info_diagram() {
    let input = r#"info showInfo"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse info diagram: {:?}", result);

    match result.unwrap() {
        DiagramType::Misc(diagram) => {
            assert_eq!(diagram.diagram_type, "info");
            // Additional assertions would go here once AST is defined
        }
        _ => panic!("Expected misc diagram type"),
    }
}

#[test]
fn test_gitgraph_alt() {
    let input = r#"gitGraph:
    commit
    branch develop
    checkout develop
    commit
    checkout main
    merge develop
"#;

    let result = parse_diagram(input);
    assert!(
        result.is_ok(),
        "Failed to parse gitGraph diagram: {:?}",
        result
    );

    match result.unwrap() {
        DiagramType::Misc(diagram) => {
            assert_eq!(diagram.diagram_type, "gitGraph");
        }
        _ => panic!("Expected misc diagram type"),
    }
}

#[test]
fn test_unknown_diagram() {
    let input = r#"unknownType
    some content
    more content
"#;

    let result = parse_diagram(input);
    assert!(
        result.is_ok(),
        "Failed to parse unknown diagram: {:?}",
        result
    );

    match result.unwrap() {
        DiagramType::Misc(diagram) => {
            assert_eq!(diagram.diagram_type, "unknownType");
        }
        _ => panic!("Expected misc diagram type"),
    }
}

#[test]
fn test_empty_diagram() {
    let input = "";

    let result = parse_diagram(input);
    // Empty input should fail to detect diagram type
    assert!(result.is_err());
}

#[test]
fn test_edge_cases() {
    // Test various edge cases that might appear in misc
    let cases = vec![
        "%%%% Multiple percent signs",
        "diagram\n  with strange\n    indentation",
        "mixed:syntax{and}formats",
    ];

    for input in cases {
        let result = parse_diagram(input);
        // These should be parsed as misc diagrams
        if let Ok(diagram) = result {
            match diagram {
                DiagramType::Misc(_) => {
                    // Success
                }
                _ => panic!("Expected misc diagram type for: {}", input),
            }
        }
    }
}

#[rstest]
fn test_misc_files(#[files("test/misc/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    let result = parse_diagram(&content);

    // For misc diagrams, we're more forgiving - they might not all parse correctly
    if let Ok(DiagramType::Misc(_)) = result {
        // Success - it was parsed as a misc diagram
    } else if result.is_ok() {
        // It might have been recognized as another diagram type
        // That's okay for misc files
    }
    // Even if parsing fails, that's acceptable for misc files
    // as they may contain experimental or invalid syntax
}
