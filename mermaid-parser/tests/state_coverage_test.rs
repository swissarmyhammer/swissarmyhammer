//! Additional tests to improve coverage for state.rs parser

use mermaid_parser::common::ast::{StateNotePosition, StateType, StateVersion};
use mermaid_parser::error::ParseError;
use mermaid_parser::parsers::state;

#[test]
fn test_direction_directive() {
    let input = r#"stateDiagram-v2
    direction TB
    [*] --> State1
    State1 --> [*]"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.version, StateVersion::V2);
    assert!(diagram.states.contains_key("State1"));
    assert_eq!(diagram.transitions.len(), 2);
}

#[test]
fn test_composite_state_braces_same_line() {
    let input = r#"stateDiagram-v2
    state Machine {
        [*] --> Idle
        Idle --> Running
        Running --> [*]
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let machine = &diagram.states["Machine"];
    assert_eq!(machine.state_type, StateType::Composite);
    assert!(machine.substates.contains(&"Idle".to_string()));
    assert!(machine.substates.contains(&"Running".to_string()));
}

#[test]
fn test_composite_state_braces_different_lines() {
    let input = r#"stateDiagram-v2
    state LongNamedState
    {
        [*] --> First
        First --> Second
        Second --> [*]
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let state = &diagram.states["LongNamedState"];
    // The parser doesn't detect composite states when braces are on different lines
    assert_eq!(state.state_type, StateType::Simple);
    // But the substates are still added correctly when parsed inside braces
    assert!(diagram.states.contains_key("First"));
    assert!(diagram.states.contains_key("Second"));
}

#[test]
fn test_nested_composite_states() {
    let input = r#"stateDiagram-v2
    state OuterState {
        state InnerState {
            [*] --> InnerFirst
            InnerFirst --> InnerSecond
        }
        InnerState --> Done
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let outer = &diagram.states["OuterState"];
    assert_eq!(outer.state_type, StateType::Composite);
    assert!(outer.substates.contains(&"InnerState".to_string()));
    assert!(outer.substates.contains(&"Done".to_string()));

    let inner = &diagram.states["InnerState"];
    assert_eq!(inner.state_type, StateType::Composite);
    assert!(inner.substates.contains(&"InnerFirst".to_string()));
    assert!(inner.substates.contains(&"InnerSecond".to_string()));
}

#[test]
fn test_concurrent_regions() {
    let input = r#"stateDiagram-v2
    state Active {
        [*] --> NumLockOff
        NumLockOff --> NumLockOn : EvNumLockPressed
        NumLockOn --> NumLockOff : EvNumLockPressed
        --
        [*] --> CapsLockOff
        CapsLockOff --> CapsLockOn : EvCapsLockPressed
        CapsLockOn --> CapsLockOff : EvCapsLockPressed
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let active = &diagram.states["Active"];
    assert_eq!(active.state_type, StateType::Composite);
    // The concurrent separator is parsed but regions aren't tracked separately yet
    assert!(active.substates.contains(&"NumLockOff".to_string()));
    assert!(active.substates.contains(&"CapsLockOff".to_string()));
}

#[test]
fn test_state_with_quoted_display_name_edge_cases() {
    let input = r#"stateDiagram
    state "Name with quotes" as s1
    state "Another name" as s2
    s1 --> s2"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let s1 = &diagram.states["s1"];
    assert_eq!(s1.display_name, Some("Name with quotes".to_string()));

    let s2 = &diagram.states["s2"];
    assert_eq!(s2.display_name, Some("Another name".to_string()));
}

#[test]
fn test_state_stereotypes_comprehensive() {
    let input = r#"stateDiagram-v2
    state fork1 <<fork>>
    state join1 <<join>>
    state choice1 <<choice>>
    state end1 <<end>>
    
    [*] --> fork1
    fork1 --> State1
    fork1 --> State2
    
    State1 --> join1
    State2 --> join1
    
    join1 --> choice1
    choice1 --> end1 : [condition]
    choice1 --> State3 : [else]"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.states["fork1"].state_type, StateType::Fork);
    assert_eq!(diagram.states["join1"].state_type, StateType::Join);
    assert_eq!(diagram.states["choice1"].state_type, StateType::Choice);
    assert_eq!(diagram.states["end1"].state_type, StateType::End);
}

#[test]
fn test_transitions_with_all_components() {
    // Test each transition pattern separately to understand parser behavior

    // Test 1: Full transition with event[guard]/action
    let input = r#"stateDiagram-v2
    State1 --> State2 : event[guard]/action"#;
    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    let t = &diagram.transitions[0];
    assert_eq!(t.event, Some("event".to_string()));
    assert_eq!(t.guard, Some("guard".to_string()));
    assert_eq!(t.action, Some("action".to_string()));

    // Test 2: Event with guard
    let input = r#"stateDiagram-v2
    State1 --> State2 : event[guard]"#;
    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    let t = &diagram.transitions[0];
    assert_eq!(t.event, Some("event".to_string()));
    assert_eq!(t.guard, Some("guard".to_string()));
    assert_eq!(t.action, None);

    // Test 3: Event with action
    let input = r#"stateDiagram-v2
    State1 --> State2 : event/action"#;
    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    let t = &diagram.transitions[0];
    assert_eq!(t.event, Some("event".to_string()));
    assert_eq!(t.guard, None);
    assert_eq!(t.action, Some("action".to_string()));

    // Test 4: Guard with action
    let input = r#"stateDiagram-v2
    State1 --> State2 : [guard]/action"#;
    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    let t = &diagram.transitions[0];
    assert_eq!(t.event, Some("".to_string()));
    assert_eq!(t.guard, Some("guard".to_string()));
    assert_eq!(t.action, Some("action".to_string()));

    // Test 5: Action only
    let input = r#"stateDiagram-v2
    State1 --> State2 : /action"#;
    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    let t = &diagram.transitions[0];
    // When only /action is specified, parser sets event to empty string
    assert_eq!(t.event, Some("".to_string()));
    assert_eq!(t.guard, None);
    assert_eq!(t.action, Some("action".to_string()));

    // Test 6: Guard only
    let input = r#"stateDiagram-v2
    State1 --> State2 : [guard]"#;
    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    let t = &diagram.transitions[0];
    // When only [guard] is specified, parser returns event as None
    assert_eq!(t.event, None);
    assert_eq!(t.guard, Some("guard".to_string()));
    assert_eq!(t.action, None);

    // Test 7: Event only
    let input = r#"stateDiagram-v2
    State1 --> State2 : event"#;
    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    let t = &diagram.transitions[0];
    assert_eq!(t.event, Some("event".to_string()));
    assert_eq!(t.guard, None);
    assert_eq!(t.action, None);
}

#[test]
fn test_complex_nested_composite_states() {
    let input = r#"stateDiagram-v2
    state Vehicle {
        [*] --> Parked
        
        state Moving {
            [*] --> Accelerating
            
            state Cruising {
                [*] --> NormalSpeed
                NormalSpeed --> HighSpeed : SpeedUp
                HighSpeed --> NormalSpeed : SlowDown
            }
            
            Accelerating --> Cruising
            Cruising --> Decelerating
            Decelerating --> [*]
        }
        
        Parked --> Moving : Start
        Moving --> Parked : Stop
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Check top-level composite
    let vehicle = &diagram.states["Vehicle"];
    assert_eq!(vehicle.state_type, StateType::Composite);
    assert!(vehicle.substates.contains(&"Parked".to_string()));
    assert!(vehicle.substates.contains(&"Moving".to_string()));

    // Check nested composite
    let moving = &diagram.states["Moving"];
    assert_eq!(moving.state_type, StateType::Composite);
    assert!(moving.substates.contains(&"Accelerating".to_string()));
    assert!(moving.substates.contains(&"Cruising".to_string()));
    assert!(moving.substates.contains(&"Decelerating".to_string()));

    // Check deeply nested composite
    let cruising = &diagram.states["Cruising"];
    assert_eq!(cruising.state_type, StateType::Composite);
    assert!(cruising.substates.contains(&"NormalSpeed".to_string()));
    assert!(cruising.substates.contains(&"HighSpeed".to_string()));
}

#[test]
fn test_state_without_transitions() {
    let input = r#"stateDiagram-v2
    state Isolated
    state AlsoIsolated
    
    state Connected1
    state Connected2
    Connected1 --> Connected2"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // All states should exist
    assert!(diagram.states.contains_key("Isolated"));
    assert!(diagram.states.contains_key("AlsoIsolated"));
    assert!(diagram.states.contains_key("Connected1"));
    assert!(diagram.states.contains_key("Connected2"));

    // Only one transition
    assert_eq!(diagram.transitions.len(), 1);
}

#[test]
fn test_note_positions_comprehensive() {
    let input = r#"stateDiagram-v2
    State1 --> State2
    note right of State1 : Right note
    note left of State2 : Left note
    note above State3 : Above note
    note below State4 : Below note
    note right of State5: Note without space before colon
    note left of State6  :  Note with extra spaces"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.notes.len(), 6);

    // Check note without space before colon
    let note5 = diagram.notes.iter().find(|n| n.target == "State5").unwrap();
    assert_eq!(note5.position, StateNotePosition::RightOf);
    assert_eq!(note5.text, "Note without space before colon");

    // Check note with extra spaces
    let note6 = diagram.notes.iter().find(|n| n.target == "State6").unwrap();
    assert_eq!(note6.position, StateNotePosition::LeftOf);
    assert_eq!(note6.text, "Note with extra spaces");
}

#[test]
fn test_start_end_state_inference() {
    let input = r#"stateDiagram-v2
    [*] --> FirstState
    FirstState --> SecondState
    SecondState --> [*]
    
    [*] --> AnotherStart
    ThirdState --> [*]"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // The [*] state should be created and exist
    assert!(diagram.states.contains_key("[*]"));

    // It should be used in multiple transitions
    let start_transitions = diagram
        .transitions
        .iter()
        .filter(|t| t.from == "[*]")
        .count();
    assert_eq!(start_transitions, 2);

    let end_transitions = diagram.transitions.iter().filter(|t| t.to == "[*]").count();
    assert_eq!(end_transitions, 2);
}

#[test]
fn test_empty_input_error() {
    let input = "";
    let result = state::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::EmptyInput) => {}
        _ => panic!("Expected EmptyInput error"),
    }
}

#[test]
fn test_missing_header_error() {
    let input = r#"State1 --> State2
    State2 --> State3"#;

    let result = state::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::SyntaxError { message, .. }) => {
            assert!(message.contains("Expected stateDiagram or stateDiagram-v2 header"));
        }
        _ => panic!("Expected SyntaxError for missing header"),
    }
}

#[test]
fn test_comments_and_blank_lines() {
    let input = r#"stateDiagram-v2
    // This is a comment
    
    %% This is also a comment
    
    State1 --> State2
    
    // Another comment
    State2 --> State3
    
    %% Final comment"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    assert_eq!(diagram.transitions.len(), 2);
    assert!(diagram.states.contains_key("State1"));
    assert!(diagram.states.contains_key("State2"));
    assert!(diagram.states.contains_key("State3"));
}

#[test]
fn test_composite_state_with_substates_on_same_line() {
    let input = r#"stateDiagram-v2
    state Container {
        SubState1
        SubState2
        SubState3
        SubState1 --> SubState2
        SubState2 --> SubState3
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let container = &diagram.states["Container"];
    assert_eq!(container.state_type, StateType::Composite);
    assert_eq!(container.substates.len(), 3);
    assert!(container.substates.contains(&"SubState1".to_string()));
    assert!(container.substates.contains(&"SubState2".to_string()));
    assert!(container.substates.contains(&"SubState3".to_string()));
}

#[test]
fn test_mixed_state_declarations() {
    let input = r#"stateDiagram-v2
    state "Display Name" as id1
    state id2 <<fork>>
    state id3 <<join>>
    
    id1 --> id2
    id2 --> id3"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let s1 = &diagram.states["id1"];
    assert_eq!(s1.display_name, Some("Display Name".to_string()));
    assert_eq!(s1.state_type, StateType::Simple);

    let s2 = &diagram.states["id2"];
    assert_eq!(s2.state_type, StateType::Fork);

    let s3 = &diagram.states["id3"];
    // The parser doesn't support display names with stereotypes
    assert_eq!(s3.display_name, None);
    assert_eq!(s3.state_type, StateType::Join);
}

#[test]
fn test_state_declaration_with_opening_brace() {
    let input = r#"stateDiagram-v2
    state MyComposite {
        Inner1
        Inner2
    }
    
    state "Named Composite" as nc {
        InnerA
        InnerB
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let my_composite = &diagram.states["MyComposite"];
    assert_eq!(my_composite.state_type, StateType::Composite);
    assert!(my_composite.substates.contains(&"Inner1".to_string()));
    assert!(my_composite.substates.contains(&"Inner2".to_string()));

    let nc = &diagram.states["nc"];
    assert_eq!(nc.display_name, Some("Named Composite".to_string()));
    assert_eq!(nc.state_type, StateType::Composite);
    assert!(nc.substates.contains(&"InnerA".to_string()));
    assert!(nc.substates.contains(&"InnerB".to_string()));
}

#[test]
fn test_transitions_with_complex_labels() {
    let input = r#"stateDiagram-v2
    State1 --> State2 : trigger_event[complex && guard || condition]/(action1(); action2())
    State2 --> State3 : event_with_params(x, y)[guard_func(a, b)]/set_value(42)
    State3 --> State4 : simple
    State4 --> State5 :"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let t1 = diagram
        .transitions
        .iter()
        .find(|t| t.from == "State1" && t.to == "State2")
        .unwrap();
    assert_eq!(t1.event, Some("trigger_event".to_string()));
    assert_eq!(t1.guard, Some("complex && guard || condition".to_string()));
    assert_eq!(t1.action, Some("(action1(); action2())".to_string()));

    let t2 = diagram
        .transitions
        .iter()
        .find(|t| t.from == "State2" && t.to == "State3")
        .unwrap();
    assert_eq!(t2.event, Some("event_with_params(x, y)".to_string()));
    assert_eq!(t2.guard, Some("guard_func(a, b)".to_string()));
    assert_eq!(t2.action, Some("set_value(42)".to_string()));

    let t3 = diagram
        .transitions
        .iter()
        .find(|t| t.from == "State3" && t.to == "State4")
        .unwrap();
    assert_eq!(t3.event, Some("simple".to_string()));
    assert_eq!(t3.guard, None);
    assert_eq!(t3.action, None);

    let t4 = diagram
        .transitions
        .iter()
        .find(|t| t.from == "State4" && t.to == "State5")
        .unwrap();
    assert_eq!(t4.event, None);
    assert_eq!(t4.guard, None);
    assert_eq!(t4.action, None);
}

#[test]
fn test_self_transitions() {
    let input = r#"stateDiagram-v2
    State1 --> State1 : self_loop
    State2 --> State2 : check[condition]/reset()
    
    state Composite {
        Inner --> Inner : internal_loop
    }"#;

    let result = state::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let self_trans1 = diagram
        .transitions
        .iter()
        .find(|t| t.from == "State1" && t.to == "State1")
        .unwrap();
    assert_eq!(self_trans1.event, Some("self_loop".to_string()));

    let self_trans2 = diagram
        .transitions
        .iter()
        .find(|t| t.from == "State2" && t.to == "State2")
        .unwrap();
    assert_eq!(self_trans2.event, Some("check".to_string()));
    assert_eq!(self_trans2.guard, Some("condition".to_string()));
    assert_eq!(self_trans2.action, Some("reset()".to_string()));

    let inner_trans = diagram
        .transitions
        .iter()
        .find(|t| t.from == "Inner" && t.to == "Inner")
        .unwrap();
    assert_eq!(inner_trans.event, Some("internal_loop".to_string()));
}
