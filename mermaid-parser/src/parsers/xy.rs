//! XY chart diagram parser
//!
//! Parses XY charts with line and bar data visualization,
//! configurable axes and data series.

use crate::common::ast::{
    AccessibilityInfo, ChartOrientation, DataSeries, SeriesType, XAxis, XyChartDiagram, YAxis,
};
use crate::common::parser_utils::validate_diagram_header;
use crate::error::{ParseError, Result};

/// Simple string-based parser for XY chart diagrams
pub fn parse(input: &str) -> Result<XyChartDiagram> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut diagram = XyChartDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        orientation: ChartOrientation::Vertical,
        x_axis: XAxis {
            title: None,
            labels: Vec::new(),
            range: None,
        },
        y_axis: YAxis {
            title: None,
            range: None,
        },
        data_series: Vec::new(),
    };

    let mut first_line_processed = false;

    for (line_num, line) in lines.iter().enumerate() {
        // Use shared header validation utility
        match validate_diagram_header(
            line,
            line_num,
            &["xychart", "xychart-beta"],
            &mut first_line_processed,
        ) {
            Ok((true, trimmed)) => {
                // Check for horizontal orientation in header
                if trimmed.contains("horizontal") {
                    diagram.orientation = ChartOrientation::Horizontal;
                }
                continue;
            }
            Ok((false, _trimmed)) => {
                // Line should be processed by parser - trimmed is now available
            }
            Err(_) => {
                return Ok(diagram); // Lenient parsing
            }
        }

        let trimmed = line.trim();

        // Handle title directive
        if trimmed.starts_with("title ") {
            let title_part = trimmed.strip_prefix("title ").unwrap().trim();
            diagram.title = Some(unquote_string(title_part));
            continue;
        }

        // Handle x-axis directive
        if trimmed.starts_with("x-axis ") {
            parse_x_axis(trimmed, &mut diagram.x_axis)?;
            continue;
        }

        // Handle y-axis directive
        if trimmed.starts_with("y-axis ") {
            parse_y_axis(trimmed, &mut diagram.y_axis)?;
            continue;
        }

        // Handle bar series
        if trimmed.starts_with("bar ") {
            let series = parse_data_series(trimmed, SeriesType::Bar)?;
            diagram.data_series.push(series);
            continue;
        }

        // Handle line series
        if trimmed.starts_with("line ") {
            let series = parse_data_series(trimmed, SeriesType::Line)?;
            diagram.data_series.push(series);
        }
    }

    Ok(diagram)
}

fn parse_x_axis(line: &str, x_axis: &mut XAxis) -> Result<()> {
    let content = line.strip_prefix("x-axis ").unwrap().trim();

    // Check if it starts with a quoted string (title)
    if let Some(stripped) = content.strip_prefix('"') {
        if let Some(quote_end) = stripped.find('"') {
            x_axis.title = Some(stripped[..quote_end].to_string());
            let remaining = content[quote_end + 2..].trim();

            if remaining.starts_with('[') {
                x_axis.labels = parse_label_array(remaining)?;
            } else if remaining.contains("-->") {
                x_axis.range = parse_range(remaining)?;
            }
        }
    } else if content.starts_with('[') {
        // Just labels, no title
        x_axis.labels = parse_label_array(content)?;
    } else if content.contains("-->") {
        // Could be "name range" or just "range"
        let arrow_pos = content.find("-->").unwrap();
        let before_arrow = content[..arrow_pos].trim();

        // Check if there are multiple words before the arrow
        let parts: Vec<&str> = before_arrow.split_whitespace().collect();
        if parts.len() > 1 {
            // First part is the title, rest is the start of range
            x_axis.title = Some(parts[0].to_string());
            let range_part = &content[parts[0].len()..];
            x_axis.range = parse_range(range_part.trim())?;
        } else {
            // Just a range
            x_axis.range = parse_range(content)?;
        }
    } else {
        // It's an unquoted title
        x_axis.title = Some(content.to_string());
    }

    Ok(())
}

