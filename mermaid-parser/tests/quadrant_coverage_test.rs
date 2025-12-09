//! Additional tests to improve coverage for quadrant.rs parser

use mermaid_parser::error::ParseError;
use mermaid_parser::parsers::quadrant;

#[test]
fn test_empty_input_error() {
    let input = "";
    let result = quadrant::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::EmptyInput) => {}
        _ => panic!("Expected EmptyInput error"),
    }
}

#[test]
fn test_non_quadrant_header_returns_empty_diagram() {
    let input = r#"flowchart TD
    A --> B"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Should return empty diagram for non-quadrant input
    assert_eq!(diagram.title, None);
    assert_eq!(diagram.points.len(), 0);
    assert_eq!(diagram.styles.len(), 0);
}

#[test]
fn test_accessibility_with_space_syntax() {
    let input = r#"quadrantChart
    title Test Chart
    accTitle Test Accessibility Title
    accDescr Test accessibility description
    Point A: [0.5, 0.5]"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(
        diagram.accessibility.title,
        Some("Test Accessibility Title".to_string())
    );
    assert_eq!(
        diagram.accessibility.description,
        Some("Test accessibility description".to_string())
    );
}

#[test]
fn test_axis_definition_edge_cases() {
    // Test axis without arrow
    let input1 = r#"quadrantChart
    x-axis Bad axis definition
    y-axis Low --> High"#;

    let result1 = quadrant::parse(input1);
    assert!(result1.is_ok());
    let diagram1 = result1.unwrap();
    assert!(diagram1.x_axis.is_none()); // Should be None due to no arrow
    assert!(diagram1.y_axis.is_some());

    // Test axis with empty labels
    let input2 = r#"quadrantChart
    x-axis  --> 
    y-axis --> High"#;

    let result2 = quadrant::parse(input2);
    assert!(result2.is_ok());
    let diagram2 = result2.unwrap();

    let x_axis = diagram2.x_axis.unwrap();
    assert_eq!(x_axis.label_start, None);
    assert_eq!(x_axis.label_end, None);

    let y_axis = diagram2.y_axis.unwrap();
    assert_eq!(y_axis.label_start, None);
    assert_eq!(y_axis.label_end, Some("High".to_string()));
}

#[test]
fn test_comments_and_empty_lines() {
    let input = r#"quadrantChart
    // This is a comment
    title Test Chart
    
    %% Another comment style
    
    x-axis Low --> High
    // Comment between elements
    Point A: [0.3, 0.6]
    
    %% Final comment"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.title, Some("Test Chart".to_string()));
    assert!(diagram.x_axis.is_some());
    assert_eq!(diagram.points.len(), 1);
}

#[test]
fn test_data_point_edge_cases() {
    // Test point with empty class name
    let input1 = r#"quadrantChart
    Point A:::: [0.3, 0.6]"#;

    let result1 = quadrant::parse(input1);
    assert!(result1.is_ok());
    let diagram1 = result1.unwrap();
    assert_eq!(diagram1.points.len(), 1);
    assert_eq!(diagram1.points[0].name, "Point A");
    assert_eq!(diagram1.points[0].class, None);

    // Test point with malformed coordinates (wrong number of values)
    let input2 = r#"quadrantChart
    Point A: [0.3]
    Point B: [0.3, 0.6, 0.9]
    Point C: [0.5, 0.5]"#;

    let result2 = quadrant::parse(input2);
    assert!(result2.is_ok());
    let diagram2 = result2.unwrap();
    // Only Point C should be valid
    assert_eq!(diagram2.points.len(), 1);
    assert_eq!(diagram2.points[0].name, "Point C");

    // Test point with non-numeric coordinates
    let input3 = r#"quadrantChart
    Point A: [abc, 0.6]
    Point B: [0.3, def]
    Point C: [0.5, 0.5]"#;

    let result3 = quadrant::parse(input3);
    assert!(result3.is_ok());
    let diagram3 = result3.unwrap();
    // Only Point C should be valid
    assert_eq!(diagram3.points.len(), 1);
    assert_eq!(diagram3.points[0].name, "Point C");
}

