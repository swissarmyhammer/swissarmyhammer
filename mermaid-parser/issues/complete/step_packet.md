# Implementation Plan: Packet Diagrams

## Overview
Packet diagrams represent network packet structures with bit-level field definitions.
TypeScript-based parser focusing on binary data representation and packet field layouts.

## TypeScript Parser Analysis

### Key Features (from packet parser.ts)
- Packet structure definition with bit fields
- Field ranges (e.g., 0-15: Source Port)
- Multi-bit fields spanning ranges
- Optional field descriptions
- Comments: `%%` for line comments

### Example Input
```
packet-beta
0-15: "Source Port"
16-31: "Destination Port"
32-63: "Sequence Number"
64-95: "Acknowledgment Number"
96-99: "Data Offset"
100-105: "Reserved"
106: "URG"
107: "ACK"
108: "PSH"
109: "RST"
110: "SYN"
111: "FIN"
112-127: "Window"
128-143: "Checksum"
144-159: "Urgent Pointer"
160-191: "(Options and Padding)"
192-223: "data"
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PacketDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub fields: Vec<PacketField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketField {
    pub start_bit: u32,
    pub end_bit: u32,
    pub name: String,
    pub is_optional: bool, // Indicated by parentheses
}

#[derive(Debug, Clone, PartialEq)]
pub enum PacketToken {
    PacketBeta,             // "packet-beta"
    Number(u32),            // Bit position number
    Dash,                   // -
    Colon,                  // :
    QuotedString(String),   // "field name"
    ParenString(String),    // (optional field)
    Identifier(String),     // Unquoted field name
    Comment(String),        // %% comment
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn packet_lexer() -> impl Parser<char, Vec<PacketToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| PacketToken::Comment(text.into_iter().collect()));
    
    let packet_beta = text::keyword("packet-beta")
        .map(|_| PacketToken::PacketBeta);
    
    // Number (bit position)
    let number = text::int(10)
        .map(|s: &str| PacketToken::Number(s.parse().unwrap()));
    
    // Quoted string
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(PacketToken::QuotedString);
    
    // Parenthetical string (optional fields)
    let paren_string = just('(')
        .ignore_then(
            none_of(")")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just(')'))
        .map(PacketToken::ParenString);
    
    // Identifier (unquoted field names)
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_' || *c == ' '
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(|s| PacketToken::Identifier(s.trim().to_string()));
    
    let dash = just('-').map(|_| PacketToken::Dash);
    let colon = just(':').map(|_| PacketToken::Colon);
    
    let newline = just('\n').map(|_| PacketToken::NewLine);
    
    let token = choice((
        comment,
        packet_beta,
        number,
        quoted_string,
        paren_string,
        dash,
        colon,
        identifier,
    ));
    
    whitespace
        .ignore_then(token)
        .or(newline)
        .repeated()
        .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Simple Field Parser
```rust
pub fn packet_parser() -> impl Parser<PacketToken, PacketDiagram, Error = Simple<PacketToken>> {
    just(PacketToken::PacketBeta)
        .then_ignore(
            filter(|t| matches!(t, PacketToken::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(PacketToken::Eof).or_not())
        .map(|(_, tokens)| {
            let mut fields = Vec::new();
            let mut i = 0;
            
            while i < tokens.len() {
                match &tokens[i] {
                    PacketToken::Comment(_) | PacketToken::NewLine => {
                        i += 1;
                    }
                    PacketToken::Number(start) => {
                        if let Some((field, consumed)) = parse_field(&tokens[i..], *start) {
                            fields.push(field);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            
            PacketDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                fields,
            }
        })
}

fn parse_field(tokens: &[PacketToken], start_bit: u32) -> Option<(PacketField, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip start number
    
    // Check for range or single bit
    let end_bit = if matches!(&tokens[i], PacketToken::Dash) {
        i += 1;
        match &tokens[i] {
            PacketToken::Number(end) => {
                i += 1;
                *end
            }
            _ => return None,
        }
    } else {
        start_bit // Single bit field
    };
    
    // Expect colon
    if !matches!(&tokens[i], PacketToken::Colon) {
        return None;
    }
    i += 1;
    
    // Parse field name
    let (name, is_optional) = match &tokens[i] {
        PacketToken::QuotedString(name) => {
            i += 1;
            (name.clone(), false)
        }
        PacketToken::ParenString(name) => {
            i += 1;
            (name.clone(), true)
        }
        PacketToken::Identifier(name) => {
            i += 1;
            (name.clone(), false)
        }
        _ => return None,
    };
    
    Some((
        PacketField {
            start_bit,
            end_bit,
            name,
            is_optional,
        },
        i,
    ))
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/packet/`
- Expected count: 29 files
- Copy to: `mermaid-parser/test/packet/`

### Command
```bash
cp -r ../mermaid-samples/packet/* ./test/packet/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_packet_files(#[files("test/packet/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = packet_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = packet_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.fields.is_empty(), "Should have at least one field");
}

#[test]
fn test_simple_packet() {
    let input = r#"packet-beta
0-15: "Source Port"
16-31: "Destination Port"
32-63: "Sequence Number"
"#;
    
    let tokens = packet_lexer().parse(input.chars()).unwrap();
    let diagram = packet_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.fields.len(), 3);
    
    let source_port = &diagram.fields[0];
    assert_eq!(source_port.start_bit, 0);
    assert_eq!(source_port.end_bit, 15);
    assert_eq!(source_port.name, "Source Port");
    assert!(!source_port.is_optional);
}

#[test]
fn test_single_bit_fields() {
    let input = r#"packet-beta
0: "Version"
1: "IHL"
2: "Type of Service"
"#;
    
    let tokens = packet_lexer().parse(input.chars()).unwrap();
    let diagram = packet_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.fields.len(), 3);
    
    for field in &diagram.fields {
        assert_eq!(field.start_bit, field.end_bit);
    }
}

#[test]
fn test_optional_fields() {
    let input = r#"packet-beta
0-31: "Header"
32-63: "(Options)"
64-95: "Data"
"#;
    
    let tokens = packet_lexer().parse(input.chars()).unwrap();
    let diagram = packet_parser().parse(tokens).unwrap();
    
    assert!(!diagram.fields[0].is_optional);
    assert!(diagram.fields[1].is_optional);
    assert!(!diagram.fields[2].is_optional);
}

#[test]
fn test_unquoted_identifiers() {
    let input = r#"packet-beta
0-7: Source
8-15: Destination
16-31: Payload
"#;
    
    let tokens = packet_lexer().parse(input.chars()).unwrap();
    let diagram = packet_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.fields[0].name, "Source");
    assert_eq!(diagram.fields[1].name, "Destination");
    assert_eq!(diagram.fields[2].name, "Payload");
}

#[test]
fn test_tcp_header() {
    let input = r#"packet-beta
0-15: "Source Port"
16-31: "Destination Port"
32-63: "Sequence Number"
64-95: "Acknowledgment Number"
96-99: "Data Offset"
100-105: "Reserved"
106: "URG"
107: "ACK"
108: "PSH"
109: "RST"
110: "SYN"
111: "FIN"
112-127: "Window"
128-143: "Checksum"
144-159: "Urgent Pointer"
160-191: "(Options and Padding)"
"#;
    
    let tokens = packet_lexer().parse(input.chars()).unwrap();
    let diagram = packet_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.fields.len(), 16);
    
    // Verify bit flags
    let urg = diagram.fields.iter().find(|f| f.name == "URG").unwrap();
    assert_eq!(urg.start_bit, 106);
    assert_eq!(urg.end_bit, 106);
    
    // Verify optional field
    let options = diagram.fields.iter().find(|f| f.name == "Options and Padding").unwrap();
    assert!(options.is_optional);
}
```

## Success Criteria
1. ✅ Parse all 29 packet sample files successfully
2. ✅ Handle bit range specifications (start-end)
3. ✅ Support single bit fields
4. ✅ Parse quoted field names
5. ✅ Handle optional fields in parentheses
6. ✅ Support unquoted identifiers
7. ✅ Maintain field order
8. ✅ Parse without formal grammar definition

## Implementation Priority
**Priority 11** - Implement in Phase 2 alongside other data visualization types. Packet diagrams are relatively simple compared to other TypeScript parsers, making them a good introduction to TypeScript-style parsing. The bit-field representation is unique among Mermaid diagrams.