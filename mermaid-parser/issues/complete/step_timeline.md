# Implementation Plan: Timeline Diagrams

## Overview
Timeline diagrams display chronological sequences of events and periods.
Simple grammar (79 lines) with sections, periods, and events.

## Grammar Analysis

### Key Features
- Header: `timeline`
- Title support: `title My Timeline`
- Sections: `section Section Name`
- Periods: Plain text representing time periods
- Events: Prefixed with `: ` (colon-space)
- Accessibility support

### Example Input
```
timeline
    title My Working Day
    section Go to work
        Make tea     : 5: Me
        Go upstairs  : 3: Me
        Do work      : 1: Me, Cat
    section Go home
        Go downstairs : 5: Me
        Sit down      : 3: Me
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub sections: Vec<TimelineSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSection {
    pub name: String,
    pub items: Vec<TimelineItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimelineItem {
    Period(String),
    Event(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessibilityInfo {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimelineToken {
    Timeline,              // "timeline"
    Title(String),         // "title My Title"
    Section(String),       // "section Section Name"
    Period(String),        // Period text
    Event(String),         // ": Event text"
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

pub fn timeline_lexer() -> impl Parser<char, Vec<TimelineToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('#')
        .then(take_until(just('\n')))
        .ignored();
    
    let timeline_keyword = text::keyword("timeline")
        .map(|_| TimelineToken::Timeline);
    
    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n').or(end()))
                .collect::<String>()
        )
        .map(|(_, text)| TimelineToken::Title(text.trim().to_string()));
    
    let section = text::keyword("section")
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n').or(end()))
                .collect::<String>()
        )
        .map(|(_, text)| TimelineToken::Section(text.trim().to_string()));
    
    let acc_title = text::keyword("accTitle")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| TimelineToken::AccTitle);
    
    let acc_descr = text::keyword("accDescr")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| TimelineToken::AccDescr);
    
    let event = just(':')
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n').or(end()))
                .collect::<String>()
        )
        .map(|(_, text)| TimelineToken::Event(text.trim().to_string()));
    
    let period = filter(|c| !matches!(*c, '\n' | '#' | ':'))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|text| TimelineToken::Period(text.trim().to_string()))
        .filter(|token| {
            if let TimelineToken::Period(text) = token {
                !text.is_empty() && 
                !text.starts_with("timeline") &&
                !text.starts_with("title") &&
                !text.starts_with("section") &&
                !text.starts_with("accTitle") &&
                !text.starts_with("accDescr")
            } else {
                true
            }
        });
    
    let newline = just('\n')
        .map(|_| TimelineToken::NewLine);
    
    choice((
        comment.ignored(),
        timeline_keyword,
        title,
        section,
        acc_title,
        acc_descr,
        event,
        period,
        newline,
    ))
    .padded_by(just(' ').or(just('\t')).repeated())
    .repeated()
    .then_ignore(end())
    .map(|tokens| {
        tokens.into_iter()
            .filter(|token| !matches!(token, TimelineToken::Period(text) if text.is_empty()))
            .collect()
    })
}
```

## Step 3: Parser Implementation

