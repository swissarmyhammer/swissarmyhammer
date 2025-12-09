use mermaid_parser::error::ParseError;
use mermaid_parser::parsers::radar;

#[test]
fn test_empty_input() {
    let input = "";
    let result = radar::parse(input);

    assert!(result.is_err());
    match result {
        Err(ParseError::EmptyInput) => {
            // Expected error
        }
        _ => panic!("Expected EmptyInput error"),
    }
}

#[test]
fn test_accessibility_title() {
    let input = r#"radar
    accTitle: Chart for Screen Readers
    ds Data
    "A" : 50
    "B" : 75
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(
        diagram.accessibility.title,
        Some("Chart for Screen Readers".to_string())
    );
}

#[test]
fn test_accessibility_description() {
    let input = r#"radar
    accDescr: This chart shows performance metrics across different categories
    ds Performance
    "Speed" : 80
    "Quality" : 90
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(
        diagram.accessibility.description,
        Some("This chart shows performance metrics across different categories".to_string())
    );
}

#[test]
fn test_accessibility_multiline_description() {
    let input = r#"radar
    accDescr {
        This is a multi-line description
        that spans multiple lines
        to provide detailed accessibility information
    }
    ds Data
    "X" : 50
    "Y" : 60
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(
        diagram.accessibility.description,
        Some("This is a multi-line description that spans multiple lines to provide detailed accessibility information".to_string())
    );
}

#[test]
fn test_accessibility_multiline_description_empty() {
    let input = r#"radar
    accDescr {
    }
    ds Data
    "X" : 50
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.accessibility.description, Some("".to_string()));
}

#[test]
fn test_invalid_numeric_value() {
    let input = r#"radar
    ds Data
    "A" : invalid_number
    "B" : 75
"#;

    let result = radar::parse(input);

    assert!(result.is_err());
    match result {
        Err(ParseError::SyntaxError { message, .. }) => {
            assert!(message.contains("Invalid number"));
        }
        _ => panic!("Expected SyntaxError for invalid number"),
    }
}

#[test]
fn test_non_radar_file() {
    let input = r#"graph TD
    A --> B
    B --> C
"#;

    let result = radar::parse(input);

    // Should return empty diagram for non-radar content
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.datasets.len(), 0);
    assert_eq!(diagram.axes.len(), 0);
}

#[test]
fn test_axis_without_quotes() {
    let input = r#"radar
    ds Test
    Frontend : 80
    Backend : 90
    Database : 70
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.axes.len(), 3);
    assert!(diagram.axes.contains(&"Frontend".to_string()));
    assert!(diagram.axes.contains(&"Backend".to_string()));
    assert!(diagram.axes.contains(&"Database".to_string()));

    let dataset = &diagram.datasets[0];
    assert_eq!(dataset.values.len(), 3);
}

#[test]
fn test_double_slash_comments() {
    let input = r#"// This is a comment at the beginning
radar
    // Comment after radar
    title Test
    // Comment before dataset
    ds Data
    // Comment in dataset
    "A" : 50
    // Final comment
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Test".to_string()));
    assert_eq!(diagram.datasets.len(), 1);
    assert_eq!(diagram.axes.len(), 1);
}

#[test]
fn test_dataset_with_missing_values() {
    let input = r#"radar
    ds Dataset1
    "A" : 50
    "B" : 60
    "C" : 70
    ds Dataset2
    "A" : 80
    "C" : 90
"#;

    let diagram = radar::parse(input).unwrap();

    // All datasets should have values for all axes
    assert_eq!(diagram.axes.len(), 3);
    assert_eq!(diagram.datasets[0].values.len(), 3);
    assert_eq!(diagram.datasets[1].values.len(), 3);

    // Dataset2 should have 0.0 for missing "B" axis
    let b_index = diagram.axes.iter().position(|a| a == "B").unwrap();
    assert_eq!(diagram.datasets[1].values[b_index], 0.0);
}

