# Implementation Plan: Mindmap Diagrams

## Overview
Mindmap diagrams represent hierarchical information in a tree structure radiating from a central concept.
Simple grammar (127 lines) with nested nodes, icons, and styling.

## Grammar Analysis

### Key Features
- Header: `mindmap`
- Node types: Various shapes with `()`, `[]`, `{}`, `(())`, `(-)`, `-)`, etc.
- Node content: Text descriptions and markdown support
- Icons: `::icon(icon-name)`
- Classes: `:::className` for styling
- Hierarchical structure through indentation

### Example Input
```
mindmap
  root((mindmap))
    Origins
      Long history
      ::icon(fa fa-book)
      Popularisation
        British popular psychology author Tony Buzan
    Research
      On effectiveness<br/>and features
      On Automatic creation
        Uses
            Creative techniques
            Strategic planning
            Argument mapping
    Tools
      Pen and paper
      Mermaid
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct MindmapDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub root: MindmapNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MindmapNode {
    pub id: String,
    pub text: String,
    pub shape: NodeShape,
    pub icon: Option<String>,
    pub class: Option<String>,
    pub children: Vec<MindmapNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeShape {
    Default,        // No brackets
    Square,         // [text]
    Rounded,        // (text)
    Circle,         // ((text))
    Cloud,          // (-text-)
    Bang,           // ))text((
    Hexagon,        // {{text}}
}

#[derive(Debug, Clone, PartialEq)]
pub enum MindmapToken {
    Mindmap,              // "mindmap"
    NodeStart(NodeShape), // Various opening brackets
    NodeEnd(NodeShape),   // Various closing brackets
    NodeId(String),       // Node identifier
    NodeDescr(String),    // Node description/text
    Icon(String),         // Icon specification
    Class(String),        // CSS class
    SpaceLine,            // Blank line
    SpaceList,            // Spaces (for indentation)
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn mindmap_lexer() -> impl Parser<char, Vec<MindmapToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .ignored();
    
    let mindmap_keyword = text::keyword("mindmap")
        .map(|_| MindmapToken::Mindmap);
    
    // Node shape starters
    let node_start = choice((
        text::string("))").map(|_| MindmapToken::NodeStart(NodeShape::Bang)),
        text::string("(-").map(|_| MindmapToken::NodeStart(NodeShape::Cloud)),
        text::string("-)").map(|_| MindmapToken::NodeStart(NodeShape::Cloud)),
        text::string("((").map(|_| MindmapToken::NodeStart(NodeShape::Circle)),
        text::string("{{").map(|_| MindmapToken::NodeStart(NodeShape::Hexagon)),
        just('(').map(|_| MindmapToken::NodeStart(NodeShape::Rounded)),
        just('[').map(|_| MindmapToken::NodeStart(NodeShape::Square)),
    ));
    
    // Node shape enders
    let node_end = choice((
        text::string("))").map(|_| MindmapToken::NodeEnd(NodeShape::Bang)),
        text::string("-)").map(|_| MindmapToken::NodeEnd(NodeShape::Cloud)),
        text::string("(-").map(|_| MindmapToken::NodeEnd(NodeShape::Cloud)),
        text::string("))").map(|_| MindmapToken::NodeEnd(NodeShape::Circle)),
        text::string("}}").map(|_| MindmapToken::NodeEnd(NodeShape::Hexagon)),
        just(')').map(|_| MindmapToken::NodeEnd(NodeShape::Rounded)),
        just(']').map(|_| MindmapToken::NodeEnd(NodeShape::Square)),
    ));
    
    // Icon specification: ::icon(fa fa-book)
    let icon = text::string("::icon(")
        .ignore_then(
            take_until(just(')'))
                .collect::<String>()
        )
        .then_ignore(just(')'))
        .map(MindmapToken::Icon);
    
    // Class specification: :::className
    let class = text::string(":::")
        .ignore_then(
            filter(|c| !c.is_whitespace() && *c != '\n')
                .repeated()
                .at_least(1)
                .collect::<String>()
        )
        .map(MindmapToken::Class);
    
    // Node ID (unbracketed text)
    let node_id = filter(|c| !matches!(*c, '(' | '[' | '\n' | ')' | '{' | '}'))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|text| MindmapToken::NodeId(text.trim().to_string()));
    
    // Node description (quoted or markdown)
    let node_descr_quoted = just('"')
        .ignore_then(
            filter(|c| *c != '"')
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(MindmapToken::NodeDescr);
    
    let node_descr_markdown = just('"')
        .then(just('`'))
        .ignore_then(
            take_until(just('`').then(just('"')))
                .collect::<String>()
        )
        .then_ignore(just('`'))
        .then_ignore(just('"'))
        .map(MindmapToken::NodeDescr);
    
    let spaceline = whitespace
        .then(just('\n'))
        .map(|_| MindmapToken::SpaceLine);
    
    let spacelist = just(' ')
        .repeated()
        .at_least(1)
        .map(|spaces| MindmapToken::SpaceList);
    
    let newline = just('\n')
        .map(|_| MindmapToken::NewLine);
    
    choice((
        comment.ignored(),
        mindmap_keyword,
        icon,
        class,
        node_start,
        node_end,
        node_descr_markdown,
        node_descr_quoted,
        node_id,
        spaceline,
        spacelist,
        newline,
    ))
    .repeated()
    .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Indentation-Based Hierarchy Parser
