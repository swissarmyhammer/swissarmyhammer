# Implementation Plan: User Journey Diagrams

## Overview
User journey diagrams map user experiences with satisfaction scores and actors.
Simple grammar (69 lines) with sections, tasks, and scoring data.

## Grammar Analysis

### Key Features
- Header: `journey`
- Title support: `title My Journey`
- Sections: `section Section Name`
- Tasks: `Task name : score: Actor1, Actor2`
- Score format: Numbers (1-5 scale typically)
- Multiple actors per task

### Example Input
```
journey
    title My working day
    section Go to work
        Make tea: 5: Me
        Go upstairs: 3: Me
        Do work: 1: Me, Cat
    section Go home
        Go downstairs: 5: Me
        Sit down: 3: Me
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct JourneyDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub sections: Vec<JourneySection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JourneySection {
    pub name: String,
    pub tasks: Vec<JourneyTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JourneyTask {
    pub name: String,
    pub score: i32,
    pub actors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessibilityInfo {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JourneyToken {
    Journey,               // "journey"
    Title(String),         // "title My Title"
    Section(String),       // "section Section Name"
    TaskName(String),      // Task name
    TaskData(String),      // ": score: Actor1, Actor2"
    Colon,                // ":"
    AccTitle,              // "accTitle"
    AccTitleValue(String), // Accessibility title value
    AccDescr,              // "accDescr"
    AccDescrValue(String), // Accessibility description value
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn journey_lexer() -> impl Parser<char, Vec<JourneyToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('#')
        .then(take_until(just('\n')))
        .ignored();
    
    let journey_keyword = text::keyword("journey")
        .map(|_| JourneyToken::Journey);
    
    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, text)| JourneyToken::Title(text.trim().to_string()));
    
    let section = text::keyword("section")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just('#'), just(':'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, text)| JourneyToken::Section(text.trim().to_string()));
    
    let acc_title = text::keyword("accTitle")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| JourneyToken::AccTitle);
    
    let acc_descr = text::keyword("accDescr")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| JourneyToken::AccDescr);
    
    let task_data = just(':')
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, text)| JourneyToken::TaskData(text.trim().to_string()));
    
    let colon = just(':')
        .map(|_| JourneyToken::Colon);
    
    let task_name = filter(|c| !matches!(*c, '\n' | '#' | ':' | ';'))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|text| JourneyToken::TaskName(text.trim().to_string()))
        .filter(|token| {
            if let JourneyToken::TaskName(text) = token {
                !text.is_empty() && 
                !text.starts_with("journey") &&
                !text.starts_with("title") &&
                !text.starts_with("section") &&
                !text.starts_with("accTitle") &&
                !text.starts_with("accDescr")
            } else {
                true
            }
        });
    
    let newline = just('\n')
        .map(|_| JourneyToken::NewLine);
    
    choice((
        comment.ignored(),
        journey_keyword,
        title,
        section,
        acc_title,
        acc_descr,
        task_data,
        colon,
        task_name,
        newline,
    ))
    .padded_by(just(' ').or(just('\t')).repeated())
    .repeated()
    .then_ignore(end())
    .map(|tokens| {
        tokens.into_iter()
            .filter(|token| !matches!(token, JourneyToken::TaskName(text) if text.is_empty()))
            .collect()
    })
}
```

## Step 3: Parser Implementation

### Task Data Parser
```rust
fn parse_task_data(data: &str) -> (i32, Vec<String>) {
    let parts: Vec<&str> = data.split(':').collect();
    
    if parts.len() >= 2 {
        let score = parts[0].trim().parse::<i32>().unwrap_or(0);
        let actors: Vec<String> = parts[1]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        (score, actors)
    } else {
        (0, vec![])
    }
}
```

