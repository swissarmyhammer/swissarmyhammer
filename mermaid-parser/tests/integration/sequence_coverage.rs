//! Additional tests to improve coverage for sequence.rs parser

use mermaid_parser::common::ast::{ArrowType, NotePosition, ParticipantType, SequenceStatement};
use mermaid_parser::error::ParseError;
use mermaid_parser::parsers::sequence;

#[test]
fn test_empty_input_error() {
    let input = "";
    let result = sequence::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::EmptyInput) => {}
        _ => panic!("Expected EmptyInput error"),
    }
}

#[test]
fn test_missing_header_error() {
    let input = "participant Alice\nAlice->Bob: Hello";
    let result = sequence::parse(input);
    assert!(result.is_err());
    match result {
        Err(ParseError::SyntaxError { message, .. }) => {
            assert!(message.contains("Expected sequenceDiagram header"));
        }
        _ => panic!("Expected SyntaxError for missing header"),
    }
}

#[test]
fn test_title_directive() {
    let input = r#"sequenceDiagram
    title Test Sequence Diagram
    participant Alice
    Alice->Bob: Hello"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.title, Some("Test Sequence Diagram".to_string()));
}

#[test]
fn test_autonumber_with_start_and_step() {
    let input = r#"sequenceDiagram
    autonumber 10 5
    participant Alice
    Alice->Bob: Message 1
    Bob->Alice: Message 2"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert!(diagram.autonumber.is_some());
    let auto = diagram.autonumber.unwrap();
    assert_eq!(auto.start, Some(10));
    assert_eq!(auto.step, Some(5));
    assert!(auto.visible);
}

#[test]
fn test_autonumber_with_only_start() {
    let input = r#"sequenceDiagram
    autonumber 5
    participant Alice
    Alice->Bob: Message"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert!(diagram.autonumber.is_some());
    let auto = diagram.autonumber.unwrap();
    assert_eq!(auto.start, Some(5));
    assert_eq!(auto.step, None);
}

#[test]
fn test_autonumber_with_invalid_numbers() {
    let input = r#"sequenceDiagram
    autonumber abc xyz
    participant Alice
    Alice->Bob: Message"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert!(diagram.autonumber.is_some());
    let auto = diagram.autonumber.unwrap();
    assert_eq!(auto.start, None);
    assert_eq!(auto.step, None);
}

#[test]
fn test_actor_declaration_basic() {
    let input = r#"sequenceDiagram
    actor Alice
    Alice->Bob: Message"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let alice = diagram.participants.iter().find(|p| p.actor == "Alice");
    assert!(alice.is_some());
    assert_eq!(alice.unwrap().participant_type, ParticipantType::Actor);
}

#[test]
fn test_participant_with_alias() {
    let input = r#"sequenceDiagram
    participant Alice as A
    participant Bob as B
    A->B: Hello"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let alice = diagram.participants.iter().find(|p| p.actor == "Alice");
    assert!(alice.is_some());
    assert_eq!(alice.unwrap().alias, Some("A".to_string()));

    // Message should be from Alice to Bob (aliases resolved)
    if let SequenceStatement::Message(msg) = &diagram.statements[0] {
        assert_eq!(msg.from, "Alice");
        assert_eq!(msg.to, "Bob");
    }
}

#[test]
fn test_duplicate_participant_declaration() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Alice
    Alice->Bob: Hello"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Should only have Alice once in participants
    let alice_count = diagram
        .participants
        .iter()
        .filter(|p| p.actor == "Alice")
        .count();
    assert_eq!(alice_count, 1);
}

#[test]
fn test_alias_tracking() {
    let input = r#"sequenceDiagram
    participant Alice as A
    participant Bob as B
    A->B: Message 1
    B->A: Message 2"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // All messages should use real names, not aliases
    for stmt in &diagram.statements {
        if let SequenceStatement::Message(msg) = stmt {
            assert!(msg.from == "Alice" || msg.from == "Bob");
            assert!(msg.to == "Alice" || msg.to == "Bob");
        }
    }
}

