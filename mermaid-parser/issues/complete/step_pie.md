# Implementation Plan: Pie Charts

## Overview
Pie charts represent data as slices of a circle showing proportional values.
Simple TypeScript-based parser (no jison) with straightforward data format.

## Parser Analysis

### Key Features  
- Header: `pie` or `pie title Chart Title`
- Data format: `"Label" : value`
- Percentage display: `showData`
- Title support: inline or separate

### Example Input
```
pie title NETFLIX
    "Time spent looking for movie" : 90
    "Time spent watching it" : 10
```

```
pie showData
    "A" : 386
    "B" : 85
    "C" : 15
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PieDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub show_data: bool,
    pub data: Vec<PieSlice>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PieSlice {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PieToken {
    Pie,                   // "pie"
    Title(String),         // "title Chart Title" 
    ShowData,              // "showData"
    Label(String),         // Quoted label
    Value(f64),            // Numeric value
    Colon,                 // ":"
    AccTitle,              // "accTitle"
    AccTitleValue(String), // Accessibility title
    AccDescr,              // "accDescr"
    AccDescrValue(String), // Accessibility description
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn pie_lexer() -> impl Parser<char, Vec<PieToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .ignored();
    
    let pie_keyword = text::keyword("pie")
        .map(|_| PieToken::Pie);
    
    let show_data = text::keyword("showData")
        .map(|_| PieToken::ShowData);
    
    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n'))
                .collect::<String>()
        )
        .map(|(_, title)| PieToken::Title(title.trim().to_string()));
    
    // Inline title: pie title Chart Name
    let pie_with_title = text::keyword("pie")
        .then(whitespace.at_least(1))
        .then(text::keyword("title"))
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n'))
                .collect::<String>()
        )
        .map(|((((_, _), _), _), title)| PieToken::Title(title.trim().to_string()));
    
    // Quoted labels
    let quoted_label = just('"')
        .ignore_then(
            filter(|c| *c != '"')
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(PieToken::Label);
    
    // Numeric values (integers and floats)
    let number = text::int(10)
        .then(
            just('.')
                .then(text::digits(10))
                .or_not()
        )
        .collect::<String>()
        .map(|n| PieToken::Value(n.parse().unwrap_or(0.0)));
    
    let colon = just(':')
        .map(|_| PieToken::Colon);
    
    let acc_title = text::keyword("accTitle")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| PieToken::AccTitle);
    
    let acc_descr = text::keyword("accDescr")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| PieToken::AccDescr);
    
    let newline = just('\n')
        .map(|_| PieToken::NewLine);
    
    choice((
        comment.ignored(),
        pie_with_title,
        pie_keyword,
        show_data,
        title,
        acc_title,
        acc_descr,
        quoted_label,
        number,
        colon,
        newline,
    ))
    .padded_by(whitespace)
    .repeated()
    .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Data Entry Parser
```rust
fn parse_pie_entry() -> impl Parser<PieToken, PieSlice, Error = Simple<PieToken>> {
    select! { PieToken::Label(label) => label }
        .then_ignore(just(PieToken::Colon))
        .then(select! { PieToken::Value(value) => value })
        .map(|(label, value)| PieSlice { label, value })
}
```

### Main Parser
```rust
pub fn pie_parser() -> impl Parser<PieToken, PieDiagram, Error = Simple<PieToken>> {
    // Handle pie with inline title or separate
    choice((
        just(PieToken::Pie)
            .then(select! { PieToken::Title(title) => title }.or_not()),
        select! { PieToken::Title(title) => (PieToken::Pie, Some(title)) },
    ))
    .then_ignore(just(PieToken::NewLine).or_not())
    .then(
        choice((
            // Configuration
            just(PieToken::ShowData).map(|_| ("show_data", "true".to_string())),
            select! { PieToken::Title(title) => ("title", title) },
            
            // Data entries
            parse_pie_entry().map(|entry| ("entry", format!("{}:{}", entry.label, entry.value))),
            
            // Accessibility
            just(PieToken::AccTitle)
                .then(select! { PieToken::Label(title) => title })
                .map(|(_, title)| ("acc_title", title)),
            just(PieToken::AccDescr)
                .then(select! { PieToken::Label(desc) => desc })
                .map(|(_, desc)| ("acc_descr", desc)),
        ))
        .separated_by(just(PieToken::NewLine))
        .allow_trailing()
    )
    .then_ignore(just(PieToken::Eof).or_not())
    .map(|((_, inline_title), statements)| {
        let mut diagram = PieDiagram {
            title: inline_title,
            accessibility: AccessibilityInfo::default(),
            show_data: false,
            data: Vec::new(),
        };
        
        for (stmt_type, content) in statements {
            match stmt_type {
                "title" => diagram.title = Some(content),
                "show_data" => diagram.show_data = true,
                "acc_title" => diagram.accessibility.title = Some(content),
                "acc_descr" => diagram.accessibility.description = Some(content),
                "entry" => {
                    let parts: Vec<&str> = content.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let label = parts[0].to_string();
                        let value = parts[1].parse().unwrap_or(0.0);
                        diagram.data.push(PieSlice { label, value });
                    }
                }
                _ => {}
            }
        }
        
        diagram
    })
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/pie/`
- Expected count: 64 files
- Copy to: `mermaid-parser/test/pie/`

### Command
```bash
cp -r ../mermaid-samples/pie/* ./test/pie/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_pie_files(#[files("test/pie/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = pie_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = pie_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(
        diagram.data.len() > 0 || diagram.title.is_some(),
        "Pie chart should have data or title"
    );
}

#[test]
fn test_simple_pie() {
    let input = r#"pie title NETFLIX
    "Time spent looking for movie" : 90
    "Time spent watching it" : 10
"#;
    
    let tokens = pie_lexer().parse(input.chars()).unwrap();
    let diagram = pie_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("NETFLIX".to_string()));
    assert_eq!(diagram.data.len(), 2);
    assert_eq!(diagram.data[0].label, "Time spent looking for movie");
    assert_eq!(diagram.data[0].value, 90.0);
    assert_eq!(diagram.data[1].value, 10.0);
}

#[test]
fn test_pie_with_show_data() {
    let input = r#"pie showData
    "A" : 386
    "B" : 85.5
    "C" : 15
"#;
    
    let tokens = pie_lexer().parse(input.chars()).unwrap();
    let diagram = pie_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.show_data, true);
    assert_eq!(diagram.data.len(), 3);
    assert_eq!(diagram.data[1].value, 85.5); // Test decimal values
}

#[test]
fn test_pie_separate_title() {
    let input = r#"pie
title My Chart Title
    "Category A" : 40
    "Category B" : 60
"#;
    
    let tokens = pie_lexer().parse(input.chars()).unwrap();
    let diagram = pie_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("My Chart Title".to_string()));
    assert_eq!(diagram.data.len(), 2);
}

#[test]
fn test_accessibility() {
    let input = r#"pie
accTitle: Pie Chart Accessibility Title
accDescr: This chart shows distribution
    "A" : 50
    "B" : 50
"#;
    
    let tokens = pie_lexer().parse(input.chars()).unwrap();
    let diagram = pie_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.accessibility.title, Some("Pie Chart Accessibility Title".to_string()));
    assert_eq!(diagram.accessibility.description, Some("This chart shows distribution".to_string()));
}

#[test]
fn test_value_parsing() {
    let values = vec![
        ("42", 42.0),
        ("3.14", 3.14),
        ("100", 100.0),
        ("0.5", 0.5),
    ];
    
    for (input, expected) in values {
        let parsed: f64 = input.parse().unwrap();
        assert!((parsed - expected).abs() < 0.001);
    }
}
```

## Success Criteria
1. ✅ Parse all 64 pie sample files successfully
2. ✅ Handle inline and separate title formats
3. ✅ Support showData configuration
4. ✅ Parse quoted labels and numeric values
5. ✅ Handle decimal numbers correctly
6. ✅ Support accessibility attributes
7. ✅ Validate data structure integrity

## Implementation Priority
**Priority 4** - Implement early as it's simple and establishes patterns for data visualization diagrams. TypeScript-based parser means we need to adapt the parsing approach but the syntax is straightforward.