#[test]
fn test_config_with_single_theme_variable() {
    let input = r#"%%{init: {'themeVariables': {'radarBackgroundColor': '#ffffff'}}}%%
radar
    ds Data
    "X" : 50
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.config.background_color, Some("#ffffff".to_string()));
    assert_eq!(diagram.config.grid_color, None);
}

#[test]
fn test_config_with_malformed_quotes() {
    // Test various quote scenarios that should be handled gracefully
    let input = r#"%%{init: {'themeVariables': {'radarGridColor': '#abc'}}}%%
radar
    ds Data
    "X" : 50
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.config.grid_color, Some("#abc".to_string()));
}

#[test]
fn test_empty_lines_ignored() {
    let input = r#"radar

    title Test Chart

    ds Data

    "A" : 50

    "B" : 75

"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Test Chart".to_string()));
    assert_eq!(diagram.axes.len(), 2);
    assert_eq!(diagram.datasets.len(), 1);
}

#[test]
fn test_line_with_only_whitespace() {
    let input = "radar\n    \n    ds Data\n    \"A\" : 50\n        \n    \"B\" : 75";

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.datasets.len(), 1);
    assert_eq!(diagram.axes.len(), 2);
}

#[test]
fn test_zero_values() {
    let input = r#"radar
    ds Performance
    "Task 1" : 0
    "Task 2" : 0.0
    "Task 3" : 100
"#;

    let diagram = radar::parse(input).unwrap();

    let values = &diagram.datasets[0].values;
    assert_eq!(values[0], 0.0);
    assert_eq!(values[1], 0.0);
    assert_eq!(values[2], 100.0);
}

#[test]
fn test_negative_values() {
    let input = r#"radar
    ds Delta
    "Metric A" : -10.5
    "Metric B" : 25.0
    "Metric C" : -5
"#;

    let diagram = radar::parse(input).unwrap();

    let values = &diagram.datasets[0].values;
    assert_eq!(values[0], -10.5);
    assert_eq!(values[1], 25.0);
    assert_eq!(values[2], -5.0);
}

#[test]
fn test_axis_value_without_colon() {
    let input = r#"radar
    ds Data
    "A" 50
    "B" : 75
"#;

    let diagram = radar::parse(input).unwrap();

    // Line without colon should be ignored
    assert_eq!(diagram.axes.len(), 1);
    assert_eq!(diagram.axes[0], "B");
}

#[test]
fn test_multiple_colons_in_line() {
    let input = r#"radar
    ds Data
    "Time: 10:30" : 50
    "Ratio 1:2" : 75
"#;

    let result = radar::parse(input);

    // The parser takes the first colon as separator, so "Time: 10:30" : 50
    // becomes axis="Time" and tries to parse "10:30\" : 50" as a number, which fails
    assert!(result.is_err());
}

#[test]
fn test_dataset_normalization_edge_cases() {
    let input = r#"radar
    ds Empty
    ds OnlyOne
    "X" : 100
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.datasets.len(), 2);

    // Empty dataset should have one value (0.0) for the single axis
    assert_eq!(diagram.datasets[0].values.len(), 1);
    assert_eq!(diagram.datasets[0].values[0], 0.0);

    // OnlyOne dataset should have the provided value
    assert_eq!(diagram.datasets[1].values.len(), 1);
    assert_eq!(diagram.datasets[1].values[0], 100.0);
}

#[test]
fn test_config_without_theme_variables() {
    let input = r#"%%{init: {'theme': 'dark'}}%%
radar
    ds Data
    "X" : 50
"#;

    let diagram = radar::parse(input).unwrap();

    // Config should remain at defaults
    assert_eq!(diagram.config.background_color, None);
    assert_eq!(diagram.config.grid_color, None);
}

#[test]
fn test_very_large_values() {
    let input = r#"radar
    ds BigNumbers
    "A" : 1000000
    "B" : 1e6
    "C" : 0.000001
"#;

    let diagram = radar::parse(input).unwrap();

    let values = &diagram.datasets[0].values;
    assert_eq!(values[0], 1000000.0);
    assert_eq!(values[1], 1000000.0);
    assert_eq!(values[2], 0.000001);
}