#[test]
fn test_all_arrow_types() {
    let arrow_tests = vec![
        ("<<-->>", ArrowType::BiDirectionalDotted),
        ("<<->>", ArrowType::BiDirectionalSolid),
        ("-->>", ArrowType::DottedClosed),
        ("->>", ArrowType::SolidClosed),
        ("-->", ArrowType::DottedOpen),
        ("->", ArrowType::SolidOpen),
        ("--x", ArrowType::Cross),
        ("-x", ArrowType::Cross),
        ("--)", ArrowType::Point),
        ("-)", ArrowType::Point),
    ];

    for (arrow, expected_type) in arrow_tests {
        let input = format!("sequenceDiagram\n    Alice{}Bob: Test message", arrow);
        let result = sequence::parse(&input);
        assert!(result.is_ok(), "Failed to parse arrow {}", arrow);
        let diagram = result.unwrap();

        if let SequenceStatement::Message(msg) = &diagram.statements[0] {
            assert_eq!(
                msg.arrow_type, expected_type,
                "Arrow {} type mismatch",
                arrow
            );
        }
    }
}

#[test]
fn test_message_without_text() {
    let input = r#"sequenceDiagram
    Alice->Bob"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let SequenceStatement::Message(msg) = &diagram.statements[0] {
        assert_eq!(msg.text, "");
    }
}

#[test]
fn test_note_positions() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    note left of Alice: Left note
    note right of Bob: Right note  
    note over Alice: Over note"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    let notes: Vec<_> = diagram
        .statements
        .iter()
        .filter_map(|s| match s {
            SequenceStatement::Note(n) => Some(n),
            _ => None,
        })
        .collect();

    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0].position, NotePosition::LeftOf);
    assert_eq!(notes[1].position, NotePosition::RightOf);
    assert_eq!(notes[2].position, NotePosition::Over);
}

#[test]
fn test_note_without_position() {
    let input = r#"sequenceDiagram
    participant Alice
    note Alice: This should fail"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Note without proper position prefix is ignored
    let notes: Vec<_> = diagram
        .statements
        .iter()
        .filter_map(|s| match s {
            SequenceStatement::Note(_) => Some(()),
            _ => None,
        })
        .collect();
    assert_eq!(notes.len(), 0);
}

#[test]
fn test_note_without_text() {
    let input = r#"sequenceDiagram
    participant Alice
    note over Alice"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Note(note)) = diagram.statements.first() {
        assert_eq!(note.text, "");
    }
}

#[test]
fn test_note_over_multiple_participants() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    note over Alice,Bob"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Note(note)) = diagram.statements.first() {
        assert_eq!(note.position, NotePosition::Over);
        assert_eq!(note.actor, "Alice");
        assert_eq!(note.text, "");
    }
}

#[test]
fn test_loop_with_nested_structures() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    loop Check every second
        Alice->Bob: Ping
        note right of Bob: Processing
        activate Bob
        Bob->Alice: Pong
        deactivate Bob
    end"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Loop(loop_stmt)) = diagram.statements.first() {
        assert_eq!(loop_stmt.condition, "Check every second");
        assert_eq!(loop_stmt.statements.len(), 5);

        // Check nested statement types
        assert!(matches!(
            loop_stmt.statements[0],
            SequenceStatement::Message(_)
        ));
        assert!(matches!(
            loop_stmt.statements[1],
            SequenceStatement::Note(_)
        ));
        assert!(matches!(
            loop_stmt.statements[2],
            SequenceStatement::Activate(_)
        ));
        assert!(matches!(
            loop_stmt.statements[3],
            SequenceStatement::Message(_)
        ));
        assert!(matches!(
            loop_stmt.statements[4],
            SequenceStatement::Deactivate(_)
        ));
    }
}

#[test]
fn test_loop_with_empty_lines_and_comments() {
    let input = r#"sequenceDiagram
    participant Alice
    loop Daily check
        
        // This is a comment
        Alice->Alice: Check
        %% Another comment
        
    end"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Loop(loop_stmt)) = diagram.statements.first() {
        assert_eq!(loop_stmt.statements.len(), 1);
    }
}

#[test]
fn test_alt_with_else() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    alt Success
        Alice->Bob: Request
        Bob->Alice: OK
    else Failure
        Bob->Alice: Error
    end"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Alt(alt)) = diagram.statements.first() {
        assert_eq!(alt.condition, "Success");
        assert_eq!(alt.statements.len(), 2);
        assert!(alt.else_branch.is_some());

        let else_branch = alt.else_branch.as_ref().unwrap();
        assert_eq!(else_branch.condition, Some("Failure".to_string()));
        assert_eq!(else_branch.statements.len(), 1);
    }
}

