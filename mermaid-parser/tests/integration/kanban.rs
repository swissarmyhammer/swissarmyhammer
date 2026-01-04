use mermaid_parser::parsers::kanban;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_kanban_files(#[files("test/kanban/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read file {:?}: {}", path, e));

    // Remove metadata comments that might interfere with parsing
    let content = content
        .lines()
        .filter(|line| !line.trim().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    // Skip empty files
    if content.trim().is_empty() {
        return;
    }

    let result = kanban::parse(&content);

    match result {
        Ok(diagram) => {
            // Basic validation - kanban should have at least one section in most cases
            println!("✅ Successfully parsed {:?}", path.file_name().unwrap());
            println!("   Sections: {}", diagram.sections.len());
            for section in &diagram.sections {
                println!("   - {}: {} items", section.title, section.items.len());
            }
            println!("   Title: {:?}", diagram.title);
            println!("   Accessibility: {:?}", diagram.accessibility);
        }
        Err(e) => {
            panic!(
                "Failed to parse kanban file {:?}:\nContent:\n{}\nError: {:?}",
                path.file_name().unwrap(),
                content,
                e
            );
        }
    }
}

#[test]
fn test_simple_kanban() {
    let input = r#"kanban
  Todo
    item1[Buy milk]
    @assigned[Alice]
    
  Done
    item2[Task complete]
"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse simple kanban: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.sections.len(), 2);
    assert_eq!(diagram.sections[0].title, "Todo");
    assert_eq!(diagram.sections[0].items.len(), 1);
    assert_eq!(diagram.sections[0].items[0].text, "Buy milk");
    assert_eq!(diagram.sections[0].items[0].assigned, vec!["Alice"]);

    assert_eq!(diagram.sections[1].title, "Done");
    assert_eq!(diagram.sections[1].items.len(), 1);
}

#[test]
fn test_multiple_assignments() {
    let input = r#"kanban
  In Progress
    item1[Complex task]
    @assigned[Alice, Bob, Charlie]
"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse multiple assignments: {:?}",
        result
    );

    let diagram = result.unwrap();
    let item = &diagram.sections[0].items[0];
    assert_eq!(item.assigned.len(), 3);
    assert_eq!(item.assigned, vec!["Alice", "Bob", "Charlie"]);
}

#[test]
fn test_items_without_ids() {
    let input = r#"kanban
  Backlog
    [First task]
    [Second task]
    @assigned[Team]
"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse items without IDs: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.sections[0].items.len(), 2);
    assert!(diagram.sections[0].items[0].id.is_none());
    assert_eq!(diagram.sections[0].items[0].text, "First task");
    assert_eq!(diagram.sections[0].items[1].text, "Second task");
    assert_eq!(diagram.sections[0].items[1].assigned, vec!["Team"]);
}

#[test]
fn test_empty_sections() {
    let input = r#"kanban
  Todo
  In Progress
    item1[Working on it]
  Done
"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse empty sections: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.sections.len(), 3);
    assert_eq!(diagram.sections[0].items.len(), 0); // Todo is empty
    assert_eq!(diagram.sections[1].items.len(), 1); // In Progress has one
    assert_eq!(diagram.sections[2].items.len(), 0); // Done is empty
}

#[test]
fn test_comments() {
    let input = r#"kanban
  %% This is a comment
  Todo
    item1[Task with comment]
    %% Another comment
    @assigned[Alice]
"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse with comments: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.sections.len(), 1);
    assert_eq!(diagram.sections[0].items.len(), 1);
}

#[test]
fn test_minimal_kanban() {
    let input = "kanban";

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse minimal kanban: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.sections.len(), 0);
}

#[test]
fn test_section_ordering() {
    let input = r#"kanban
  First Section
    item1[Task 1]
  Second Section
    item2[Task 2]
  Third Section
    item3[Task 3]
"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse section ordering: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.sections.len(), 3);
    assert_eq!(diagram.sections[0].title, "First Section");
    assert_eq!(diagram.sections[1].title, "Second Section");
    assert_eq!(diagram.sections[2].title, "Third Section");
}

#[test]
fn test_mixed_item_types() {
    let input = r#"kanban
  Mixed Items
    item1[Item with ID]
    @assigned[Alice]
    [Item without ID]
    @assigned[Bob]
    item2[Another with ID]
    [Another without ID]
"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse mixed item types: {:?}",
        result
    );

    let diagram = result.unwrap();
    let section = &diagram.sections[0];
    assert_eq!(section.items.len(), 4);

    // Check ID presence
    assert!(section.items[0].id.is_some());
    assert_eq!(section.items[0].id, Some("item1".to_string()));
    assert!(section.items[1].id.is_none());
    assert!(section.items[2].id.is_some());
    assert_eq!(section.items[2].id, Some("item2".to_string()));
    assert!(section.items[3].id.is_none());

    // Check assignments
    assert_eq!(section.items[0].assigned, vec!["Alice"]);
    assert_eq!(section.items[1].assigned, vec!["Bob"]);
    assert!(section.items[2].assigned.is_empty());
    assert!(section.items[3].assigned.is_empty());
}

#[test]
fn test_whitespace_handling() {
    let input = r#"kanban

  Todo   
    item1[   Task with spaces   ]
    @assigned[  Alice  ,  Bob  ]

  In Progress
    [  Another task  ]
    
  Done
    item2[Done task]

"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse with whitespace: {:?}",
        result
    );

    let diagram = result.unwrap();
    assert_eq!(diagram.sections.len(), 3);
    assert_eq!(diagram.sections[0].title, "Todo");
    assert_eq!(diagram.sections[0].items[0].text, "   Task with spaces   ");
    assert_eq!(diagram.sections[0].items[0].assigned, vec!["Alice", "Bob"]);
}

#[test]
fn test_section_id_generation() {
    let input = r#"kanban
  Todo Items
  In Progress!!!
  Done & Tested
"#;

    let result = kanban::parse(input);
    assert!(result.is_ok(), "Failed to parse section IDs: {:?}", result);

    let diagram = result.unwrap();
    assert_eq!(diagram.sections.len(), 3);
    assert_eq!(diagram.sections[0].id, "todoitems");
    assert_eq!(diagram.sections[1].id, "inprogress");
    assert_eq!(diagram.sections[2].id, "donetested");
}

#[test]
fn test_assignments_only() {
    let input = r#"kanban
  Team Tasks
    [Unassigned task]
    @assigned[Alice, Bob, Charlie, Dave]
"#;

    let result = kanban::parse(input);
    assert!(
        result.is_ok(),
        "Failed to parse assignments only: {:?}",
        result
    );

    let diagram = result.unwrap();
    let item = &diagram.sections[0].items[0];
    assert_eq!(item.assigned.len(), 4);
    assert_eq!(item.assigned, vec!["Alice", "Bob", "Charlie", "Dave"]);
}