#[test]
fn test_axis_ordering_preserved() {
    let input = r#"radar
    ds First
    "Z" : 1
    "A" : 2
    "M" : 3
    ds Second  
    "A" : 4
    "Z" : 5
    "B" : 6
"#;

    let diagram = radar::parse(input).unwrap();

    // Axes should be in order of first appearance
    assert_eq!(diagram.axes[0], "Z");
    assert_eq!(diagram.axes[1], "A");
    assert_eq!(diagram.axes[2], "M");
    assert_eq!(diagram.axes[3], "B");
}

#[test]
fn test_mixed_quote_styles() {
    let input = r#"radar
    ds Data
    "Double" : 50
    'Single' : 60
    Unquoted : 70
"#;

    let diagram = radar::parse(input).unwrap();

    // Parser handles all three styles - single quotes are treated as part of the axis name
    assert_eq!(diagram.axes.len(), 3);
    assert!(diagram.axes.contains(&"Double".to_string()));
    assert!(diagram.axes.contains(&"'Single'".to_string())); // Single quotes are kept as part of the name
    assert!(diagram.axes.contains(&"Unquoted".to_string()));
}

#[test]
fn test_whitespace_in_axis_names() {
    let input = r#"radar
    ds Metrics
    "  Leading Spaces" : 10
    "Trailing Spaces  " : 20
    "  Both  " : 30
    "Multiple   Internal   Spaces" : 40
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.axes.len(), 4);
    // Axis names should preserve internal structure
    assert!(diagram.axes.contains(&"  Leading Spaces".to_string()));
    assert!(diagram.axes.contains(&"Trailing Spaces  ".to_string()));
    assert!(diagram.axes.contains(&"  Both  ".to_string()));
    assert!(diagram
        .axes
        .contains(&"Multiple   Internal   Spaces".to_string()));
}

#[test]
fn test_special_characters_in_axis_names() {
    let input = r#"radar
    ds Test
    "A/B Testing" : 50
    "Cost ($)" : 60
    "Growth (%)" : 70
    "C++ Skills" : 80
    "Node.js" : 90
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.axes.len(), 5);
    assert!(diagram.axes.contains(&"A/B Testing".to_string()));
    assert!(diagram.axes.contains(&"Cost ($)".to_string()));
    assert!(diagram.axes.contains(&"Growth (%)".to_string()));
    assert!(diagram.axes.contains(&"C++ Skills".to_string()));
    assert!(diagram.axes.contains(&"Node.js".to_string()));
}

#[test]
fn test_unicode_in_content() {
    let input = r#"radar
    title 技能评估
    ds 当前水平
    "编程" : 85
    "设计" : 70
    "沟通" : 90
"#;

    let diagram = radar::parse(input).unwrap();

    assert_eq!(diagram.title, Some("技能评估".to_string()));
    assert_eq!(diagram.datasets[0].name, "当前水平");
    assert!(diagram.axes.contains(&"编程".to_string()));
    assert!(diagram.axes.contains(&"设计".to_string()));
    assert!(diagram.axes.contains(&"沟通".to_string()));
}

#[test]
fn test_extract_quoted_value_edge_cases() {
    // Test config parsing with edge cases
    let input1 = r#"%%{init: {'themeVariables': {'radarBackgroundColor': ''}}}%%
radar
    ds Data
    "X" : 50
"#;

    let diagram1 = radar::parse(input1).unwrap();
    assert_eq!(diagram1.config.background_color, Some("".to_string()));

    // Config with no closing quote should not extract value
    let input2 = r#"%%{init: {'themeVariables': {'radarGridColor': '#abc}}}%%
radar  
    ds Data
    "X" : 50
"#;

    let diagram2 = radar::parse(input2).unwrap();
    assert_eq!(diagram2.config.grid_color, None);
}

#[test]
fn test_radar_only_line() {
    let input = r#"radar
"#;

    let result = radar::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.datasets.len(), 0);
    assert_eq!(diagram.axes.len(), 0);
}