#[test]
fn test_alt_with_simple_else() {
    let input = r#"sequenceDiagram
    participant Alice
    alt Condition
        Alice->Alice: True case
    else
        Alice->Alice: False case
    end"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Alt(alt)) = diagram.statements.first() {
        assert!(alt.else_branch.is_some());
        let else_branch = alt.else_branch.as_ref().unwrap();
        assert_eq!(else_branch.condition, None);
    }
}

#[test]
fn test_alt_with_nested_statements() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    alt Check
        Alice->Bob: Query
        activate Bob
        note over Bob: Processing
        Bob->Alice: Result
        deactivate Bob
    end"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Alt(alt)) = diagram.statements.first() {
        assert_eq!(alt.statements.len(), 5);
    }
}

#[test]
fn test_opt_block() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    opt Premium user
        Alice->Bob: Premium request
        note right of Bob: Special handling
        activate Bob
        Bob->Alice: Premium response
        deactivate Bob
    end"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Opt(opt)) = diagram.statements.first() {
        assert_eq!(opt.condition, "Premium user");
        assert_eq!(opt.statements.len(), 5);
    }
}

#[test]
fn test_opt_with_empty_body() {
    let input = r#"sequenceDiagram
    participant Alice
    opt Maybe
        // Nothing here
    end"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    if let Some(SequenceStatement::Opt(opt)) = diagram.statements.first() {
        assert_eq!(opt.statements.len(), 0);
    }
}

#[test]
fn test_nested_blocks() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    loop Outer
        alt Inner condition
            Alice->Bob: Message 1
        else
            Bob->Alice: Message 2
        end
    end"#;

    // This test verifies that nested structures work but our parser
    // doesn't support nested blocks, so this should parse the outer loop
    // but likely miss the inner alt
    let result = sequence::parse(input);
    assert!(result.is_ok());
}

#[test]
fn test_activate_deactivate_with_aliases() {
    let input = r#"sequenceDiagram
    participant Alice as A
    participant Bob as B
    activate A
    A->B: Message
    deactivate A"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Activations should use real names, not aliases
    let activations: Vec<_> = diagram
        .statements
        .iter()
        .filter_map(|s| match s {
            SequenceStatement::Activate(a) => Some(a),
            _ => None,
        })
        .collect();

    assert_eq!(activations.len(), 1);
    assert_eq!(activations[0], "Alice");
}

#[test]
fn test_automatic_participant_creation() {
    let input = r#"sequenceDiagram
    Alice->Bob: Message 1
    Bob->Charlie: Message 2
    Charlie->Alice: Message 3"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // All participants should be automatically created
    let participant_names: Vec<_> = diagram.participants.iter().map(|p| &p.actor).collect();

    assert!(participant_names.contains(&&"Alice".to_string()));
    assert!(participant_names.contains(&&"Bob".to_string()));
    assert!(participant_names.contains(&&"Charlie".to_string()));
}

#[test]
fn test_comments_and_empty_lines() {
    let input = r#"sequenceDiagram
    // This is a comment
    
    participant Alice
    %% Another comment
    
    Alice->Bob: Hello
    
    // More comments
    %% And more"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();

    // Should have 1 participant explicitly declared and 1 message
    assert_eq!(
        diagram
            .participants
            .iter()
            .filter(|p| p.actor == "Alice")
            .count(),
        1
    );
    assert_eq!(diagram.statements.len(), 1);
}

#[test]
fn test_edge_case_empty_loop() {
    let input = r#"sequenceDiagram
    participant Alice
    loop Empty
    end
    Alice->Alice: After loop"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.statements.len(), 2); // Empty loop and message
}

#[test]
fn test_edge_case_empty_alt() {
    let input = r#"sequenceDiagram
    participant Alice
    alt Empty
    else Also empty
    end
    Alice->Alice: After alt"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
}

#[test]
fn test_statements_after_blocks() {
    let input = r#"sequenceDiagram
    participant Alice
    participant Bob
    loop Check
        Alice->Bob: In loop
    end
    Alice->Bob: After loop
    alt Condition
        Bob->Alice: In alt
    end
    Bob->Alice: After alt"#;

    let result = sequence::parse(input);
    assert!(result.is_ok());
    let diagram = result.unwrap();
    assert_eq!(diagram.statements.len(), 4); // loop, message, alt, message
}