fn parse_y_axis(line: &str, y_axis: &mut YAxis) -> Result<()> {
    let content = line.strip_prefix("y-axis ").unwrap().trim();

    // Check if it starts with a quoted string (title)
    if let Some(stripped) = content.strip_prefix('"') {
        if let Some(quote_end) = stripped.find('"') {
            y_axis.title = Some(stripped[..quote_end].to_string());
            let remaining = content[quote_end + 2..].trim();

            if remaining.contains("-->") {
                y_axis.range = parse_range(remaining)?;
            }
        }
    } else if content.contains("-->") {
        // Could be "name range" or just "range"
        let arrow_pos = content.find("-->").unwrap();
        let before_arrow = content[..arrow_pos].trim();

        // Check if there are multiple words before the arrow
        let parts: Vec<&str> = before_arrow.split_whitespace().collect();
        if parts.len() > 1 {
            // First part is the title, rest is the start of range
            y_axis.title = Some(parts[0].to_string());
            let range_part = &content[parts[0].len()..];
            y_axis.range = parse_range(range_part.trim())?;
        } else {
            // Just a range
            y_axis.range = parse_range(content)?;
        }
    } else {
        // Unquoted title
        y_axis.title = Some(content.to_string());
    }

    Ok(())
}

fn parse_data_series(line: &str, series_type: SeriesType) -> Result<DataSeries> {
    let type_str = match series_type {
        SeriesType::Bar => "bar ",
        SeriesType::Line => "line ",
    };

    let content = line.strip_prefix(type_str).unwrap().trim();

    let (name, data_part) = if let Some(stripped) = content.strip_prefix('"') {
        // Has a quoted name
        if let Some(quote_end) = stripped.find('"') {
            let name = Some(stripped[..quote_end].to_string());
            let remaining = content[quote_end + 2..].trim();
            (name, remaining)
        } else {
            (None, content)
        }
    } else if content.starts_with('[') {
        // No name, just data
        (None, content)
    } else {
        // Unquoted name followed by data
        if let Some(bracket_pos) = content.find('[') {
            let name = Some(content[..bracket_pos].trim().to_string());
            let data_part = &content[bracket_pos..];
            (name, data_part)
        } else {
            (None, content)
        }
    };

    let data = parse_data_array(data_part)?;

    Ok(DataSeries {
        series_type,
        name,
        data,
    })
}

fn parse_label_array(content: &str) -> Result<Vec<String>> {
    if !content.starts_with('[') || !content.ends_with(']') {
        return Err(ParseError::SyntaxError {
            message: "Expected array format [item1, item2, ...]".to_string(),
            expected: vec!["[".to_string()],
            found: content.to_string(),
            line: 0,
            column: 0,
        });
    }

    let inner = &content[1..content.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }

    let labels = inner.split(',').map(|s| unquote_string(s.trim())).collect();

    Ok(labels)
}

fn parse_data_array(content: &str) -> Result<Vec<f64>> {
    if !content.starts_with('[') || !content.ends_with(']') {
        return Err(ParseError::SyntaxError {
            message: "Expected array format [num1, num2, ...]".to_string(),
            expected: vec!["[".to_string()],
            found: content.to_string(),
            line: 0,
            column: 0,
        });
    }

    let inner = &content[1..content.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut data = Vec::new();
    for item in inner.split(',') {
        let trimmed = item.trim();
        if !trimmed.is_empty() {
            let value = trimmed
                .parse::<f64>()
                .map_err(|_| ParseError::SyntaxError {
                    message: format!("Invalid number: {}", trimmed),
                    expected: vec!["number".to_string()],
                    found: trimmed.to_string(),
                    line: 0,
                    column: 0,
                })?;
            data.push(value);
        }
    }

    Ok(data)
}

fn parse_range(content: &str) -> Result<Option<(f64, f64)>> {
    if let Some(arrow_pos) = content.find("-->") {
        let start_str = content[..arrow_pos].trim();
        let end_str = content[arrow_pos + 3..].trim();

        let start = start_str
            .parse::<f64>()
            .map_err(|_| ParseError::SyntaxError {
                message: format!("Invalid start number: {}", start_str),
                expected: vec!["number".to_string()],
                found: start_str.to_string(),
                line: 0,
                column: 0,
            })?;

        let end = end_str
            .parse::<f64>()
            .map_err(|_| ParseError::SyntaxError {
                message: format!("Invalid end number: {}", end_str),
                expected: vec!["number".to_string()],
                found: end_str.to_string(),
                line: 0,
                column: 0,
            })?;

        Ok(Some((start, end)))
    } else {
        Ok(None)
    }
}

fn unquote_string(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_bar_chart() {
        let input = r#"xychart-beta
    title "Sales"
    x-axis [Q1, Q2, Q3, Q4]
    y-axis "Revenue" 0 --> 10000
    bar [2500, 5000, 7500, 10000]
"#;

        let diagram = parse(input).unwrap();

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

        let diagram = parse(input).unwrap();

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

        let diagram = parse(input).unwrap();

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

        let diagram = parse(input).unwrap();

        assert_eq!(diagram.orientation, ChartOrientation::Horizontal);
        assert_eq!(diagram.data_series[0].data, vec![25.0, 45.0]);
    }
}
