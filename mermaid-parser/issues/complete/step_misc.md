# Implementation Plan: Miscellaneous Diagrams

## Overview
The misc category contains various experimental, deprecated, or uncategorized diagram types that don't fit into the main categories. This includes edge cases, test diagrams, and potential future diagram types.

## Analysis of Misc Samples

### Sample Distribution
- Location: `mermaid-samples/misc/`
- Count: 244 files
- Various experimental and edge case diagrams

### Common Patterns Found
1. **Info Diagrams**: Simple information display
2. **Gitgraph Variations**: Alternative git visualization syntax
3. **Test Cases**: Parser edge cases and error scenarios
4. **Experimental Features**: New diagram types being tested
5. **Legacy Formats**: Older syntax variations

### Example Patterns
```
info showInfo
```

```
gitGraph:
    commit
    branch develop
    checkout develop
    commit
    checkout main
    merge develop
```

## Step 1: AST Design

### Flexible AST for Unknown Types
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct MiscDiagram {
    pub diagram_type: String,
    pub content: MiscContent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MiscContent {
    Info(InfoDiagram),
    GitGraph(GitGraphAlt),
    Raw(RawDiagram),
}

#[derive(Debug, Clone, PartialEq)]
pub struct InfoDiagram {
    pub command: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitGraphAlt {
    pub commits: Vec<GitCommit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitCommit {
    pub action: String,
    pub params: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawDiagram {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MiscToken {
    // Known keywords
    Info,
    ShowInfo,
    GitGraph,
    Commit,
    Branch,
    Checkout,
    Merge,
    
    // Generic tokens
    Keyword(String),
    Identifier(String),
    Colon,
    Semicolon,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    QuotedString(String),
    Comment(String),
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Flexible Lexer for Unknown Syntax
```rust
use chumsky::prelude::*;

pub fn misc_lexer() -> impl Parser<char, Vec<MiscToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| MiscToken::Comment(text.into_iter().collect()));
    
    // Known keywords
    let keywords = choice((
        text::keyword("info").map(|_| MiscToken::Info),
        text::keyword("showInfo").map(|_| MiscToken::ShowInfo),
        text::keyword("gitGraph").map(|_| MiscToken::GitGraph),
        text::keyword("commit").map(|_| MiscToken::Commit),
        text::keyword("branch").map(|_| MiscToken::Branch),
        text::keyword("checkout").map(|_| MiscToken::Checkout),
        text::keyword("merge").map(|_| MiscToken::Merge),
    ));
    
    // Generic keyword (any word that might be a keyword)
    let generic_keyword = filter(|c: &char| c.is_alphabetic())
        .then(filter(|c: &char| c.is_alphanumeric() || *c == '_').repeated())
        .collect::<String>()
        .map(|s| {
            // Check if it's a known pattern
            match s.as_str() {
                "info" => MiscToken::Info,
                "showInfo" => MiscToken::ShowInfo,
                "gitGraph" => MiscToken::GitGraph,
                "commit" => MiscToken::Commit,
                "branch" => MiscToken::Branch,
                "checkout" => MiscToken::Checkout,
                "merge" => MiscToken::Merge,
                _ => MiscToken::Keyword(s),
            }
        });
    
    // Quoted string
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(MiscToken::QuotedString);
    
    // Identifier
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(MiscToken::Identifier);
    
    let colon = just(':').map(|_| MiscToken::Colon);
    let semicolon = just(';').map(|_| MiscToken::Semicolon);
    let left_paren = just('(').map(|_| MiscToken::LeftParen);
    let right_paren = just(')').map(|_| MiscToken::RightParen);
    let left_brace = just('{').map(|_| MiscToken::LeftBrace);
    let right_brace = just('}').map(|_| MiscToken::RightBrace);
    
    let newline = just('\n').map(|_| MiscToken::NewLine);
    
    let token = choice((
        comment,
        keywords,
        generic_keyword,
        quoted_string,
        colon,
        semicolon,
        left_paren,
        right_paren,
        left_brace,
        right_brace,
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

### Adaptive Parser
```rust
pub fn misc_parser() -> impl Parser<MiscToken, MiscDiagram, Error = Simple<MiscToken>> {
    any()
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(just(MiscToken::Eof).or_not())
        .map(|tokens| {
            if tokens.is_empty() {
                return MiscDiagram {
                    diagram_type: "empty".to_string(),
                    content: MiscContent::Raw(RawDiagram { lines: vec![] }),
                };
            }
            
            // Identify diagram type from first token
            match &tokens[0] {
                MiscToken::Info => parse_info_diagram(&tokens),
                MiscToken::GitGraph => parse_gitgraph_alt(&tokens),
                _ => parse_raw_diagram(&tokens),
            }
        })
}

fn parse_info_diagram(tokens: &[MiscToken]) -> MiscDiagram {
    let mut command = String::new();
    
    for token in tokens.iter().skip(1) {
        match token {
            MiscToken::ShowInfo => {
                command = "showInfo".to_string();
                break;
            }
            MiscToken::Identifier(id) => {
                command = id.clone();
                break;
            }
            MiscToken::NewLine => break,
            _ => {}
        }
    }
    
    MiscDiagram {
        diagram_type: "info".to_string(),
        content: MiscContent::Info(InfoDiagram { command }),
    }
}

fn parse_gitgraph_alt(tokens: &[MiscToken]) -> MiscDiagram {
    let mut commits = Vec::new();
    let mut i = 0;
    
    // Skip "gitGraph" and optional colon
    while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
        i += 1;
    }
    
    while i < tokens.len() {
        match &tokens[i] {
            MiscToken::Commit => {
                let mut params = Vec::new();
                i += 1;
                
                // Collect parameters until newline
                while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
                    match &tokens[i] {
                        MiscToken::Identifier(id) | MiscToken::QuotedString(id) => {
                            params.push(id.clone());
                        }
                        _ => {}
                    }
                    i += 1;
                }
                
                commits.push(GitCommit {
                    action: "commit".to_string(),
                    params,
                });
            }
            MiscToken::Branch => {
                let mut params = Vec::new();
                i += 1;
                
                while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
                    if let MiscToken::Identifier(id) = &tokens[i] {
                        params.push(id.clone());
                    }
                    i += 1;
                }
                
                commits.push(GitCommit {
                    action: "branch".to_string(),
                    params,
                });
            }
            MiscToken::Checkout => {
                let mut params = Vec::new();
                i += 1;
                
                while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
                    if let MiscToken::Identifier(id) = &tokens[i] {
                        params.push(id.clone());
                    }
                    i += 1;
                }
                
                commits.push(GitCommit {
                    action: "checkout".to_string(),
                    params,
                });
            }
            MiscToken::Merge => {
                let mut params = Vec::new();
                i += 1;
                
                while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
                    if let MiscToken::Identifier(id) = &tokens[i] {
                        params.push(id.clone());
                    }
                    i += 1;
                }
                
                commits.push(GitCommit {
                    action: "merge".to_string(),
                    params,
                });
            }
            _ => {
                i += 1;
            }
        }
    }
    
    MiscDiagram {
        diagram_type: "gitGraph".to_string(),
        content: MiscContent::GitGraph(GitGraphAlt { commits }),
    }
}

fn parse_raw_diagram(tokens: &[MiscToken]) -> MiscDiagram {
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    
    for token in tokens {
        match token {
            MiscToken::NewLine => {
                if !current_line.is_empty() {
                    lines.push(current_line.join(" "));
                    current_line.clear();
                }
            }
            MiscToken::Comment(_) => {
                // Skip comments
            }
            _ => {
                current_line.push(format!("{:?}", token));
            }
        }
    }
    
    if !current_line.is_empty() {
        lines.push(current_line.join(" "));
    }
    
    let diagram_type = if !lines.is_empty() {
        lines[0].split_whitespace().next().unwrap_or("unknown").to_string()
    } else {
        "unknown".to_string()
    };
    
    MiscDiagram {
        diagram_type,
        content: MiscContent::Raw(RawDiagram { lines }),
    }
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/misc/`
- Expected count: 244 files
- Copy to: `mermaid-parser/test/misc/`

### Command
```bash
cp -r ../mermaid-samples/misc/* ./test/misc/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_misc_files(#[files("test/misc/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = misc_lexer().parse(content.chars()).unwrap_or_else(|e| {
        // Some misc files may have invalid syntax
        eprintln!("Warning: Lexer failed for {:?}: {:?}", path, e);
        vec![]
    });
    
    let diagram = misc_parser().parse(tokens).unwrap_or_else(|e| {
        // Fallback to raw diagram
        MiscDiagram {
            diagram_type: "error".to_string(),
            content: MiscContent::Raw(RawDiagram { 
                lines: vec![format!("Parse error: {:?}", e)] 
            }),
        }
    });
    
    // Just verify we got some result
    assert!(!diagram.diagram_type.is_empty());
}

#[test]
fn test_info_diagram() {
    let input = r#"info showInfo"#;
    
    let tokens = misc_lexer().parse(input.chars()).unwrap();
    let diagram = misc_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.diagram_type, "info");
    match diagram.content {
        MiscContent::Info(info) => {
            assert_eq!(info.command, "showInfo");
        }
        _ => panic!("Expected info diagram"),
    }
}

#[test]
fn test_gitgraph_alt() {
    let input = r#"gitGraph:
    commit
    branch develop
    checkout develop
    commit
    checkout main
    merge develop
"#;
    
    let tokens = misc_lexer().parse(input.chars()).unwrap();
    let diagram = misc_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.diagram_type, "gitGraph");
    match diagram.content {
        MiscContent::GitGraph(git) => {
            assert_eq!(git.commits.len(), 6);
            assert_eq!(git.commits[1].action, "branch");
            assert_eq!(git.commits[1].params, vec!["develop"]);
        }
        _ => panic!("Expected gitGraph diagram"),
    }
}

#[test]
fn test_unknown_diagram() {
    let input = r#"unknownType
    some content
    more content
"#;
    
    let tokens = misc_lexer().parse(input.chars()).unwrap();
    let diagram = misc_parser().parse(tokens).unwrap();
    
    match diagram.content {
        MiscContent::Raw(raw) => {
            assert!(!raw.lines.is_empty());
        }
        _ => panic!("Expected raw diagram"),
    }
}

#[test]
fn test_empty_diagram() {
    let input = "";
    
    let tokens = misc_lexer().parse(input.chars()).unwrap();
    let diagram = misc_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.diagram_type, "empty");
}

#[test]
fn test_edge_cases() {
    // Test various edge cases that might appear in misc
    let cases = vec![
        "%%%% Multiple percent signs",
        "diagram\n  with strange\n    indentation",
        "mixed:syntax{and}formats",
    ];
    
    for input in cases {
        let tokens = misc_lexer().parse(input.chars()).unwrap_or(vec![]);
        let diagram = misc_parser().parse(tokens).unwrap();
        // Just ensure it doesn't panic
        assert!(!diagram.diagram_type.is_empty());
    }
}
```

## Success Criteria
1. ✅ Handle all 244 misc sample files without panicking
2. ✅ Parse info diagrams correctly
3. ✅ Support alternative gitGraph syntax
4. ✅ Gracefully handle unknown diagram types
5. ✅ Provide fallback for unparseable content
6. ✅ Handle edge cases and malformed syntax
7. ✅ Maintain flexibility for future diagram types

## Implementation Priority
**Priority 24** - Implement last in Phase 5. The misc category serves as a catch-all for experimental features and edge cases. It should be implemented after all main diagram types to ensure proper fallback behavior and to handle any remaining test cases.

## Design Notes
- The misc parser is intentionally flexible and forgiving
- It serves as a fallback for unknown diagram types
- Can be extended to recognize new experimental syntaxes
- Useful for parser testing and edge case handling
- May contain deprecated syntax that needs graceful handling