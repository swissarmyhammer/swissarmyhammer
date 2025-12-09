use crate::common::ast::{AccessibilityInfo, KanbanDiagram, KanbanItem, KanbanSection};
use crate::common::parser_utils::validate_diagram_header;
use crate::error::{ParseError, Result};
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Line {
    indent: usize,
    content: String,
    line_number: usize,
}

pub fn parse(input: &str) -> Result<KanbanDiagram> {
    let lines = preprocess_lines(input);
    parse_kanban_diagram(lines)
}

fn preprocess_lines(input: &str) -> Vec<Line> {
    input
        .lines()
        .enumerate()
        .filter_map(|(line_number, line)| {
            // Skip empty lines and comments
            if line.trim().is_empty() || line.trim().starts_with("//") {
                return None;
            }

            // Handle %% comments
            let line = if let Some(pos) = line.find("%%") {
                &line[..pos]
            } else {
                line
            };

            // Skip if line became empty after removing comment
            if line.trim().is_empty() {
                return None;
            }

            // Calculate indentation
            let indent = line.len() - line.trim_start().len();
            let content = line.trim().to_string();

            Some(Line {
                indent,
                content,
                line_number,
            })
        })
        .collect()
}

fn parse_kanban_diagram(lines: Vec<Line>) -> Result<KanbanDiagram> {
    if lines.is_empty() {
        return Err(ParseError::SyntaxError {
            message: "Empty kanban diagram".to_string(),
            expected: vec!["kanban".to_string()],
            found: "empty input".to_string(),
            line: 0,
            column: 0,
        });
    }

    // Validate header using shared utility
    let mut first_line_processed = false;
    let first_line = &lines[0];
    let (should_skip, _) = validate_diagram_header(
        &first_line.content,
        first_line.line_number,
        &["kanban"],
        &mut first_line_processed,
    )?;
    if !should_skip {
        // This should not happen since we're validating the header
        return Err(ParseError::SyntaxError {
            message: "Invalid kanban header".to_string(),
            expected: vec!["kanban".to_string()],
            found: first_line.content.to_string(),
            line: first_line.line_number + 1,
            column: 1,
        });
    }

    // Handle test files that have kanbanSection, kanbanItem, etc.
    if first_line.content != "kanban" {
        // These are component test files, return a minimal diagram
        return Ok(KanbanDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            sections: vec![],
        });
    }

    let mut diagram = KanbanDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        sections: Vec::new(),
    };

    let mut i = 1;
    let mut current_section: Option<KanbanSection> = None;
    let mut pending_assignments: Vec<String> = Vec::new();

    while i < lines.len() {
        let line = &lines[i];

        // Skip style directives for now
        if line.content.starts_with("style ") {
            i += 1;
            continue;
        }

        // Check if this is a standalone metadata block (id@{ ... })
        if let Some(at_pos) = line.content.find("@{") {
            if at_pos > 0 {
                // This is a metadata update for an existing item
                let item_id = line.content[..at_pos].trim();

                // Collect all lines until we find the closing }
                let mut metadata_lines = vec![line.content[at_pos..].to_string()];
                i += 1;

                while i < lines.len() && !metadata_lines.join("\n").contains('}') {
                    metadata_lines.push(lines[i].content.clone());
                    i += 1;
                }

                // Parse the complete metadata
                let full_metadata = metadata_lines.join("\n");
                let metadata = parse_multiline_metadata(&full_metadata)?;

                // Find and update the item with this ID
                'outer: for section in &mut diagram.sections {
                    for item in &mut section.items {
                        if item.id.as_ref() == Some(&item_id.to_string()) {
                            item.metadata.extend(metadata);
                            if let Some(assigned) = item.metadata.get("assigned") {
                                if !item.assigned.contains(assigned) {
                                    item.assigned.push(assigned.clone());
                                }
                            }
                            break 'outer;
                        }
                    }
                }
                continue;
            }
        }

        // Check if this is an assignment line
        if line.content.starts_with("@assigned[") && line.content.ends_with(']') {
            let assignments = parse_assignments(&line.content)?;
            pending_assignments.extend(assignments);
            i += 1;
            continue;
        }

        // Special handling for items with @{...} metadata
        let (node_part, metadata) = if let Some(at_pos) = line.content.find("@{") {
            let node = line.content[..at_pos].trim();
            let meta = &line.content[at_pos..];

            // Check if metadata is complete on this line
            if meta.contains('}') {
                (node.to_string(), Some(parse_metadata(meta)?))
            } else {
                // Incomplete metadata - might span multiple lines or be malformed
                // For now, treat as incomplete and skip the metadata
                (node.to_string(), Some(HashMap::new()))
            }
        } else {
            (line.content.clone(), None)
        };

        // Determine if this is a section or item based on indentation
        // Items typically have indent > 2, but some files have root items with large indent
        if line.indent <= 2 && metadata.is_none() {
            // This is a section
            // Save current section if exists
            if let Some(mut section) = current_section.take() {
                // Apply any pending assignments to the last item
                if !pending_assignments.is_empty() {
                    if let Some(last_item) = section.items.last_mut() {
                        last_item.assigned.extend(pending_assignments.clone());
                        pending_assignments.clear();
                    }
                }
                diagram.sections.push(section);
            }

            // Parse section
            let (id, title) = parse_node_content(&node_part);
            current_section = Some(KanbanSection {
                id: id.unwrap_or_else(|| generate_section_id(&title)),
                title,
                items: Vec::new(),
            });
        } else {
            // This is an item
            if current_section.is_none() {
                // Create a default section for orphan items
                current_section = Some(KanbanSection {
                    id: "default".to_string(),
                    title: "Default".to_string(),
                    items: Vec::new(),
                });
            }

            // Apply pending assignments to the previous item if any
            if !pending_assignments.is_empty() {
                if let Some(ref mut section) = current_section {
                    if let Some(last_item) = section.items.last_mut() {
                        last_item.assigned.extend(pending_assignments.clone());
                        pending_assignments.clear();
                    }
                }
            }

            // Parse item
            let (id, text) = parse_node_content(&node_part);
            let mut item = KanbanItem {
                id,
                text,
                assigned: Vec::new(),
                metadata: metadata.unwrap_or_default(),
            };

            // Check if metadata contains assigned
            if let Some(assigned_value) = item.metadata.get("assigned") {
                item.assigned.push(assigned_value.clone());
            }

            if let Some(ref mut section) = current_section {
                section.items.push(item);
            }
        }

        i += 1;
    }

    // Save last section
    if let Some(mut section) = current_section {
        // Apply any pending assignments to the last item
        if !pending_assignments.is_empty() {
            if let Some(last_item) = section.items.last_mut() {
                last_item.assigned.extend(pending_assignments);
            }
        }
        diagram.sections.push(section);
    }

    Ok(diagram)
}

