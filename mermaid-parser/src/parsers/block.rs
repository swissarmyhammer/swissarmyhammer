//! Block diagram parser implementation

use crate::common::ast::{
    AccessibilityInfo, Block, BlockArrowType, BlockConnection, BlockDiagram, BlockShape,
};
use crate::error::{ParseError, Result};
use chumsky::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum BlockToken {
    BlockBeta,                     // "block-beta"
    Columns(i32),                  // "columns 3"
    BlockStart(String),            // "block:ID"
    BlockEnd,                      // "end"
    Space,                         // "space"
    SpaceSize(i32),                // Space with size
    BlockId(String),               // Block identifier
    BlockLabel(String),            // Block label in quotes or brackets
    Arrow,                         // "-->"
    DottedArrow,                   // "-.->
    ThickArrow,                    // "==>"
    InvisibleArrow,                // "~~~"
    BiArrow,                       // "<-->"
    ArrowLabel(String),            // Arrow label
    Style,                         // "style"
    StyleProperty(String, String), // Property: value pair
    Class,                         // "class"
    ClassName(String),             // CSS class name
    Title(String),                 // "title Text"
    AccTitle,                      // "accTitle"
    AccTitleValue(String),         // Accessibility title
    AccDescr,                      // "accDescr"
    AccDescrValue(String),         // Accessibility description
    Comment(String),               // Comments
    NewLine,
    Eof,
}

pub fn parse(input: &str) -> Result<BlockDiagram> {
    let tokens = block_lexer()
        .parse(input)
        .into_result()
        .map_err(|e| ParseError::SyntaxError {
            message: "Failed to tokenize block diagram".to_string(),
            expected: vec![],
            found: format!("{:?}", e),
            line: 0,
            column: 0,
        })?;

    let result = block_parser()
        .parse(&tokens[..])
        .into_result()
        .map_err(|e| ParseError::SyntaxError {
            message: "Failed to parse block diagram".to_string(),
            expected: vec![],
            found: format!("{:?}", e),
            line: 0,
            column: 0,
        });
    result
}

fn block_lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<BlockToken>, extra::Err<Simple<'src, char>>> {
    let comment = choice((
        just("%%").then(none_of('\n').repeated()),
        just("//").then(none_of('\n').repeated()),
    ))
    .map(|_| BlockToken::Comment("".to_string()));

    let block_beta = just("block-beta").map(|_| BlockToken::BlockBeta);

    // Columns keyword with number
    let columns = text::keyword("columns")
        .padded()
        .ignore_then(text::int(10))
        .map(|n: &str| BlockToken::Columns(n.parse().unwrap_or(1)));

    // Block start: "block:ID"
    let block_start = text::keyword("block")
        .ignore_then(just(':'))
        .ignore_then(text::ident())
        .map(|id: &str| BlockToken::BlockStart(id.to_string()));

    // Block end
    let block_end = text::keyword("end").map(|_| BlockToken::BlockEnd);

    // Space blocks
    let space = text::keyword("space")
        .then(just(' ').repeated().ignore_then(text::int(10)).or_not())
        .map(|(_, size): (&str, Option<&str>)| {
            if let Some(n) = size {
                BlockToken::SpaceSize(n.parse().unwrap_or(1))
            } else {
                BlockToken::Space
            }
        });

    // Arrows
    let arrows = choice((
        just("<-->").map(|_| BlockToken::BiArrow),
        just("-->").map(|_| BlockToken::Arrow),
        just("-.->").map(|_| BlockToken::DottedArrow),
        just("==>").map(|_| BlockToken::ThickArrow),
        just("~~~").map(|_| BlockToken::InvisibleArrow),
    ));

    // Block with rounded rect label: A["Label"]
    let block_rounded = text::ident()
        .then_ignore(just('['))
        .then_ignore(just('"'))
        .then(none_of('"').repeated().collect::<String>())
        .then_ignore(just('"'))
        .then_ignore(just(']'))
        .map(|(id, label)| (id, label, BlockShape::RoundedRect));

    // Block with circle/cylinder label: A(("Label"))
    let block_circle = text::ident()
        .then_ignore(just('('))
        .then_ignore(just('('))
        .then_ignore(just('"'))
        .then(none_of('"').repeated().collect::<String>())
        .then_ignore(just('"'))
        .then_ignore(just(')'))
        .then_ignore(just(')'))
        .map(|(id, label)| (id, label, BlockShape::Circle));

    // Simple identifier (must come after more specific patterns)
    let identifier = text::ident().map(|s: &str| BlockToken::BlockId(s.to_string()));

    let newline = text::newline().map(|_| BlockToken::NewLine);

    let token = choice((
        comment,
        block_beta,
        columns,
        block_start,
        block_end,
        space,
        arrows,
        block_rounded.map(|(id, label, shape)| {
            BlockToken::BlockLabel(format!("{}:{}:{:?}", id, label, shape))
        }),
        block_circle.map(|(id, label, shape)| {
            BlockToken::BlockLabel(format!("{}:{}:{:?}", id, label, shape))
        }),
        identifier,
    ))
    .padded();

    token.or(newline).repeated().collect::<Vec<_>>()
}

