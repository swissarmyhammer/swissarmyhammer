use regex::Regex;

use crate::model::entity::{build_entity_id, SemanticEntity};
use crate::parser::plugin::SemanticParserPlugin;
use crate::utils::hash::content_hash;

pub struct MarkdownParserPlugin;

impl SemanticParserPlugin for MarkdownParserPlugin {
    fn id(&self) -> &str {
        "markdown"
    }

    fn extensions(&self) -> &[&str] {
        &[".md", ".mdx"]
    }

    fn extract_entities(&self, content: &str, file_path: &str) -> Vec<SemanticEntity> {
        let mut entities = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let heading_re = Regex::new(r"^(#{1,6})\s+(.+)").unwrap();

        struct Section {
            level: usize,
            name: String,
            start_line: usize,
            lines: Vec<String>,
            parent_id: Option<String>,
        }

        let mut sections: Vec<Section> = Vec::new();
        let mut current_section: Option<Section> = None;
        let mut section_stack: Vec<(usize, String)> = Vec::new(); // (level, name)

        for (i, &line) in lines.iter().enumerate() {
            if let Some(caps) = heading_re.captures(line) {
                // Close previous section
                if let Some(sec) = current_section.take() {
                    sections.push(sec);
                }

                let level = caps[1].len();
                let name = caps[2].trim().to_string();

                // Find parent: pop headings with >= level
                while section_stack.last().is_some_and(|(l, _)| *l >= level) {
                    section_stack.pop();
                }

                let parent_id = section_stack.last().map(|(_, parent_name)| {
                    build_entity_id(file_path, "heading", parent_name, None)
                });

                current_section = Some(Section {
                    level,
                    name: name.clone(),
                    start_line: i + 1,
                    lines: vec![line.to_string()],
                    parent_id,
                });

                section_stack.push((level, name));
            } else if let Some(ref mut sec) = current_section {
                sec.lines.push(line.to_string());
            } else {
                // Content before first heading — preamble
                if !line.trim().is_empty() && current_section.is_none() {
                    current_section = Some(Section {
                        level: 0,
                        name: "(preamble)".to_string(),
                        start_line: i + 1,
                        lines: vec![line.to_string()],
                        parent_id: None,
                    });
                }
            }
        }

        if let Some(sec) = current_section {
            sections.push(sec);
        }

        for section in &sections {
            let section_content = section.lines.join("\n").trim().to_string();
            if section_content.is_empty() {
                continue;
            }

            let entity_type = if section.level == 0 {
                "preamble"
            } else {
                "heading"
            };

            entities.push(SemanticEntity {
                id: build_entity_id(file_path, entity_type, &section.name, None),
                file_path: file_path.to_string(),
                entity_type: entity_type.to_string(),
                name: section.name.clone(),
                parent_id: section.parent_id.clone(),
                content_hash: content_hash(&section_content),
                structural_hash: None,
                content: section_content,
                start_line: section.start_line,
                end_line: section.start_line + section.lines.len() - 1,
                metadata: None,
            });
        }

        entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin() -> MarkdownParserPlugin {
        MarkdownParserPlugin
    }

    #[test]
    fn test_single_heading() {
        let content = "# Hello\n\nSome content here.\n";
        let entities = plugin().extract_entities(content, "doc.md");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_type, "heading");
        assert_eq!(entities[0].name, "Hello");
        assert_eq!(entities[0].start_line, 1);
        assert!(entities[0].parent_id.is_none());
    }

    #[test]
    fn test_heading_hierarchy_parent_assignment() {
        let content = "# Parent\n\nIntro.\n\n## Child\n\nDetails.\n";
        let entities = plugin().extract_entities(content, "doc.md");
        // Expect: heading "Parent", heading "Child"
        assert_eq!(entities.len(), 2);
        let parent = &entities[0];
        let child = &entities[1];
        assert_eq!(parent.name, "Parent");
        assert!(parent.parent_id.is_none());
        assert_eq!(child.name, "Child");
        // Child's parent_id should be set to Parent's id
        let expected_parent_id = build_entity_id("doc.md", "heading", "Parent", None);
        assert_eq!(
            child.parent_id.as_deref(),
            Some(expected_parent_id.as_str())
        );
    }

    #[test]
    fn test_nested_sections_three_levels() {
        let content = "# Top\n## Mid\n### Deep\n";
        let entities = plugin().extract_entities(content, "readme.md");
        assert_eq!(entities.len(), 3);
        assert_eq!(entities[0].name, "Top");
        assert!(entities[0].parent_id.is_none());
        assert_eq!(entities[1].name, "Mid");
        let top_id = build_entity_id("readme.md", "heading", "Top", None);
        assert_eq!(entities[1].parent_id.as_deref(), Some(top_id.as_str()));
        assert_eq!(entities[2].name, "Deep");
        let mid_id = build_entity_id("readme.md", "heading", "Mid", None);
        assert_eq!(entities[2].parent_id.as_deref(), Some(mid_id.as_str()));
    }

    #[test]
    fn test_preamble_detected_before_first_heading() {
        let content = "This is preamble text.\nMore preamble.\n\n# Section\n\nContent.\n";
        let entities = plugin().extract_entities(content, "guide.md");
        // Should have preamble + heading
        assert!(entities.len() >= 2);
        let preamble = entities.iter().find(|e| e.entity_type == "preamble");
        assert!(preamble.is_some(), "expected a preamble entity");
        let p = preamble.unwrap();
        assert_eq!(p.name, "(preamble)");
        assert!(p.parent_id.is_none());
    }

    #[test]
    fn test_no_preamble_when_heading_first() {
        let content = "# Title\n\nContent.\n";
        let entities = plugin().extract_entities(content, "doc.md");
        assert!(!entities.iter().any(|e| e.entity_type == "preamble"));
    }

    #[test]
    fn test_sibling_headings_reset_parent() {
        // h2 under h1, then another h2 — both should have the same h1 as parent
        let content = "# Top\n## First\n## Second\n";
        let entities = plugin().extract_entities(content, "doc.md");
        assert_eq!(entities.len(), 3);
        let top_id = build_entity_id("doc.md", "heading", "Top", None);
        assert_eq!(entities[1].name, "First");
        assert_eq!(entities[1].parent_id.as_deref(), Some(top_id.as_str()));
        assert_eq!(entities[2].name, "Second");
        assert_eq!(entities[2].parent_id.as_deref(), Some(top_id.as_str()));
    }

    #[test]
    fn test_empty_content_returns_no_entities() {
        let entities = plugin().extract_entities("", "empty.md");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_entity_ids_include_file_path() {
        let content = "# My Section\n";
        let entities = plugin().extract_entities(content, "path/to/file.md");
        assert!(entities[0].id.contains("path/to/file.md"));
    }

    #[test]
    fn test_section_content_includes_body_lines() {
        let content = "# Section\nLine one.\nLine two.\n";
        let entities = plugin().extract_entities(content, "doc.md");
        assert_eq!(entities.len(), 1);
        assert!(entities[0].content.contains("Line one."));
        assert!(entities[0].content.contains("Line two."));
    }

    #[test]
    fn test_start_and_end_line_numbers() {
        let content = "# Heading\nBody line.\n";
        let entities = plugin().extract_entities(content, "doc.md");
        assert_eq!(entities[0].start_line, 1);
        assert_eq!(entities[0].end_line, 2);
    }
}
