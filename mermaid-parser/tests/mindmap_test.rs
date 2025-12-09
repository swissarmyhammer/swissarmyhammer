use mermaid_parser::common::ast::MindmapNodeShape;
use mermaid_parser::parsers::mindmap;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_mindmap_files(#[files("test/mindmap/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read file {:?}: {}", path, e));

    // Remove metadata comments that might interfere with parsing
    let content = content
        .lines()
        .filter(|line| !line.trim().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    // Skip empty files
    if content.trim().is_empty() {
        return;
    }

    let result = mindmap::parse(&content);

    match result {
        Ok(diagram) => {
            // Basic validation - mindmap should have a root node
            println!("✅ Successfully parsed {:?}", path.file_name().unwrap());
            println!("   Root: {}", diagram.root.text);
            println!("   Children: {}", diagram.root.children.len());
            assert!(!diagram.root.text.is_empty(), "Root node should have text");
        }
        Err(e) => {
            panic!(
                "Failed to parse file {:?}: {:?}\nContent:\n{}",
                path.file_name().unwrap(),
                e,
                content
            );
        }
    }
}

#[test]
fn test_simple_mindmap() {
    let input = r#"mindmap
  root((mindmap))
    Origins
      Long history
      Popularisation
        British author
    Research
      On effectiveness
      On features
"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok(), "Should parse simple mindmap successfully");

    let diagram = result.unwrap();
    assert_eq!(diagram.root.text, "mindmap");
    assert_eq!(diagram.root.shape, MindmapNodeShape::Circle);
    assert_eq!(diagram.root.children.len(), 2); // Origins, Research

    let origins = &diagram.root.children[0];
    assert_eq!(origins.text, "Origins");
    assert_eq!(origins.children.len(), 2); // Long history, Popularisation
}

#[test]
fn test_node_shapes() {
    let shapes_and_inputs = vec![
        ("(text)", MindmapNodeShape::Rounded),
        ("[text]", MindmapNodeShape::Square),
        ("((text))", MindmapNodeShape::Circle),
        ("{{text}}", MindmapNodeShape::Hexagon),
        ("(-text-)", MindmapNodeShape::Cloud),
        ("))text((", MindmapNodeShape::Bang),
        ("text", MindmapNodeShape::Default), // No brackets
    ];

    for (input, expected_shape) in shapes_and_inputs {
        let full_input = format!("mindmap\n  {}", input);
        let result = mindmap::parse(&full_input);
        assert!(result.is_ok(), "Should parse {} successfully", input);

        let diagram = result.unwrap();
        if diagram.root.children.is_empty() {
            // Single node case - shape applies to root
            assert_eq!(
                diagram.root.shape, expected_shape,
                "Shape mismatch for {}",
                input
            );
        } else {
            // Child node case
            assert_eq!(
                diagram.root.children[0].shape, expected_shape,
                "Shape mismatch for {}",
                input
            );
        }
    }
}

#[test]
fn test_icons_and_classes() {
    let input = r#"mindmap
  root
    ::icon(fa fa-book)
    Node with icon
    :::myClass
    Node with class
"#;

    let result = mindmap::parse(input);
    assert!(
        result.is_ok(),
        "Should parse mindmap with icons and classes"
    );

    let diagram = result.unwrap();

    // Check that we have parsed nodes with icon and class
    let has_icon = diagram.root.children.iter().any(|n| n.icon.is_some());
    let has_class = diagram.root.children.iter().any(|n| n.class.is_some());

    assert!(has_icon, "Should have at least one node with icon");
    assert!(has_class, "Should have at least one node with class");

    // Check specific icon and class values
    let icon_node = diagram.root.children.iter().find(|n| n.icon.is_some());
    if let Some(node) = icon_node {
        assert_eq!(node.icon.as_ref().unwrap(), "fa fa-book");
    }

    let class_node = diagram.root.children.iter().find(|n| n.class.is_some());
    if let Some(node) = class_node {
        assert_eq!(node.class.as_ref().unwrap(), "myClass");
    }
}

#[test]
fn test_hierarchical_structure() {
    let input = r#"mindmap
  root
    Parent 1
      Child 1.1
        Grandchild 1.1.1
      Child 1.2
    Parent 2
      Child 2.1
"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok(), "Should parse hierarchical mindmap");

    let diagram = result.unwrap();
    assert_eq!(diagram.root.children.len(), 2); // Parent 1, Parent 2

    let parent1 = &diagram.root.children[0];
    assert_eq!(parent1.text, "Parent 1");
    assert_eq!(parent1.children.len(), 2); // Child 1.1, Child 1.2

    let child11 = &parent1.children[0];
    assert_eq!(child11.text, "Child 1.1");
    assert_eq!(child11.children.len(), 1); // Grandchild 1.1.1

    let grandchild = &child11.children[0];
    assert_eq!(grandchild.text, "Grandchild 1.1.1");
    assert_eq!(grandchild.children.len(), 0); // No further children
}

#[test]
fn test_mixed_content() {
    let input = r#"mindmap
  root((Central Idea))
    [Topic A]
      ::icon(fa fa-lightbulb)
      Bright idea
    (Topic B)
      :::highlighted
      Important point
    {{Topic C}}
      Regular content
"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok(), "Should parse mindmap with mixed content");

    let diagram = result.unwrap();
    assert_eq!(diagram.root.text, "Central Idea");
    assert_eq!(diagram.root.shape, MindmapNodeShape::Circle);
    assert_eq!(diagram.root.children.len(), 3);

    // Check different node shapes
    let topic_a = &diagram.root.children[0];
    assert_eq!(topic_a.shape, MindmapNodeShape::Square);

    let topic_b = &diagram.root.children[1];
    assert_eq!(topic_b.shape, MindmapNodeShape::Rounded);

    let topic_c = &diagram.root.children[2];
    assert_eq!(topic_c.shape, MindmapNodeShape::Hexagon);
}

#[test]
fn test_empty_mindmap() {
    let input = "mindmap\n  root";

    let result = mindmap::parse(input);
    assert!(result.is_ok(), "Should parse minimal mindmap");

    let diagram = result.unwrap();
    assert_eq!(diagram.root.text, "root");
    assert_eq!(diagram.root.children.len(), 0);
}

#[test]
fn test_invalid_mindmap() {
    let input = "not_mindmap\n  some content";

    let result = mindmap::parse(input);
    assert!(result.is_err(), "Should fail to parse invalid mindmap");
}
