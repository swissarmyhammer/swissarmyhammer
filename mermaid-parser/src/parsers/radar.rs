//! Radar chart parser implementation
//!
//! This module handles parsing of Mermaid radar chart diagrams, which display
//! multivariate data on axes starting from the same point.

use crate::common::ast::{AccessibilityInfo, Dataset, RadarConfig, RadarDiagram};
use crate::common::parser_utils::validate_diagram_header;
use crate::error::{ParseError, Result};
use std::collections::HashMap;

/// Simple string-based parser for radar diagrams
pub fn parse(input: &str) -> Result<RadarDiagram> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut diagram = RadarDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        config: RadarConfig::default(),
        axes: Vec::new(),
        datasets: Vec::new(),
    };

    let mut first_line_processed = false;
    let mut current_dataset: Option<(String, HashMap<String, f64>)> = None;
    let mut in_multiline_acc_descr = false;
    let mut multiline_content = Vec::new();

    for line in lines.iter() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Handle comments and configuration
        if trimmed.starts_with("//") {
            continue;
        }

        // Handle configuration blocks
        if trimmed.starts_with("%%{init:") && trimmed.ends_with("}%%") {
            parse_config_line(trimmed, &mut diagram.config);
            continue;
        }

        // Handle regular comments
        if trimmed.starts_with("%%") {
            continue;
        }

        // Handle accessibility multi-line descriptors
        if trimmed == "accDescr {" {
            in_multiline_acc_descr = true;
            multiline_content.clear();
            continue;
        }

        if in_multiline_acc_descr {
            if trimmed == "}" {
                in_multiline_acc_descr = false;
                diagram.accessibility.description = Some(multiline_content.join(" "));
                multiline_content.clear();
            } else {
                multiline_content.push(trimmed.to_string());
            }
            continue;
        }

        // Use shared header validation utility
        match validate_diagram_header(line, 0, &["radar"], &mut first_line_processed) {
            Ok((true, _)) => continue, // Header was handled, skip to next line
            Ok((false, _)) => {}       // Line should be processed by parser
            Err(_) => {
                // For lenient parsing, skip files that don't start with radar
                return Ok(diagram);
            }
        }

        // Handle title directive
        if trimmed.starts_with("title ") {
            let title = trimmed.strip_prefix("title ").unwrap().trim();
            diagram.title = Some(title.to_string());
            continue;
        }

        // Handle accessibility directives
        if trimmed.starts_with("accTitle:") {
            let title = trimmed.strip_prefix("accTitle:").unwrap().trim();
            diagram.accessibility.title = Some(title.to_string());
            continue;
        }

        if trimmed.starts_with("accDescr:") {
            let desc = trimmed.strip_prefix("accDescr:").unwrap().trim();
            diagram.accessibility.description = Some(desc.to_string());
            continue;
        }

        // Handle dataset declarations
        if trimmed.starts_with("ds ") {
            // Save previous dataset if exists
            if let Some((name, values)) = current_dataset.take() {
                let dataset = create_dataset(name, values, &diagram.axes);
                diagram.datasets.push(dataset);
            }

            // Start new dataset
            let dataset_name = trimmed.strip_prefix("ds ").unwrap().trim();
            current_dataset = Some((dataset_name.to_string(), HashMap::new()));
            continue;
        }

        // Handle axis-value pairs
        if let Some((_, ref mut values)) = current_dataset {
            if let Some((axis, value)) = parse_axis_value(trimmed)? {
                // Add axis to global list if not already present
                if !diagram.axes.contains(&axis) {
                    diagram.axes.push(axis.clone());
                }
                values.insert(axis, value);
            }
        }
    }

    // Save final dataset
    if let Some((name, values)) = current_dataset {
        let dataset = create_dataset(name, values, &diagram.axes);
        diagram.datasets.push(dataset);
    }

    // Ensure all datasets have values for all axes
    normalize_datasets(&mut diagram);

    Ok(diagram)
}

fn parse_config_line(line: &str, config: &mut RadarConfig) {
    // Extract content between %%{init: and }%%
    if let Some(content) = line
        .strip_prefix("%%{init:")
        .and_then(|s| s.strip_suffix("}%%"))
    {
        parse_config_content(content.trim(), config);
    }
}

