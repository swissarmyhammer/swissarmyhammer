use mermaid_parser::common::ast::{StateNotePosition, StateType, StateVersion};
use mermaid_parser::parse_diagram;
use mermaid_parser::parsers::state;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_state_files(#[files("test/state/*.mermaid")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));

    // Remove metadata comments
    let content = content
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    // Skip empty files or files with only test identifiers
    if content.is_empty() {
        return;
    }

    let result = parse_diagram(&content);

    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result);

    match result.unwrap() {
        mermaid_parser::DiagramType::State(_diagram) => {
            // Just verify it parsed successfully - specific functionality tested in unit tests
        }
        _ => panic!("Expected State diagram from {:?}", path),
    }
}

#[test]
fn test_simple_state_diagram() {
    let input = r#"stateDiagram-v2
    [*] --> Still
    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.version, StateVersion::V2);
    assert!(diagram.states.contains_key("Still"));
    assert!(diagram.states.contains_key("Moving"));
    assert!(diagram.states.contains_key("Crash"));
    assert!(diagram.states.contains_key("[*]"));
    assert_eq!(diagram.transitions.len(), 5);
}

#[test]
fn test_v1_state_diagram() {
    let input = r#"stateDiagram
    [*] --> State1
    State1 --> [*]"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.version, StateVersion::V1);
    assert!(diagram.states.contains_key("State1"));
    assert_eq!(diagram.transitions.len(), 2);
}

#[test]
fn test_state_with_display_name() {
    let input = r#"stateDiagram
    state "This is the display name" as s1
    s1 --> s2"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let s1 = &diagram.states["s1"];
    assert_eq!(
        s1.display_name,
        Some("This is the display name".to_string())
    );
    assert_eq!(diagram.transitions.len(), 1);
}

#[test]
fn test_choice_state() {
    let input = r#"stateDiagram-v2
    state choice1 <<choice>>
    Moving --> choice1
    choice1 --> Crash: [speed > 100]
    choice1 --> Still: [speed <= 100]"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let choice = &diagram.states["choice1"];
    assert_eq!(choice.state_type, StateType::Choice);

    // Find transitions with guards
    let crash_trans = diagram
        .transitions
        .iter()
        .find(|t| t.from == "choice1" && t.to == "Crash")
        .unwrap();
    assert_eq!(crash_trans.guard, Some("speed > 100".to_string()));

    let still_trans = diagram
        .transitions
        .iter()
        .find(|t| t.from == "choice1" && t.to == "Still")
        .unwrap();
    assert_eq!(still_trans.guard, Some("speed <= 100".to_string()));
}

#[test]
fn test_transitions_with_events() {
    let input = r#"stateDiagram-v2
    Idle --> Running: start
    Running --> Idle: stop / cleanup
    State1 --> State2: event[guard]/action"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let start_trans = diagram
        .transitions
        .iter()
        .find(|t| t.from == "Idle" && t.to == "Running")
        .unwrap();
    assert_eq!(start_trans.event, Some("start".to_string()));
    assert_eq!(start_trans.guard, None);
    assert_eq!(start_trans.action, None);

    let stop_trans = diagram
        .transitions
        .iter()
        .find(|t| t.from == "Running" && t.to == "Idle")
        .unwrap();
    assert_eq!(stop_trans.event, Some("stop".to_string()));
    assert_eq!(stop_trans.action, Some("cleanup".to_string()));

    let complex_trans = diagram
        .transitions
        .iter()
        .find(|t| t.from == "State1" && t.to == "State2")
        .unwrap();
    assert_eq!(complex_trans.event, Some("event".to_string()));
    assert_eq!(complex_trans.guard, Some("guard".to_string()));
    assert_eq!(complex_trans.action, Some("action".to_string()));
}

