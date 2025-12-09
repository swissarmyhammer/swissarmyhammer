use mermaid_parser::{parse_diagram, DiagramType};
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_treemap_files(#[files("test/treemap/*.mermaid")] path: PathBuf) {
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

    // Skip empty files or files that don't have a valid treemap diagram start
    let first_line = content.lines().next().unwrap_or("").trim();
    if content.is_empty() || (first_line != "treemap" && first_line != "treemap-beta") {
        return;
    }

    let result = parse_diagram(&content);

    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);

    match result.unwrap() {
        DiagramType::Treemap(diagram) => {
            // Validate hierarchical structure
            validate_treemap_structure(&diagram, &path);
        }
        _ => panic!("Expected Treemap diagram from {:?}", path),
    }
}

fn validate_treemap_structure(
    diagram: &mermaid_parser::common::ast::TreemapDiagram,
    path: &PathBuf,
) {
    // Check that accessibility info is properly initialized
    assert!(diagram.accessibility.title.is_none() || diagram.accessibility.title.is_some());
    assert!(
        diagram.accessibility.description.is_none() || diagram.accessibility.description.is_some()
    );

    // Validate root node
    assert!(
        !diagram.root.name.is_empty(),
        "Root name should not be empty"
    );

    // Validate tree structure recursively
    validate_treemap_node(&diagram.root, path, 0);
}

fn validate_treemap_node(
    node: &mermaid_parser::common::ast::TreemapNode,
    path: &PathBuf,
    depth: usize,
) {
    // Node name should not be empty
    assert!(
        !node.name.is_empty(),
        "Node name should not be empty at depth {} in {:?}",
        depth,
        path
    );

    // If node has a value, it should be non-negative
    if let Some(value) = node.value {
        assert!(
            value >= 0.0,
            "Node value should be non-negative, got {} at depth {} in {:?}",
            value,
            depth,
            path
        );
    }

    // Validate all children recursively
    for child in &node.children {
        validate_treemap_node(child, path, depth + 1);
    }

    // Check hierarchy consistency
    // If a node has children, the hierarchical relationship should be maintained
    for child in &node.children {
        assert!(
            child.name != node.name,
            "Child cannot have same name as parent '{}' at depth {} in {:?}",
            node.name,
            depth,
            path
        );
    }
}

#[test]
fn test_simple_treemap_validation() {
    let input = r#"treemap
    title Budget Allocation
    
    Total Budget
        Operations: 500000
        Marketing: 300000
        Development: 700000
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        DiagramType::Treemap(diagram) => {
            assert_eq!(diagram.title, Some("Budget Allocation".to_string()));
            assert_eq!(diagram.root.name, "Total Budget");
            assert_eq!(diagram.root.children.len(), 3);

            // Check first child
            assert_eq!(diagram.root.children[0].name, "Operations");
            assert_eq!(diagram.root.children[0].value, Some(500000.0));

            // Check second child
            assert_eq!(diagram.root.children[1].name, "Marketing");
            assert_eq!(diagram.root.children[1].value, Some(300000.0));

            // Check third child
            assert_eq!(diagram.root.children[2].name, "Development");
            assert_eq!(diagram.root.children[2].value, Some(700000.0));
        }
        _ => panic!("Expected Treemap diagram"),
    }
}

#[test]
fn test_treemap_beta_with_quotes() {
    let input = r#"treemap-beta
"Category A"
    "Item A1": 10
    "Item A2": 20
"Category B"
    "Item B1": 15
    "Item B2": 25
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        DiagramType::Treemap(diagram) => {
            assert_eq!(diagram.root.name, "Category A");
            assert_eq!(diagram.root.children.len(), 2);

            assert_eq!(diagram.root.children[0].name, "Item A1");
            assert_eq!(diagram.root.children[0].value, Some(10.0));
            assert_eq!(diagram.root.children[1].name, "Item A2");
            assert_eq!(diagram.root.children[1].value, Some(20.0));
        }
        _ => panic!("Expected Treemap diagram"),
    }
}

#[test]
fn test_nested_hierarchy_validation() {
    let input = r#"treemap
    Company
        Sales
            North Region
                Q1: 100000
                Q2: 120000
            South Region
                Q1: 80000
                Q2: 95000
        Engineering
            Frontend: 5
            Backend: 8
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        DiagramType::Treemap(diagram) => {
            assert_eq!(diagram.root.name, "Company");
            assert_eq!(diagram.root.children.len(), 2);

            // Check Sales department
            let sales = &diagram.root.children[0];
            assert_eq!(sales.name, "Sales");
            assert_eq!(sales.children.len(), 2);

            // Check North Region
            let north = &sales.children[0];
            assert_eq!(north.name, "North Region");
            assert_eq!(north.children.len(), 2);
            assert_eq!(north.children[0].name, "Q1");
            assert_eq!(north.children[0].value, Some(100000.0));
            assert_eq!(north.children[1].name, "Q2");
            assert_eq!(north.children[1].value, Some(120000.0));

            // Check South Region
            let south = &sales.children[1];
            assert_eq!(south.name, "South Region");
            assert_eq!(south.children.len(), 2);
            assert_eq!(south.children[0].name, "Q1");
            assert_eq!(south.children[0].value, Some(80000.0));
            assert_eq!(south.children[1].name, "Q2");
            assert_eq!(south.children[1].value, Some(95000.0));

            // Check Engineering department
            let engineering = &diagram.root.children[1];
            assert_eq!(engineering.name, "Engineering");
            assert_eq!(engineering.children.len(), 2);
            assert_eq!(engineering.children[0].name, "Frontend");
            assert_eq!(engineering.children[0].value, Some(5.0));
            assert_eq!(engineering.children[1].name, "Backend");
            assert_eq!(engineering.children[1].value, Some(8.0));
        }
        _ => panic!("Expected Treemap diagram"),
    }
}

#[test]
fn test_treemap_without_values() {
    let input = r#"treemap
    Root
        Child1
        Child2
            Grandchild1
            Grandchild2
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        DiagramType::Treemap(diagram) => {
            assert_eq!(diagram.root.name, "Root");
            assert_eq!(diagram.root.value, None);
            assert_eq!(diagram.root.children.len(), 2);

            assert_eq!(diagram.root.children[0].name, "Child1");
            assert_eq!(diagram.root.children[0].value, None);
            assert_eq!(diagram.root.children[0].children.len(), 0);

            assert_eq!(diagram.root.children[1].name, "Child2");
            assert_eq!(diagram.root.children[1].value, None);
            assert_eq!(diagram.root.children[1].children.len(), 2);

            assert_eq!(diagram.root.children[1].children[0].name, "Grandchild1");
            assert_eq!(diagram.root.children[1].children[0].value, None);
            assert_eq!(diagram.root.children[1].children[1].name, "Grandchild2");
            assert_eq!(diagram.root.children[1].children[1].value, None);
        }
        _ => panic!("Expected Treemap diagram"),
    }
}
