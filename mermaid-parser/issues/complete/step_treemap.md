# Implementation Plan: Treemap Diagrams

## Overview
Treemap diagrams display hierarchical data as nested rectangles with size representing values.
TypeScript-based parser for hierarchical data visualization with space-filling algorithm.

## TypeScript Parser Analysis

### Key Features (from treemap parser.ts)
- Hierarchical structure with indentation
- Values associated with leaf nodes
- Nested categories
- Comments: `%%` for line comments
- Simple syntax focusing on structure and values

### Example Input
```
treemap
    title Company Revenue Distribution
    
    Total Revenue
        Product Sales
            Hardware
                Laptops: 15000000
                Desktops: 8000000
                Accessories: 3000000
            Software
                Licenses: 12000000
                Subscriptions: 18000000
                Support: 5000000
        Services
            Consulting: 25000000
            Training: 8000000
            Implementation: 12000000
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TreemapDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub root: TreemapNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreemapNode {
    pub name: String,
    pub value: Option<f64>,
    pub children: Vec<TreemapNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreemapToken {
    Treemap,                // "treemap"
    Title,                  // "title"
    Identifier(String),     // Node name
    Colon,                  // :
    Number(f64),            // Numeric value
    Indent(usize),          // Indentation level (spaces)
    Comment(String),        // %% comment
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Indentation-Aware Lexer
```rust
use chumsky::prelude::*;

pub fn treemap_lexer() -> impl Parser<char, Vec<TreemapToken>, Error = Simple<char>> {
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| TreemapToken::Comment(text.into_iter().collect()));
    
    // Keywords
    let keywords = choice((
        text::keyword("treemap").map(|_| TreemapToken::Treemap),
        text::keyword("title").map(|_| TreemapToken::Title),
    ));
    
    // Number (integer or float)
    let number = text::int(10)
        .then(just('.').then(text::digits(10)).or_not())
        .map(|(int, frac)| {
            let mut num_str = int.to_string();
            if let Some((_, frac)) = frac {
                num_str.push('.');
                num_str.push_str(&frac);
            }
            TreemapToken::Number(num_str.parse().unwrap())
        });
    
    // Colon
    let colon = just(':').map(|_| TreemapToken::Colon);
    
    // Line with possible indentation
    let line = just(' ')
        .repeated()
        .collect::<Vec<_>>()
        .then(
            choice((
                comment,
                keywords,
                colon.then(just(' ').repeated()).then(number)
                    .map(|((c, _), n)| vec![c, n]),
                filter(|c: &char| *c != '\n' && *c != ':')
                    .repeated()
                    .at_least(1)
                    .collect::<String>()
                    .map(|s| vec![TreemapToken::Identifier(s.trim().to_string())]),
            ))
        )
        .map(|(spaces, mut tokens)| {
            if !spaces.is_empty() && !matches!(tokens.first(), Some(TreemapToken::Comment(_))) {
                let mut result = vec![TreemapToken::Indent(spaces.len())];
                result.append(&mut tokens);
                result
            } else {
                tokens
            }
        });
    
    let newline = just('\n').map(|_| vec![TreemapToken::NewLine]);
    
    line.or(newline)
        .repeated()
        .flatten()
        .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Hierarchical Parser
```rust
pub fn treemap_parser() -> impl Parser<TreemapToken, TreemapDiagram, Error = Simple<TreemapToken>> {
    any()
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(just(TreemapToken::Eof).or_not())
        .map(|tokens| {
            let mut title = None;
            let mut i = 0;
            
            // Skip to treemap keyword
            while i < tokens.len() && !matches!(&tokens[i], TreemapToken::Treemap) {
                i += 1;
            }
            
            if i < tokens.len() {
                i += 1; // Skip "treemap"
                
                // Skip newlines
                while i < tokens.len() && matches!(&tokens[i], TreemapToken::NewLine) {
                    i += 1;
                }
                
                // Check for title
                if i < tokens.len() && matches!(&tokens[i], TreemapToken::Title) {
                    i += 1;
                    if i < tokens.len() {
                        if let TreemapToken::Identifier(t) = &tokens[i] {
                            title = Some(t.clone());
                            i += 1;
                        }
                    }
                    // Skip to next content
                    while i < tokens.len() && matches!(&tokens[i], TreemapToken::NewLine) {
                        i += 1;
                    }
                }
            }
            
            // Parse tree structure
            let (root, _) = parse_tree_nodes(&tokens[i..], 0);
            
            TreemapDiagram {
                title,
                accessibility: AccessibilityInfo::default(),
                root: root.unwrap_or_else(|| TreemapNode {
                    name: "Root".to_string(),
                    value: None,
                    children: Vec::new(),
                }),
            }
        })
}

fn parse_tree_nodes(
    tokens: &[TreemapToken],
    expected_indent: usize
) -> (Option<TreemapNode>, usize) {
    let mut i = 0;
    
    // Skip comments and newlines
    while i < tokens.len() {
        match &tokens[i] {
            TreemapToken::Comment(_) | TreemapToken::NewLine => {
                i += 1;
            }
            _ => break,
        }
    }
    
    if i >= tokens.len() {
        return (None, i);
    }
    
    // Check indentation
    let current_indent = match &tokens[i] {
        TreemapToken::Indent(level) => {
            i += 1;
            *level
        }
        TreemapToken::Identifier(_) if expected_indent == 0 => {
            0
        }
        _ => return (None, i),
    };
    
    if current_indent < expected_indent {
        return (None, 0);
    }
    
    // Parse node name
    let name = match &tokens[i] {
        TreemapToken::Identifier(n) => {
            i += 1;
            n.clone()
        }
        _ => return (None, i),
    };
    
    // Check for value
    let value = if i < tokens.len() && matches!(&tokens[i], TreemapToken::Colon) {
        i += 1;
        if i < tokens.len() {
            match &tokens[i] {
                TreemapToken::Number(v) => {
                    i += 1;
                    Some(*v)
                }
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    };
    
    // Skip to next line
    while i < tokens.len() && !matches!(&tokens[i], TreemapToken::NewLine) {
        i += 1;
    }
    if i < tokens.len() && matches!(&tokens[i], TreemapToken::NewLine) {
        i += 1;
    }
    
    // Parse children
    let mut children = Vec::new();
    let child_indent = current_indent + 4; // Assuming 4-space indentation
    
    loop {
        let start_i = i;
        
        // Peek at next indentation
        let mut peek_i = i;
        while peek_i < tokens.len() {
            match &tokens[peek_i] {
                TreemapToken::Comment(_) | TreemapToken::NewLine => {
                    peek_i += 1;
                }
                TreemapToken::Indent(level) => {
                    if *level < child_indent {
                        // End of children
                        return (Some(TreemapNode { name, value, children }), i);
                    }
                    break;
                }
                TreemapToken::Identifier(_) if current_indent == 0 => {
                    // Another root-level node
                    return (Some(TreemapNode { name, value, children }), i);
                }
                _ => break,
            }
        }
        
        if let (Some(child), consumed) = parse_tree_nodes(&tokens[i..], child_indent) {
            children.push(child);
            i += consumed;
            
            if consumed == 0 {
                break;
            }
        } else {
            break;
        }
        
        if i == start_i {
            break;
        }
    }
    
    (Some(TreemapNode { name, value, children }), i)
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/treemap/`
- Expected count: 43 files
- Copy to: `mermaid-parser/test/treemap/`

