//! Miscellaneous diagram parser implementation

use crate::common::ast::{
    GitGraphAlt, InfoDiagram, MiscContent, MiscDiagram, MiscGitCommit, RawDiagram,
};
use crate::error::{ParseError, Result};
use chumsky::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

impl From<&MiscToken> for String {
    fn from(token: &MiscToken) -> Self {
        format!("{:?}", token)
    }
}

fn misc_lexer<'src>() -> impl Parser<'src, &'src str, Vec<MiscToken>, extra::Err<Simple<'src, char>>>
{
    let whitespace = just(' ').or(just('\t')).repeated();

    let comment = just('%')
        .then(just('%'))
        .then(none_of('\n').repeated())
        .map(|_| MiscToken::Comment("".to_string()));

    // Quoted string
    let quoted_string = just('"')
        .ignore_then(none_of('"').repeated().collect::<String>())
        .then_ignore(just('"'))
        .map(MiscToken::QuotedString);

    // Identifier or keyword
    let identifier = text::ident().map(|s: &str| {
        // Check if it's a known keyword
        match s {
            "info" => MiscToken::Info,
            "showInfo" => MiscToken::ShowInfo,
            "gitGraph" => MiscToken::GitGraph,
            "commit" => MiscToken::Commit,
            "branch" => MiscToken::Branch,
            "checkout" => MiscToken::Checkout,
            "merge" => MiscToken::Merge,
            _ => MiscToken::Identifier(s.to_string()),
        }
    });

    let colon = just(':').to(MiscToken::Colon);
    let semicolon = just(';').to(MiscToken::Semicolon);
    let left_paren = just('(').to(MiscToken::LeftParen);
    let right_paren = just(')').to(MiscToken::RightParen);
    let left_brace = just('{').to(MiscToken::LeftBrace);
    let right_brace = just('}').to(MiscToken::RightBrace);
    let newline = just('\n').to(MiscToken::NewLine);

    let token = choice((
        comment,
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
        .collect::<Vec<_>>()
        .then_ignore(end())
}

fn misc_parser<'src>(
) -> impl Parser<'src, &'src [MiscToken], MiscDiagram, extra::Err<Simple<'src, MiscToken>>> {
    any().repeated().collect::<Vec<_>>().map(|tokens| {
        if tokens.is_empty() {
            return MiscDiagram {
                diagram_type: "empty".to_string(),
                content: MiscContent::Raw(RawDiagram { lines: vec![] }),
            };
        }

        // Identify diagram type from first non-comment/newline token
        let mut first_token = None;
        for token in &tokens {
            match token {
                MiscToken::Comment(_) | MiscToken::NewLine => {}
                _ => {
                    first_token = Some(token);
                    break;
                }
            }
        }

        match first_token {
            Some(MiscToken::Info) => parse_info_diagram(&tokens),
            Some(MiscToken::GitGraph) => parse_gitgraph_alt(&tokens),
            _ => parse_raw_diagram(&tokens),
        }
    })
}

