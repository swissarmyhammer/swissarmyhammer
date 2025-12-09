use mermaid_parser::parsers::quadrant;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_quadrant_files(#[files("test/quadrant/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let _diagram = quadrant::parse(&content).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });

    // Basic validation - quadrant diagrams should parse successfully
    // Note: some test files might be empty or incomplete, so we'll just ensure parsing succeeds
}

#[test]
fn test_simple_quadrant() {
    let input = r#"quadrantChart
    title Reach and influence
    x-axis Low Reach --> High Reach
    y-axis Low Influence --> High Influence
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.80]
    Campaign C: [0.57, 0.95]
"#;

    let diagram = quadrant::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Reach and influence".to_string()));
    assert_eq!(
        diagram.quadrants.quadrant_1,
        Some("We should expand".to_string())
    );
    assert_eq!(
        diagram.quadrants.quadrant_2,
        Some("Need to promote".to_string())
    );
    assert_eq!(
        diagram.quadrants.quadrant_3,
        Some("Re-evaluate".to_string())
    );
    assert_eq!(
        diagram.quadrants.quadrant_4,
        Some("May be improved".to_string())
    );
    assert_eq!(diagram.points.len(), 3);
    assert_eq!(diagram.points[0].name, "Campaign A");
    assert_eq!(diagram.points[0].x, 0.3);
    assert_eq!(diagram.points[0].y, 0.6);
    assert_eq!(diagram.points[1].name, "Campaign B");
    assert_eq!(diagram.points[1].x, 0.45);
    assert_eq!(diagram.points[1].y, 0.80);
    assert_eq!(diagram.points[2].name, "Campaign C");
    assert_eq!(diagram.points[2].x, 0.57);
    assert_eq!(diagram.points[2].y, 0.95);
}

#[test]
fn test_axis_definitions() {
    let input = r#"quadrantChart
    x-axis Low --> High
    y-axis Bad --> Good
"#;

    let diagram = quadrant::parse(input).unwrap();

    assert!(diagram.x_axis.is_some());
    assert!(diagram.y_axis.is_some());

    let x_axis = diagram.x_axis.unwrap();
    assert_eq!(x_axis.label_start, Some("Low".to_string()));
    assert_eq!(x_axis.label_end, Some("High".to_string()));

    let y_axis = diagram.y_axis.unwrap();
    assert_eq!(y_axis.label_start, Some("Bad".to_string()));
    assert_eq!(y_axis.label_end, Some("Good".to_string()));
}

#[test]
fn test_coordinate_validation() {
    // Valid coordinates
    let input = r#"quadrantChart
    Point A: [0.0, 0.0]
    Point B: [1.0, 1.0]
    Point C: [0.5, 0.5]
"#;

    let diagram = quadrant::parse(input).unwrap();
    assert_eq!(diagram.points.len(), 3);

    // Invalid coordinates should be ignored
    let input_invalid = r#"quadrantChart
    Point A: [0.3, 0.6]
    Point B: [-0.1, 0.5]
    Point C: [0.5, 1.1]
    Point D: [1.5, 0.5]
"#;

    let diagram_invalid = quadrant::parse(input_invalid).unwrap();
    // Only Point A should be valid
    assert_eq!(diagram_invalid.points.len(), 1);
    assert_eq!(diagram_invalid.points[0].name, "Point A");
}

#[test]
fn test_accessibility_attributes() {
    let input = r#"quadrantChart
    title My Chart
    accTitle: Accessibility Title
    accDescr: This is an accessible description
    Point A: [0.3, 0.6]
"#;

    let diagram = quadrant::parse(input).unwrap();

    assert_eq!(diagram.title, Some("My Chart".to_string()));
    assert_eq!(
        diagram.accessibility.title,
        Some("Accessibility Title".to_string())
    );
    assert_eq!(
        diagram.accessibility.description,
        Some("This is an accessible description".to_string())
    );
}

#[test]
fn test_class_definitions() {
    let input = r#"quadrantChart
    classDef myClass fill:#ff0000 stroke:#000000
    classDef otherClass color:blue
    Point A: [0.3, 0.6]
"#;

    let diagram = quadrant::parse(input).unwrap();

    assert_eq!(diagram.styles.len(), 2);
    assert_eq!(diagram.styles[0].name, "myClass");
    assert_eq!(diagram.styles[0].styles.len(), 2);
    assert_eq!(diagram.styles[1].name, "otherClass");
    assert_eq!(diagram.styles[1].styles.len(), 1);
}

#[test]
fn test_minimal_quadrant() {
    let input = r#"quadrantChart
    Point A: [0.5, 0.5]
"#;

    let diagram = quadrant::parse(input).unwrap();

    assert_eq!(diagram.title, None);
    assert_eq!(diagram.x_axis, None);
    assert_eq!(diagram.y_axis, None);
    assert_eq!(diagram.quadrants.quadrant_1, None);
    assert_eq!(diagram.quadrants.quadrant_2, None);
    assert_eq!(diagram.quadrants.quadrant_3, None);
    assert_eq!(diagram.quadrants.quadrant_4, None);
    assert_eq!(diagram.points.len(), 1);
    assert_eq!(diagram.points[0].name, "Point A");
    assert_eq!(diagram.styles.len(), 0);
}

#[test]
fn test_empty_quadrant() {
    let input = r#"quadrantChart
    title Empty Chart
"#;

    let diagram = quadrant::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Empty Chart".to_string()));
    assert_eq!(diagram.points.len(), 0);
}

#[test]
fn test_data_point_with_class() {
    let input = r#"quadrantChart
    classDef important fill:#ff0000
    Point A:::important: [0.3, 0.6]
"#;

    let diagram = quadrant::parse(input).unwrap();

    assert_eq!(diagram.points.len(), 1);
    assert_eq!(diagram.points[0].name, "Point A");
    assert_eq!(diagram.points[0].class, Some("important".to_string()));
    assert_eq!(diagram.styles.len(), 1);
    assert_eq!(diagram.styles[0].name, "important");
}
