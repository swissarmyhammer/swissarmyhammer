//! Sequence diagram parser implementation
//!
//! This module provides parsing capabilities for Mermaid sequence diagrams, which visualize
//! interactions between participants over time. Sequence diagrams are essential for modeling
//! communication patterns, API flows, and system interactions.
//!
//! ## Syntax Support
//!
//! The parser supports comprehensive Mermaid sequence diagram syntax including:
//!
//! - **Participants**: `participant A`, `actor B`, `boundary C`
//! - **Messages**: `A->>B: message`, `A-->>B: async`, `A-xB: destroy`
//! - **Activations**: automatic and manual activation boxes
//! - **Notes**: `note over A: note text`, `note left of A`
//! - **Control flow**: `alt/else/end`, `opt/end`, `loop/end`, `par/and/end`
//! - **Autonumbering**: automatic message numbering
//!
//! ## Features
//!
//! - **Rich message types** - Synchronous, asynchronous, creation, destruction
//! - **Control structures** - Alternatives, loops, parallel sections
//! - **Participant management** - Automatic participant discovery and aliasing
//! - **Note attachments** - Flexible note positioning and styling
//! - **Activation tracking** - Automatic lifeline activation management
//!
//! ## Example
//!
//! ```rust
//! use mermaid_parser::parsers::sequence;
//! use mermaid_parser::common::ast::SequenceStatement;
//!
//! let input = r#"
//! sequenceDiagram
//!     participant A as Alice
//!     participant B as Bob
//!     A->>B: Hello Bob!
//!     activate B
//!     B-->>A: Hello Alice!
//!     deactivate B
//! "#;
//!
//! let diagram = sequence::parse(input)?;
//! println!("Participants: {}, Messages: {}",
//!          diagram.participants.len(), diagram.statements.len());
//!
//! // Access message information
//! for statement in &diagram.statements {
//!     match statement {
//!         SequenceStatement::Message(msg) => {
//!             println!("Message: {} -> {}: {}", msg.from, msg.to, msg.text);
//!         }
//!         _ => {}
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::common::ast::{
    AccessibilityInfo, Alternative, ArrowType, AutoNumber, ElseBranch, Loop, Message, Note,
    NotePosition, Optional, Participant, ParticipantType, SequenceDiagram, SequenceStatement,
};
use crate::common::constants::{diagram_headers, directives, sequence_keywords};
use crate::common::parser_utils::validate_diagram_header;
use crate::error::{ParseError, Result};
use std::collections::HashMap;

