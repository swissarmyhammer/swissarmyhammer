# Implementation Plan: Block Diagrams

## Overview
Block diagrams represent structured data with blocks, connections, and spatial relationships.
Medium complexity grammar (290 lines) supporting various block types, styling, and layout.

## Grammar Analysis

### Key Features
- Header: `block-beta`
- Block types: Simple blocks, composite blocks, space blocks
- Connections: Arrow-based relationships between blocks
- Styling: CSS classes and inline styles
- Layout: Spatial positioning and sizing
- Labels: Text labels on connections

### Example Input
```
block-beta
columns 1
  db(("DB"))
  blockArrowId6<["&nbsp;&nbsp;&nbsp;"]>(down)
  block:ID
    A
    B["A wide one in the middle"]
    C
  end
  space
  D
  ID --> D
  C --> D
  style A fill:#969,stroke:#333,stroke-width:4px
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BlockDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub columns: Option<i32>,
    pub blocks: Vec<Block>,
    pub connections: Vec<Connection>,
    pub styles: Vec<StyleDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Simple {
        id: String,
        label: Option<String>,
        shape: BlockShape,
    },
    Composite {
        id: String,
        label: Option<String>,
        blocks: Vec<Block>,
    },
    Space {
        size: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockShape {
    Rectangle,      // Basic block
    RoundedRect,    // Rounded corners
    Rhombus,        // Diamond shape
    Circle,         // Circular
    Ellipse,        // Oval
    Cylinder,       // Database-style
    Custom(String), // Custom shape definition
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub arrow_type: ArrowType,
    pub style: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrowType {
    Normal,     // -->
    Dotted,     // -.->
    Thick,      // ==>
    Invisible,  // ~~~
    Bidirectional, // <-->
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleDefinition {
    pub target: String,
    pub properties: Vec<StyleProperty>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleProperty {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockToken {
    BlockBeta,             // "block-beta"
    Columns(i32),          // "columns 3"
    BlockStart(String),    // "block:ID"
    BlockEnd,              // "end"
    Space,                 // "space"
    SpaceSize(i32),        // Space with size
    BlockId(String),       // Block identifier
    BlockLabel(String),    // Block label in quotes or brackets
    Arrow,                 // "-->"
    DottedArrow,          // "-.->
    ThickArrow,           // "==>"
    InvisibleArrow,       // "~~~"
    BiArrow,              // "<-->"
    ArrowLabel(String),   // Arrow label
    Style,                // "style"
    StyleProperty(String, String), // Property: value pair
    Class,                // "class"
    ClassName(String),    // CSS class name
    Title(String),        // "title Text"
    AccTitle,             // "accTitle"
    AccTitleValue(String), // Accessibility title
    AccDescr,             // "accDescr"  
    AccDescrValue(String), // Accessibility description
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn block_lexer() -> impl Parser<char, Vec<BlockToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .ignored();
    
    let block_beta = text::keyword("block-beta")
        .map(|_| BlockToken::BlockBeta);
    
    let columns = text::keyword("columns")
        .then(whitespace.at_least(1))
        .then(text::int(10))
        .map(|(_, n): (_, String)| BlockToken::Columns(n.parse().unwrap_or(1)));
    
    let block_start = text::keyword("block")
        .then(just(':'))
        .then(text::ident())
        .map(|((_, _), id)| BlockToken::BlockStart(id));
    
    let block_end = text::keyword("end")
        .map(|_| BlockToken::BlockEnd);
    
    let space = text::keyword("space")
        .then(
            whitespace
                .then(text::int(10))
                .map(|(_, n): (_, String)| n.parse().unwrap_or(1))
                .or_not()
        )
        .map(|(_, size)| {
            if let Some(s) = size {
                BlockToken::SpaceSize(s)
            } else {
                BlockToken::Space
            }
        });
    
    // Block shapes and labels
    let block_with_shape = choice((
        // Cylinder/Database: db(("Label"))
        text::ident()
            .then(just('(').then(just('(')))
            .then(just('"'))
            .then(
                take_until(just('"'))
                    .collect::<String>()
            )
            .then_ignore(just('"'))
            .then_ignore(just(')').then(just(')')))
            .map(|(((id, _), _), label)| (id, Some(label), BlockShape::Cylinder)),
        
        // Rounded rectangle: A["Label"]
        text::ident()
            .then(just('['))
            .then(just('"'))
            .then(
                take_until(just('"'))
                    .collect::<String>()
            )
            .then_ignore(just('"'))
            .then_ignore(just(']'))
            .map(|(((id, _), _), label)| (id, Some(label), BlockShape::RoundedRect)),
        
        // Circle: A(("Label"))
        text::ident()
            .then(just('('))
            .then(just('"'))
            .then(
                take_until(just('"'))
                    .collect::<String>()
            )
            .then_ignore(just('"'))
            .then_ignore(just(')'))
            .map(|(((id, _), _), label)| (id, Some(label), BlockShape::Circle)),
        
        // Simple block: A
        text::ident()
            .map(|id| (id, None, BlockShape::Rectangle)),
    ))
    .map(|(id, label, shape)| {
        if let Some(label) = label {
            BlockToken::BlockLabel(format!("{}:{}", id, label))
        } else {
            BlockToken::BlockId(id)
        }
    });
    
    // Arrows
    let arrows = choice((
        text::string("<-->").map(|_| BlockToken::BiArrow),
        text::string("-->").map(|_| BlockToken::Arrow),
        text::string("-.->").map(|_| BlockToken::DottedArrow),
        text::string("==>").map(|_| BlockToken::ThickArrow),
        text::string("~~~").map(|_| BlockToken::InvisibleArrow),
    ));
    
    // Style definitions: style A fill:#969,stroke:#333
    let style = text::keyword("style")
        .then(whitespace.at_least(1))
        .then(text::ident()) // Target element
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n'))
                .collect::<String>()
        )
        .map(|(((_, id), _), props)| {
            BlockToken::StyleProperty(id, props.trim().to_string())
        });
    
    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n'))
                .collect::<String>()
        )
        .map(|(_, title)| BlockToken::Title(title.trim().to_string()));
    
    let acc_title = text::keyword("accTitle")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| BlockToken::AccTitle);
    
    let acc_descr = text::keyword("accDescr")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| BlockToken::AccDescr);
    
    let newline = just('\n')
        .map(|_| BlockToken::NewLine);
    
    choice((
        comment.ignored(),
        block_beta,
        columns,
        block_start,
        block_end,
        space,
        style,
        title,
        acc_title,
        acc_descr,
        arrows,
        block_with_shape,
        newline,
    ))
    .padded_by(whitespace)
    .repeated()
    .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Connection Parser
```rust
fn parse_connection() -> impl Parser<BlockToken, Connection, Error = Simple<BlockToken>> {
    select! { BlockToken::BlockId(from) => from }
        .then(choice((
            just(BlockToken::Arrow).map(|_| ArrowType::Normal),
            just(BlockToken::DottedArrow).map(|_| ArrowType::Dotted),
            just(BlockToken::ThickArrow).map(|_| ArrowType::Thick),
            just(BlockToken::InvisibleArrow).map(|_| ArrowType::Invisible),
            just(BlockToken::BiArrow).map(|_| ArrowType::Bidirectional),
        )))
        .then(select! { BlockToken::BlockId(to) => to })
        .then(
            // Optional arrow label
            select! { BlockToken::BlockLabel(label) => label }
                .or_not()
        )
        .map(|(((from, arrow_type), to), label)| Connection {
            from,
            to,
            label,
            arrow_type,
            style: None,
        })
}
```

### Main Parser
```rust
pub fn block_parser() -> impl Parser<BlockToken, BlockDiagram, Error = Simple<BlockToken>> {
    just(BlockToken::BlockBeta)
        .then_ignore(just(BlockToken::NewLine).or_not())
        .then(
            choice((
                // Configuration
                select! { BlockToken::Columns(n) => ("columns", n.to_string()) },
                select! { BlockToken::Title(title) => ("title", title) },
                
                // Blocks
                select! { BlockToken::BlockId(id) => ("block", id) },
                select! { BlockToken::BlockLabel(label) => ("block_label", label) },
                just(BlockToken::Space).map(|_| ("space", "1".to_string())),
                select! { BlockToken::SpaceSize(n) => ("space", n.to_string()) },
                
                // Block definitions
                select! { BlockToken::BlockStart(id) => ("block_start", id) },
                just(BlockToken::BlockEnd).map(|_| ("block_end", "".to_string())),
                
                // Connections
                parse_connection().map(|conn| ("connection", format!("{}-->{}", conn.from, conn.to))),
                
                // Styling
                select! { BlockToken::StyleProperty(target, props) => ("style", format!("{}:{}", target, props)) },
            ))
            .separated_by(just(BlockToken::NewLine))
            .allow_trailing()
        )
        .then_ignore(just(BlockToken::Eof).or_not())
        .map(|(_, statements)| {
            let mut diagram = BlockDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                columns: None,
                blocks: Vec::new(),
                connections: Vec::new(),
                styles: Vec::new(),
            };
            
            for (stmt_type, content) in statements {
                match stmt_type {
                    "title" => diagram.title = Some(content),
                    "columns" => diagram.columns = content.parse().ok(),
                    "block" => {
                        diagram.blocks.push(Block::Simple {
                            id: content,
                            label: None,
                            shape: BlockShape::Rectangle,
                        });
                    }
                    "space" => {
                        let size = content.parse().unwrap_or(1);
                        diagram.blocks.push(Block::Space { size: Some(size) });
                    }
                    // Handle other statement types...
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
- Location: `mermaid-samples/block/`
- Expected count: 115 files
- Copy to: `mermaid-parser/test/block/`

### Command
```bash
cp -r ../mermaid-samples/block/* ./test/block/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_block_files(#[files("test/block/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = block_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = block_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(
        diagram.blocks.len() > 0 || diagram.title.is_some(),
        "Block diagram should have blocks or title"
    );
}

#[test]
fn test_simple_block() {
    let input = r#"block-beta
columns 1
  A
  B["Wide Block"]
  A --> B
"#;
    
    let tokens = block_lexer().parse(input.chars()).unwrap();
    let diagram = block_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.columns, Some(1));
    assert_eq!(diagram.blocks.len(), 2);
    assert_eq!(diagram.connections.len(), 1);
}

#[test]
fn test_block_shapes() {
    let input = r#"block-beta
  A
  B["Rectangle"]
  C(("Circle"))
  D(("Database"))
"#;
    
    let tokens = block_lexer().parse(input.chars()).unwrap();
    let diagram = block_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.blocks.len(), 4);
    
    // Verify different block shapes are recognized
    for block in &diagram.blocks {
        match block {
            Block::Simple { shape, .. } => {
                assert!(matches!(shape, 
                    BlockShape::Rectangle | 
                    BlockShape::RoundedRect | 
                    BlockShape::Circle | 
                    BlockShape::Cylinder
                ));
            }
            _ => {}
        }
    }
}

#[test]
fn test_composite_blocks() {
    let input = r#"block-beta
block:group1
  A
  B
end
group1 --> C
"#;
    
    let tokens = block_lexer().parse(input.chars()).unwrap();
    let diagram = block_parser().parse(tokens).unwrap();
    
    // Should have composite block and connection
    assert!(diagram.blocks.iter().any(|b| matches!(b, Block::Composite { .. })));
    assert_eq!(diagram.connections.len(), 1);
}

#[test]
fn test_arrow_types() {
    let arrows = vec![
        ("A --> B", ArrowType::Normal),
        ("A -.-> B", ArrowType::Dotted), 
        ("A ==> B", ArrowType::Thick),
        ("A ~~~ B", ArrowType::Invisible),
        ("A <--> B", ArrowType::Bidirectional),
    ];
    
    for (arrow_str, expected_type) in arrows {
        let input = format!("block-beta\n{}", arrow_str);
        let tokens = block_lexer().parse(input.chars()).unwrap();
        let diagram = block_parser().parse(tokens).unwrap();
        
        assert_eq!(diagram.connections[0].arrow_type, expected_type);
    }
}
```

## Success Criteria
1. ✅ Parse all 115 block sample files successfully  
2. ✅ Handle different block shapes (rectangle, circle, cylinder, etc.)
3. ✅ Support composite blocks with nested content
4. ✅ Parse all arrow types and connections
5. ✅ Handle column layout configuration
6. ✅ Support space blocks for layout
7. ✅ Process styling definitions
8. ✅ Handle accessibility attributes

## Implementation Priority
**Priority 7** - Implement after Quadrant charts. Block diagrams introduce spatial layout and composite structures that are foundational for more complex architectural diagrams.