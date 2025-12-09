# Implementation Plan: Git Graphs

## Overview
Git graphs represent git workflow with commits, branches, merges, and cherry-picks.
TypeScript-based parser (no jison) with git-specific commands and flow visualization.

## Parser Analysis

### Key Features
- Header: `gitGraph` or `gitGraph:`
- Commands: `commit`, `branch`, `checkout`, `merge`, `cherry-pick`
- Commit properties: `id`, `type` (NORMAL, REVERSE, HIGHLIGHT)
- Branch operations: create, switch, merge
- Styling and themes

### Example Input
```
gitGraph
    commit id: "Alpha"
    commit id: "Beta"
    branch develop
    checkout develop
    commit id: "Charlie"
    checkout main
    merge develop
    commit id: "Delta"
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct GitDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub theme: Option<String>,
    pub commits: Vec<GitCommit>,
    pub branches: Vec<GitBranch>,
    pub operations: Vec<GitOperation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitCommit {
    pub id: Option<String>,
    pub commit_type: CommitType,
    pub tag: Option<String>,
    pub branch: String, // Which branch this commit is on
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitType {
    Normal,
    Reverse,
    Highlight,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitBranch {
    pub name: String,
    pub order: Option<i32>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GitOperation {
    Commit {
        id: Option<String>,
        commit_type: CommitType,
        tag: Option<String>,
    },
    Branch {
        name: String,
        order: Option<i32>,
    },
    Checkout {
        branch: String,
    },
    Merge {
        branch: String,
        id: Option<String>,
        tag: Option<String>,
        commit_type: CommitType,
    },
    CherryPick {
        id: String,
        parent: Option<String>,
        tag: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GitToken {
    GitGraph,              // "gitGraph"
    Commit,                // "commit"
    Branch,                // "branch"
    Checkout,              // "checkout"
    Merge,                 // "merge"
    CherryPick,           // "cherry-pick"
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
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn git_lexer() -> impl Parser<char, Vec<GitToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .ignored();
    
    let git_graph = choice((
        text::keyword("gitGraph:"),
        text::keyword("gitGraph"),
    ))
    .map(|_| GitToken::GitGraph);
    
    let commit = text::keyword("commit")
        .map(|_| GitToken::Commit);
    
    let branch = text::keyword("branch")
        .map(|_| GitToken::Branch);
    
    let checkout = text::keyword("checkout")
        .map(|_| GitToken::Checkout);
    
    let merge = text::keyword("merge")
        .map(|_| GitToken::Merge);
    
    let cherry_pick = text::keyword("cherry-pick")
        .map(|_| GitToken::CherryPick);
    
    // Properties with values
    let id_prop = text::keyword("id")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .then(
            choice((
                just('"')
                    .ignore_then(take_until(just('"')).collect::<String>())
                    .then_ignore(just('"')),
                take_until(choice((just('\n'), just(' '), end())))
                    .collect::<String>(),
            ))
        )
        .map(|((((_, _), _), _), id)| GitToken::Id(id));
    
    let tag_prop = text::keyword("tag")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .then(
            choice((
                just('"')
                    .ignore_then(take_until(just('"')).collect::<String>())
                    .then_ignore(just('"')),
                take_until(choice((just('\n'), just(' '), end())))
                    .collect::<String>(),
            ))
        )
        .map(|((((_, _), _), _), tag)| GitToken::Tag(tag));
    
    let type_prop = text::keyword("type")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .then(choice((
            text::keyword("NORMAL").map(|_| CommitType::Normal),
            text::keyword("REVERSE").map(|_| CommitType::Reverse),
            text::keyword("HIGHLIGHT").map(|_| CommitType::Highlight),
        )))
        .map(|((((_, _), _), _), commit_type)| GitToken::Type(commit_type));
    
    let order_prop = text::keyword("order")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .then(text::int(10))
        .map(|((((_, _), _), _), order): ((((_, _), _), _), String)| {
            GitToken::Order(order.parse().unwrap_or(0))
        });
    
    let parent_prop = text::keyword("parent")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .then(
            choice((
                just('"')
                    .ignore_then(take_until(just('"')).collect::<String>())
                    .then_ignore(just('"')),
                take_until(choice((just('\n'), just(' '), end())))
                    .collect::<String>(),
            ))
        )
        .map(|((((_, _), _), _), parent)| GitToken::Parent(parent));
    
    // Branch and commit identifiers
    let identifier = text::ident()
        .map(|id| {
            // Context-dependent - could be branch name or commit id
            GitToken::BranchName(id)
        });
    
    let theme = text::keyword("theme")
        .then(whitespace.at_least(1))
        .then(text::ident())
        .map(|((_, _), theme)| GitToken::Theme(theme));
    
    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .then(
            take_until(just('\n'))
                .collect::<String>()
        )
        .map(|(_, title)| GitToken::Title(title.trim().to_string()));
    
    let acc_title = text::keyword("accTitle")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| GitToken::AccTitle);
    
    let acc_descr = text::keyword("accDescr")
        .then(whitespace)
        .then(just(':'))
        .then(whitespace)
        .map(|_| GitToken::AccDescr);
    
    let colon = just(':')
        .map(|_| GitToken::Colon);
    
    let newline = just('\n')
        .map(|_| GitToken::NewLine);
    
    choice((
        comment.ignored(),
        git_graph,
        commit,
        branch,
        checkout,
        merge,
        cherry_pick,
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
    ))
    .padded_by(whitespace)
    .repeated()
    .then_ignore(end())
}
```

