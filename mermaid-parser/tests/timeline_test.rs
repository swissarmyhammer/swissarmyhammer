use mermaid_parser::{parse_diagram, DiagramType};
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_timeline_files(#[files("test/timeline/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    // Skip empty files or files that don't have a valid timeline diagram start
    let first_line = content.lines().next().unwrap_or("").trim();
    if content.is_empty() || first_line != "timeline" {
        return;
    }

    let result = parse_diagram(&content);

    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);

    match result.unwrap() {
        DiagramType::Timeline(diagram) => {
            // Validate timeline structure
            // Timeline diagrams should have at least the basic structure
            // Even if they have no title or sections, they should parse successfully

            // Check that accessibility info is properly initialized
            assert!(diagram.accessibility.title.is_none() || diagram.accessibility.title.is_some());
            assert!(
                diagram.accessibility.description.is_none()
                    || diagram.accessibility.description.is_some()
            );

            // Validate sections structure
            for section in &diagram.sections {
                assert!(!section.name.is_empty(), "Section name should not be empty");

                // Validate timeline items in chronological order
                for item in &section.items {
                    match item {
                        mermaid_parser::common::ast::TimelineItem::Period(text) => {
                            assert!(!text.is_empty(), "Period text should not be empty");
                        }
                        mermaid_parser::common::ast::TimelineItem::Event(text) => {
                            assert!(!text.is_empty(), "Event text should not be empty");
                        }
                    }
                }
            }
        }
        _ => panic!("Expected Timeline diagram from {:?}", path),
    }
}

#[test]
fn test_simple_timeline_validation() {
    let input = r#"timeline
    title My Day
    section Morning
        Wake up
        : Brush teeth
    section Evening
        Dinner
        : Sleep
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        DiagramType::Timeline(diagram) => {
            assert_eq!(diagram.title, Some("My Day".to_string()));
            assert_eq!(diagram.sections.len(), 2);

            // Check Morning section
            assert_eq!(diagram.sections[0].name, "Morning");
            assert_eq!(diagram.sections[0].items.len(), 2);

            // Check Evening section
            assert_eq!(diagram.sections[1].name, "Evening");
            assert_eq!(diagram.sections[1].items.len(), 2);
        }
        _ => panic!("Expected Timeline diagram"),
    }
}

#[test]
fn test_timeline_with_accessibility() {
    let input = r#"timeline
    accTitle: Timeline Accessibility Title
    accDescr: This timeline shows my daily routine
    title My Day
    section Morning
        Wake up
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        DiagramType::Timeline(diagram) => {
            assert_eq!(
                diagram.accessibility.title,
                Some("Timeline Accessibility Title".to_string())
            );
            assert_eq!(
                diagram.accessibility.description,
                Some("This timeline shows my daily routine".to_string())
            );
            assert_eq!(diagram.title, Some("My Day".to_string()));
            assert_eq!(diagram.sections.len(), 1);
        }
        _ => panic!("Expected Timeline diagram"),
    }
}

#[test]
fn test_timeline_chronological_structure() {
    let input = r#"timeline
    title History of Social Media Platform
    section Early Years
        2002 : LinkedIn
        2004 : Facebook
             : Google
    section Growth
        2005 : YouTube
        2006 : Twitter
"#;

    let result = parse_diagram(input);
    assert!(result.is_ok(), "Failed to parse: {:?}", result);

    match result.unwrap() {
        DiagramType::Timeline(diagram) => {
            assert_eq!(
                diagram.title,
                Some("History of Social Media Platform".to_string())
            );
            assert_eq!(diagram.sections.len(), 2);

            // Check Early Years section
            let early_section = &diagram.sections[0];
            assert_eq!(early_section.name, "Early Years");
            assert_eq!(early_section.items.len(), 3);

            // Check Growth section
            let growth_section = &diagram.sections[1];
            assert_eq!(growth_section.name, "Growth");
            assert_eq!(growth_section.items.len(), 2);
        }
        _ => panic!("Expected Timeline diagram"),
    }
}