### Main Parser
```rust
pub fn timeline_parser() -> impl Parser<TimelineToken, TimelineDiagram, Error = Simple<TimelineToken>> {
    let title = select! {
        TimelineToken::Title(text) => text,
    };
    
    let acc_title = just(TimelineToken::AccTitle)
        .then(select! {
            TimelineToken::Period(text) => text,
        })
        .map(|(_, text)| text);
    
    let acc_descr = just(TimelineToken::AccDescr)
        .then(select! {
            TimelineToken::Period(text) => text,
        })
        .map(|(_, text)| text);
    
    let period = select! {
        TimelineToken::Period(text) => TimelineItem::Period(text),
    };
    
    let event = select! {
        TimelineToken::Event(text) => TimelineItem::Event(text),
    };
    
    let timeline_item = choice((period, event));
    
    let section = select! {
        TimelineToken::Section(name) => name,
    }
    .then(
        just(TimelineToken::NewLine)
            .or_not()
            .then(timeline_item)
            .map(|(_, item)| item)
            .repeated()
    )
    .map(|(name, items)| TimelineSection { name, items });
    
    let statement = choice((
        title.map(Some),
        acc_title.map(|_| None), // Handle separately
        acc_descr.map(|_| None), // Handle separately
        section.map(|_| None),   // Handle separately
    ));
    
    // Main parser
    just(TimelineToken::Timeline)
        .then_ignore(just(TimelineToken::NewLine).or_not())
        .then(
            choice((
                title.map(|t| ("title", t)),
                acc_title.map(|t| ("acc_title", t)),
                acc_descr.map(|t| ("acc_descr", t)),
                section.map(|s| ("section", format!("section:{}", s.name))),
            ))
            .separated_by(just(TimelineToken::NewLine))
            .allow_trailing()
        )
        .then_ignore(just(TimelineToken::Eof).or_not())
        .map(|(_, statements)| {
            let mut diagram = TimelineDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                sections: Vec::new(),
            };
            
            let mut current_section: Option<TimelineSection> = None;
            
            for (stmt_type, content) in statements {
                match stmt_type {
                    "title" => diagram.title = Some(content),
                    "acc_title" => diagram.accessibility.title = Some(content),
                    "acc_descr" => diagram.accessibility.description = Some(content),
                    "section" => {
                        if let Some(section) = current_section.take() {
                            diagram.sections.push(section);
                        }
                        let section_name = content.strip_prefix("section:").unwrap_or(&content);
                        current_section = Some(TimelineSection {
                            name: section_name.to_string(),
                            items: Vec::new(),
                        });
                    }
                    _ => {}
                }
            }
            
            if let Some(section) = current_section {
                diagram.sections.push(section);
            }
            
            diagram
        })
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/timeline/`
- Expected count: 25 files
- Copy to: `mermaid-parser/test/timeline/`

### Command
```bash
cp -r ../mermaid-samples/timeline/* ./test/timeline/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_timeline_files(#[files("test/timeline/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = timeline_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = timeline_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Basic validation
    assert!(diagram.sections.len() > 0 || diagram.title.is_some(), 
            "Timeline should have sections or title");
}

#[test]
fn test_simple_timeline() {
    let input = r#"timeline
    title My Day
    section Morning
        Wake up
        : Brush teeth
    section Evening
        Dinner
        : Sleep
"#;
    
    let tokens = timeline_lexer().parse(input.chars()).unwrap();
    let diagram = timeline_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("My Day".to_string()));
    assert_eq!(diagram.sections.len(), 2);
    assert_eq!(diagram.sections[0].name, "Morning");
    assert_eq!(diagram.sections[0].items.len(), 2);
    
    match &diagram.sections[0].items[0] {
        TimelineItem::Period(text) => assert_eq!(text, "Wake up"),
        _ => panic!("Expected period"),
    }
    
    match &diagram.sections[0].items[1] {
        TimelineItem::Event(text) => assert_eq!(text, "Brush teeth"),
        _ => panic!("Expected event"),
    }
}

#[test]
fn test_accessibility() {
    let input = r#"timeline
    accTitle: Timeline Accessibility Title
    accDescr: This timeline shows my daily routine
    title My Day
"#;
    
    let tokens = timeline_lexer().parse(input.chars()).unwrap();
    let diagram = timeline_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.accessibility.title, Some("Timeline Accessibility Title".to_string()));
    assert_eq!(diagram.accessibility.description, Some("This timeline shows my daily routine".to_string()));
}
```

## Success Criteria
1. ✅ Parse all 25 timeline sample files successfully
2. ✅ Handle title, sections, periods, and events
3. ✅ Support accessibility attributes
4. ✅ Distinguish between periods and events (colon prefix)
5. ✅ Handle empty sections and missing titles
6. ✅ Process comments correctly

## Implementation Priority
**Priority 2** - Implement after Sankey as it's still relatively simple but introduces concepts like sections and structured content that will be useful for more complex grammars.