fn parse_config_content(content: &str, config: &mut RadarConfig) {
    // Simple parsing for theme variables
    if content.contains("radarBackgroundColor") {
        if let Some(color) = extract_quoted_value(content, "radarBackgroundColor") {
            config.background_color = Some(color);
        }
    }
    if content.contains("radarGridColor") {
        if let Some(color) = extract_quoted_value(content, "radarGridColor") {
            config.grid_color = Some(color);
        }
    }
}

fn extract_quoted_value(content: &str, key: &str) -> Option<String> {
    content.find(key).and_then(|pos| {
        let after_key = &content[pos + key.len()..];
        // Look for colon first, then the quoted value
        after_key.find(':').and_then(|colon_pos| {
            let after_colon = &after_key[colon_pos + 1..];
            // Find the opening quote
            after_colon.find("'").and_then(|start| {
                let value_start = start + 1;
                after_colon[value_start..]
                    .find("'")
                    .map(|end| after_colon[value_start..value_start + end].to_string())
            })
        })
    })
}

fn parse_axis_value(line: &str) -> Result<Option<(String, f64)>> {
    // Look for pattern: "axis name" : value
    if let Some(colon_pos) = line.find(':') {
        let axis_part = line[..colon_pos].trim();
        let value_part = line[colon_pos + 1..].trim();

        // Extract axis name (remove quotes if present)
        let axis_name = if axis_part.starts_with('"') && axis_part.ends_with('"') {
            axis_part[1..axis_part.len() - 1].to_string()
        } else {
            axis_part.to_string()
        };

        // Parse value
        match value_part.parse::<f64>() {
            Ok(value) => Ok(Some((axis_name, value))),
            Err(_) => Err(ParseError::SyntaxError {
                message: format!("Invalid number: {}", value_part),
                expected: vec!["number".to_string()],
                found: value_part.to_string(),
                line: 0, // We don't track line numbers in this simple parser
                column: 0,
            }),
        }
    } else {
        Ok(None)
    }
}

fn create_dataset(name: String, values: HashMap<String, f64>, axes: &[String]) -> Dataset {
    // Create values vector in the order of axes
    let mut dataset_values = Vec::new();
    for axis in axes.iter() {
        dataset_values.push(values.get(axis).copied().unwrap_or(0.0));
    }

    Dataset {
        name,
        values: dataset_values,
    }
}

fn normalize_datasets(diagram: &mut RadarDiagram) {
    // Ensure all datasets have values for all axes
    for dataset in &mut diagram.datasets {
        while dataset.values.len() < diagram.axes.len() {
            dataset.values.push(0.0);
        }
        dataset.values.truncate(diagram.axes.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let diagram = parse(input).unwrap();

        assert_eq!(diagram.title, Some("Skills".to_string()));
        assert_eq!(diagram.axes.len(), 4);
        assert_eq!(diagram.datasets.len(), 1);

        let dataset = &diagram.datasets[0];
        assert_eq!(dataset.name, "Developer");
        assert_eq!(dataset.values.len(), 4);
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

        let diagram = parse(input).unwrap();

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

        let diagram = parse(input).unwrap();

        assert_eq!(diagram.config.background_color, Some("#f4f4f4".to_string()));
        assert_eq!(diagram.config.grid_color, Some("#333".to_string()));
    }

    #[test]
    fn test_decimal_values() {
        let input = r#"radar
    ds Scores
    "Task 1" : 85.5
    "Task 2" : 92.75
    "Task 3" : 78.25
"#;

        let diagram = parse(input).unwrap();

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

        let diagram = parse(input).unwrap();

        assert_eq!(diagram.title, None);
        assert_eq!(diagram.axes.len(), 2);
        assert_eq!(diagram.datasets.len(), 1);
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

        let diagram = parse(input).unwrap();

        assert_eq!(diagram.title, Some("Test Chart".to_string()));
        assert_eq!(diagram.axes.len(), 2);
        assert_eq!(diagram.datasets.len(), 1);
    }
}
