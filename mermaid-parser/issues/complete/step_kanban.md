# Implementation Plan: Kanban Diagrams

## Overview
Kanban diagrams represent task board workflows with columns and cards for project management.
Medium complexity grammar (166 lines) with sections, items, and assignments.

## Grammar Analysis

### Key Features
- Header: `kanban`
- Sections: Named columns in the kanban board
- Items: Tasks/cards within sections
- Assignments: `@assigned[person1, person2]`
- Task IDs: Optional identifiers for tasks
- Comments: `%%` for line comments

### Example Input
```
kanban
  Todo
    item1[Buy milk]
    @assigned[Alice, Bob]
    
    item2[Walk the dog]
    @assigned[Charlie]
    
  In Progress
    item3[Write documentation]
    @assigned[Alice]
    
  Done
    item4[Deploy to production]
    item5[Review PR]
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct KanbanDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub sections: Vec<KanbanSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KanbanSection {
    pub id: String,
    pub title: String,
    pub items: Vec<KanbanItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KanbanItem {
    pub id: Option<String>,
    pub text: String,
    pub assigned: Vec<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KanbanToken {
    Kanban,               // "kanban"
    SectionTitle(String), // Section name
    ItemId(String),       // item1, item2, etc.
    ItemText(String),     // [Task description]
    Assigned,             // "@assigned"
    AssignedList(Vec<String>), // [person1, person2]
    Comment(String),      // %% comment
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn kanban_lexer() -> impl Parser<char, Vec<KanbanToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| KanbanToken::Comment(text.into_iter().collect()));
    
    let kanban_keyword = text::keyword("kanban")
        .map(|_| KanbanToken::Kanban);
    
    // Section title (lines without special syntax)
    let section_title = none_of("\n[]@")
        .repeated()
        .at_least(1)
        .collect::<String>()
        .then_ignore(just('\n').or(end()))
        .map(|title| KanbanToken::SectionTitle(title.trim().to_string()))
        .boxed();
    
    // Item ID: item1, item2, etc.
    let item_id = text::keyword("item")
        .then(text::digits(10))
        .map(|(_, num)| KanbanToken::ItemId(format!("item{}", num)));
    
    // Item text: [Task description]
    let item_text = just('[')
        .ignore_then(
            none_of("]")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just(']'))
        .map(KanbanToken::ItemText);
    
    // Assignment: @assigned[person1, person2]
    let assigned = just('@')
        .then(text::keyword("assigned"))
        .then_ignore(just('['))
        .then(
            none_of(",]")
                .repeated()
                .collect::<String>()
                .separated_by(just(',').padded())
                .allow_trailing()
        )
        .then_ignore(just(']'))
        .map(|(_, people)| {
            KanbanToken::AssignedList(
                people.into_iter()
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            )
        });
    
    let newline = just('\n').map(|_| KanbanToken::NewLine);
    
    choice((
        comment,
        kanban_keyword,
        assigned,
        item_id,
        item_text,
        section_title,
        newline,
        whitespace.ignored(),
    ))
    .repeated()
    .then_ignore(end())
}
```

## Step 3: Parser Implementation

