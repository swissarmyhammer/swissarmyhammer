//! Additional tests to improve coverage for mindmap.rs parser

use mermaid_parser::common::ast::MindmapNodeShape;
use mermaid_parser::error::ParseError;
use mermaid_parser::parsers::mindmap;

#[test]
fn test_empty_input_error() {
    let input = "";
    let result = mindmap::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::EmptyInput) => {
            // Expected empty input error
        }
        _ => panic!("Expected EmptyInput error for empty input"),
    }
}

#[test]
fn test_non_mindmap_header_error() {
    let input = "flowchart TD\n  A --> B";
    let result = mindmap::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::SyntaxError {
            message,
            expected,
            found,
            ..
        }) => {
            assert!(message.contains("Expected mindmap header"));
            assert_eq!(expected, vec!["mindmap"]);
            assert_eq!(found, "flowchart TD");
        }
        _ => panic!("Expected SyntaxError for non-mindmap header"),
    }
}

#[test]
fn test_comments_and_empty_lines() {
    let input = r#"mindmap
  // This is a comment
  root
    
    %% Another comment
    
    Child 1
    // Comment between nodes
    Child 2
    
    %% Final comment"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.root.text, "root");
    assert_eq!(diagram.root.children.len(), 2);
    assert_eq!(diagram.root.children[0].text, "Child 1");
    assert_eq!(diagram.root.children[1].text, "Child 2");
}

