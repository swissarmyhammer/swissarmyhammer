use crate::common::ast::{AccessibilityInfo, CommitType, GitBranch, GitDiagram, GitOperation};
use crate::error::{ParseError, Result};
use chumsky::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum GitToken {
    GitGraph,              // "gitGraph" or "gitGraph:"
    Commit,                // "commit"
    Branch,                // "branch"
    Checkout,              // "checkout"
    Merge,                 // "merge"
    CherryPick,            // "cherry-pick"
    Id(String),            // "id: value"
    Tag(String),           // "tag: value"
    Type(CommitType),      // "type: NORMAL"
    Order(i32),            // "order: 1"
    Parent(String),        // "parent: commit-id"
    BranchName(String),    // Branch identifier
    CommitId(String),      // Commit identifier
    Colon,                 // ":"
    Theme(String),         // Theme specification
    Title(String),         // "title Text"
    AccTitle,              // "accTitle"
    AccTitleValue(String), // Accessibility title
    AccDescr,              // "accDescr"
    AccDescrValue(String), // Accessibility description
    NewLine,
    Eof,
}

pub fn parse(input: &str) -> Result<GitDiagram> {
    let tokens = git_lexer()
        .parse(input)
        .into_result()
        .map_err(|e| ParseError::SyntaxError {
            message: "Failed to tokenize git diagram".to_string(),
            expected: vec![],
            found: format!("{:?}", e),
            line: 0,
            column: 0,
        })?;

    parse_git_diagram(&tokens)
}

fn parse_git_diagram(tokens: &[GitToken]) -> Result<GitDiagram> {
    let mut i = 0;

    // Find and skip the "gitGraph" header
    while i < tokens.len() && !matches!(tokens[i], GitToken::GitGraph) {
        i += 1;
    }

    if i >= tokens.len() {
        return Err(ParseError::SyntaxError {
            message: "Expected 'gitGraph' keyword".to_string(),
            expected: vec!["gitGraph".to_string()],
            found: "end of input".to_string(),
            line: 0,
            column: 0,
        });
    }

    i += 1; // Skip "gitGraph"

    // Skip optional newline after gitGraph
    if i < tokens.len() && matches!(tokens[i], GitToken::NewLine) {
        i += 1;
    }

    let mut diagram = GitDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        theme: None,
        commits: Vec::new(),
        branches: vec![GitBranch {
            name: "main".to_string(),
            order: Some(0),
            color: None,
        }],
        operations: Vec::new(),
    };

    let mut _current_branch = "main".to_string();

    while i < tokens.len() {
        match &tokens[i] {
            GitToken::Title(title) => {
                diagram.title = Some(title.clone());
            }
            GitToken::Theme(theme) => {
                diagram.theme = Some(theme.clone());
            }
            GitToken::Commit => {
                let (operation, next_i) = parse_commit_operation(tokens, i)?;
                diagram.operations.push(operation);
                i = next_i;
                continue;
            }
            GitToken::Branch => {
                let (operation, next_i) = parse_branch_operation(tokens, i)?;
                if let GitOperation::Branch {
                    ref name,
                    ref order,
                } = operation
                {
                    diagram.branches.push(GitBranch {
                        name: name.clone(),
                        order: *order,
                        color: None,
                    });
                }
                diagram.operations.push(operation);
                i = next_i;
                continue;
            }
            GitToken::Checkout => {
                let (operation, next_i) = parse_checkout_operation(tokens, i)?;
                if let GitOperation::Checkout { ref branch } = operation {
                    _current_branch = branch.clone();
                }
                diagram.operations.push(operation);
                i = next_i;
                continue;
            }
            GitToken::Merge => {
                let (operation, next_i) = parse_merge_operation(tokens, i)?;
                diagram.operations.push(operation);
                i = next_i;
                continue;
            }
            GitToken::CherryPick => {
                let (operation, next_i) = parse_cherry_pick_operation(tokens, i)?;
                diagram.operations.push(operation);
                i = next_i;
                continue;
            }
            GitToken::AccTitle => {
                // Next token should be the title value
                if i + 1 < tokens.len() {
                    if let GitToken::AccTitleValue(title) = &tokens[i + 1] {
                        diagram.accessibility.title = Some(title.clone());
                        i += 1; // Skip the value token
                    }
                }
            }
            GitToken::AccTitleValue(title) => {
                // Direct accessibility title value
                diagram.accessibility.title = Some(title.clone());
            }
            GitToken::AccDescr => {
                // Next token should be the description value
                if i + 1 < tokens.len() {
                    if let GitToken::AccDescrValue(descr) = &tokens[i + 1] {
                        diagram.accessibility.description = Some(descr.clone());
                        i += 1; // Skip the value token
                    }
                }
            }
            GitToken::AccDescrValue(descr) => {
                // Direct accessibility description value
                diagram.accessibility.description = Some(descr.clone());
            }
            _ => {
                // Skip other tokens
            }
        }
        i += 1;
    }

    Ok(diagram)
}