fn block_parser<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, &'tokens [BlockToken], BlockDiagram, extra::Err<Simple<'tokens, BlockToken>>>
       + Clone {
    // Skip comments and newlines before block-beta
    select! {
        BlockToken::Comment(_) => (),
        BlockToken::NewLine => (),
    }
    .repeated()
    .ignore_then(just(&BlockToken::BlockBeta))
    .then_ignore(
        select! {
            BlockToken::NewLine => ()
        }
        .repeated(),
    )
    .then(any().repeated().collect::<Vec<_>>())
    .map(|(_, tokens)| {
        let mut blocks = Vec::new();
        let mut connections = Vec::new();
        let mut columns = None;
        let mut i = 0;

        while i < tokens.len() {
            match &tokens[i] {
                BlockToken::Comment(_) | BlockToken::NewLine => {
                    i += 1;
                }
                BlockToken::Columns(n) => {
                    columns = Some(*n);
                    i += 1;
                }
                BlockToken::BlockId(id) => {
                    // Check if this is a connection
                    if i + 2 < tokens.len() {
                        if let (
                            BlockToken::Arrow
                            | BlockToken::DottedArrow
                            | BlockToken::ThickArrow
                            | BlockToken::InvisibleArrow
                            | BlockToken::BiArrow,
                            BlockToken::BlockId(to),
                        ) = (&tokens[i + 1], &tokens[i + 2])
                        {
                            let arrow_type = match &tokens[i + 1] {
                                BlockToken::Arrow => BlockArrowType::Normal,
                                BlockToken::DottedArrow => BlockArrowType::Dotted,
                                BlockToken::ThickArrow => BlockArrowType::Thick,
                                BlockToken::InvisibleArrow => BlockArrowType::Invisible,
                                BlockToken::BiArrow => BlockArrowType::Bidirectional,
                                _ => unreachable!(),
                            };
                            connections.push(BlockConnection {
                                from: id.clone(),
                                to: to.clone(),
                                label: None,
                                arrow_type,
                                style: None,
                            });
                            i += 3;
                            continue;
                        }
                    }

                    // Not a connection, just a block
                    blocks.push(Block::Simple {
                        id: id.clone(),
                        label: None,
                        shape: BlockShape::Rectangle,
                    });
                    i += 1;
                }
                BlockToken::BlockLabel(label_info) => {
                    // Parse the label info format: "id:label:shape"
                    let parts: Vec<&str> = label_info.splitn(3, ':').collect();
                    if parts.len() >= 3 {
                        let shape = match parts[2] {
                            "RoundedRect" => BlockShape::RoundedRect,
                            "Circle" => BlockShape::Circle,
                            _ => BlockShape::Rectangle,
                        };
                        blocks.push(Block::Simple {
                            id: parts[0].to_string(),
                            label: Some(parts[1].to_string()),
                            shape,
                        });
                    }
                    i += 1;
                }
                BlockToken::BlockStart(id) => {
                    // Parse composite block
                    let mut inner_blocks = Vec::new();
                    i += 1;

                    while i < tokens.len() {
                        match &tokens[i] {
                            BlockToken::BlockEnd => {
                                i += 1;
                                break;
                            }
                            BlockToken::BlockId(inner_id) => {
                                inner_blocks.push(Block::Simple {
                                    id: inner_id.clone(),
                                    label: None,
                                    shape: BlockShape::Rectangle,
                                });
                                i += 1;
                            }
                            BlockToken::NewLine | BlockToken::Comment(_) => {
                                i += 1;
                            }
                            _ => {
                                i += 1;
                            }
                        }
                    }

                    blocks.push(Block::Composite {
                        id: id.clone(),
                        label: None,
                        blocks: inner_blocks,
                    });
                }
                BlockToken::Space => {
                    blocks.push(Block::Space { size: Some(1) });
                    i += 1;
                }
                BlockToken::SpaceSize(n) => {
                    blocks.push(Block::Space { size: Some(*n) });
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }

        BlockDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            columns,
            blocks,
            connections,
            styles: Vec::new(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_block() {
        let input = r#"block-beta
  a b c
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.blocks.len(), 3);
    }

    #[test]
    fn test_block_with_columns() {
        let input = r#"block-beta
columns 3
  A B C
  D E F
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.columns, Some(3));
        assert_eq!(diagram.blocks.len(), 6);
    }

    #[test]
    fn test_block_with_labels() {
        let input = r#"block-beta
  A["Label for A"]
  B(("Circle B"))
  C
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.blocks.len(), 3);

        // Check first block has label
        if let Block::Simple { id, label, shape } = &diagram.blocks[0] {
            assert_eq!(id, "A");
            assert_eq!(label, &Some("Label for A".to_string()));
            assert_eq!(shape, &BlockShape::RoundedRect);
        }
    }

    #[test]
    fn test_block_connections() {
        let input = r#"block-beta
  A B
  A --> B
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.blocks.len(), 2);
        assert_eq!(diagram.connections.len(), 1);

        let conn = &diagram.connections[0];
        assert_eq!(conn.from, "A");
        assert_eq!(conn.to, "B");
        assert_eq!(conn.arrow_type, BlockArrowType::Normal);
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

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        // Should have composite block and a single C block
        assert!(diagram
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Composite { .. })));
    }

    #[test]
    fn test_space_blocks() {
        let input = r#"block-beta
  A
  space
  B
  space 3
  C
"#;

        let result = parse(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        let diagram = result.unwrap();
        assert_eq!(diagram.blocks.len(), 5); // A, space, B, space(3), C

        // Check we have space blocks
        let space_count = diagram
            .blocks
            .iter()
            .filter(|b| matches!(b, Block::Space { .. }))
            .count();
        assert_eq!(space_count, 2);
    }

    #[test]
    fn test_real_world_example() {
        // Test with actual sample from our test data
        let input = std::fs::read_to_string("test/block/block_md_001.mermaid")
            .expect("Failed to read test file");

        let result = parse(&input);
        assert!(
            result.is_ok(),
            "Failed to parse real-world example: {:?}",
            result
        );
    }

    #[test]
    fn test_arrow_types() {
        let arrows = vec![
            ("A --> B", BlockArrowType::Normal),
            ("A -.-> B", BlockArrowType::Dotted),
            ("A ==> B", BlockArrowType::Thick),
            ("A ~~~ B", BlockArrowType::Invisible),
            ("A <--> B", BlockArrowType::Bidirectional),
        ];

        for (arrow_str, expected_type) in arrows {
            let input = format!("block-beta\n  {}", arrow_str);
            let result = parse(&input);
            assert!(result.is_ok(), "Failed to parse: {}", arrow_str);

            let diagram = result.unwrap();
            assert_eq!(diagram.connections[0].arrow_type, expected_type);
        }
    }
}
