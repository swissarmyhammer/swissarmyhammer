use crate::common::ast::{
    AccessibilityInfo, AxisDefinition, ClassDefinition, DataPoint, QuadrantDiagram, QuadrantLabels,
};
use crate::common::parser_utils::{parse_common_directives, validate_diagram_header};
use crate::error::{ParseError, Result};

/// Simple string-based parser for quadrant diagrams
pub fn parse(input: &str) -> Result<QuadrantDiagram> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut diagram = QuadrantDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        x_axis: None,
        y_axis: None,
        quadrants: QuadrantLabels::default(),
        points: Vec::new(),
        styles: Vec::new(),
    };

    let mut first_line_processed = false;

    for (line_num, line) in lines.iter().enumerate() {
        // Use shared header validation utility
        match validate_diagram_header(
            line,
            line_num,
            &["quadrantChart"],
            &mut first_line_processed,
        ) {
            Ok((true, _)) => continue, // Header was handled, skip to next line
            Ok((false, _)) => {}       // Line should be processed by parser
            Err(_) => {
                // For lenient parsing, skip files that don't start with quadrantChart
                // These are likely configuration files or test files, not actual diagrams
                return Ok(diagram);
            }
        }

        let trimmed = line.trim();

        // Handle common directives (title, accTitle, accDescr)
        if parse_common_directives(line, &mut diagram.title, &mut diagram.accessibility) {
            continue;
        }

        // Handle x-axis definition: "x-axis Low Reach --> High Reach"
        if trimmed.starts_with("x-axis ") {
            let axis_text = trimmed.strip_prefix("x-axis ").unwrap().trim();
            if let Some(axis) = parse_axis_definition(axis_text) {
                diagram.x_axis = Some(axis);
            }
            continue;
        }

        // Handle y-axis definition: "y-axis Low Influence --> High Influence"
        if trimmed.starts_with("y-axis ") {
            let axis_text = trimmed.strip_prefix("y-axis ").unwrap().trim();
            if let Some(axis) = parse_axis_definition(axis_text) {
                diagram.y_axis = Some(axis);
            }
            continue;
        }

        // Handle quadrant labels
        if trimmed.starts_with("quadrant-1 ") {
            let label = trimmed.strip_prefix("quadrant-1 ").unwrap().trim();
            diagram.quadrants.quadrant_1 = Some(label.to_string());
            continue;
        }

        if trimmed.starts_with("quadrant-2 ") {
            let label = trimmed.strip_prefix("quadrant-2 ").unwrap().trim();
            diagram.quadrants.quadrant_2 = Some(label.to_string());
            continue;
        }

        if trimmed.starts_with("quadrant-3 ") {
            let label = trimmed.strip_prefix("quadrant-3 ").unwrap().trim();
            diagram.quadrants.quadrant_3 = Some(label.to_string());
            continue;
        }

        if trimmed.starts_with("quadrant-4 ") {
            let label = trimmed.strip_prefix("quadrant-4 ").unwrap().trim();
            diagram.quadrants.quadrant_4 = Some(label.to_string());
            continue;
        }

        // Handle classDef: "classDef className fill:#color"
        if trimmed.starts_with("classDef ") {
            if let Some(class_def) = parse_class_definition(trimmed) {
                diagram.styles.push(class_def);
            }
            continue;
        }

        // Handle data points: "Campaign A: [0.3, 0.6]" or "Point A:::important: [0.3, 0.6]"
        // Find the colon that precedes coordinates
        if let Some(bracket_start) = trimmed.find('[') {
            if let Some(bracket_end) = trimmed.find(']') {
                if bracket_start < bracket_end {
                    // Look for colon that comes before the bracket
                    if let Some(colon_pos) = trimmed[..bracket_start].rfind(':') {
                        let name_part = trimmed[..colon_pos].trim();
                        let value_part = trimmed[colon_pos + 1..].trim();

                        if let Some(point) = parse_data_point(name_part, value_part, line_num) {
                            diagram.points.push(point);
                        }
                    }
                }
            }
        }

        // Ignore other unrecognized lines for lenient parsing
    }

    Ok(diagram)
}

/// Parse axis definition: "Low Reach --> High Reach"
fn parse_axis_definition(text: &str) -> Option<AxisDefinition> {
    if let Some(arrow_pos) = text.find("-->") {
        let start_label = text[..arrow_pos].trim();
        let end_label = text[arrow_pos + 3..].trim();

        Some(AxisDefinition {
            label_start: if start_label.is_empty() {
                None
            } else {
                Some(start_label.to_string())
            },
            label_end: if end_label.is_empty() {
                None
            } else {
                Some(end_label.to_string())
            },
        })
    } else {
        None
    }
}

/// Parse data point: name="Campaign A", coords="[0.3, 0.6]"
fn parse_data_point(name: &str, coords: &str, _line_num: usize) -> Option<DataPoint> {
    // Remove brackets
    let coords_inner = coords.strip_prefix('[')?.strip_suffix(']')?;

    // Split by comma
    let parts: Vec<&str> = coords_inner.split(',').collect();
    if parts.len() != 2 {
        return None;
    }

    // Parse x and y coordinates
    let x = parts[0].trim().parse::<f64>().ok()?;
    let y = parts[1].trim().parse::<f64>().ok()?;

    // Validate coordinate range (0.0 to 1.0)
    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
        return None;
    }

    // Handle potential class name after triple colon
    let (point_name, class_name) = if let Some(class_pos) = name.find(":::") {
        let name_part = name[..class_pos].trim();
        let class_part = name[class_pos + 3..].trim();
        (
            name_part,
            if class_part.is_empty() {
                None
            } else {
                Some(class_part.to_string())
            },
        )
    } else {
        (name, None)
    };

    Some(DataPoint {
        name: point_name.to_string(),
        x,
        y,
        class: class_name,
    })
}

/// Parse class definition: "classDef className fill:#color"
fn parse_class_definition(text: &str) -> Option<ClassDefinition> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() < 3 || parts[0] != "classDef" {
        return None;
    }

    let class_name = parts[1];
    let styles = parts[2..].iter().map(|s| s.to_string()).collect();

    Some(ClassDefinition {
        name: class_name.to_string(),
        styles,
    })
}
