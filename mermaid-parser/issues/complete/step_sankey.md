# Implementation Plan: Sankey Diagrams

## Overview
Sankey diagrams represent flow data in CSV format with three columns: source, target, value.
This is the simplest grammar (66 lines) and follows RFC 4180 CSV specification.

## Grammar Analysis

### Key Features
- CSV-based format: `source,target,value`
- Supports quoted fields with escaped quotes
- Allows blank lines for visual formatting
- Header: `sankey-beta`

### Example Input
```
sankey-beta
Agricultural 'waste',Bio-conversion,124.729
Bio-conversion,Liquid,0.597
Bio-conversion,Losses,26.862
"Quoted Source","Quoted Target",123.45
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SankeyDiagram {
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SankeyNode {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SankeyLink {
    pub source: String,
    pub target: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SankeyToken {
    Header,           // "sankey-beta"
    Comma,           // ","
    NewLine,         // "\n" | "\r\n"
    QuotedText(String),    // "text"
    UnquotedText(String),  // text
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn sankey_lexer() -> impl Parser<char, Vec<SankeyToken>, Error = Simple<char>> {
    let header = text::keyword("sankey-beta")
        .map(|_| SankeyToken::Header);
    
    let comma = just(',')
        .map(|_| SankeyToken::Comma);
    
    let newline = choice((
        text::newline().map(|_| '\n'),
        just('\r').then(just('\n')).map(|_| '\n'),
    )).map(|_| SankeyToken::NewLine);
    
    let quoted_text = just('"')
        .ignore_then(
            filter(|c| *c != '"')
                .or(just('"').then(just('"')).map(|_| '"')) // Handle escaped quotes
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(SankeyToken::QuotedText);
    
    let unquoted_text = filter(|c| !matches!(*c, ',' | '\n' | '\r' | '"'))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(SankeyToken::UnquotedText);
    
    choice((
        header,
        comma,
        newline,
        quoted_text,
        unquoted_text,
    ))
    .repeated()
    .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Main Parser
```rust
pub fn sankey_parser() -> impl Parser<SankeyToken, SankeyDiagram, Error = Simple<SankeyToken>> {
    let field = select! {
        SankeyToken::QuotedText(text) => text,
        SankeyToken::UnquotedText(text) => text,
    };
    
    let record = field.clone()  // source
        .then_ignore(just(SankeyToken::Comma))
        .then(field.clone())    // target
        .then_ignore(just(SankeyToken::Comma))
        .then(field)            // value
        .map(|((source, target), value_str)| {
            let value = value_str.trim().parse::<f64>().unwrap_or(0.0);
            SankeyLink {
                source: source.trim().to_string(),
                target: target.trim().to_string(),
                value,
            }
        });
    
    let csv_line = record
        .then_ignore(just(SankeyToken::NewLine).or_not());
    
    just(SankeyToken::Header)
        .then_ignore(just(SankeyToken::NewLine))
        .then(csv_line.repeated())
        .then_ignore(just(SankeyToken::Eof).or_not())
        .map(|(_, links)| {
            let mut nodes = std::collections::HashSet::new();
            for link in &links {
                nodes.insert(link.source.clone());
                nodes.insert(link.target.clone());
            }
            
            SankeyDiagram {
                nodes: nodes.into_iter()
                    .map(|name| SankeyNode {
                        id: name.clone(),
                        name,
                    })
                    .collect(),
                links,
            }
        })
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/sankey/`
- Expected count: 7 files
- Copy to: `mermaid-parser/test/sankey/`

### Command
```bash
cp -r ../mermaid-samples/sankey/* ./test/sankey/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_sankey_files(#[files("test/sankey/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = sankey_lexer().parse(content.chars()).unwrap();
    let diagram = sankey_parser().parse(tokens).unwrap();
    
    // Verify basic structure
    assert!(!diagram.nodes.is_empty(), "Diagram should have nodes");
    assert!(!diagram.links.is_empty(), "Diagram should have links");
    
    // Verify all links reference existing nodes
    let node_names: std::collections::HashSet<_> = 
        diagram.nodes.iter().map(|n| &n.name).collect();
    
    for link in &diagram.links {
        assert!(node_names.contains(&link.source), 
                "Link source '{}' not found in nodes", link.source);
        assert!(node_names.contains(&link.target), 
                "Link target '{}' not found in nodes", link.target);
        assert!(link.value >= 0.0, "Link value should be non-negative");
    }
}

#[test]
fn test_simple_sankey() {
    let input = r#"sankey-beta
A,B,10
B,C,5
"#;
    
    let tokens = sankey_lexer().parse(input.chars()).unwrap();
    let diagram = sankey_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.nodes.len(), 3);
    assert_eq!(diagram.links.len(), 2);
    assert_eq!(diagram.links[0].value, 10.0);
    assert_eq!(diagram.links[1].value, 5.0);
}

#[test]
fn test_quoted_fields() {
    let input = r#"sankey-beta
"Source Node","Target Node",25.5
"Another ""Quoted"" Source",Destination,15.0
"#;
    
    let tokens = sankey_lexer().parse(input.chars()).unwrap();
    let diagram = sankey_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.links[0].source, "Source Node");
    assert_eq!(diagram.links[0].target, "Target Node");
    assert_eq!(diagram.links[1].source, "Another \"Quoted\" Source");
}
```

## Success Criteria
1. ✅ Parse all 7 sankey sample files successfully
2. ✅ Handle quoted and unquoted text fields
3. ✅ Process escaped quotes correctly (RFC 4180 compliance)
4. ✅ Generate correct node and link structures
5. ✅ Validate numeric values
6. ✅ Handle empty lines gracefully

## Implementation Priority
**Priority 1** - Start with this grammar as it's the simplest and will establish the basic parser patterns for other grammars.