#[test]
fn test_coordinate_range_validation() {
    let input = r#"quadrantChart
    Point A: [0.0, 0.0]
    Point B: [1.0, 1.0]
    Point C: [-0.1, 0.5]
    Point D: [0.5, 1.1]
    Point E: [1.1, 0.5]
    Point F: [0.5, -0.1]"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Only Points A and B should be valid (within 0.0-1.0 range)
    assert_eq!(diagram.points.len(), 2);
    assert_eq!(diagram.points[0].name, "Point A");
    assert_eq!(diagram.points[1].name, "Point B");
}

#[test]
fn test_malformed_data_points() {
    let input = r#"quadrantChart
    Point A [0.3, 0.6]
    Point B: 0.3, 0.6
    Point C: [0.3 0.6]
    Point D: [0.3, 0.6"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // None of these should parse as valid points
    assert_eq!(diagram.points.len(), 0);
}

#[test]
fn test_class_definition_edge_cases() {
    // Test malformed classDef statements
    let input1 = r#"quadrantChart
    classDef
    classDef myClass
    classDef validClass fill:#ff0000"#;

    let result1 = quadrant::parse(input1);
    assert!(result1.is_ok());
    let diagram1 = result1.unwrap();

    // Only the valid classDef should be parsed
    assert_eq!(diagram1.styles.len(), 1);
    assert_eq!(diagram1.styles[0].name, "validClass");

    // Test classDef with multiple style properties
    let input2 = r#"quadrantChart
    classDef complexClass fill:#ff0000 stroke:#000000 stroke-width:2px color:white"#;

    let result2 = quadrant::parse(input2);
    assert!(result2.is_ok());
    let diagram2 = result2.unwrap();

    assert_eq!(diagram2.styles.len(), 1);
    assert_eq!(diagram2.styles[0].name, "complexClass");
    assert_eq!(diagram2.styles[0].styles.len(), 4);
}

#[test]
fn test_quadrant_labels_all_variants() {
    let input = r#"quadrantChart
    quadrant-1 First Quadrant
    quadrant-2 Second Quadrant  
    quadrant-3 Third Quadrant
    quadrant-4 Fourth Quadrant"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(
        diagram.quadrants.quadrant_1,
        Some("First Quadrant".to_string())
    );
    assert_eq!(
        diagram.quadrants.quadrant_2,
        Some("Second Quadrant".to_string())
    );
    assert_eq!(
        diagram.quadrants.quadrant_3,
        Some("Third Quadrant".to_string())
    );
    assert_eq!(
        diagram.quadrants.quadrant_4,
        Some("Fourth Quadrant".to_string())
    );
}

#[test]
fn test_data_point_with_class_edge_cases() {
    // Test point with class containing special characters
    let input1 = r#"quadrantChart
    classDef my-class fill:#ff0000
    Point A:::my-class: [0.3, 0.6]"#;

    let result1 = quadrant::parse(input1);
    assert!(result1.is_ok());
    let diagram1 = result1.unwrap();

    assert_eq!(diagram1.points.len(), 1);
    assert_eq!(diagram1.points[0].class, Some("my-class".to_string()));

    // Test point with whitespace around class name
    let input2 = r#"quadrantChart
    Point A:::  spaced-class  : [0.3, 0.6]"#;

    let result2 = quadrant::parse(input2);
    assert!(result2.is_ok());
    let diagram2 = result2.unwrap();

    assert_eq!(diagram2.points.len(), 1);
    assert_eq!(diagram2.points[0].class, Some("spaced-class".to_string()));
}

#[test]
fn test_unrecognized_lines_ignored() {
    let input = r#"quadrantChart
    title Valid Chart
    some random line
    invalid directive here
    Point A: [0.3, 0.6]
    another unknown line
    unknown-quadrant Invalid Quadrant"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Should parse successfully, ignoring unrecognized lines
    assert_eq!(diagram.title, Some("Valid Chart".to_string()));
    assert_eq!(diagram.points.len(), 1);
    assert_eq!(diagram.points[0].name, "Point A");
}

#[test]
fn test_boundary_coordinates() {
    let input = r#"quadrantChart
    Origin: [0.0, 0.0]
    TopRight: [1.0, 1.0]
    Center: [0.5, 0.5]
    AlmostZero: [0.000001, 0.000001]
    AlmostOne: [0.999999, 0.999999]"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // All should be valid as they're within 0.0-1.0 range
    assert_eq!(diagram.points.len(), 5);

    // Verify specific coordinate values
    assert_eq!(diagram.points[0].x, 0.0);
    assert_eq!(diagram.points[0].y, 0.0);
    assert_eq!(diagram.points[1].x, 1.0);
    assert_eq!(diagram.points[1].y, 1.0);
}

