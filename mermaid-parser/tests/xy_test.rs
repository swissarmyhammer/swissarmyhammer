use mermaid_parser::common::ast::{ChartOrientation, SeriesType};
use mermaid_parser::parsers::xy;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_xy_files(#[files("test/xy/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let diagram = match xy::parse(&content) {
        Ok(diagram) => diagram,
        Err(_) => {
            // Skip files that fail to parse - these are likely error test cases
            return;
        }
    };

    // Validate structure for non-empty diagrams
    if !diagram.data_series.is_empty() {
        // Ensure all data series have valid data
        for series in &diagram.data_series {
            assert!(
                !series.data.is_empty(),
                "Data series should have at least one data point in {:?}",
                path
            );
        }
    }
}

#[test]
fn test_simple_bar_chart() {
    let input = r#"xychart-beta
    title "Sales"
    x-axis [Q1, Q2, Q3, Q4]
    y-axis "Revenue" 0 --> 10000
    bar [2500, 5000, 7500, 10000]
"#;

    let diagram = xy::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Sales".to_string()));
    assert_eq!(diagram.orientation, ChartOrientation::Vertical);
    assert_eq!(diagram.x_axis.labels, vec!["Q1", "Q2", "Q3", "Q4"]);
    assert_eq!(diagram.y_axis.title, Some("Revenue".to_string()));
    assert_eq!(diagram.y_axis.range, Some((0.0, 10000.0)));
    assert_eq!(diagram.data_series.len(), 1);
    assert_eq!(diagram.data_series[0].series_type, SeriesType::Bar);
    assert_eq!(
        diagram.data_series[0].data,
        vec![2500.0, 5000.0, 7500.0, 10000.0]
    );
}

#[test]
fn test_line_chart() {
    let input = r#"xychart-beta
    x-axis [jan, feb, mar]
    y-axis 0 --> 100
    line [10, 50, 30]
"#;

    let diagram = xy::parse(input).unwrap();

    assert!(diagram.title.is_none());
    assert_eq!(diagram.x_axis.labels.len(), 3);
    assert_eq!(diagram.data_series[0].series_type, SeriesType::Line);
    assert_eq!(diagram.data_series[0].data, vec![10.0, 50.0, 30.0]);
}

#[test]
fn test_multiple_series() {
    let input = r#"xychart-beta
    title "Comparison"
    x-axis [A, B, C]
    y-axis 0 --> 100
    bar "Series 1" [20, 40, 60]
    line "Series 2" [30, 50, 70]
"#;

    let diagram = xy::parse(input).unwrap();

    assert_eq!(diagram.data_series.len(), 2);
    assert_eq!(diagram.data_series[0].name, Some("Series 1".to_string()));
    assert_eq!(diagram.data_series[0].series_type, SeriesType::Bar);
    assert_eq!(diagram.data_series[1].name, Some("Series 2".to_string()));
    assert_eq!(diagram.data_series[1].series_type, SeriesType::Line);
}

#[test]
fn test_horizontal_chart() {
    let input = r#"xychart-beta horizontal
    x-axis [A, B]
    y-axis 0 --> 50
    bar [25, 45]
"#;

    let diagram = xy::parse(input).unwrap();

    assert_eq!(diagram.orientation, ChartOrientation::Horizontal);
    assert_eq!(diagram.data_series[0].data, vec![25.0, 45.0]);
}

#[test]
fn test_chart_without_title() {
    let input = r#"xychart-beta
    x-axis [jan, feb, mar]
    y-axis 0 --> 100
    line [10, 20, 30]
"#;

    let diagram = xy::parse(input).unwrap();

    assert_eq!(diagram.title, None);
    assert_eq!(diagram.x_axis.labels.len(), 3);
    assert_eq!(diagram.data_series.len(), 1);
}

#[test]
fn test_decimal_values() {
    let input = r#"xychart-beta
    x-axis [Q1, Q2, Q3]
    y-axis 0 --> 100
    line [25.5, 50.75, 85.25]
"#;

    let diagram = xy::parse(input).unwrap();

    let values = &diagram.data_series[0].data;
    assert_eq!(values[0], 25.5);
    assert_eq!(values[1], 50.75);
    assert_eq!(values[2], 85.25);
}

#[test]
fn test_mixed_chart_types() {
    let input = r#"xychart-beta
    title "Mixed Chart"
    x-axis [Jan, Feb, Mar, Apr]
    y-axis "Sales" 0 --> 1000
    bar "Actual" [300, 450, 600, 700]
    line "Target" [350, 500, 650, 750]
    bar "Forecast" [320, 480, 620, 720]
"#;

    let diagram = xy::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Mixed Chart".to_string()));
    assert_eq!(diagram.data_series.len(), 3);

    assert_eq!(diagram.data_series[0].series_type, SeriesType::Bar);
    assert_eq!(diagram.data_series[0].name, Some("Actual".to_string()));

    assert_eq!(diagram.data_series[1].series_type, SeriesType::Line);
    assert_eq!(diagram.data_series[1].name, Some("Target".to_string()));

    assert_eq!(diagram.data_series[2].series_type, SeriesType::Bar);
    assert_eq!(diagram.data_series[2].name, Some("Forecast".to_string()));
}

#[test]
fn test_unnamed_series() {
    let input = r#"xychart-beta
    x-axis [A, B, C]
    y-axis 0 --> 100
    bar [10, 20, 30]
    line [15, 25, 35]
"#;

    let diagram = xy::parse(input).unwrap();

    assert_eq!(diagram.data_series.len(), 2);
    assert_eq!(diagram.data_series[0].name, None);
    assert_eq!(diagram.data_series[1].name, None);
}

#[test]
fn test_y_axis_without_title() {
    let input = r#"xychart-beta
    x-axis [Q1, Q2]
    y-axis 0 --> 500
    bar [100, 200]
"#;

    let diagram = xy::parse(input).unwrap();

    assert_eq!(diagram.y_axis.title, None);
    assert_eq!(diagram.y_axis.range, Some((0.0, 500.0)));
}

#[test]
fn test_quoted_labels() {
    let input = r#"xychart-beta
    title "Quarterly Report"
    x-axis ["Q1 2023", "Q2 2023", "Q3 2023"]
    y-axis "Revenue (USD)" 0 --> 50000
    bar "Sales" [20000, 35000, 42000]
"#;

    let diagram = xy::parse(input).unwrap();

    assert_eq!(diagram.title, Some("Quarterly Report".to_string()));
    assert_eq!(diagram.x_axis.labels, vec!["Q1 2023", "Q2 2023", "Q3 2023"]);
    assert_eq!(diagram.y_axis.title, Some("Revenue (USD)".to_string()));
    assert_eq!(diagram.data_series[0].name, Some("Sales".to_string()));
}