fn parse_commit_operation(tokens: &[GitToken], start: usize) -> Result<(GitOperation, usize)> {
    let mut i = start + 1; // Skip "commit"
    let mut id = None;
    let mut tag = None;
    let mut commit_type = CommitType::Normal;

    // Parse optional properties
    while i < tokens.len() {
        match &tokens[i] {
            GitToken::Id(commit_id) => {
                id = Some(commit_id.clone());
            }
            GitToken::Tag(tag_value) => {
                tag = Some(tag_value.clone());
            }
            GitToken::Type(ctype) => {
                commit_type = ctype.clone();
            }
            GitToken::NewLine
            | GitToken::Commit
            | GitToken::Branch
            | GitToken::Checkout
            | GitToken::Merge
            | GitToken::CherryPick => {
                break;
            }
            _ => {}
        }
        i += 1;
    }

    Ok((
        GitOperation::Commit {
            id,
            commit_type,
            tag,
        },
        i,
    ))
}

fn parse_branch_operation(tokens: &[GitToken], start: usize) -> Result<(GitOperation, usize)> {
    let mut i = start + 1; // Skip "branch"
    let mut name = None;
    let mut order = None;

    // Get branch name
    if i < tokens.len() {
        if let GitToken::BranchName(branch_name) = &tokens[i] {
            name = Some(branch_name.clone());
            i += 1;
        }
    }

    // Parse optional order
    while i < tokens.len() {
        match &tokens[i] {
            GitToken::Order(order_value) => {
                order = Some(*order_value);
            }
            GitToken::NewLine
            | GitToken::Commit
            | GitToken::Branch
            | GitToken::Checkout
            | GitToken::Merge
            | GitToken::CherryPick => {
                break;
            }
            _ => {}
        }
        i += 1;
    }

    let branch_name = name.ok_or_else(|| ParseError::SyntaxError {
        message: "Branch operation requires a name".to_string(),
        expected: vec!["branch name".to_string()],
        found: "end of input".to_string(),
        line: 0,
        column: 0,
    })?;

    Ok((
        GitOperation::Branch {
            name: branch_name,
            order,
        },
        i,
    ))
}

fn parse_checkout_operation(tokens: &[GitToken], start: usize) -> Result<(GitOperation, usize)> {
    let mut i = start + 1; // Skip "checkout"

    // Get branch name
    if i < tokens.len() {
        if let GitToken::BranchName(branch_name) = &tokens[i] {
            i += 1;
            return Ok((
                GitOperation::Checkout {
                    branch: branch_name.clone(),
                },
                i,
            ));
        }
    }

    Err(ParseError::SyntaxError {
        message: "Checkout operation requires a branch name".to_string(),
        expected: vec!["branch name".to_string()],
        found: "end of input".to_string(),
        line: 0,
        column: 0,
    })
}

fn parse_merge_operation(tokens: &[GitToken], start: usize) -> Result<(GitOperation, usize)> {
    let mut i = start + 1; // Skip "merge"
    let mut branch = None;
    let mut id = None;
    let mut tag = None;
    let mut commit_type = CommitType::Normal;

    // Get branch name
    if i < tokens.len() {
        if let GitToken::BranchName(branch_name) = &tokens[i] {
            branch = Some(branch_name.clone());
            i += 1;
        }
    }

    // Parse optional properties
    while i < tokens.len() {
        match &tokens[i] {
            GitToken::Id(merge_id) => {
                id = Some(merge_id.clone());
            }
            GitToken::Tag(tag_value) => {
                tag = Some(tag_value.clone());
            }
            GitToken::Type(ctype) => {
                commit_type = ctype.clone();
            }
            GitToken::NewLine
            | GitToken::Commit
            | GitToken::Branch
            | GitToken::Checkout
            | GitToken::Merge
            | GitToken::CherryPick => {
                break;
            }
            _ => {}
        }
        i += 1;
    }

    let branch_name = branch.ok_or_else(|| ParseError::SyntaxError {
        message: "Merge operation requires a branch name".to_string(),
        expected: vec!["branch name".to_string()],
        found: "end of input".to_string(),
        line: 0,
        column: 0,
    })?;

    Ok((
        GitOperation::Merge {
            branch: branch_name,
            id,
            tag,
            commit_type,
        },
        i,
    ))
}

fn parse_cherry_pick_operation(tokens: &[GitToken], start: usize) -> Result<(GitOperation, usize)> {
    let mut i = start + 1; // Skip "cherry-pick"
    let mut id = None;
    let mut parent = None;
    let mut tag = None;

    // Parse properties
    while i < tokens.len() {
        match &tokens[i] {
            GitToken::Id(commit_id) => {
                id = Some(commit_id.clone());
            }
            GitToken::Parent(parent_id) => {
                parent = Some(parent_id.clone());
            }
            GitToken::Tag(tag_value) => {
                tag = Some(tag_value.clone());
            }
            GitToken::NewLine
            | GitToken::Commit
            | GitToken::Branch
            | GitToken::Checkout
            | GitToken::Merge
            | GitToken::CherryPick => {
                break;
            }
            _ => {}
        }
        i += 1;
    }

    let commit_id = id.ok_or_else(|| ParseError::SyntaxError {
        message: "Cherry-pick operation requires a commit id".to_string(),
        expected: vec!["id".to_string()],
        found: "end of input".to_string(),
        line: 0,
        column: 0,
    })?;

    Ok((
        GitOperation::CherryPick {
            id: commit_id,
            parent,
            tag,
        },
        i,
    ))
}