#[test]
fn test_composite_states() {
    let input = r#"stateDiagram-v2
    state Moving {
        [*] --> Idle
        Idle --> Running
        Running --> Idle
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let moving = &diagram.states["Moving"];
    assert_eq!(moving.state_type, StateType::Composite);
    assert!(moving.substates.contains(&"Idle".to_string()));
    assert!(moving.substates.contains(&"Running".to_string()));

    // Should have states for Idle and Running
    assert!(diagram.states.contains_key("Idle"));
    assert!(diagram.states.contains_key("Running"));
}

#[test]
fn test_notes() {
    let input = r#"stateDiagram-v2
    State1 --> State2
    note right of State1: This is a note
    note left of State2: Another note
    note above State3: Above note
    note below State4: Below note"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.notes.len(), 4);

    let note1 = diagram.notes.iter().find(|n| n.target == "State1").unwrap();
    assert_eq!(note1.position, StateNotePosition::RightOf);
    assert_eq!(note1.text, "This is a note");

    let note2 = diagram.notes.iter().find(|n| n.target == "State2").unwrap();
    assert_eq!(note2.position, StateNotePosition::LeftOf);
    assert_eq!(note2.text, "Another note");

    let note3 = diagram.notes.iter().find(|n| n.target == "State3").unwrap();
    assert_eq!(note3.position, StateNotePosition::Above);

    let note4 = diagram.notes.iter().find(|n| n.target == "State4").unwrap();
    assert_eq!(note4.position, StateNotePosition::Below);
}

#[test]
fn test_state_stereotypes() {
    let input = r#"stateDiagram-v2
    state choice1 <<choice>>
    state fork1 <<fork>>
    state join1 <<join>>
    state end1 <<end>>"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.states["choice1"].state_type, StateType::Choice);
    assert_eq!(diagram.states["fork1"].state_type, StateType::Fork);
    assert_eq!(diagram.states["join1"].state_type, StateType::Join);
    assert_eq!(diagram.states["end1"].state_type, StateType::End);
}

#[test]
fn test_title() {
    let input = r#"stateDiagram-v2
    title My State Diagram
    State1 --> State2"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.title, Some("My State Diagram".to_string()));
}

#[test]
fn test_complex_example() {
    let input = r#"stateDiagram-v2
    title Authentication State Machine
    
    [*] --> Idle
    Idle --> Authenticating: login
    
    state Authenticating {
        [*] --> CheckingCredentials
        CheckingCredentials --> ValidCredentials: valid
        CheckingCredentials --> InvalidCredentials: invalid
        ValidCredentials --> [*]
        InvalidCredentials --> [*]
    }
    
    Authenticating --> Authenticated: success
    Authenticating --> Idle: failure
    
    state choice1 <<choice>>
    Authenticated --> choice1
    choice1 --> AdminPanel: [role = admin]
    choice1 --> UserPanel: [role = user]
    
    AdminPanel --> Idle: logout
    UserPanel --> Idle: logout
    
    note right of Authenticating: Complex state with nested logic
    note left of choice1: Route based on user role"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(
        diagram.title,
        Some("Authentication State Machine".to_string())
    );
    assert_eq!(diagram.version, StateVersion::V2);

    // Check composite state
    let auth_state = &diagram.states["Authenticating"];
    assert_eq!(auth_state.state_type, StateType::Composite);
    assert!(!auth_state.substates.is_empty());

    // Check choice state
    let choice_state = &diagram.states["choice1"];
    assert_eq!(choice_state.state_type, StateType::Choice);

    // Check transitions with guards
    let admin_trans = diagram
        .transitions
        .iter()
        .find(|t| t.from == "choice1" && t.to == "AdminPanel")
        .unwrap();
    assert_eq!(admin_trans.guard, Some("role = admin".to_string()));

    // Check notes
    assert_eq!(diagram.notes.len(), 2);
}

#[test]
fn test_error_cases() {
    // Test empty input
    let result = state::parse("");
    assert!(result.is_err());

    // Test invalid header
    let result = state::parse("not a state diagram");
    assert!(result.is_err());
}
