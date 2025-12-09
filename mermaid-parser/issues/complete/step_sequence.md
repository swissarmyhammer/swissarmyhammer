# Implementation Plan: Sequence Diagrams

## Overview
Sequence diagrams show message exchanges between actors/participants over time.
Medium complexity grammar (329 lines) with participants, messages, loops, alternatives, and notes.

## Grammar Analysis

### Key Features
- Header: `sequenceDiagram`
- Participants: `participant Alice`, `actor Bob`
- Messages: `Alice->Bob: Hello`, `Bob-->>Alice: Hi`
- Arrows: `->`, `-->>`, `->>`, etc. (solid/dotted, open/closed)
- Control structures: `loop`, `alt`, `opt`, `par`, `critical`
- Notes: `note left of Alice: Note text`
- Activation: `activate Alice`, `deactivate Alice`
- Autonumbering: `autonumber`

### Message Types
- `->`: Solid open arrow
- `->>`: Solid closed arrow  
- `-->`: Dotted open arrow
- `-->>`: Dotted closed arrow
- `-x`: Cross (destroy)
- `-)`: Circle (async)

### Example Input
```
sequenceDiagram
    participant Alice
    participant Bob
    Alice->John: Hello John, how are you?
    loop HealthCheck
        John->John: Fight against hypochondria
    end
    Note right of John: Rational thoughts!
    John-->>Alice: Great!
    John->Bob: How about you?
    Bob-->>John: Jolly good!
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub participants: Vec<Participant>,
    pub statements: Vec<Statement>,
    pub autonumber: Option<AutoNumber>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    pub actor: String,
    pub alias: Option<String>,
    pub participant_type: ParticipantType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParticipantType {
    Participant,
    Actor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Message(Message),
    Note(Note),
    Loop(Loop),
    Alt(Alternative),
    Opt(Optional),
    Par(Parallel),
    Critical(Critical),
    Activate(String),
    Deactivate(String),
    Create(Participant),
    Destroy(String),
    Box(Box),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub text: String,
    pub arrow_type: ArrowType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrowType {
    SolidOpen,        // ->
    SolidClosed,      // ->>
    DottedOpen,       // -->
    DottedClosed,     // -->>
    Cross,            // -x
    Point,            // -)
    BiDirectionalSolid,   // <<->>
    BiDirectionalDotted,  // <<-->>
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub position: NotePosition,
    pub actor: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotePosition {
    LeftOf,
    RightOf,
    Over,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loop {
    pub condition: String,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alternative {
    pub condition: String,
    pub statements: Vec<Statement>,
    pub else_branch: Option<ElseBranch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElseBranch {
    pub condition: Option<String>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutoNumber {
    pub start: Option<i32>,
    pub step: Option<i32>,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SequenceToken {
    SequenceDiagram,       // "sequenceDiagram"
    Participant,           // "participant"
    Actor,                 // "actor"
    ActorName(String),     // Actor/participant name
    As,                    // "as"
    Alias(String),         // Alias name
    SolidOpenArrow,        // "->"
    SolidClosedArrow,      // "->>"
    DottedOpenArrow,       // "-->"
    DottedClosedArrow,     // "-->>"
    Cross,                 // "-x"
    Point,                 // "-)"
    BiDirSolid,           // "<<->>"
    BiDirDotted,          // "<<-->>"
    Text(String),          // Message text after ":"
    Loop,                  // "loop"
    Alt,                   // "alt"
    Else,                  // "else"
    Opt,                   // "opt"
    Par,                   // "par"
    And,                   // "and"
    Critical,              // "critical"
    Option,                // "option"
    Break,                 // "break"
    End,                   // "end"
    Note,                  // "note"
    LeftOf,                // "left of"
    RightOf,               // "right of"
    Over,                  // "over"
    Activate,              // "activate"
    Deactivate,            // "deactivate"
    Create,                // "create"
    Destroy,               // "destroy"
    AutoNumber,            // "autonumber"
    Off,                   // "off"
    Number(i32),           // Numeric value
    Title(String),         // "title Text"
    RestOfLine(String),    // Rest of line text
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn sequence_lexer() -> impl Parser<char, Vec<SequenceToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('#')
        .then(take_until(just('\n')))
        .ignored();
    
    let sequence_diagram = text::keyword("sequenceDiagram")
        .map(|_| SequenceToken::SequenceDiagram);
    
    let participant = text::keyword("participant")
        .map(|_| SequenceToken::Participant);
    
    let actor = text::keyword("actor")
        .map(|_| SequenceToken::Actor);
    
    let keywords = choice((
        text::keyword("loop").map(|_| SequenceToken::Loop),
        text::keyword("alt").map(|_| SequenceToken::Alt),
        text::keyword("else").map(|_| SequenceToken::Else),
        text::keyword("opt").map(|_| SequenceToken::Opt),
        text::keyword("par").map(|_| SequenceToken::Par),
        text::keyword("and").map(|_| SequenceToken::And),
        text::keyword("critical").map(|_| SequenceToken::Critical),
        text::keyword("option").map(|_| SequenceToken::Option),
        text::keyword("break").map(|_| SequenceToken::Break),
        text::keyword("end").map(|_| SequenceToken::End),
        text::keyword("note").map(|_| SequenceToken::Note),
        text::keyword("left of").map(|_| SequenceToken::LeftOf),
        text::keyword("right of").map(|_| SequenceToken::RightOf),
        text::keyword("over").map(|_| SequenceToken::Over),
        text::keyword("activate").map(|_| SequenceToken::Activate),
        text::keyword("deactivate").map(|_| SequenceToken::Deactivate),
        text::keyword("create").map(|_| SequenceToken::Create),
        text::keyword("destroy").map(|_| SequenceToken::Destroy),
        text::keyword("autonumber").map(|_| SequenceToken::AutoNumber),
        text::keyword("off").map(|_| SequenceToken::Off),
        text::keyword("as").map(|_| SequenceToken::As),
    ));
    
    let arrows = choice((
        text::string("<<->>").map(|_| SequenceToken::BiDirSolid),
        text::string("<<-->>").map(|_| SequenceToken::BiDirDotted),
        text::string("->>").map(|_| SequenceToken::SolidClosedArrow),
        text::string("-->>").map(|_| SequenceToken::DottedClosedArrow),
        text::string("->").map(|_| SequenceToken::SolidOpenArrow),
        text::string("-->").map(|_| SequenceToken::DottedOpenArrow),
        text::string("-x").map(|_| SequenceToken::Cross),
        text::string("--x").map(|_| SequenceToken::Cross),
        text::string("-)").map(|_| SequenceToken::Point),
        text::string("--)").map(|_| SequenceToken::Point),
    ));
    
    let number = text::int(10)
        .map(|n: String| SequenceToken::Number(n.parse().unwrap_or(0)));
    
    let title = text::keyword("title")
        .then(whitespace.at_least(1))
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, text)| SequenceToken::Title(text.trim().to_string()));
    
    let text_after_colon = just(':')
        .then(
            take_until(choice((just('\n'), just('#'), just(';'), end())))
                .collect::<String>()
        )
        .map(|(_, text)| SequenceToken::Text(text.trim().to_string()));
    
    let actor_name = filter(|c| !matches!(*c, '\n' | '#' | ':' | ';' | '-' | '<' | '>'))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(|text| SequenceToken::ActorName(text.trim().to_string()))
        .filter(|token| {
            if let SequenceToken::ActorName(text) = token {
                !text.is_empty() && !is_keyword(text)
            } else {
                true
            }
        });
    
    let rest_of_line = take_until(choice((just('\n'), just('#'), just(';'), end())))
        .collect::<String>()
        .map(|text| SequenceToken::RestOfLine(text.trim().to_string()));
    
    let newline = choice((
        just('\n'),
        just(';'),
    ))
    .map(|_| SequenceToken::NewLine);
    
    choice((
        comment.ignored(),
        sequence_diagram,
        participant,
        actor,
        keywords,
        arrows,
        number,
        title,
        text_after_colon,
        actor_name,
        newline,
    ))
    .padded_by(just(' ').or(just('\t')).repeated())
    .repeated()
    .then_ignore(end())
}

fn is_keyword(text: &str) -> bool {
    matches!(text.to_lowercase().as_str(),
        "sequencediagram" | "participant" | "actor" | "loop" | "alt" | "else" |
        "opt" | "par" | "and" | "critical" | "option" | "break" | "end" |
        "note" | "activate" | "deactivate" | "create" | "destroy" |
        "autonumber" | "off" | "as" | "over" | "title"
    )
}
```

