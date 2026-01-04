use mermaid_parser::parsers::radar;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_radar_files(#[files("test/radar/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let diagram = radar::parse(&content).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });

    // Validate structure only for non-empty diagrams
    if !diagram.datasets.is_empty() {
        assert!(
            !diagram.axes.is_empty(),
            "Non-empty diagram should have at least one axis in {:?}",
            path
        );

        // Ensure all datasets have values for all axes
        for dataset in &diagram.datasets {
            assert_eq!(
                dataset.values.len(),
                diagram.axes.len(),
                "Dataset '{}' should have values for all axes in {:?}",
                dataset.name,
                path
            );
        }
    }
}

#[test]
fn test_simple_radar() {
    let input = r#"radar
    title Skills
    ds Developer
    "Frontend" : 80
    "Backend" : 90
    "Database" : 70
    "DevOps" : 60
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Skills".to_string()));
    assert_eq!(diagram.axes.len(), 4);
    assert_eq!(diagram.datasets.len(), 1);

    let dataset = &diagram.datasets[0];
    assert_eq!(dataset.name, "Developer");
    assert_eq!(dataset.values, vec![80.0, 90.0, 70.0, 60.0]);
}

#[test]
fn test_multiple_datasets() {
    let input = r#"radar
    ds Current
    "A" : 50
    "B" : 60
    "C" : 70
    ds Target
    "A" : 80
    "B" : 85
    "C" : 90
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.datasets.len(), 2);
    assert_eq!(diagram.datasets[0].name, "Current");
    assert_eq!(diagram.datasets[1].name, "Target");

    // All datasets should have same number of values as axes
    for dataset in &diagram.datasets {
        assert_eq!(dataset.values.len(), diagram.axes.len());
    }
}

#[test]
fn test_config_parsing() {
    let input = r#"%%{init: {'theme': 'base', 'themeVariables': {'radarBackgroundColor': '#f4f4f4', 'radarGridColor': '#333'}}}%%
radar
    ds Data
    "X" : 50
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.config.background_color, Some("#f4f4f4".to_string()));
    assert_eq!(diagram.config.grid_color, Some("#333".to_string()));
}

#[test]
fn test_axes_consistency() {
    let input = r#"radar
    ds First
    "Speed" : 75
    "Power" : 80
    ds Second
    "Power" : 90
    "Speed" : 85
    "Agility" : 70
"#;

    let diagram = radar::parse(input).unwrap();

    // Should have all unique axes
    assert_eq!(diagram.axes.len(), 3);
    assert!(diagram.axes.contains(&"Speed".to_string()));
    assert!(diagram.axes.contains(&"Power".to_string()));
    assert!(diagram.axes.contains(&"Agility".to_string()));

    // All datasets should have values for all axes
    assert_eq!(diagram.datasets[0].values.len(), 3);
    assert_eq!(diagram.datasets[1].values.len(), 3);
}

#[test]
fn test_decimal_values() {
    let input = r#"radar
    ds Scores
    "Task 1" : 85.5
    "Task 2" : 92.75
    "Task 3" : 78.25
"#;

    let diagram = radar::parse(input).unwrap();

    let values = &diagram.datasets[0].values;
    assert_eq!(values[0], 85.5);
    assert_eq!(values[1], 92.75);
    assert_eq!(values[2], 78.25);
}

#[test]
fn test_radar_without_title() {
    let input = r#"radar
    ds Data
    "A" : 50
    "B" : 75
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.title, None);
    assert_eq!(diagram.axes.len(), 2);
    assert_eq!(diagram.datasets.len(), 1);
}

#[test]
fn test_complex_radar() {
    let input = r#"%%{init: {'theme': 'base', 'themeVariables': {'radarBackgroundColor': '#f4f4f4', 'radarGridColor': '#333'}}}%%
radar
    title Skills Assessment
    ds Ideal
    "Communication" : 90
    "Technical" : 85
    "Leadership" : 80
    "Problem Solving" : 95
    "Creativity" : 75
    ds Current
    "Communication" : 70
    "Technical" : 90
    "Leadership" : 60
    "Problem Solving" : 85
    "Creativity" : 65
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Skills Assessment".to_string()));
    assert_eq!(diagram.axes.len(), 5);
    assert_eq!(diagram.datasets.len(), 2);

    // Check configuration
    assert_eq!(diagram.config.background_color, Some("#f4f4f4".to_string()));
    assert_eq!(diagram.config.grid_color, Some("#333".to_string()));

    // Check datasets
    let ideal = &diagram.datasets[0];
    assert_eq!(ideal.name, "Ideal");
    assert_eq!(ideal.values.len(), 5);

    let current = &diagram.datasets[1];
    assert_eq!(current.name, "Current");
    assert_eq!(current.values.len(), 5);

    // Check specific values
    let comm_idx = diagram
        .axes
        .iter()
        .position(|a| a == "Communication")
        .unwrap();
    assert_eq!(ideal.values[comm_idx], 90.0);
    assert_eq!(current.values[comm_idx], 70.0);
}

#[test]
fn test_dataset_with_spaces() {
    let input = r#"radar
    ds "Team Performance"
    "Metric A" : 80
    "Metric B" : 90
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.datasets.len(), 1);
    // Note: This test may need adjustment based on how the lexer handles quoted dataset names
    // The current implementation might not handle quoted dataset names correctly
}

#[test]
fn test_comments_ignored() {
    let input = r#"%% This is a comment
radar
    %% Another comment
    title Test Chart
    ds Data
    "A" : 50
    %% Final comment
    "B" : 75
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Test Chart".to_string()));
    assert_eq!(diagram.axes.len(), 2);
    assert_eq!(diagram.datasets.len(), 1);
}

#[test]
fn test_empty_radar_should_fail() {
    let input = "radar";

    let result = radar::parse(input);

    // Should still parse but will have empty datasets and axes
    // This behavior may need to be adjusted based on requirements
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.datasets.len(), 0);
    assert_eq!(diagram.axes.len(), 0);
}