### Command
```bash
cp -r ../mermaid-samples/treemap/* ./test/treemap/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_treemap_files(#[files("test/treemap/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = treemap_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = treemap_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.root.name.is_empty(), "Should have root node");
}

#[test]
fn test_simple_treemap() {
    let input = r#"treemap
    title Budget Allocation
    
    Total Budget
        Operations: 500000
        Marketing: 300000
        Development: 700000
"#;
    
    let tokens = treemap_lexer().parse(input.chars()).unwrap();
    let diagram = treemap_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.title, Some("Budget Allocation".to_string()));
    assert_eq!(diagram.root.name, "Total Budget");
    assert_eq!(diagram.root.children.len(), 3);
    
    assert_eq!(diagram.root.children[0].name, "Operations");
    assert_eq!(diagram.root.children[0].value, Some(500000.0));
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
    
    let tokens = treemap_lexer().parse(input.chars()).unwrap();
    let diagram = treemap_parser().parse(tokens).unwrap();
    
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
fn test_mixed_nodes() {
    let input = r#"treemap
    Root
        Branch1
            Leaf1: 100
            Leaf2: 200
        Branch2: 300
        Branch3
            SubBranch
                DeepLeaf: 50
"#;
    
    let tokens = treemap_lexer().parse(input.chars()).unwrap();
    let diagram = treemap_parser().parse(tokens).unwrap();
    
    // Branch2 has a value (unusual but valid)
    assert_eq!(diagram.root.children[1].name, "Branch2");
    assert_eq!(diagram.root.children[1].value, Some(300.0));
    assert_eq!(diagram.root.children[1].children.len(), 0);
    
    // Branch3 has nested structure
    assert_eq!(diagram.root.children[2].children[0].children[0].name, "DeepLeaf");
    assert_eq!(diagram.root.children[2].children[0].children[0].value, Some(50.0));
}

#[test]
fn test_decimal_values() {
    let input = r#"treemap
    Portfolio
        Stocks: 150000.50
        Bonds: 75000.25
        Cash: 25000.75
"#;
    
    let tokens = treemap_lexer().parse(input.chars()).unwrap();
    let diagram = treemap_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.root.children[0].value, Some(150000.50));
    assert_eq!(diagram.root.children[1].value, Some(75000.25));
    assert_eq!(diagram.root.children[2].value, Some(25000.75));
}

#[test]
fn test_indentation_levels() {
    let input = r#"treemap
    A
        B
            C
                D
                    E: 1
"#;
    
    let tokens = treemap_lexer().parse(input.chars()).unwrap();
    let diagram = treemap_parser().parse(tokens).unwrap();
    
    // Verify deep nesting
    let mut current = &diagram.root;
    let expected_names = ["A", "B", "C", "D", "E"];
    
    for (i, name) in expected_names.iter().enumerate() {
        assert_eq!(current.name, *name);
        if i < expected_names.len() - 1 {
            assert_eq!(current.children.len(), 1);
            current = &current.children[0];
        }
    }
    
    assert_eq!(current.value, Some(1.0));
}
```

## Success Criteria
1. ✅ Parse all 43 treemap sample files successfully
2. ✅ Handle hierarchical structure with indentation
3. ✅ Support leaf nodes with numeric values
4. ✅ Parse nested categories correctly
5. ✅ Handle mixed nodes (branches with values)
6. ✅ Support decimal values
7. ✅ Parse title and metadata
8. ✅ Maintain hierarchy through indentation parsing

## Implementation Priority
**Priority 12** - Implement in Phase 2 as the last TypeScript parser. Treemaps combine hierarchical structure (like mindmaps) with numeric values (like charts), making them a good culmination of TypeScript parsing patterns. The indentation-based parsing is similar to Python-like syntax.