fn git_lexer<'src>() -> impl Parser<'src, &'src str, Vec<GitToken>, extra::Err<Simple<'src, char>>>
{
    let whitespace = one_of(" \t").repeated();

    let comment = just("%%").then(none_of('\n').repeated()).ignored();

    let git_graph =
        choice((text::keyword("gitGraph:"), text::keyword("gitGraph"))).map(|_| GitToken::GitGraph);

    let commit = text::keyword("commit").map(|_| GitToken::Commit);

    let branch = text::keyword("branch").map(|_| GitToken::Branch);

    let checkout = text::keyword("checkout").map(|_| GitToken::Checkout);

    let merge = text::keyword("merge").map(|_| GitToken::Merge);

    let cherry_pick = just("cherry")
        .then(just("-"))
        .then(just("pick"))
        .map(|_| GitToken::CherryPick);

    // Properties with values
    let id_prop = text::keyword("id")
        .padded_by(whitespace)
        .then_ignore(just(':'))
        .padded_by(whitespace)
        .ignore_then(choice((
            just('"')
                .ignore_then(none_of('"').repeated().collect::<String>())
                .then_ignore(just('"')),
            none_of(" \t\n").repeated().at_least(1).collect::<String>(),
        )))
        .map(GitToken::Id);

    let tag_prop = text::keyword("tag")
        .padded_by(whitespace)
        .then_ignore(just(':'))
        .padded_by(whitespace)
        .ignore_then(choice((
            just('"')
                .ignore_then(none_of('"').repeated().collect::<String>())
                .then_ignore(just('"')),
            none_of(" \t\n").repeated().at_least(1).collect::<String>(),
        )))
        .map(GitToken::Tag);

    let type_prop = text::keyword("type")
        .padded_by(whitespace)
        .then_ignore(just(':'))
        .padded_by(whitespace)
        .ignore_then(choice((
            text::keyword("NORMAL").map(|_| CommitType::Normal),
            text::keyword("REVERSE").map(|_| CommitType::Reverse),
            text::keyword("HIGHLIGHT").map(|_| CommitType::Highlight),
        )))
        .map(GitToken::Type);

    let order_prop = text::keyword("order")
        .padded_by(whitespace)
        .then_ignore(just(':'))
        .padded_by(whitespace)
        .ignore_then(text::int(10))
        .map(|order: &str| GitToken::Order(order.parse().unwrap_or(0)));

    let parent_prop = text::keyword("parent")
        .padded_by(whitespace)
        .then_ignore(just(':'))
        .padded_by(whitespace)
        .ignore_then(choice((
            just('"')
                .ignore_then(none_of('"').repeated().collect::<String>())
                .then_ignore(just('"')),
            none_of(" \t\n").repeated().at_least(1).collect::<String>(),
        )))
        .map(GitToken::Parent);

    // Branch and commit identifiers
    let identifier = text::ident().map(|s: &str| GitToken::BranchName(s.to_string()));

    let theme = text::keyword("theme")
        .then(whitespace.at_least(1))
        .ignore_then(text::ident())
        .map(|s: &str| GitToken::Theme(s.to_string()));

    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|title| GitToken::Title(title.trim().to_string()));

    let acc_title = text::keyword("accTitle")
        .then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|title| GitToken::AccTitleValue(title.trim().to_string()));

    let acc_descr = text::keyword("accDescr")
        .then(whitespace.at_least(1))
        .ignore_then(none_of('\n').repeated().collect::<String>())
        .map(|descr| GitToken::AccDescrValue(descr.trim().to_string()));

    let colon = just(':').map(|_| GitToken::Colon);

    let newline = choice((just("\n"), just("\r\n"))).map(|_| GitToken::NewLine);

    let token = choice((
        git_graph,
        cherry_pick, // Put cherry-pick before other keywords to ensure proper parsing
        commit,
        branch,
        checkout,
        merge,
        id_prop,
        tag_prop,
        type_prop,
        order_prop,
        parent_prop,
        theme,
        title,
        acc_title,
        acc_descr,
        identifier,
        colon,
        newline,
    ));

    // Skip any leading whitespace/newlines before the first token
    let leading_ws = choice((one_of(" \t\n\r").ignored(), comment)).repeated();

    leading_ws.ignore_then(
        choice((comment.map(|_| None), token.map(Some)))
            .padded_by(whitespace)
            .repeated()
            .collect::<Vec<_>>()
            .map(|tokens| tokens.into_iter().flatten().collect()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_git_graph() {
        let input = r#"gitGraph
    commit id: "Alpha"
    commit id: "Beta"
    branch develop
    checkout develop
    commit id: "Charlie"
    checkout main
    merge develop
"#;

        let result = parse(input);
        assert!(result.is_ok());

        let diagram = result.unwrap();
        assert!(diagram.operations.len() >= 6); // Various operations
        assert!(diagram.branches.len() >= 2); // main + develop
    }
}