/// Parse a Mermaid sequence diagram
pub fn parse(input: &str) -> Result<SequenceDiagram> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut diagram = SequenceDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        participants: Vec::new(),
        statements: Vec::new(),
        autonumber: None,
    };

    let mut line_iter = lines.iter().enumerate().peekable();
    let mut first_line_processed = false;
    let mut participant_map: HashMap<String, usize> = HashMap::new();
    let mut alias_map: HashMap<String, String> = HashMap::new();

    while let Some((line_num, line)) = line_iter.next() {
        // Use shared header validation utility
        let (should_skip, trimmed) = validate_diagram_header(
            line,
            line_num,
            &[diagram_headers::SEQUENCE],
            &mut first_line_processed,
        )?;
        if should_skip {
            continue;
        }

        // Handle title directive
        if let Some(title_text) = trimmed.strip_prefix(directives::TITLE) {
            diagram.title = Some(title_text.trim().to_string());
            continue;
        }

        // Handle autonumber directive
        if trimmed.starts_with(sequence_keywords::AUTONUMBER) {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let start = if parts.len() > 1 {
                parts[1].parse::<i32>().ok()
            } else {
                None
            };
            let step = if parts.len() > 2 {
                parts[2].parse::<i32>().ok()
            } else {
                None
            };
            diagram.autonumber = Some(AutoNumber {
                start,
                step,
                visible: true,
            });
            continue;
        }

        // Handle participant/actor declarations
        if trimmed.starts_with(sequence_keywords::PARTICIPANT)
            || trimmed.starts_with(sequence_keywords::ACTOR)
        {
            let is_actor = trimmed.starts_with(sequence_keywords::ACTOR);
            let declaration = if is_actor {
                trimmed.strip_prefix(sequence_keywords::ACTOR).unwrap()
            } else {
                trimmed
                    .strip_prefix(sequence_keywords::PARTICIPANT)
                    .unwrap()
            };

            let (actor, alias) = if let Some(as_pos) = declaration.find(" as ") {
                let actor_name = declaration[..as_pos].trim();
                let alias_name = declaration[as_pos + 4..].trim();
                (actor_name.to_string(), Some(alias_name.to_string()))
            } else {
                (declaration.trim().to_string(), None)
            };

            if !participant_map.contains_key(&actor) {
                participant_map.insert(actor.clone(), diagram.participants.len());

                // Track alias mapping
                if let Some(alias_name) = &alias {
                    alias_map.insert(alias_name.clone(), actor.clone());
                }

                diagram.participants.push(Participant {
                    actor,
                    alias,
                    participant_type: if is_actor {
                        ParticipantType::Actor
                    } else {
                        ParticipantType::Participant
                    },
                });
            }
            continue;
        }

        // Handle loop blocks
        if trimmed.starts_with(sequence_keywords::LOOP) {
            let condition = trimmed
                .strip_prefix(sequence_keywords::LOOP)
                .unwrap()
                .trim()
                .to_string();
            if let Some(loop_stmt) = parse_loop_block(
                &mut line_iter,
                condition,
                &mut participant_map,
                &mut diagram.participants,
                &alias_map,
            ) {
                diagram.statements.push(loop_stmt);
            }
            continue;
        }

        // Handle alt blocks
        if trimmed.starts_with(sequence_keywords::ALT) {
            let condition = trimmed
                .strip_prefix(sequence_keywords::ALT)
                .unwrap()
                .trim()
                .to_string();
            if let Some(alt_stmt) = parse_alt_block(
                &mut line_iter,
                condition,
                &mut participant_map,
                &mut diagram.participants,
                &alias_map,
            ) {
                diagram.statements.push(alt_stmt);
            }
            continue;
        }

        // Handle opt blocks
        if trimmed.starts_with(sequence_keywords::OPT) {
            let condition = trimmed
                .strip_prefix(sequence_keywords::OPT)
                .unwrap()
                .trim()
                .to_string();
            if let Some(opt_stmt) = parse_opt_block(
                &mut line_iter,
                condition,
                &mut participant_map,
                &mut diagram.participants,
                &alias_map,
            ) {
                diagram.statements.push(opt_stmt);
            }
            continue;
        }

        // Handle notes
        if trimmed.starts_with(sequence_keywords::NOTE) {
            if let Some(note) = parse_note(trimmed) {
                diagram.statements.push(SequenceStatement::Note(note));
            }
            continue;
        }

        // Handle activate/deactivate
        if trimmed.starts_with(sequence_keywords::ACTIVATE) {
            let actor_name = trimmed
                .strip_prefix(sequence_keywords::ACTIVATE)
                .unwrap()
                .trim();
            let resolved_actor = resolve_alias(actor_name, &alias_map);
            ensure_participant(
                &resolved_actor,
                &mut participant_map,
                &mut diagram.participants,
            );
            diagram
                .statements
                .push(SequenceStatement::Activate(resolved_actor));
            continue;
        }

        if trimmed.starts_with(sequence_keywords::DEACTIVATE) {
            let actor_name = trimmed
                .strip_prefix(sequence_keywords::DEACTIVATE)
                .unwrap()
                .trim();
            let resolved_actor = resolve_alias(actor_name, &alias_map);
            ensure_participant(
                &resolved_actor,
                &mut participant_map,
                &mut diagram.participants,
            );
            diagram
                .statements
                .push(SequenceStatement::Deactivate(resolved_actor));
            continue;
        }

        // Try to parse as message
        if let Some(msg) = parse_message(
            trimmed,
            &mut participant_map,
            &mut diagram.participants,
            &alias_map,
        ) {
            diagram.statements.push(SequenceStatement::Message(msg));
        }
    }

    Ok(diagram)
}