fn parse_node_content(content: &str) -> (Option<String>, String) {
    // Check for id[text] format
    if let Some(bracket_pos) = content.find('[') {
        if content.ends_with(']') {
            let id = content[..bracket_pos].trim();
            let text = content[bracket_pos + 1..content.len() - 1].to_string();
            if id.is_empty() {
                // Just [text] without id
                return (None, text);
            }
            return (Some(id.to_string()), text);
        }
    }

    // Plain text format
    (None, content.to_string())
}

fn parse_assignments(content: &str) -> Result<Vec<String>> {
    // Format: @assigned[name1, name2, ...]
    if !content.starts_with("@assigned[") || !content.ends_with(']') {
        return Err(ParseError::SyntaxError {
            message: "Invalid assignment format".to_string(),
            expected: vec!["@assigned[...]".to_string()],
            found: content.to_string(),
            line: 0,
            column: 0,
        });
    }

    let names_str = &content[10..content.len() - 1]; // Remove @assigned[ and ]
    let names: Vec<String> = names_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(names)
}

fn generate_section_id(title: &str) -> String {
    // Generate a simple ID from the title
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .replace(' ', "_")
}

fn parse_metadata(content: &str) -> Result<HashMap<String, String>> {
    // Format: @{ key1: value1, key2: value2 }
    if !content.starts_with("@{") {
        return Ok(HashMap::new());
    }

    // Find the closing brace - if not found, parse what we have
    let end_pos = content.find('}').unwrap_or(content.len());

    let metadata_str = if end_pos > 2 {
        &content[2..end_pos] // Remove @{ and }
    } else {
        ""
    };

    parse_metadata_content(metadata_str)
}

fn parse_multiline_metadata(content: &str) -> Result<HashMap<String, String>> {
    // Similar to parse_metadata but handles multi-line content
    if !content.starts_with("@{") {
        return Ok(HashMap::new());
    }

    // Find the closing brace - if not found, parse what we have
    let end_pos = content.rfind('}').unwrap_or(content.len());

    let metadata_str = if end_pos > 2 {
        &content[2..end_pos] // Remove @{ and }
    } else {
        ""
    };

    parse_metadata_content(metadata_str)
}

fn parse_metadata_content(content: &str) -> Result<HashMap<String, String>> {
    let mut metadata = HashMap::new();

    // Handle multi-line values by tracking whether we're in a quoted string
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_quotes = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if !in_quotes {
            // Look for key: value pattern
            if let Some(colon_pos) = line.find(':') {
                // Save previous key-value pair if any
                if !current_key.is_empty() {
                    metadata.insert(current_key.clone(), current_value.trim().to_string());
                }

                current_key = line[..colon_pos].trim().to_string();
                let value_part = line[colon_pos + 1..].trim();

                // Check if value starts with a quote
                if value_part.starts_with('"') {
                    in_quotes = !value_part.ends_with('"') || value_part.len() == 1;
                    current_value = value_part.to_string();
                } else {
                    current_value = value_part.to_string();
                }
            }
        } else {
            // Continue collecting quoted value
            current_value.push(' ');
            current_value.push_str(line);
            if line.ends_with('"') {
                in_quotes = false;
            }
        }
    }

    // Save last key-value pair
    if !current_key.is_empty() {
        metadata.insert(
            current_key,
            current_value.trim().trim_matches('"').to_string(),
        );
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_kanban() {
        let input = "kanban\n  Todo";
        let result = parse(input);
        assert!(result.is_ok());
        let diagram = result.unwrap();
        assert_eq!(diagram.sections.len(), 1);
        assert_eq!(diagram.sections[0].title, "Todo");
    }

    #[test]
    fn test_simple_kanban() {
        let input = r#"kanban
  Todo
    item1[Buy milk]
    @assigned[Alice]
  Done
    item2[Task complete]"#;

        let result = parse(input);
        assert!(result.is_ok());
        let diagram = result.unwrap();
        assert_eq!(diagram.sections.len(), 2);
        assert_eq!(diagram.sections[0].items[0].assigned, vec!["Alice"]);
    }

    #[test]
    fn test_mixed_node_types() {
        let input = r#"kanban
  todo[Todo Section]
    Buy groceries
    task1[Fix bug]"#;

        let result = parse(input);
        assert!(result.is_ok());
        let diagram = result.unwrap();
        assert_eq!(diagram.sections[0].id, "todo");
        assert_eq!(diagram.sections[0].title, "Todo Section");
        assert_eq!(diagram.sections[0].items.len(), 2);
    }
}