### State Machine Parser
```rust
pub fn kanban_parser() -> impl Parser<KanbanToken, KanbanDiagram, Error = Simple<KanbanToken>> {
    enum ParseState {
        Initial,
        InDiagram,
        InSection(String),
    }
    
    just(KanbanToken::Kanban)
        .then_ignore(
            filter(|t| matches!(t, KanbanToken::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(KanbanToken::Eof).or_not())
        .map(|(_, tokens)| {
            let mut sections = Vec::new();
            let mut current_section: Option<KanbanSection> = None;
            let mut current_item: Option<KanbanItem> = None;
            let mut state = ParseState::InDiagram;
            
            for token in tokens {
                match (&state, token) {
                    (_, KanbanToken::Comment(_)) => {
                        // Skip comments
                    }
                    (_, KanbanToken::NewLine) => {
                        // Commit current item if exists
                        if let Some(item) = current_item.take() {
                            if let Some(ref mut section) = current_section {
                                section.items.push(item);
                            }
                        }
                    }
                    (ParseState::InDiagram | ParseState::InSection(_), KanbanToken::SectionTitle(title)) => {
                        // New section
                        if let Some(section) = current_section.take() {
                            sections.push(section);
                        }
                        current_section = Some(KanbanSection {
                            id: generate_section_id(&title),
                            title: title.clone(),
                            items: Vec::new(),
                        });
                        state = ParseState::InSection(title);
                    }
                    (ParseState::InSection(_), KanbanToken::ItemId(id)) => {
                        // Start new item
                        if let Some(item) = current_item.take() {
                            if let Some(ref mut section) = current_section {
                                section.items.push(item);
                            }
                        }
                        current_item = Some(KanbanItem {
                            id: Some(id),
                            text: String::new(),
                            assigned: Vec::new(),
                            metadata: HashMap::new(),
                        });
                    }
                    (ParseState::InSection(_), KanbanToken::ItemText(text)) => {
                        if let Some(ref mut item) = current_item {
                            item.text = text;
                        } else {
                            // Item without ID
                            current_item = Some(KanbanItem {
                                id: None,
                                text,
                                assigned: Vec::new(),
                                metadata: HashMap::new(),
                            });
                        }
                    }
                    (ParseState::InSection(_), KanbanToken::AssignedList(people)) => {
                        if let Some(ref mut item) = current_item {
                            item.assigned = people;
                        }
                    }
                    _ => {}
                }
            }
            
            // Commit final item and section
            if let Some(item) = current_item {
                if let Some(ref mut section) = current_section {
                    section.items.push(item);
                }
            }
            if let Some(section) = current_section {
                sections.push(section);
            }
            
            KanbanDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                sections,
            }
        })
}

fn generate_section_id(title: &str) -> String {
    title.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/kanban/`
- Expected count: 41 files
- Copy to: `mermaid-parser/test/kanban/`

### Command
```bash
cp -r ../mermaid-samples/kanban/* ./test/kanban/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_kanban_files(#[files("test/kanban/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = kanban_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = kanban_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.sections.is_empty(), "Should have at least one section");
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
    
    let tokens = kanban_lexer().parse(input.chars()).unwrap();
    let diagram = kanban_parser().parse(tokens).unwrap();
    
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
    
    let tokens = kanban_lexer().parse(input.chars()).unwrap();
    let diagram = kanban_parser().parse(tokens).unwrap();
    
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
    
    let tokens = kanban_lexer().parse(input.chars()).unwrap();
    let diagram = kanban_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.sections[0].items.len(), 2);
    assert!(diagram.sections[0].items[0].id.is_none());
    assert_eq!(diagram.sections[0].items[0].text, "First task");
}

#[test]
fn test_empty_sections() {
    let input = r#"kanban
  Todo
  In Progress
    item1[Working on it]
  Done
"#;
    
    let tokens = kanban_lexer().parse(input.chars()).unwrap();
    let diagram = kanban_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.sections.len(), 3);
    assert_eq!(diagram.sections[0].items.len(), 0); // Todo is empty
    assert_eq!(diagram.sections[1].items.len(), 1); // In Progress has one
    assert_eq!(diagram.sections[2].items.len(), 0); // Done is empty
}
```

## Success Criteria
1. ✅ Parse all 41 kanban sample files successfully
2. ✅ Handle multiple sections (columns)
3. ✅ Support items with and without IDs
4. ✅ Parse assignment lists correctly
5. ✅ Handle empty sections
6. ✅ Support comments in the diagram
7. ✅ Maintain section order

## Implementation Priority
**Priority 8** - Implement after mindmap. Kanban boards share hierarchical concepts with mindmap but add the workflow dimension. Understanding from mindmap will help with section/item relationships.