```rust
fn parse_mindmap_nodes(
    tokens: &[MindmapToken], 
    current_indent: usize
) -> (Vec<MindmapNode>, usize) {
    let mut nodes = Vec::new();
    let mut i = 0;
    
    while i < tokens.len() {
        match &tokens[i] {
            MindmapToken::SpaceList => {
                // Count indentation level
                let indent = count_spaces(&tokens[i]);
                if indent <= current_indent {
                    // End of this level
                    break;
                }
                i += 1;
            }
            MindmapToken::NodeStart(shape) => {
                // Parse a node
                let (node, consumed) = parse_single_node(&tokens[i..], shape.clone());
                nodes.push(node);
                i += consumed;
            }
            MindmapToken::NodeId(text) => {
                // Simple text node without brackets
                let node = MindmapNode {
                    id: generate_id(),
                    text: text.clone(),
                    shape: NodeShape::Default,
                    icon: None,
                    class: None,
                    children: Vec::new(),
                };
                nodes.push(node);
                i += 1;
            }
            MindmapToken::NewLine | MindmapToken::SpaceLine => {
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    
    (nodes, i)
}

fn parse_single_node(tokens: &[MindmapToken], shape: NodeShape) -> (MindmapNode, usize) {
    let mut i = 1; // Skip NodeStart
    let mut text = String::new();
    let mut icon = None;
    let mut class = None;
    
    // Parse node content
    loop {
        if i >= tokens.len() {
            break;
        }
        
        match &tokens[i] {
            MindmapToken::NodeDescr(t) | MindmapToken::NodeId(t) => {
                text.push_str(t);
                i += 1;
            }
            MindmapToken::Icon(ic) => {
                icon = Some(ic.clone());
                i += 1;
            }
            MindmapToken::Class(cl) => {
                class = Some(cl.clone());
                i += 1;
            }
            MindmapToken::NodeEnd(_) => {
                i += 1;
                break;
            }
            _ => {
                break;
            }
        }
    }
    
    // Parse children (if any)
    let mut children = Vec::new();
    if i < tokens.len() {
        let (child_nodes, consumed) = parse_mindmap_nodes(&tokens[i..], 0);
        children = child_nodes;
        i += consumed;
    }
    
    let node = MindmapNode {
        id: generate_id(),
        text: text.trim().to_string(),
        shape,
        icon,
        class,
        children,
    };
    
    (node, i)
}
```