fn parse_info_diagram(tokens: &[MiscToken]) -> MiscDiagram {
    let mut command = String::new();
    let mut found_info = false;

    for token in tokens {
        if found_info {
            match token {
                MiscToken::ShowInfo => {
                    command = "showInfo".to_string();
                    break;
                }
                MiscToken::Identifier(id) | MiscToken::Keyword(id) => {
                    command = id.clone();
                    break;
                }
                MiscToken::NewLine => break,
                _ => {}
            }
        } else if matches!(token, MiscToken::Info) {
            found_info = true;
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
    while i < tokens.len() {
        match &tokens[i] {
            MiscToken::GitGraph => {
                i += 1;
                // Skip optional colon
                if i < tokens.len() && matches!(&tokens[i], MiscToken::Colon) {
                    i += 1;
                }
                break;
            }
            _ => i += 1,
        }
    }

    while i < tokens.len() {
        match &tokens[i] {
            MiscToken::Commit => {
                let mut params = Vec::new();
                i += 1;

                // Collect parameters until newline
                while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
                    match &tokens[i] {
                        MiscToken::Identifier(id)
                        | MiscToken::QuotedString(id)
                        | MiscToken::Keyword(id) => {
                            params.push(id.clone());
                        }
                        _ => {}
                    }
                    i += 1;
                }

                commits.push(MiscGitCommit {
                    action: "commit".to_string(),
                    params,
                });
            }
            MiscToken::Branch => {
                let mut params = Vec::new();
                i += 1;

                while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
                    match &tokens[i] {
                        MiscToken::Identifier(id) | MiscToken::Keyword(id) => {
                            params.push(id.clone());
                        }
                        _ => {}
                    }
                    i += 1;
                }

                commits.push(MiscGitCommit {
                    action: "branch".to_string(),
                    params,
                });
            }
            MiscToken::Checkout => {
                let mut params = Vec::new();
                i += 1;

                while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
                    match &tokens[i] {
                        MiscToken::Identifier(id) | MiscToken::Keyword(id) => {
                            params.push(id.clone());
                        }
                        _ => {}
                    }
                    i += 1;
                }

                commits.push(MiscGitCommit {
                    action: "checkout".to_string(),
                    params,
                });
            }
            MiscToken::Merge => {
                let mut params = Vec::new();
                i += 1;

                while i < tokens.len() && !matches!(&tokens[i], MiscToken::NewLine) {
                    match &tokens[i] {
                        MiscToken::Identifier(id) | MiscToken::Keyword(id) => {
                            params.push(id.clone());
                        }
                        _ => {}
                    }
                    i += 1;
                }

                commits.push(MiscGitCommit {
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
        // Extract the first keyword/identifier as diagram type
        let first_line_tokens: Vec<&str> = lines[0].split_whitespace().collect();
        if !first_line_tokens.is_empty() {
            // Remove quotes from debug format
            first_line_tokens[0]
                .trim_start_matches("Keyword(\"")
                .trim_start_matches("Identifier(\"")
                .trim_end_matches("\")")
                .to_string()
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };

    MiscDiagram {
        diagram_type,
        content: MiscContent::Raw(RawDiagram { lines }),
    }
}

pub fn parse(input: &str) -> Result<MiscDiagram> {
    // Strip metadata comments before parsing
    let clean_input = crate::common::lexer::strip_metadata_comments(input);

    let tokens = misc_lexer()
        .parse(&clean_input)
        .into_result()
        .map_err(|e| ParseError::SyntaxError {
            message: "Failed to tokenize misc diagram".to_string(),
            expected: vec![],
            found: format!("{:?}", e),
            line: 0,
            column: 0,
        })?;

    let result = misc_parser()
        .parse(&tokens[..])
        .into_result()
        .map_err(|e| ParseError::SyntaxError {
            message: "Failed to parse misc diagram".to_string(),
            expected: vec![],
            found: format!("{:?}", e),
            line: 0,
            column: 0,
        })?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_info() {
        let input = "info showInfo";
        let tokens = misc_lexer().parse(input).unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], MiscToken::Info));
        assert!(matches!(tokens[1], MiscToken::ShowInfo));
    }

    #[test]
    fn test_lexer_gitgraph() {
        let input = "gitGraph:\n    commit\n    branch develop";
        let tokens = misc_lexer().parse(input).unwrap();
        assert!(!tokens.is_empty());
        assert!(matches!(tokens[0], MiscToken::GitGraph));
    }

    #[test]
    fn test_parser_info() {
        let input = "info showInfo";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.diagram_type, "info");
        match diagram.content {
            MiscContent::Info(info) => assert_eq!(info.command, "showInfo"),
            _ => panic!("Expected info content"),
        }
    }

    #[test]
    fn test_parser_gitgraph() {
        let input = "gitGraph:\n    commit\n    branch develop\n    checkout develop";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.diagram_type, "gitGraph");
        match diagram.content {
            MiscContent::GitGraph(git) => {
                assert_eq!(git.commits.len(), 3);
                assert_eq!(git.commits[0].action, "commit");
                assert_eq!(git.commits[1].action, "branch");
                assert_eq!(git.commits[1].params, vec!["develop"]);
            }
            _ => panic!("Expected gitGraph content"),
        }
    }

    #[test]
    fn test_parser_unknown() {
        let input = "unknownDiagram\n    some content";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.diagram_type, "unknownDiagram");
        match diagram.content {
            MiscContent::Raw(raw) => assert!(!raw.lines.is_empty()),
            _ => panic!("Expected raw content"),
        }
    }

    #[test]
    fn test_parser_empty() {
        let input = "";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.diagram_type, "empty");
    }
}