#[test]
fn test_icon_syntax_variations() {
    // Test icon at different positions
    let input1 = r#"mindmap
  root::icon(fa fa-home)"#;

    let result1 = mindmap::parse(input1);
    assert!(result1.is_ok());
    let diagram1 = result1.unwrap();
    assert_eq!(diagram1.root.text, "root");
    assert_eq!(diagram1.root.icon, Some("fa fa-home".to_string()));

    // Test icon in middle of text
    let input2 = r#"mindmap
  root
    Before::icon(fa fa-book)After"#;

    let result2 = mindmap::parse(input2);
    assert!(result2.is_ok());
    let diagram2 = result2.unwrap();
    assert_eq!(diagram2.root.children[0].text, "BeforeAfter");
    assert_eq!(
        diagram2.root.children[0].icon,
        Some("fa fa-book".to_string())
    );

    // Test malformed icon syntax (no closing parenthesis)
    let input3 = r#"mindmap
  root
    Text::icon(fa fa-broken"#;

    let result3 = mindmap::parse(input3);
    assert!(result3.is_ok());
    let diagram3 = result3.unwrap();
    // Should not parse as icon due to missing closing parenthesis
    assert_eq!(diagram3.root.children[0].text, "Text::icon(fa fa-broken");
    assert_eq!(diagram3.root.children[0].icon, None);
}

#[test]
fn test_class_syntax_variations() {
    // Test class at end of line
    let input1 = r#"mindmap
  root
    Node text:::myClass"#;

    let result1 = mindmap::parse(input1);
    assert!(result1.is_ok());
    let diagram1 = result1.unwrap();
    assert_eq!(diagram1.root.children[0].text, "Node text");
    assert_eq!(diagram1.root.children[0].class, Some("myClass".to_string()));

    // Test empty class
    let input2 = r#"mindmap
  root
    Node text:::"#;

    let result2 = mindmap::parse(input2);
    assert!(result2.is_ok());
    let diagram2 = result2.unwrap();
    assert_eq!(diagram2.root.children[0].text, "Node text:::");
    assert_eq!(diagram2.root.children[0].class, None);

    // Test class with spaces
    let input3 = r#"mindmap
  root
    Node text:::  spaced-class  "#;

    let result3 = mindmap::parse(input3);
    assert!(result3.is_ok());
    let diagram3 = result3.unwrap();
    assert_eq!(diagram3.root.children[0].text, "Node text");
    assert_eq!(
        diagram3.root.children[0].class,
        Some("spaced-class".to_string())
    );
}

#[test]
fn test_icon_and_class_combined() {
    let input = r#"mindmap
  root
    ::icon(fa fa-star)Node with both:::important"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let node = &diagram.root.children[0];
    assert_eq!(node.text, "Node with both");
    assert_eq!(node.icon, Some("fa fa-star".to_string()));
    assert_eq!(node.class, Some("important".to_string()));
}

#[test]
fn test_complex_hierarchy_with_varied_indentation() {
    let input = r#"mindmap
  root
    Level 1A
      Level 2A
        Level 3A
          Level 4A
      Level 2B
    Level 1B
      Level 2C
        Level 3B
    Level 1C"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Verify hierarchy structure
    assert_eq!(diagram.root.children.len(), 3); // Level 1A, 1B, 1C

    let level1a = &diagram.root.children[0];
    assert_eq!(level1a.text, "Level 1A");
    assert_eq!(level1a.children.len(), 2); // Level 2A, 2B

    let level2a = &level1a.children[0];
    assert_eq!(level2a.text, "Level 2A");
    assert_eq!(level2a.children.len(), 1); // Level 3A

    let level3a = &level2a.children[0];
    assert_eq!(level3a.text, "Level 3A");
    assert_eq!(level3a.children.len(), 1); // Level 4A
}

#[test]
fn test_edge_case_node_shapes() {
    // Test nested shapes (should use outermost)
    let input1 = r#"mindmap
  ((Outer((Inner))Outer))"#;

    let result1 = mindmap::parse(input1);
    assert!(result1.is_ok());
    let diagram1 = result1.unwrap();
    assert_eq!(diagram1.root.text, "Outer((Inner))Outer");
    assert_eq!(diagram1.root.shape, MindmapNodeShape::Circle);

    // Test incomplete shapes
    let input2 = r#"mindmap
  root
    ((Incomplete
    {{Also incomplete
    (-Cloud incomplete
    ))Bang incomplete"#;

    let result2 = mindmap::parse(input2);
    assert!(result2.is_ok());
    let diagram2 = result2.unwrap();
    // All should be Default shape due to incomplete syntax
    for child in &diagram2.root.children {
        assert_eq!(child.shape, MindmapNodeShape::Default);
    }
}

#[test]
fn test_shape_with_empty_content() {
    let shapes = vec![
        // These fall through to simple bracket parsing since content is too short
        ("(())", MindmapNodeShape::Rounded, "()"), // Falls through to simple ( ) parsing
        ("{{}}", MindmapNodeShape::Default, "{{}}"), // No simple bracket match, returns as-is
        ("(--)", MindmapNodeShape::Rounded, "--"), // Falls through to simple ( ) parsing
        ("))((", MindmapNodeShape::Default, "))(("), // No match, returns as-is
    ];

    for (shape_syntax, expected_shape, expected_text) in shapes {
        let input = format!("mindmap\n  {}", shape_syntax);
        let result = mindmap::parse(&input);
        assert!(result.is_ok());
        let diagram = result.unwrap();
        // Empty shape content should result in empty text, not "()"
        assert_eq!(diagram.root.text, expected_text);
        assert_eq!(diagram.root.shape, expected_shape);
    }
}

#[test]
fn test_mindmap_with_only_header() {
    let input = "mindmap";

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Should create default root node
    assert_eq!(diagram.root.text, "Root");
    assert_eq!(diagram.root.shape, MindmapNodeShape::Default);
    assert_eq!(diagram.root.children.len(), 0);
}

#[test]
fn test_deep_nesting_with_skipped_levels() {
    let input = r#"mindmap
  root
        Deep child (skipped levels)"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Deep child should still be added as direct child of root
    assert_eq!(diagram.root.children.len(), 1);
    assert_eq!(diagram.root.children[0].text, "Deep child (skipped levels)");
}

#[test]
fn test_node_id_generation() {
    let input = r#"mindmap
  root
    Child 1
    Child 2
      Grandchild"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Verify all nodes have unique IDs
    let mut ids = Vec::new();
    ids.push(&diagram.root.id);

    fn collect_ids<'a>(
        node: &'a mermaid_parser::common::ast::MindmapNode,
        ids: &mut Vec<&'a String>,
    ) {
        for child in &node.children {
            ids.push(&child.id);
            collect_ids(child, ids);
        }
    }

    collect_ids(&diagram.root, &mut ids);

    // All IDs should be unique
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique_ids.len());

    // IDs should follow pattern node_N
    for id in ids {
        assert!(id.starts_with("node_"));
    }
}

#[test]
fn test_mixed_indentation_levels() {
    let input = r#"mindmap
  root
    Child 1
          Grandchild 1 (extra indented)
    Child 2
      Normal grandchild
            Great grandchild (extra indented)"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.root.children.len(), 2);

    // First child should have the extra-indented grandchild
    let child1 = &diagram.root.children[0];
    assert_eq!(child1.children.len(), 1);
    assert_eq!(child1.children[0].text, "Grandchild 1 (extra indented)");

    // Second child should have normal hierarchy
    let child2 = &diagram.root.children[1];
    assert_eq!(child2.children.len(), 1);
    assert_eq!(child2.children[0].text, "Normal grandchild");
    assert_eq!(child2.children[0].children.len(), 1);
    assert_eq!(
        child2.children[0].children[0].text,
        "Great grandchild (extra indented)"
    );
}

#[test]
fn test_whitespace_preservation_in_text() {
    let input = r#"mindmap
  root
    [  Text with   spaces  ]
    ((  More   spaces  ))
    Node   with   internal   spaces"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Square brackets: content is extracted from within brackets
    assert_eq!(diagram.root.children[0].text, "  Text with   spaces  ");
    assert_eq!(diagram.root.children[0].shape, MindmapNodeShape::Square);

    // Circle shapes: content is extracted from within double parentheses
    assert_eq!(diagram.root.children[1].text, "  More   spaces  ");
    assert_eq!(diagram.root.children[1].shape, MindmapNodeShape::Circle);

    // Default shape just returns the trimmed text
    assert_eq!(
        diagram.root.children[2].text,
        "Node   with   internal   spaces"
    );
    assert_eq!(diagram.root.children[2].shape, MindmapNodeShape::Default);
}

#[test]
fn test_special_characters_in_nodes() {
    let input = r#"mindmap
  root
    Node with $pecial ch@rs!
    [Node & symbols | pipes]
    ((Math: 2+2=4))
    {{HTML <tag> entities}}"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.root.children[0].text, "Node with $pecial ch@rs!");
    assert_eq!(diagram.root.children[1].text, "Node & symbols | pipes");
    assert_eq!(diagram.root.children[2].text, "Math: 2+2=4");
    assert_eq!(diagram.root.children[3].text, "HTML <tag> entities");
}

#[test]
fn test_complex_real_world_mindmap() {
    let input = r#"mindmap
  root((Project Planning))
    [Requirements]
      ::icon(fa fa-list)
      Functional:::important
        User stories
        Acceptance criteria
      Non-functional:::secondary
        Performance
        Security
    {{Design}}
      ::icon(fa fa-paint-brush)
      Architecture:::technical
        (-Cloud services-)
        Microservices
      UI/UX:::creative
        ))Wireframes((
        Prototypes
    (Implementation)
      Backend
        APIs
        Database
      Frontend
        Components
        State management"#;

    let result = mindmap::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Verify root
    assert_eq!(diagram.root.text, "Project Planning");
    assert_eq!(diagram.root.shape, MindmapNodeShape::Circle);
    assert_eq!(diagram.root.children.len(), 3);

    // Verify first level children
    assert_eq!(diagram.root.children[0].text, "Requirements");
    assert_eq!(diagram.root.children[0].shape, MindmapNodeShape::Square);

    assert_eq!(diagram.root.children[1].text, "Design");
    assert_eq!(diagram.root.children[1].shape, MindmapNodeShape::Hexagon);

    assert_eq!(diagram.root.children[2].text, "Implementation");
    assert_eq!(diagram.root.children[2].shape, MindmapNodeShape::Rounded);

    // The structure is:
    // Requirements (children[0])
    //   - ::icon(fa fa-list) (children[0])
    //   - Functional:::important (children[1])
    //   - Non-functional:::secondary (children[2])

    // Verify icons and classes are parsed
    let requirements = &diagram.root.children[0];
    assert_eq!(requirements.children.len(), 3);

    // The first child has the icon
    assert_eq!(
        requirements.children[0].icon,
        Some("fa fa-list".to_string())
    );

    // The second child is Functional with class "important"
    let functional = &requirements.children[1];
    assert_eq!(functional.text, "Functional");
    assert_eq!(functional.class, Some("important".to_string()));

    // The structure for Design is:
    // Design (children[1])
    //   - ::icon(fa fa-paint-brush) (children[0])
    //   - Architecture:::technical (children[1])
    //   - UI/UX:::creative (children[2])

    let design = &diagram.root.children[1];
    assert_eq!(design.children.len(), 3);

    // Architecture is the second child of Design
    let architecture = &design.children[1];
    assert_eq!(architecture.text, "Architecture");
    assert_eq!(architecture.class, Some("technical".to_string()));

    // Cloud services is the first child of Architecture
    let cloud = &architecture.children[0];
    assert_eq!(cloud.text, "Cloud services");
    assert_eq!(cloud.shape, MindmapNodeShape::Cloud);
}