#[test]
fn test_title_with_special_characters() {
    let input = r#"radar
    title Skills & Competencies (2024) - Q1
    ds Data
    "A" : 50
"#;

    let diagram = radar::parse(input).unwrap();
    assert_eq!(
        diagram.title,
        Some("Skills & Competencies (2024) - Q1".to_string())
    );
}

#[test]
fn test_empty_dataset_name() {
    // Test what actually happens with "ds" (no space after)
    let input = "radar\n    ds\n    \"A\" : 50\n    \"B\" : 60\n";

    let diagram = radar::parse(input).unwrap();

    // The line "ds" without trailing space doesn't match "ds " so no dataset is created
    // and subsequent axis-value pairs are ignored
    assert_eq!(diagram.datasets.len(), 0);
    assert_eq!(diagram.axes.len(), 0);
}

#[test]
fn test_values_in_different_formats() {
    let input = r#"radar
    ds Numbers
    "Integer" : 42
    "Decimal" : 42.0
    "Scientific" : 4.2e1
    "Leading Zero" : 042
"#;

    let diagram = radar::parse(input).unwrap();

    let values = &diagram.datasets[0].values;
    assert_eq!(values[0], 42.0);
    assert_eq!(values[1], 42.0);
    assert_eq!(values[2], 42.0);
    assert_eq!(values[3], 42.0);
}

#[test]
fn test_dataset_declaration_without_space() {
    // Test that "ds" (without space) doesn't create a new dataset
    let input = r#"radar
    ds DatasetOne
    "A" : 10
    ds
    "B" : 20
"#;

    let diagram = radar::parse(input).unwrap();

    // "ds" without space doesn't match "ds " so it doesn't create a new dataset
    // The "B" value is added to the current dataset (DatasetOne)
    assert_eq!(diagram.datasets.len(), 1);
    assert_eq!(diagram.datasets[0].name, "DatasetOne");
    assert_eq!(diagram.axes.len(), 2);
    assert_eq!(diagram.datasets[0].values, vec![10.0, 20.0]);
}

#[test]
fn test_title_with_only_whitespace() {
    let input = r#"radar
    title    
    ds Data
    "X" : 50
"#;

    let diagram = radar::parse(input).unwrap();

    // Title with only whitespace becomes None since trim() produces empty string
    assert_eq!(diagram.title, None);
}

#[test]
fn test_edge_case_combinations() {
    // Test multiple edge cases in one diagram
    let input = r#"%%{init: {'themeVariables': {'radarBackgroundColor': '#fff', 'radarGridColor': '#000'}}}%%
// File comment
radar
    %% Diagram comment
    title Empty Title
    accTitle: Accessibility Title
    accDescr: Single line description
    accDescr {
        This multiline description
        should be ignored because we already have one
    }
    ds Dataset One
    Unquoted : 100
    "Quoted" : 200
    ds Empty Dataset
    "Third Axis" : 50
"#;

    let diagram = radar::parse(input).unwrap();

    // Check various aspects
    assert_eq!(diagram.title, Some("Empty Title".to_string()));
    assert_eq!(
        diagram.accessibility.title,
        Some("Accessibility Title".to_string())
    );
    // The multiline description replaces the single line one
    let expected_desc = "This multiline description should be ignored because we already have one";
    assert_eq!(
        diagram.accessibility.description,
        Some(expected_desc.to_string())
    );
    assert_eq!(diagram.config.background_color, Some("#fff".to_string()));
    assert_eq!(diagram.config.grid_color, Some("#000".to_string()));
    assert_eq!(diagram.datasets.len(), 2); // "Dataset One" and "Empty Dataset"
    assert_eq!(diagram.axes.len(), 3); // Unquoted, Quoted, and "Third Axis"

    // Verify the datasets
    assert_eq!(diagram.datasets[0].name, "Dataset One");
    assert_eq!(diagram.datasets[1].name, "Empty Dataset");
}