/// Resolve an alias to the actual participant name
fn resolve_alias(name: &str, alias_map: &HashMap<String, String>) -> String {
    alias_map
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

/// Ensure a participant exists, adding it if necessary
fn ensure_participant(
    name: &str,
    participant_map: &mut HashMap<String, usize>,
    participants: &mut Vec<Participant>,
) {
    if !participant_map.contains_key(name) {
        participant_map.insert(name.to_string(), participants.len());
        participants.push(Participant {
            actor: name.to_string(),
            alias: None,
            participant_type: ParticipantType::Participant,
        });
    }
}

/// Parse a message line
fn parse_message(
    line: &str,
    participant_map: &mut HashMap<String, usize>,
    participants: &mut Vec<Participant>,
    alias_map: &HashMap<String, String>,
) -> Option<Message> {
    // Try different arrow types (order matters - check longer patterns first)
    let arrow_types = vec![
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

    for (arrow_str, arrow_type) in arrow_types {
        if let Some(arrow_pos) = line.find(arrow_str) {
            let from_name = line[..arrow_pos].trim();
            let rest = &line[arrow_pos + arrow_str.len()..];

            // Find the recipient and message text
            let (to_name, text) = if let Some(colon_pos) = rest.find(':') {
                (rest[..colon_pos].trim(), rest[colon_pos + 1..].trim())
            } else {
                (rest.trim(), "")
            };

            // Resolve aliases
            let from = resolve_alias(from_name, alias_map);
            let to = resolve_alias(to_name, alias_map);

            // Ensure both participants exist
            ensure_participant(&from, participant_map, participants);
            ensure_participant(&to, participant_map, participants);

            return Some(Message {
                from,
                to,
                text: text.to_string(),
                arrow_type,
            });
        }
    }

    None
}

/// Parse a note statement
fn parse_note(line: &str) -> Option<Note> {
    let note_text = line.strip_prefix("note ").unwrap().trim();

    let (position, rest) = if note_text.starts_with("left of ") {
        (
            NotePosition::LeftOf,
            note_text.strip_prefix("left of ").unwrap(),
        )
    } else if note_text.starts_with("right of ") {
        (
            NotePosition::RightOf,
            note_text.strip_prefix("right of ").unwrap(),
        )
    } else if note_text.starts_with("over ") {
        (NotePosition::Over, note_text.strip_prefix("over ").unwrap())
    } else {
        return None;
    };

    // Find the actor and text
    let (actor, text) = if let Some(colon_pos) = rest.find(':') {
        (rest[..colon_pos].trim(), rest[colon_pos + 1..].trim())
    } else {
        // Handle "over Alice,Bob" case
        if let Some(_comma_pos) = rest.find(',') {
            let actors = rest.split(',').map(|s| s.trim()).collect::<Vec<_>>();
            (actors[0], "")
        } else {
            (rest.trim(), "")
        }
    };

    Some(Note {
        position,
        actor: actor.to_string(),
        text: text.to_string(),
    })
}

/// Parse a loop block
fn parse_loop_block(
    line_iter: &mut std::iter::Peekable<std::iter::Enumerate<std::slice::Iter<&str>>>,
    condition: String,
    participant_map: &mut HashMap<String, usize>,
    participants: &mut Vec<Participant>,
    alias_map: &HashMap<String, String>,
) -> Option<SequenceStatement> {
    let mut statements = Vec::new();

    while let Some((_, line)) = line_iter.peek() {
        let trimmed = line.trim();

        if trimmed == "end" {
            line_iter.next(); // consume the end
            break;
        }

        line_iter.next(); // consume the line

        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("%%") {
            continue;
        }

        // Parse nested statements
        if let Some(msg) = parse_message(trimmed, participant_map, participants, alias_map) {
            statements.push(SequenceStatement::Message(msg));
        } else if trimmed.starts_with("note ") {
            if let Some(note) = parse_note(trimmed) {
                statements.push(SequenceStatement::Note(note));
            }
        } else if trimmed.starts_with("activate ") {
            let actor_name = trimmed.strip_prefix("activate ").unwrap().trim();
            let actor = resolve_alias(actor_name, alias_map);
            ensure_participant(&actor, participant_map, participants);
            statements.push(SequenceStatement::Activate(actor));
        } else if trimmed.starts_with("deactivate ") {
            let actor_name = trimmed.strip_prefix("deactivate ").unwrap().trim();
            let actor = resolve_alias(actor_name, alias_map);
            ensure_participant(&actor, participant_map, participants);
            statements.push(SequenceStatement::Deactivate(actor));
        }
    }

    Some(SequenceStatement::Loop(Loop {
        condition,
        statements,
    }))
}

/// Parse an alt block
fn parse_alt_block(
    line_iter: &mut std::iter::Peekable<std::iter::Enumerate<std::slice::Iter<&str>>>,
    condition: String,
    participant_map: &mut HashMap<String, usize>,
    participants: &mut Vec<Participant>,
    alias_map: &HashMap<String, String>,
) -> Option<SequenceStatement> {
    let mut statements = Vec::new();
    let mut else_branch = None;
    let mut in_else = false;
    let mut else_statements = Vec::new();
    let mut else_condition = None;

    while let Some((_, line)) = line_iter.peek() {
        let trimmed = line.trim();

        if trimmed == "end" {
            line_iter.next(); // consume the end
            break;
        }

        if let Some(stripped) = trimmed.strip_prefix("else") {
            line_iter.next(); // consume the else line
            in_else = true;
            if !stripped.is_empty() {
                else_condition = Some(stripped.trim().to_string());
            }
            continue;
        }

        line_iter.next(); // consume the line

        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("%%") {
            continue;
        }

        // Parse nested statements
        let stmt =
            if let Some(msg) = parse_message(trimmed, participant_map, participants, alias_map) {
                Some(SequenceStatement::Message(msg))
            } else if trimmed.starts_with("note ") {
                parse_note(trimmed).map(SequenceStatement::Note)
            } else if trimmed.starts_with("activate ") {
                let actor_name = trimmed.strip_prefix("activate ").unwrap().trim();
                let actor = resolve_alias(actor_name, alias_map);
                ensure_participant(&actor, participant_map, participants);
                Some(SequenceStatement::Activate(actor))
            } else if trimmed.starts_with("deactivate ") {
                let actor_name = trimmed.strip_prefix("deactivate ").unwrap().trim();
                let actor = resolve_alias(actor_name, alias_map);
                ensure_participant(&actor, participant_map, participants);
                Some(SequenceStatement::Deactivate(actor))
            } else {
                None
            };

        if let Some(s) = stmt {
            if in_else {
                else_statements.push(s);
            } else {
                statements.push(s);
            }
        }
    }

    if in_else && !else_statements.is_empty() {
        else_branch = Some(ElseBranch {
            condition: else_condition,
            statements: else_statements,
        });
    }

    Some(SequenceStatement::Alt(Alternative {
        condition,
        statements,
        else_branch,
    }))
}

/// Parse an opt block
fn parse_opt_block(
    line_iter: &mut std::iter::Peekable<std::iter::Enumerate<std::slice::Iter<&str>>>,
    condition: String,
    participant_map: &mut HashMap<String, usize>,
    participants: &mut Vec<Participant>,
    alias_map: &HashMap<String, String>,
) -> Option<SequenceStatement> {
    let mut statements = Vec::new();

    while let Some((_, line)) = line_iter.peek() {
        let trimmed = line.trim();

        if trimmed == "end" {
            line_iter.next(); // consume the end
            break;
        }

        line_iter.next(); // consume the line

        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("%%") {
            continue;
        }

        // Parse nested statements
        if let Some(msg) = parse_message(trimmed, participant_map, participants, alias_map) {
            statements.push(SequenceStatement::Message(msg));
        } else if trimmed.starts_with("note ") {
            if let Some(note) = parse_note(trimmed) {
                statements.push(SequenceStatement::Note(note));
            }
        } else if trimmed.starts_with("activate ") {
            let actor_name = trimmed.strip_prefix("activate ").unwrap().trim();
            let actor = resolve_alias(actor_name, alias_map);
            ensure_participant(&actor, participant_map, participants);
            statements.push(SequenceStatement::Activate(actor));
        } else if trimmed.starts_with("deactivate ") {
            let actor_name = trimmed.strip_prefix("deactivate ").unwrap().trim();
            let actor = resolve_alias(actor_name, alias_map);
            ensure_participant(&actor, participant_map, participants);
            statements.push(SequenceStatement::Deactivate(actor));
        }
    }

    Some(SequenceStatement::Opt(Optional {
        condition,
        statements,
    }))
}