#[test]
fn test_whitespace_in_coordinates() {
    let input = r#"quadrantChart
    Point A: [ 0.3 , 0.6 ]
    Point B: [  0.5  ,  0.7  ]"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.points.len(), 2);
    assert_eq!(diagram.points[0].x, 0.3);
    assert_eq!(diagram.points[0].y, 0.6);
    assert_eq!(diagram.points[1].x, 0.5);
    assert_eq!(diagram.points[1].y, 0.7);
}

#[test]
fn test_complex_point_names_with_special_characters() {
    let input = r#"quadrantChart
    Campaign A (2024): [0.3, 0.6]
    Point-B_test: [0.4, 0.7]
    "Quoted Point Name": [0.5, 0.8]"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.points.len(), 3);
    assert_eq!(diagram.points[0].name, "Campaign A (2024)");
    assert_eq!(diagram.points[1].name, "Point-B_test");
    assert_eq!(diagram.points[2].name, "\"Quoted Point Name\"");
}

#[test]
fn test_overlapping_coordinates() {
    let input = r#"quadrantChart
    Point A: [0.5, 0.5]
    Point B: [0.5, 0.5]
    Point C: [0.3, 0.7]"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // All points should be valid even if they overlap
    assert_eq!(diagram.points.len(), 3);
    assert_eq!(diagram.points[0].x, 0.5);
    assert_eq!(diagram.points[0].y, 0.5);
    assert_eq!(diagram.points[1].x, 0.5);
    assert_eq!(diagram.points[1].y, 0.5);
}

#[test]
fn test_empty_axis_labels() {
    let input = r#"quadrantChart
    x-axis --> High
    y-axis Low -->"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let x_axis = diagram.x_axis.unwrap();
    assert_eq!(x_axis.label_start, None);
    assert_eq!(x_axis.label_end, Some("High".to_string()));

    let y_axis = diagram.y_axis.unwrap();
    assert_eq!(y_axis.label_start, Some("Low".to_string()));
    assert_eq!(y_axis.label_end, None);
}

#[test]
fn test_comprehensive_quadrant_diagram() {
    let input = r#"quadrantChart
    title Comprehensive Test
    accTitle: Full Accessibility Title  
    accDescr: Complete accessibility description
    x-axis Low Impact --> High Impact
    y-axis Low Effort --> High Effort
    quadrant-1 Quick Wins
    quadrant-2 Major Projects
    quadrant-3 Fill-ins
    quadrant-4 Thankless Tasks
    classDef urgent fill:#ff0000 stroke:#000000
    classDef normal fill:#00ff00
    Task A:::urgent: [0.2, 0.8]
    Task B:::normal: [0.7, 0.9]
    Task C: [0.1, 0.1]
    Task D:::urgent: [0.9, 0.3]"#;

    let result = quadrant::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Verify all components are parsed correctly
    assert_eq!(diagram.title, Some("Comprehensive Test".to_string()));
    assert_eq!(
        diagram.accessibility.title,
        Some("Full Accessibility Title".to_string())
    );
    assert_eq!(
        diagram.accessibility.description,
        Some("Complete accessibility description".to_string())
    );

    assert!(diagram.x_axis.is_some());
    assert!(diagram.y_axis.is_some());

    assert_eq!(diagram.quadrants.quadrant_1, Some("Quick Wins".to_string()));
    assert_eq!(
        diagram.quadrants.quadrant_2,
        Some("Major Projects".to_string())
    );
    assert_eq!(diagram.quadrants.quadrant_3, Some("Fill-ins".to_string()));
    assert_eq!(
        diagram.quadrants.quadrant_4,
        Some("Thankless Tasks".to_string())
    );

    assert_eq!(diagram.styles.len(), 2);
    assert_eq!(diagram.points.len(), 4);

    // Verify class assignments
    assert_eq!(diagram.points[0].class, Some("urgent".to_string()));
    assert_eq!(diagram.points[1].class, Some("normal".to_string()));
    assert_eq!(diagram.points[2].class, None);
    assert_eq!(diagram.points[3].class, Some("urgent".to_string()));
}
