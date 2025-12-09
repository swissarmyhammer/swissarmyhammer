//! Treemap diagram parser
//!
//! Parses hierarchical treemap diagrams with indentation-based structure.

use crate::common::ast::{AccessibilityInfo, TreemapDiagram, TreemapNode};
use crate::common::parser_utils::validate_diagram_header;
use crate::error::{ParseError, Result};

pub fn parse(input: &str) -> Result<TreemapDiagram> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::SyntaxError {
            message: "Empty input".to_string(),
            expected: vec!["treemap".to_string()],
            found: "end of input".to_string(),
            line: 0,
            column: 0,
        });
    }

    // Use shared header validation utility
    let mut first_line_processed = false;
    let mut start_line = 0;
    for (i, line) in lines.iter().enumerate() {
        match validate_diagram_header(
            line,
            i,
            &["treemap", "treemap-beta"],
            &mut first_line_processed,
        ) {
            Ok((true, _)) => {
                start_line = i + 1;
                break;
            }
            Ok((false, _)) => {
                // Line should be processed by parser
            }
            Err(_) => {
                // Continue checking other lines
            }
        }
    }

    if start_line == 0 {
        return Err(ParseError::SyntaxError {
            message: "Missing 'treemap' keyword".to_string(),
            expected: vec!["treemap".to_string()],
            found: lines.first().unwrap_or(&"").to_string(),
            line: 1,
            column: 0,
        });
    }

    let mut title = None;
    let mut node_lines = Vec::new();

    // Parse lines after treemap keyword
    for line in &lines[start_line..] {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        // Check for title
        if let Some(title_text) = trimmed.strip_prefix("title ") {
            title = Some(title_text.trim().to_string());
            continue;
        }

        // Otherwise it's a node line
        node_lines.push(line.to_string());
    }

    // Parse the hierarchical structure from node lines
    let root = parse_node_hierarchy(&node_lines);

    Ok(TreemapDiagram {
        title,
        accessibility: AccessibilityInfo::default(),
        root: root.unwrap_or_else(|| TreemapNode {
            name: "Root".to_string(),
            value: None,
            children: Vec::new(),
        }),
    })
}

fn parse_node_hierarchy(lines: &[String]) -> Option<TreemapNode> {
    if lines.is_empty() {
        return None;
    }

    // Find the line with minimum indentation as root
    let mut min_indent = usize::MAX;
    let mut root_idx = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let indent = count_leading_spaces(line);
            if indent < min_indent {
                min_indent = indent;
                root_idx = Some(i);
            }
        }
    }

    if let Some(idx) = root_idx {
        let root_line = &lines[idx];
        let (name, value) = parse_node_line(root_line);
        let mut root = TreemapNode {
            name,
            value,
            children: Vec::new(),
        };

        // Parse children starting from the line after root
        let remaining_lines = &lines[idx + 1..];
        root.children = parse_children(remaining_lines, min_indent);

        Some(root)
    } else {
        None
    }
}

fn parse_children(lines: &[String], base_indent: usize) -> Vec<TreemapNode> {
    let mut children = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];
        let indent = count_leading_spaces(line);
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        if indent <= base_indent {
            // End of this level
            break;
        }

        let expected_child_indent = base_indent + 4; // Assuming 4-space indentation

        if indent == expected_child_indent {
            let (name, value) = parse_node_line(line);
            let mut child = TreemapNode {
                name,
                value,
                children: Vec::new(),
            };

            // Look for grandchildren
            let mut j = i + 1;
            while j < lines.len() {
                let next_line = &lines[j];
                let next_indent = count_leading_spaces(next_line);
                let next_trimmed = next_line.trim();

                if next_trimmed.is_empty() {
                    j += 1;
                    continue;
                }

                if next_indent <= indent {
                    break;
                }

                j += 1;
            }

            // Parse grandchildren from lines[i+1..j]
            if j > i + 1 {
                child.children = parse_children(&lines[i + 1..j], indent);
            }

            children.push(child);
            i = j;
        } else {
            i += 1;
        }
    }

    children
}

fn parse_node_line(line: &str) -> (String, Option<f64>) {
    let trimmed = line.trim();

    if let Some(colon_pos) = trimmed.find(':') {
        let name_part = trimmed[..colon_pos].trim();
        let value_str = trimmed[colon_pos + 1..].trim();
        let value = value_str.parse::<f64>().ok();
        let name = unquote_string(name_part);
        (name, value)
    } else {
        let name = unquote_string(trimmed);
        (name, None)
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

fn count_leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_treemap() {
        let input = r#"treemap
    title Budget Allocation
    
    Total Budget
        Operations: 500000
        Marketing: 300000
        Development: 700000
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.title, Some("Budget Allocation".to_string()));
        assert_eq!(diagram.root.name, "Total Budget");
        assert_eq!(diagram.root.children.len(), 3);

        assert_eq!(diagram.root.children[0].name, "Operations");
        assert_eq!(diagram.root.children[0].value, Some(500000.0));
    }

    #[test]
    fn test_basic_treemap() {
        let input = r#"treemap
    Root
        Child1: 100
        Child2: 200
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.root.name, "Root");
        assert_eq!(diagram.root.children.len(), 2);
    }

    #[test]
    fn test_nested_hierarchy() {
        let input = r#"treemap
    Company
        Sales
            North Region
                Q1: 100000
                Q2: 120000
            South Region
                Q1: 80000
                Q2: 95000
        Engineering
            Frontend: 5
            Backend: 8
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.root.name, "Company");
        assert_eq!(diagram.root.children.len(), 2);

        let sales = &diagram.root.children[0];
        assert_eq!(sales.name, "Sales");
        assert_eq!(sales.children.len(), 2);

        let north = &sales.children[0];
        assert_eq!(north.name, "North Region");
        assert_eq!(north.children.len(), 2);
        assert_eq!(north.children[0].value, Some(100000.0));
    }

    #[test]
    fn test_treemap_beta_with_quotes() {
        let input = r#"treemap-beta
"Category A"
    "Item A1": 10
    "Item A2": 20
"Category B"
    "Item B1": 15
    "Item B2": 25
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.root.name, "Category A");
        assert_eq!(diagram.root.children.len(), 2);

        assert_eq!(diagram.root.children[0].name, "Item A1");
        assert_eq!(diagram.root.children[0].value, Some(10.0));
        assert_eq!(diagram.root.children[1].name, "Item A2");
        assert_eq!(diagram.root.children[1].value, Some(20.0));
    }
}