## Step 3: Parser Implementation

### Operation Parsers
```rust
fn parse_commit() -> impl Parser<GitToken, GitOperation, Error = Simple<GitToken>> {
    just(GitToken::Commit)
        .then(
            choice((
                select! { GitToken::Id(id) => ("id", id) },
                select! { GitToken::Tag(tag) => ("tag", tag) },
                select! { GitToken::Type(t) => ("type", format!("{:?}", t)) },
            ))
            .repeated()
        )
        .map(|(_, props)| {
            let mut id = None;
            let mut tag = None;
            let mut commit_type = CommitType::Normal;
            
            for (prop_type, value) in props {
                match prop_type {
                    "id" => id = Some(value),
                    "tag" => tag = Some(value),
                    "type" => {
                        commit_type = match value.as_str() {
                            "Reverse" => CommitType::Reverse,
                            "Highlight" => CommitType::Highlight,
                            _ => CommitType::Normal,
                        };
                    }
                    _ => {}
                }
            }
            
            GitOperation::Commit { id, commit_type, tag }
        })
}

fn parse_branch() -> impl Parser<GitToken, GitOperation, Error = Simple<GitToken>> {
    just(GitToken::Branch)
        .then(select! { GitToken::BranchName(name) => name })
        .then(
            select! { GitToken::Order(order) => order }
                .or_not()
        )
        .map(|((_, name), order)| GitOperation::Branch { name, order })
}

fn parse_checkout() -> impl Parser<GitToken, GitOperation, Error = Simple<GitToken>> {
    just(GitToken::Checkout)
        .then(select! { GitToken::BranchName(branch) => branch })
        .map(|(_, branch)| GitOperation::Checkout { branch })
}

fn parse_merge() -> impl Parser<GitToken, GitOperation, Error = Simple<GitToken>> {
    just(GitToken::Merge)
        .then(select! { GitToken::BranchName(branch) => branch })
        .then(
            choice((
                select! { GitToken::Id(id) => ("id", id) },
                select! { GitToken::Tag(tag) => ("tag", tag) },
                select! { GitToken::Type(t) => ("type", format!("{:?}", t)) },
            ))
            .repeated()
        )
        .map(|((_, branch), props)| {
            let mut id = None;
            let mut tag = None;
            let mut commit_type = CommitType::Normal;
            
            for (prop_type, value) in props {
                match prop_type {
                    "id" => id = Some(value),
                    "tag" => tag = Some(value),
                    "type" => {
                        commit_type = match value.as_str() {
                            "Reverse" => CommitType::Reverse,
                            "Highlight" => CommitType::Highlight,
                            _ => CommitType::Normal,
                        };
                    }
                    _ => {}
                }
            }
            
            GitOperation::Merge { branch, id, tag, commit_type }
        })
}
```