## Step 3: Parser Implementation

### Message Parser
```rust
fn message_parser() -> impl Parser<SequenceToken, Message, Error = Simple<SequenceToken>> {
    select! {
        SequenceToken::ActorName(from) => from,
    }
    .then(choice((
        just(SequenceToken::SolidOpenArrow).map(|_| ArrowType::SolidOpen),
        just(SequenceToken::SolidClosedArrow).map(|_| ArrowType::SolidClosed),
        just(SequenceToken::DottedOpenArrow).map(|_| ArrowType::DottedOpen),
        just(SequenceToken::DottedClosedArrow).map(|_| ArrowType::DottedClosed),
        just(SequenceToken::Cross).map(|_| ArrowType::Cross),
        just(SequenceToken::Point).map(|_| ArrowType::Point),
        just(SequenceToken::BiDirSolid).map(|_| ArrowType::BiDirectionalSolid),
        just(SequenceToken::BiDirDotted).map(|_| ArrowType::BiDirectionalDotted),
    )))
    .then(select! {
        SequenceToken::ActorName(to) => to,
    })
    .then(select! {
        SequenceToken::Text(text) => text,
    }.or_not())
    .map(|(((from, arrow_type), to), text)| Message {
        from,
        to,
        text: text.unwrap_or_default(),
        arrow_type,
    })
}
```

