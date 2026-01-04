use mermaid_parser::parse_diagram;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_sankey_files(#[files("test/sankey/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    // Skip empty files or files with only whitespace
    if content.is_empty() {
        return;
    }

    let result = parse_diagram(&content);

    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Sankey(diagram) => {
            // Validate CSV-like flow data structure
            for link in &diagram.links {
                // Check source, target, and value fields
                assert!(
                    !link.source.is_empty(),
                    "Source field should not be empty in {:?}",
                    path
                );
                assert!(
                    !link.target.is_empty(),
                    "Target field should not be empty in {:?}",
                    path
                );

                // Ensure all flows have positive values
                assert!(
                    link.value >= 0.0,
                    "Flow value should be non-negative in {:?}: got {}",
                    path,
                    link.value
                );
            }

            // Verify all links reference existing nodes
            let node_names: std::collections::HashSet<_> =
                diagram.nodes.iter().map(|n| &n.name).collect();

            for link in &diagram.links {
                assert!(
                    node_names.contains(&link.source),
                    "Link source '{}' not found in nodes for file: {:?}",
                    link.source,
                    path
                );
                assert!(
                    node_names.contains(&link.target),
                    "Link target '{}' not found in nodes for file: {:?}",
                    link.target,
                    path
                );
            }
        }
        _ => panic!("Expected Sankey diagram from {:?}", path),
    }
}

#[test]
fn test_simple_sankey_diagram() {
    let input = r#"sankey-beta
A,B,10
B,C,5
C,D,2.5
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Sankey(diagram) => {
            assert_eq!(diagram.nodes.len(), 4);
            assert_eq!(diagram.links.len(), 3);

            // Check first link
            assert_eq!(diagram.links[0].source, "A");
            assert_eq!(diagram.links[0].target, "B");
            assert_eq!(diagram.links[0].value, 10.0);

            // Check second link
            assert_eq!(diagram.links[1].source, "B");
            assert_eq!(diagram.links[1].target, "C");
            assert_eq!(diagram.links[1].value, 5.0);

            // Check third link
            assert_eq!(diagram.links[2].source, "C");
            assert_eq!(diagram.links[2].target, "D");
            assert_eq!(diagram.links[2].value, 2.5);
        }
        _ => panic!("Expected Sankey diagram"),
    }
}

#[test]
fn test_sankey_with_quoted_fields() {
    let input = r#"sankey-beta
"Source Node","Target Node",25.5
"Another Source",Destination,15.0
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Sankey(diagram) => {
            assert_eq!(diagram.nodes.len(), 4);
            assert_eq!(diagram.links.len(), 2);

            assert_eq!(diagram.links[0].source, "Source Node");
            assert_eq!(diagram.links[0].target, "Target Node");
            assert_eq!(diagram.links[0].value, 25.5);

            assert_eq!(diagram.links[1].source, "Another Source");
            assert_eq!(diagram.links[1].target, "Destination");
            assert_eq!(diagram.links[1].value, 15.0);
        }
        _ => panic!("Expected Sankey diagram"),
    }
}

#[test]
fn test_empty_sankey() {
    let input = "sankey-beta\n";

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        mermaid_parser::DiagramType::Sankey(diagram) => {
            assert_eq!(diagram.nodes.len(), 0);
            assert_eq!(diagram.links.len(), 0);
        }
        _ => panic!("Expected Sankey diagram"),
    }
}