### Main Parser
```rust
pub fn git_parser() -> impl Parser<GitToken, GitDiagram, Error = Simple<GitToken>> {
    just(GitToken::GitGraph)
        .then_ignore(just(GitToken::NewLine).or_not())
        .then(
            choice((
                select! { GitToken::Title(title) => ("title", title) },
                select! { GitToken::Theme(theme) => ("theme", theme) },
                parse_commit().map(|op| ("operation", format!("{:?}", op))),
                parse_branch().map(|op| ("operation", format!("{:?}", op))),
                parse_checkout().map(|op| ("operation", format!("{:?}", op))),
                parse_merge().map(|op| ("operation", format!("{:?}", op))),
            ))
            .separated_by(just(GitToken::NewLine))
            .allow_trailing()
        )
        .then_ignore(just(GitToken::Eof).or_not())
        .map(|(_, statements)| {
            let mut diagram = GitDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                theme: None,
                commits: Vec::new(),
                branches: vec![GitBranch { 
                    name: "main".to_string(), 
                    order: Some(0), 
                    color: None 
                }],
                operations: Vec::new(),
            };
            
            for (stmt_type, content) in statements {
                match stmt_type {
                    "title" => diagram.title = Some(content),
                    "theme" => diagram.theme = Some(content),
                    "operation" => {
                        // Parse operation from debug string (simplified)
                        // In real implementation, store operations directly
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
- Location: `mermaid-samples/git/`
- Expected count: 181 files
- Copy to: `mermaid-parser/test/git/`

### Command
```bash
cp -r ../mermaid-samples/git/* ./test/git/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_git_files(#[files("test/git/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = git_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = git_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(
        diagram.operations.len() > 0 || diagram.title.is_some(),
        "Git graph should have operations or title"
    );
}

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
    
    let tokens = git_lexer().parse(input.chars()).unwrap();
    let diagram = git_parser().parse(tokens).unwrap();
    
    assert!(diagram.operations.len() >= 6); // 3 commits, 1 branch, 2 checkouts, 1 merge
    assert!(diagram.branches.len() >= 2); // main + develop
}

#[test]
fn test_commit_types() {
    let input = r#"gitGraph
    commit id: "Normal"
    commit id: "Reverse" type: REVERSE
    commit id: "Highlight" type: HIGHLIGHT
"#;
    
    let tokens = git_lexer().parse(input.chars()).unwrap();
    let diagram = git_parser().parse(tokens).unwrap();
    
    // Verify different commit types are parsed
    assert_eq!(diagram.operations.len(), 3);
}

#[test]
fn test_branch_operations() {
    let input = r#"gitGraph
    commit
    branch feature
    checkout feature
    commit
    checkout main
    merge feature
"#;
    
    let tokens = git_lexer().parse(input.chars()).unwrap();
    let diagram = git_parser().parse(tokens).unwrap();
    
    // Should handle branch creation and merging
    assert!(diagram.operations.iter().any(|op| matches!(op, GitOperation::Branch { .. })));
    assert!(diagram.operations.iter().any(|op| matches!(op, GitOperation::Merge { .. })));
}

#[test]
fn test_cherry_pick() {
    let input = r#"gitGraph
    commit id: "A"
    branch feature
    commit id: "B"
    checkout main
    cherry-pick id: "B"
"#;
    
    let tokens = git_lexer().parse(input.chars()).unwrap();
    let diagram = git_parser().parse(tokens).unwrap();
    
    // Should handle cherry-pick operation
    assert!(diagram.operations.iter().any(|op| matches!(op, GitOperation::CherryPick { .. })));
}
```

## Success Criteria
1. ✅ Parse all 181 git sample files successfully
2. ✅ Handle commit operations with IDs and types
3. ✅ Support branch creation and checkout
4. ✅ Parse merge operations
5. ✅ Handle cherry-pick with parent references
6. ✅ Support commit tags and styling
7. ✅ Process themes and configuration
8. ✅ Handle accessibility attributes

## Implementation Priority
**Priority 8** - Implement after basic diagrams are complete. Git graphs introduce workflow concepts and version control semantics that are specialized but important for development tool integration.