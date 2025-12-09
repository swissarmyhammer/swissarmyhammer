//! Mindmap diagram parser implementation

use crate::common::ast::{AccessibilityInfo, MindmapDiagram, MindmapNode, MindmapNodeShape};
use crate::common::parser_utils::validate_diagram_header;
use crate::error::{ParseError, Result};

pub fn parse(input: &str) -> Result<MindmapDiagram> {
    // Simple string-based parsing for now
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    // Use shared header validation utility
    let mut first_line_processed = false;
    let (should_skip, _) =
        validate_diagram_header(lines[0], 0, &["mindmap"], &mut first_line_processed)?;
    if !should_skip {
        // This should not happen since we're validating the header
        return Err(ParseError::SyntaxError {
            message: "Invalid mindmap header".to_string(),
            expected: vec!["mindmap".to_string()],
            found: lines[0].to_string(),
            line: 1,
            column: 1,
        });
    }

    let mut nodes = Vec::new();

    // Parse each line after the mindmap header
    for line in lines.iter().skip(1) {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("%%") {
            // Store the original line (with indentation) for hierarchy parsing
            nodes.push(line.to_string());
        }
    }

    // Parse all lines into structured data
    let mut parsed_lines = Vec::new();
    for line in &nodes {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        // Parse the line for text, icon, and class
        let parsed = parse_line_content(trimmed);
        parsed_lines.push((indent, parsed));
    }

    // Build the hierarchy
    let root = if let Some((_, first_parsed)) = parsed_lines.first() {
        let mut root_node = MindmapNode {
            id: generate_id(),
            text: first_parsed.text.clone(),
            shape: first_parsed.shape.clone(),
            icon: first_parsed.icon.clone(),
            class: first_parsed.class.clone(),
            children: Vec::new(),
        };

        // Build children hierarchy
        let root_indent = parsed_lines[0].0;
        root_node.children = build_children(&parsed_lines, 1, root_indent);
        root_node
    } else {
        MindmapNode {
            id: generate_id(),
            text: "Root".to_string(),
            shape: crate::common::ast::MindmapNodeShape::Default,
            icon: None,
            class: None,
            children: Vec::new(),
        }
    };

    Ok(MindmapDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        root,
    })
}

// Unused complex parser functions removed - using simple string-based parsing

#[derive(Debug, Clone)]
struct ParsedLine {
    text: String,
    shape: MindmapNodeShape,
    icon: Option<String>,
    class: Option<String>,
}

fn parse_line_content(line: &str) -> ParsedLine {
    let mut icon = None;
    let mut class = None;
    let mut text = line.to_string();

    // Check for icon syntax: ::icon(fa fa-book)
    if let Some(icon_start) = line.find("::icon(") {
        if let Some(icon_end) = line[icon_start..].find(')') {
            let icon_content = &line[icon_start + 7..icon_start + icon_end];
            icon = Some(icon_content.to_string());

            // Remove icon syntax from text
            let before = &line[..icon_start];
            let after = &line[icon_start + icon_end + 1..];
            text = format!("{}{}", before, after).trim().to_string();
        }
    }

    // Check for class syntax: :::myClass
    if let Some(class_start) = text.find(":::") {
        let class_content = text[class_start + 3..].trim();
        if !class_content.is_empty() {
            class = Some(class_content.to_string());

            // Remove class syntax from text
            text = text[..class_start].trim().to_string();
        }
    }

    // Parse the remaining text for shape and content
    let (final_text, shape) = parse_node_text(&text);

    ParsedLine {
        text: final_text,
        shape,
        icon,
        class,
    }
}

fn build_children(
    parsed_lines: &[(usize, ParsedLine)],
    start_index: usize,
    parent_indent: usize,
) -> Vec<MindmapNode> {
    let mut children = Vec::new();
    let mut i = start_index;

    while i < parsed_lines.len() {
        let (indent, parsed) = &parsed_lines[i];

        // If this line is at the same level or less indented than parent, stop
        if *indent <= parent_indent {
            break;
        }

        // Find the next direct child (immediate next level of indentation)
        let child_indent = *indent;

        // Skip lines that are more deeply indented than this direct child
        if i > start_index {
            let prev_child_indent = parsed_lines
                .iter()
                .skip(start_index)
                .take(i - start_index)
                .filter_map(|(ind, _)| {
                    if *ind > parent_indent {
                        Some(*ind)
                    } else {
                        None
                    }
                })
                .min()
                .unwrap_or(child_indent);

            if child_indent > prev_child_indent {
                i += 1;
                continue;
            }
        }

        // This is a direct child, create the node
        let mut child_node = MindmapNode {
            id: generate_id(),
            text: parsed.text.clone(),
            shape: parsed.shape.clone(),
            icon: parsed.icon.clone(),
            class: parsed.class.clone(),
            children: Vec::new(),
        };

        // Find the end of this child's subtree
        let mut j = i + 1;
        while j < parsed_lines.len() && parsed_lines[j].0 > child_indent {
            j += 1;
        }

        // Recursively build children for this child
        child_node.children = build_children(parsed_lines, i + 1, child_indent);
        children.push(child_node);

        // Move to the next sibling
        i = j;
    }

    children
}

fn parse_node_text(text: &str) -> (String, MindmapNodeShape) {
    let trimmed = text.trim();

    // Look for embedded shapes within the text
    if let Some(start) = trimmed.find("((") {
        if let Some(end) = trimmed.rfind("))") {
            if end > start + 2 {
                let content = &trimmed[start + 2..end];
                return (content.to_string(), MindmapNodeShape::Circle);
            }
        }
    }

    if let Some(start) = trimmed.find("{{") {
        if let Some(end) = trimmed.rfind("}}") {
            if end > start + 2 {
                let content = &trimmed[start + 2..end];
                return (content.to_string(), MindmapNodeShape::Hexagon);
            }
        }
    }

    if let Some(start) = trimmed.find("(-") {
        if let Some(end) = trimmed.rfind("-)") {
            if end > start + 2 {
                let content = &trimmed[start + 2..end];
                return (content.to_string(), MindmapNodeShape::Cloud);
            }
        }
    }

    if let Some(start) = trimmed.find("))") {
        if let Some(end) = trimmed.rfind("((") {
            if end > start + 2 {
                let content = &trimmed[start + 2..end];
                return (content.to_string(), MindmapNodeShape::Bang);
            }
        }
    }

    // Check for simple bracket shapes that span the entire text
    if trimmed.starts_with("[") && trimmed.ends_with("]") {
        let content = &trimmed[1..trimmed.len() - 1];
        (content.to_string(), MindmapNodeShape::Square)
    } else if trimmed.starts_with("(") && trimmed.ends_with(")") {
        let content = &trimmed[1..trimmed.len() - 1];
        (content.to_string(), MindmapNodeShape::Rounded)
    } else {
        (trimmed.to_string(), MindmapNodeShape::Default)
    }
}

fn generate_id() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    format!("node_{}", COUNTER.fetch_add(1, Ordering::SeqCst))
}