### Main Parser
```rust
pub fn journey_parser() -> impl Parser<JourneyToken, JourneyDiagram, Error = Simple<JourneyToken>> {
    let title = select! {
        JourneyToken::Title(text) => text,
    };
    
    let acc_title = just(JourneyToken::AccTitle)
        .then(select! {
            JourneyToken::TaskName(text) => text,
        })
        .map(|(_, text)| text);
    
    let acc_descr = just(JourneyToken::AccDescr)
        .then(select! {
            JourneyToken::TaskName(text) => text,
        })
        .map(|(_, text)| text);
    
    let task = select! {
        JourneyToken::TaskName(name) => name,
    }
    .then(select! {
        JourneyToken::TaskData(data) => data,
    })
    .map(|(name, data)| {
        let (score, actors) = parse_task_data(&data);
        JourneyTask { name, score, actors }
    });
    
    let section = select! {
        JourneyToken::Section(name) => name,
    }
    .then(
        just(JourneyToken::NewLine)
            .or_not()
            .then(task)
            .map(|(_, t)| t)
            .repeated()
    )
    .map(|(name, tasks)| JourneySection { name, tasks });
    
    // Main parser
    just(JourneyToken::Journey)
        .then_ignore(just(JourneyToken::NewLine).or_not())
        .then(
            choice((
                title.map(|t| ("title", t)),
                acc_title.map(|t| ("acc_title", t)),
                acc_descr.map(|t| ("acc_descr", t)),
                section.map(|s| ("section", format!("{}:{}", s.name, s.tasks.len()))),
            ))
            .separated_by(just(JourneyToken::NewLine))
            .allow_trailing()
        )
        .then_ignore(just(JourneyToken::Eof).or_not())
        .map(|(_, statements)| {
            let mut diagram = JourneyDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                sections: Vec::new(),
            };
            
            let mut current_section: Option<JourneySection> = None;
            let mut tasks_buffer: Vec<JourneyTask> = Vec::new();
            
            for (stmt_type, content) in statements {
                match stmt_type {
                    "title" => diagram.title = Some(content),
                    "acc_title" => diagram.accessibility.title = Some(content),
                    "acc_descr" => diagram.accessibility.description = Some(content),
                    "section" => {
                        if let Some(mut section) = current_section.take() {
                            section.tasks.extend(tasks_buffer.drain(..));
                            diagram.sections.push(section);
                        }
                        
                        let section_name = content.split(':').next().unwrap_or(&content);
                        current_section = Some(JourneySection {
                            name: section_name.to_string(),
                            tasks: Vec::new(),
                        });
                    }
                    _ => {}
                }
            }
            
            if let Some(mut section) = current_section {
                section.tasks.extend(tasks_buffer);
                diagram.sections.push(section);
            }
            
            diagram
        })
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/journey/`
- Expected count: 19 files
- Copy to: `mermaid-parser/test/journey/`

### Command
```bash
cp -r ../mermaid-samples/journey/* ./test/journey/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_journey_files(#[files("test/journey/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = journey_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = journey_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Basic validation
    assert!(diagram.sections.len() > 0 || diagram.title.is_some(), 
            "Journey should have sections or title");
}

#[test]
fn test_simple_journey() {
    let input = r#"journey
    title My working day
    section Go to work
        Make tea: 5: Me
        Go upstairs: 3: Me
        Do work: 1: Me, Cat
    section Go home
        Go downstairs: 5: Me
        Sit down: 3: Me
"#;
    
    let tokens = journey_lexer().parse(input.chars()).unwrap();
    let diagram = journey_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("My working day".to_string()));
    assert_eq!(diagram.sections.len(), 2);
    
    // Check first section
    assert_eq!(diagram.sections[0].name, "Go to work");
    assert_eq!(diagram.sections[0].tasks.len(), 3);
    
    let first_task = &diagram.sections[0].tasks[0];
    assert_eq!(first_task.name, "Make tea");
    assert_eq!(first_task.score, 5);
    assert_eq!(first_task.actors, vec!["Me"]);
    
    let third_task = &diagram.sections[0].tasks[2];
    assert_eq!(third_task.name, "Do work");
    assert_eq!(third_task.score, 1);
    assert_eq!(third_task.actors, vec!["Me", "Cat"]);
}

#[test]
fn test_task_data_parsing() {
    let (score, actors) = parse_task_data("5: Me");
    assert_eq!(score, 5);
    assert_eq!(actors, vec!["Me"]);
    
    let (score, actors) = parse_task_data("3: Me, Cat, Dog");
    assert_eq!(score, 3);
    assert_eq!(actors, vec!["Me", "Cat", "Dog"]);
    
    let (score, actors) = parse_task_data("invalid");
    assert_eq!(score, 0);
    assert_eq!(actors.len(), 0);
}

#[test]
fn test_accessibility() {
    let input = r#"journey
    accTitle: My Journey Accessibility Title
    accDescr: This journey shows user satisfaction
    title My Day
"#;
    
    let tokens = journey_lexer().parse(input.chars()).unwrap();
    let diagram = journey_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.accessibility.title, Some("My Journey Accessibility Title".to_string()));
    assert_eq!(diagram.accessibility.description, Some("This journey shows user satisfaction".to_string()));
}
```

## Success Criteria
1. ✅ Parse all 19 journey sample files successfully
2. ✅ Handle title, sections, and tasks with scores
3. ✅ Parse multiple actors per task
4. ✅ Support accessibility attributes
5. ✅ Handle score parsing (integers)
6. ✅ Process sections with multiple tasks

## Implementation Priority
**Priority 3** - Implement after Timeline as it introduces task scoring and multiple actors, which are important patterns for more complex grammars.