### Main Parser (Simplified structure)
```rust
pub fn sequence_parser() -> impl Parser<SequenceToken, SequenceDiagram, Error = Simple<SequenceToken>> {
    // This is a complex parser that would handle:
    // 1. Participant declarations
    // 2. Message statements
    // 3. Control flow structures (loops, alternatives)
    // 4. Notes and annotations
    // 5. Activation/deactivation
    // 6. Autonumbering
    
    // Implementation would be extensive - this shows the structure
    just(SequenceToken::SequenceDiagram)
        .then_ignore(just(SequenceToken::NewLine).or_not())
        .then(
            // Parse statements in sequence
            choice((
                // Participant declarations
                participant_parser().map(Statement::Create),
                // Messages
                message_parser().map(Statement::Message),
                // Control structures
                loop_parser().map(Statement::Loop),
                // Notes
                note_parser().map(Statement::Note),
                // Activation
                activation_parser(),
            ))
            .separated_by(just(SequenceToken::NewLine))
            .allow_trailing()
        )
        .then_ignore(just(SequenceToken::Eof).or_not())
        .map(|(_, statements)| {
            // Build the diagram from parsed statements
            SequenceDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                participants: Vec::new(),
                statements,
                autonumber: None,
            }
        })
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/sequence/`
- Expected count: 55 files
- Copy to: `mermaid-parser/test/sequence/`

### Command
```bash
cp -r ../mermaid-samples/sequence/* ./test/sequence/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_sequence_files(#[files("test/sequence/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = sequence_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = sequence_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(diagram.statements.len() > 0, "Should have statements");
}

#[test]
fn test_simple_sequence() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    Alice->Bob: Hello Bob
    Bob-->>Alice: Hello Alice!
"#;
    
    let tokens = sequence_lexer().parse(input.chars()).unwrap();
    let diagram = sequence_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.participants.len(), 2);
    assert!(matches!(diagram.statements[0], Statement::Message(_)));
}

#[test]
fn test_arrow_types() {
    let tests = vec![
        ("->", ArrowType::SolidOpen),
        ("->>", ArrowType::SolidClosed),
        ("-->", ArrowType::DottedOpen),
        ("-->>", ArrowType::DottedClosed),
        ("-x", ArrowType::Cross),
        ("-)", ArrowType::Point),
    ];
    
    for (arrow_str, expected_type) in tests {
        let input = format!("sequenceDiagram\nAlice{}Bob: Test", arrow_str);
        let tokens = sequence_lexer().parse(input.chars()).unwrap();
        let diagram = sequence_parser().parse(tokens).unwrap();
        
        if let Statement::Message(msg) = &diagram.statements[0] {
            assert_eq!(msg.arrow_type, expected_type);
        } else {
            panic!("Expected message statement");
        }
    }
}
```

## Success Criteria
1. ✅ Parse all 55 sequence sample files successfully
2. ✅ Handle all arrow types and message formats
3. ✅ Support participant declarations with aliases
4. ✅ Parse control structures (loop, alt, opt, par, critical)
5. ✅ Handle notes with positioning
6. ✅ Support activation/deactivation
7. ✅ Process autonumbering options
8. ✅ Handle boxes and grouping

## Implementation Priority
**Priority 4** - Implement after the simpler grammars are complete. This will establish patterns for complex control structures and message parsing that apply to other diagram types.

## Notes
This is one of the most complex grammars with many features. The full implementation would be quite extensive, but the pattern established here can be applied to other complex grammars like Flow and Class diagrams.