# Implementation Plan: State Diagrams

## Overview
State diagrams represent state machines with states, transitions, events, and hierarchical state structures.
High complexity grammar (336 lines) with nested states, concurrent regions, and various state types.

## Grammar Analysis

### Key Features
- Header: `stateDiagram` or `stateDiagram-v2`
- States: Simple states, composite states, choice states
- Special states: `[*]` for start/end states
- Transitions: With events, guards, and actions
- Concurrent states: Using `--` separator
- Notes: Positioned notes for states
- State aliases: `state "Display Name" as StateId`
- Comments: `%%` for line comments

### Example Input
```
stateDiagram-v2
    [*] --> Still
    Still --> [*]

    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]

    state Moving {
        [*] --> Idle
        Idle --> Running : start
        Running --> Idle : stop
        Running --> Running : accelerate

        state Running {
            [*] --> Slow
            Slow --> Fast : speedUp
            Fast --> Slow : slowDown
        }
    }

    state Choice <<choice>>
    Moving --> Choice
    Choice --> Crash : [speed > 100]
    Choice --> Still : [speed <= 100]

    note right of Moving : This is a note
```

## Step 1: AST Design

### Rust Enums and Structs
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct StateDiagram {
    pub title: Option<String>,
    pub accessibility: AccessibilityInfo,
    pub version: StateVersion,
    pub states: HashMap<String, State>,
    pub transitions: Vec<Transition>,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub id: String,
    pub display_name: Option<String>,
    pub state_type: StateType,
    pub substates: Vec<String>,        // IDs of child states
    pub concurrent_regions: Vec<Vec<String>>, // For parallel states
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateType {
    Simple,
    Composite,
    Start,      // [*] as source
    End,        // [*] as target
    Choice,     // <<choice>>
    Fork,       // <<fork>>
    Join,       // <<join>>
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub event: Option<String>,
    pub guard: Option<String>,
    pub action: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub position: NotePosition,
    pub target: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotePosition {
    LeftOf,
    RightOf,
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateToken {
    StateDiagram,                 // "stateDiagram"
    StateDiagramV2,               // "stateDiagram-v2"
    State,                        // "state"
    Note,                         // "note"
    Direction,                    // "direction"
    StateId(String),              // State identifier
    As,                           // "as"
    Arrow,                        // "-->"
    ConcurrentSeparator,          // "--"
    LeftBrace,                    // {
    RightBrace,                   // }
    LeftBracket,                  // [
    RightBracket,                 // ]
    Star,                         // *
    Colon,                        // :
    QuotedString(String),         // "text"
    StereotypeStart,              // <<
    StereotypeEnd,                // >>
    StereotypeName(String),       // choice, fork, join
    NotePosition(NotePosition),   // left of, right of, etc.
    Of,                           // "of"
    Identifier(String),           // General identifier
    Guard(String),                // [condition]
    Comment(String),              // %% comment
    NewLine,
    Eof,
}
```

## Step 2: Lexer Implementation

### Token Recognition
```rust
use chumsky::prelude::*;

pub fn state_lexer() -> impl Parser<char, Vec<StateToken>, Error = Simple<char>> {
    let whitespace = just(' ').or(just('\t')).repeated();
    
    let comment = just('%')
        .then(just('%'))
        .then(take_until(just('\n')))
        .map(|(_, (_, text))| StateToken::Comment(text.into_iter().collect()));
    
    // Keywords
    let state_diagram = text::keyword("stateDiagram-v2")
        .map(|_| StateToken::StateDiagramV2)
        .or(text::keyword("stateDiagram")
            .map(|_| StateToken::StateDiagram));
    
    let state_keyword = text::keyword("state")
        .map(|_| StateToken::State);
    
    let note_keyword = text::keyword("note")
        .map(|_| StateToken::Note);
    
    let direction_keyword = text::keyword("direction")
        .map(|_| StateToken::Direction);
    
    let as_keyword = text::keyword("as")
        .map(|_| StateToken::As);
    
    let of_keyword = text::keyword("of")
        .map(|_| StateToken::Of);
    
    // Note positions
    let note_position = choice((
        text::keyword("left").then_ignore(whitespace).then(text::keyword("of"))
            .map(|_| StateToken::NotePosition(NotePosition::LeftOf)),
        text::keyword("right").then_ignore(whitespace).then(text::keyword("of"))
            .map(|_| StateToken::NotePosition(NotePosition::RightOf)),
        text::keyword("above")
            .map(|_| StateToken::NotePosition(NotePosition::Above)),
        text::keyword("below")
            .map(|_| StateToken::NotePosition(NotePosition::Below)),
    ));
    
    // Arrows and symbols
    let arrow = text::string("-->").map(|_| StateToken::Arrow);
    let concurrent_sep = text::string("--").map(|_| StateToken::ConcurrentSeparator);
    
    // Special state [*]
    let special_state = just('[')
        .then(just('*'))
        .then(just(']'))
        .map(|_| StateToken::StateId("[*]".to_string()));
    
    // Stereotypes <<choice>>, <<fork>>, <<join>>
    let stereotype = text::string("<<")
        .ignore_then(
            choice((
                text::keyword("choice"),
                text::keyword("fork"),
                text::keyword("join"),
                text::keyword("end"),
            ))
        )
        .then_ignore(text::string(">>"))
        .map(|name| StateToken::StereotypeName(name.to_string()));
    
    let left_brace = just('{').map(|_| StateToken::LeftBrace);
    let right_brace = just('}').map(|_| StateToken::RightBrace);
    let left_bracket = just('[').map(|_| StateToken::LeftBracket);
    let right_bracket = just(']').map(|_| StateToken::RightBracket);
    let colon = just(':').map(|_| StateToken::Colon);
    
    // Guard condition [condition]
    let guard = just('[')
        .ignore_then(
            none_of("]")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just(']'))
        .map(|g| StateToken::Guard(g.trim().to_string()));
    
    // Quoted string
    let quoted_string = just('"')
        .ignore_then(
            none_of("\"")
                .repeated()
                .collect::<String>()
        )
        .then_ignore(just('"'))
        .map(StateToken::QuotedString);
    
    // Identifier (state names, events, etc.)
    let identifier = filter(|c: &char| {
        c.is_alphanumeric() || *c == '_' || *c == '-'
    })
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(StateToken::Identifier);
    
    let newline = just('\n').map(|_| StateToken::NewLine);
    
    let token = choice((
        comment,
        state_diagram,
        state_keyword,
        note_keyword,
        direction_keyword,
        as_keyword,
        of_keyword,
        note_position,
        arrow,
        concurrent_sep,
        special_state,
        stereotype,
        guard,
        left_brace,
        right_brace,
        colon,
        quoted_string,
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

### Complex State Machine Parser
```rust
pub fn state_parser() -> impl Parser<StateToken, StateDiagram, Error = Simple<StateToken>> {
    enum ParseContext {
        TopLevel,
        InState(String, usize), // state_id, nesting_level
    }
    
    let header = choice((
        just(StateToken::StateDiagramV2).map(|_| StateVersion::V2),
        just(StateToken::StateDiagram).map(|_| StateVersion::V1),
    ));
    
    header
        .then_ignore(
            filter(|t| matches!(t, StateToken::NewLine))
                .repeated()
        )
        .then(
            any()
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(just(StateToken::Eof).or_not())
        .map(|(version, tokens)| {
            let mut states = HashMap::new();
            let mut transitions = Vec::new();
            let mut notes = Vec::new();
            let mut context_stack = vec![ParseContext::TopLevel];
            let mut i = 0;
            
            // Add start and end states
            states.insert("[*]".to_string(), State {
                id: "[*]".to_string(),
                display_name: None,
                state_type: StateType::Start,
                substates: Vec::new(),
                concurrent_regions: Vec::new(),
            });
            
            while i < tokens.len() {
                match (&context_stack.last().unwrap(), &tokens[i]) {
                    (_, StateToken::Comment(_)) => {
                        i += 1;
                    }
                    (_, StateToken::NewLine) => {
                        i += 1;
                    }
                    (_, StateToken::State) => {
                        // Parse state definition
                        if let Some((state, consumed)) = parse_state_definition(&tokens[i..]) {
                            states.insert(state.id.clone(), state);
                            i += consumed;
                        } else {
                            i += 1;
                        }
                    }
                    (_, StateToken::StateId(from)) | (_, StateToken::Identifier(from)) => {
                        // Check if this is a transition
                        if i + 1 < tokens.len() && matches!(&tokens[i + 1], StateToken::Arrow) {
                            if let Some((trans, consumed)) = parse_transition(&tokens[i..], from) {
                                // Ensure states exist
                                ensure_state_exists(&mut states, &trans.from);
                                ensure_state_exists(&mut states, &trans.to);
                                transitions.push(trans);
                                i += consumed;
                            } else {
                                i += 1;
                            }
                        } else if i + 1 < tokens.len() && matches!(&tokens[i + 1], StateToken::LeftBrace) {
                            // Composite state
                            let state_id = from.clone();
                            ensure_state_exists(&mut states, &state_id);
                            states.get_mut(&state_id).unwrap().state_type = StateType::Composite;
                            context_stack.push(ParseContext::InState(state_id, 1));
                            i += 2; // Skip state name and {
                        } else {
                            i += 1;
                        }
                    }
                    (ParseContext::InState(parent_id, _), StateToken::RightBrace) => {
                        context_stack.pop();
                        i += 1;
                    }
                    (_, StateToken::Note) => {
                        if let Some((note, consumed)) = parse_note(&tokens[i..]) {
                            notes.push(note);
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
            
            StateDiagram {
                title: None,
                accessibility: AccessibilityInfo::default(),
                version,
                states,
                transitions,
                notes,
            }
        })
}

fn parse_state_definition(tokens: &[StateToken]) -> Option<(State, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip "state"
    
    let (display_name, id) = match &tokens[i] {
        StateToken::QuotedString(name) => {
            i += 1;
            if matches!(&tokens[i], StateToken::As) {
                i += 1;
                match &tokens[i] {
                    StateToken::Identifier(id) => {
                        i += 1;
                        (Some(name.clone()), id.clone())
                    }
                    _ => return None,
                }
            } else {
                (None, name.clone())
            }
        }
        StateToken::Identifier(id) => {
            i += 1;
            (None, id.clone())
        }
        _ => return None,
    };
    
    let state_type = if i < tokens.len() {
        match &tokens[i] {
            StateToken::StereotypeName(name) => {
                i += 1;
                match name.as_str() {
                    "choice" => StateType::Choice,
                    "fork" => StateType::Fork,
                    "join" => StateType::Join,
                    _ => StateType::Simple,
                }
            }
            _ => StateType::Simple,
        }
    } else {
        StateType::Simple
    };
    
    Some((
        State {
            id,
            display_name,
            state_type,
            substates: Vec::new(),
            concurrent_regions: Vec::new(),
        },
        i,
    ))
}

fn parse_transition(tokens: &[StateToken], from: &str) -> Option<(Transition, usize)> {
    if tokens.len() < 3 {
        return None;
    }
    
    let mut i = 1; // Skip from state
    
    if !matches!(&tokens[i], StateToken::Arrow) {
        return None;
    }
    i += 1;
    
    let to = match &tokens[i] {
        StateToken::StateId(id) | StateToken::Identifier(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    let mut event = None;
    let mut guard = None;
    let mut action = None;
    
    // Parse optional transition label
    if i < tokens.len() && matches!(&tokens[i], StateToken::Colon) {
        i += 1;
        
        // Collect transition text until newline or guard
        let mut label_parts = Vec::new();
        while i < tokens.len() {
            match &tokens[i] {
                StateToken::NewLine => break,
                StateToken::Guard(g) => {
                    guard = Some(g.clone());
                    i += 1;
                    break;
                }
                StateToken::Identifier(text) => {
                    label_parts.push(text.clone());
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
        
        if !label_parts.is_empty() {
            let full_label = label_parts.join(" ");
            // Simple heuristic: if contains '/', split into event/action
            if let Some(slash_pos) = full_label.find('/') {
                event = Some(full_label[..slash_pos].trim().to_string());
                action = Some(full_label[slash_pos + 1..].trim().to_string());
            } else {
                event = Some(full_label);
            }
        }
    }
    
    Some((
        Transition {
            from: from.to_string(),
            to,
            event,
            guard,
            action,
        },
        i,
    ))
}

fn parse_note(tokens: &[StateToken]) -> Option<(Note, usize)> {
    if tokens.len() < 5 {
        return None;
    }
    
    let mut i = 1; // Skip "note"
    
    let position = match &tokens[i] {
        StateToken::NotePosition(pos) => pos.clone(),
        _ => return None,
    };
    i += 1;
    
    if position == NotePosition::LeftOf || position == NotePosition::RightOf {
        if !matches!(&tokens[i], StateToken::Of) {
            i += 1; // Skip "of" if it's a separate token
        }
    }
    
    let target = match &tokens[i] {
        StateToken::Identifier(id) => id.clone(),
        _ => return None,
    };
    i += 1;
    
    if !matches!(&tokens[i], StateToken::Colon) {
        return None;
    }
    i += 1;
    
    let text = match &tokens[i] {
        StateToken::Identifier(t) | StateToken::QuotedString(t) => t.clone(),
        _ => return None,
    };
    i += 1;
    
    Some((
        Note {
            position,
            target,
            text,
        },
        i,
    ))
}

fn ensure_state_exists(states: &mut HashMap<String, State>, state_id: &str) {
    if state_id == "[*]" {
        // Already added in initialization
        return;
    }
    
    if !states.contains_key(state_id) {
        states.insert(state_id.to_string(), State {
            id: state_id.to_string(),
            display_name: None,
            state_type: StateType::Simple,
            substates: Vec::new(),
            concurrent_regions: Vec::new(),
        });
    }
}
```

## Step 4: Test Data Collection

### Source Files
Copy relevant `.mermaid` files from our extracted samples:
- Location: `mermaid-samples/state/`
- Expected count: 124 files
- Copy to: `mermaid-parser/test/state/`

### Command
```bash
cp -r ../mermaid-samples/state/* ./test/state/
```

## Step 5: Unit Testing

### Test Structure
```rust
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_state_files(#[files("test/state/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    // Remove metadata comments
    let content = content.lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    let tokens = state_lexer().parse(content.chars()).unwrap_or_else(|e| {
        panic!("Lexer failed for {:?}: {:?}", path, e);
    });
    
    let diagram = state_parser().parse(tokens).unwrap_or_else(|e| {
        panic!("Parser failed for {:?}: {:?}", path, e);
    });
    
    // Validate structure
    assert!(!diagram.states.is_empty(), "Should have at least one state");
}

#[test]
fn test_simple_state_diagram() {
    let input = r#"stateDiagram-v2
    [*] --> Still
    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]
"#;
    
    let tokens = state_lexer().parse(input.chars()).unwrap();
    let diagram = state_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.version, StateVersion::V2);
    assert!(diagram.states.contains_key("Still"));
    assert!(diagram.states.contains_key("Moving"));
    assert!(diagram.states.contains_key("Crash"));
    assert_eq!(diagram.transitions.len(), 5);
}

#[test]
fn test_composite_states() {
    let input = r#"stateDiagram-v2
    state Moving {
        [*] --> Idle
        Idle --> Running
        Running --> Idle
    }
"#;
    
    let tokens = state_lexer().parse(input.chars()).unwrap();
    let diagram = state_parser().parse(tokens).unwrap();
    
    let moving = &diagram.states["Moving"];
    assert_eq!(moving.state_type, StateType::Composite);
    assert!(diagram.states.contains_key("Idle"));
    assert!(diagram.states.contains_key("Running"));
}

#[test]
fn test_state_with_display_name() {
    let input = r#"stateDiagram
    state "This is the display name" as s1
    s1 --> s2
"#;
    
    let tokens = state_lexer().parse(input.chars()).unwrap();
    let diagram = state_parser().parse(tokens).unwrap();
    
    let s1 = &diagram.states["s1"];
    assert_eq!(s1.display_name, Some("This is the display name".to_string()));
}

#[test]
fn test_transitions_with_events() {
    let input = r#"stateDiagram-v2
    Idle --> Running : start
    Running --> Idle : stop / cleanup
"#;
    
    let tokens = state_lexer().parse(input.chars()).unwrap();
    let diagram = state_parser().parse(tokens).unwrap();
    
    let start_trans = diagram.transitions.iter()
        .find(|t| t.from == "Idle" && t.to == "Running")
        .unwrap();
    assert_eq!(start_trans.event, Some("start".to_string()));
    
    let stop_trans = diagram.transitions.iter()
        .find(|t| t.from == "Running" && t.to == "Idle")
        .unwrap();
    assert_eq!(stop_trans.event, Some("stop".to_string()));
    assert_eq!(stop_trans.action, Some("cleanup".to_string()));
}

#[test]
fn test_choice_state() {
    let input = r#"stateDiagram-v2
    state choice1 <<choice>>
    Moving --> choice1
    choice1 --> Crash : [speed > 100]
    choice1 --> Still : [speed <= 100]
"#;
    
    let tokens = state_lexer().parse(input.chars()).unwrap();
    let diagram = state_parser().parse(tokens).unwrap();
    
    let choice = &diagram.states["choice1"];
    assert_eq!(choice.state_type, StateType::Choice);
    
    let crash_trans = diagram.transitions.iter()
        .find(|t| t.from == "choice1" && t.to == "Crash")
        .unwrap();
    assert_eq!(crash_trans.guard, Some("speed > 100".to_string()));
}

#[test]
fn test_notes() {
    let input = r#"stateDiagram-v2
    State1 --> State2
    note right of State1 : This is a note
    note left of State2 : Another note
"#;
    
    let tokens = state_lexer().parse(input.chars()).unwrap();
    let diagram = state_parser().parse(tokens).unwrap();
    
    assert_eq!(diagram.notes.len(), 2);
    
    let note1 = &diagram.notes[0];
    assert_eq!(note1.position, NotePosition::RightOf);
    assert_eq!(note1.target, "State1");
    assert_eq!(note1.text, "This is a note");
}
```

## Success Criteria
1. ✅ Parse all 124 state diagram sample files successfully
2. ✅ Handle simple and composite states
3. ✅ Support state aliases with display names
4. ✅ Parse transitions with events, guards, and actions
5. ✅ Handle special states ([*] for start/end)
6. ✅ Support choice, fork, and join states
7. ✅ Parse notes with positioning
8. ✅ Handle nested state hierarchies
9. ✅ Support both v1 and v2 syntax

## Implementation Priority
**Priority 19** - Implement in Phase 4 after sequence diagrams. State diagrams are among the most complex with nested states, concurrent regions, and sophisticated transition syntax. The hierarchical structure patterns from earlier diagrams and the event-driven concepts from sequence diagrams provide necessary foundations.