### Main Parser
```rust
pub fn mindmap_parser() -> impl Parser<MindmapToken, MindmapDiagram, Error = Simple<MindmapToken>> {
    just(MindmapToken::Mindmap)
        .then_ignore(
            choice((
                just(MindmapToken::NewLine),
                just(MindmapToken::SpaceLine),
            ))
            .repeated()
        )
        .then(
            // Parse all tokens into a flat list first
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(MindmapToken::Eof).or_not())
        .map(|(_, tokens)| {
            // Build hierarchical structure from tokens
            let (nodes, _) = parse_mindmap_nodes(&tokens, 0);
            
            // The first node is the root
            let root = nodes.into_iter().next().unwrap_or_else(|| {
                MindmapNode {
                    id: "root".to_string(),
                    text: "Root".to_string(),
                    shape: NodeShape::Default,
                    icon: None,
                    class: None,
                    children: Vec::new(),
                }
            });
            
            MindmapDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                root,
            }
        })
}

fn generate_id() -> String {
    // Generate unique node ID
    format!("node_{}", uuid::Uuid::new_v4())
}

fn count_spaces(token: &MindmapToken) -> usize {
    // Count indentation level from SpaceList token
    match token {
        MindmapToken::SpaceList => 2, // Simplified - count actual spaces in real impl
        _ => 0,
    }
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/mindmap/`
- Expected count: 46 files
- Copy to: `mermaid-parser/test/mindmap/`

### Command
```bash
cp -r ../mermaid-samples/mindmap/* ./test/mindmap/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_mindmap_files(#[files("test/mindmap/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = mindmap_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = mindmap_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.root.text.is_empty(), "Root node should have text");
}

#[test]
fn test_simple_mindmap() {
    let input = r#"mindmap
  root((mindmap))
    Origins
      Long history
      Popularisation
        British author
    Research
      On effectiveness
      On features
"#;
    
    let tokens = mindmap_lexer().parse(input.chars()).unwrap();
    let diagram = mindmap_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.root.text, "mindmap");
    assert_eq!(diagram.root.shape, NodeShape::Circle);
    assert_eq!(diagram.root.children.len(), 2); // Origins, Research
    
    let origins = &diagram.root.children[0];
    assert_eq!(origins.text, "Origins");
    assert_eq!(origins.children.len(), 2); // Long history, Popularisation
}

#[test]
fn test_node_shapes() {
    let shapes = vec![
        ("(text)", NodeShape::Rounded),
        ("[text]", NodeShape::Square),
        ("((text))", NodeShape::Circle),
        ("{{text}}", NodeShape::Hexagon),
        ("(-text-)", NodeShape::Cloud),
        ("))text((", NodeShape::Bang),
    ];
    
    for (input, expected_shape) in shapes {
        let full_input = format!("mindmap\n  {}", input);
        let tokens = mindmap_lexer().parse(full_input.chars()).unwrap();
        let diagram = mindmap_parser().parse(tokens).unwrap();
        
        assert_eq!(diagram.root.children[0].shape, expected_shape);
    }
}

#[test]
fn test_icons_and_classes() {
    let input = r#"mindmap
  root
    ::icon(fa fa-book)
    Node with icon
    :::myClass
    Node with class
"#;
    
    let tokens = mindmap_lexer().parse(input.chars()).unwrap();
    let diagram = mindmap_parser().parse(tokens).unwrap();
    
    // Check icon and class are parsed
    let has_icon = diagram.root.children.iter().any(|n| n.icon.is_some());
    let has_class = diagram.root.children.iter().any(|n| n.class.is_some());
    
    assert!(has_icon, "Should have node with icon");
    assert!(has_class, "Should have node with class");
}
```

## Success Criteria
1. ✅ Parse all 46 mindmap sample files successfully
2. ✅ Handle hierarchical structure through indentation
3. ✅ Support all node shapes (square, round, circle, hexagon, cloud, bang)
4. ✅ Parse icons and CSS classes
5. ✅ Handle markdown in node descriptions
6. ✅ Build correct parent-child relationships
7. ✅ Support nested levels correctly

## Implementation Priority
**Priority 7** - Implement after simpler diagrams. Mindmaps introduce hierarchical parsing patterns that are useful for other tree-based diagrams but require indentation handling.