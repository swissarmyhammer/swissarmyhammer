//! Parser for Mermaid state diagrams
//!
//! State diagrams represent state machines with states, transitions, events, and hierarchical state structures.
//! High complexity grammar (336 lines) with nested states, concurrent regions, and various state types.

use crate::common::ast::{
    AccessibilityInfo, State, StateDiagram, StateNote, StateNotePosition, StateTransition,
    StateType, StateVersion,
};
use crate::common::constants::{diagram_headers, directives, state_keywords};
use crate::common::parser_utils::validate_diagram_header;
use crate::error::{ParseError, Result};
use std::collections::HashMap;

/// Parse a Mermaid state diagram
pub fn parse(input: &str) -> Result<StateDiagram> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut diagram = StateDiagram {
        title: None,
        accessibility: AccessibilityInfo::default(),
        version: StateVersion::V1,
        states: HashMap::new(),
        transitions: Vec::new(),
        notes: Vec::new(),
    };

    let mut line_iter = lines.iter().enumerate().peekable();
    let mut first_line_processed = false;
    let mut state_stack: Vec<String> = Vec::new(); // For tracking nested states
    let mut _brace_count = 0;

    while let Some((line_num, line)) = line_iter.next() {
        // Use shared header validation utility
        let (should_skip, trimmed) = validate_diagram_header(
            line,
            line_num,
            diagram_headers::STATE_HEADERS,
            &mut first_line_processed,
        )?;
        if should_skip {
            // Determine version based on which header was matched
            if trimmed.starts_with(diagram_headers::STATE_V2) {
                diagram.version = StateVersion::V2;
            } else if trimmed.starts_with(diagram_headers::STATE_V1) {
                diagram.version = StateVersion::V1;
            }
            continue;
        }

        // Handle title directive
        if let Some(title_text) = trimmed.strip_prefix(directives::TITLE) {
            diagram.title = Some(title_text.trim().to_string());
            continue;
        }

        // Handle direction directive (ignore for now)
        if trimmed.starts_with(state_keywords::DIRECTION) {
            continue;
        }

        // Handle state declarations
        if trimmed.starts_with(state_keywords::STATE) {
            if let Some(state) = parse_state_declaration(trimmed, &mut diagram.states) {
                // Check if this is a composite state (ends with {)
                if trimmed.ends_with(" {") {
                    if let Some(state_mut) = diagram.states.get_mut(&state.id) {
                        state_mut.state_type = StateType::Composite;
                    }
                    state_stack.push(state.id.clone());
                    _brace_count += 1;
                } else if let Some(parent) = state_stack.last() {
                    // Add this state as a substate of the parent
                    if let Some(parent_state) = diagram.states.get_mut(parent) {
                        parent_state.substates.push(state.id.clone());
                    }
                }
            }
            continue;
        }

        // Handle opening brace for composite state
        if trimmed == "{" {
            _brace_count += 1;
            continue;
        }

        // Handle closing brace
        if trimmed == "}" {
            _brace_count -= 1;
            if !state_stack.is_empty() {
                state_stack.pop();
            }
            continue;
        }

        // Handle state with opening brace on same line
        if !trimmed.starts_with("note") && trimmed.ends_with(" {") {
            let state_name = trimmed.trim_end_matches(" {").trim();
            ensure_state_exists(&mut diagram.states, state_name);
            if let Some(state) = diagram.states.get_mut(state_name) {
                state.state_type = StateType::Composite;
            }
            state_stack.push(state_name.to_string());
            _brace_count += 1;
            continue;
        }

        // Handle state followed by opening brace on next line
        if !state_stack.is_empty()
            && trimmed != "{"
            && trimmed != "}"
            && !trimmed.contains("-->")
            && !trimmed.starts_with("note ")
        {
            // This could be a state inside the composite state
            if let Some(next_line) = line_iter.peek() {
                if next_line.1.trim() == "{" {
                    // This is a composite state declaration
                    ensure_state_exists(&mut diagram.states, trimmed);
                    if let Some(state) = diagram.states.get_mut(trimmed) {
                        state.state_type = StateType::Composite;
                    }
                    state_stack.push(trimmed.to_string());
                    continue;
                }
            }
        }

        // Handle concurrent separator
        if trimmed == "--" {
            // TODO: Handle concurrent regions
            continue;
        }

        // Handle notes
        if trimmed.starts_with("note ") {
            if let Some(note) = parse_note(trimmed) {
                diagram.notes.push(note);
            }
            continue;
        }

        // Try to parse as transition
        if let Some(transition) = parse_transition(trimmed, &mut diagram.states) {
            // If we're inside a composite state, add the states as substates
            if let Some(parent) = state_stack.last().cloned() {
                let from_state = transition.from.clone();
                let to_state = transition.to.clone();

                if !from_state.starts_with("[") {
                    if let Some(parent_state) = diagram.states.get_mut(&parent) {
                        if !parent_state.substates.contains(&from_state) {
                            parent_state.substates.push(from_state);
                        }
                    }
                }
                if !to_state.starts_with("[") {
                    if let Some(parent_state) = diagram.states.get_mut(&parent) {
                        if !parent_state.substates.contains(&to_state) {
                            parent_state.substates.push(to_state);
                        }
                    }
                }
            }
            diagram.transitions.push(transition);
            continue;
        }

        // If we're inside a composite state, the line might be a simple state name
        if !state_stack.is_empty() && !trimmed.contains("-->") {
            ensure_state_exists(&mut diagram.states, trimmed);
            if let Some(parent) = state_stack.last() {
                if let Some(parent_state) = diagram.states.get_mut(parent) {
                    parent_state.substates.push(trimmed.to_string());
                }
            }
        }
    }

    // Add start and end states if they were used but not explicitly declared
    if !diagram.states.contains_key("[*]") {
        // Check if [*] is used in any transitions
        let used_as_start = diagram.transitions.iter().any(|t| t.from == "[*]");
        let used_as_end = diagram.transitions.iter().any(|t| t.to == "[*]");

        if used_as_start || used_as_end {
            diagram.states.insert(
                "[*]".to_string(),
                State {
                    id: "[*]".to_string(),
                    display_name: None,
                    state_type: if used_as_start && !used_as_end {
                        StateType::Start
                    } else if used_as_end && !used_as_start {
                        StateType::End
                    } else {
                        StateType::Simple // Can be both start and end
                    },
                    substates: Vec::new(),
                    concurrent_regions: Vec::new(),
                },
            );
        }
    }

    Ok(diagram)
}

/// Parse a state declaration line
fn parse_state_declaration(line: &str, states: &mut HashMap<String, State>) -> Option<State> {
    let state_text = line
        .strip_prefix("state ")
        .unwrap()
        .trim()
        .trim_end_matches(" {");

    // Handle state with display name: state "Display Name" as StateId
    if let Some(stripped) = state_text.strip_prefix('"') {
        if let Some(end_quote) = stripped.find('"') {
            let display_name = stripped[..end_quote].to_string();
            if let Some(as_part) = stripped[end_quote + 1..].strip_prefix(" as ") {
                let id = as_part.trim().to_string();
                let state = State {
                    id: id.clone(),
                    display_name: Some(display_name),
                    state_type: StateType::Simple,
                    substates: Vec::new(),
                    concurrent_regions: Vec::new(),
                };
                states.insert(id, state.clone());
                return Some(state);
            }
        }
    }

    // Handle state with stereotype: state StateId <<choice>>
    let (state_id, state_type) = if let Some(stereotype_start) = state_text.find("<<") {
        let id = state_text[..stereotype_start].trim().to_string();
        let stereotype = state_text[stereotype_start + 2..]
            .trim_end_matches(">>")
            .trim();
        let stype = match stereotype {
            state_keywords::CHOICE => StateType::Choice,
            state_keywords::FORK => StateType::Fork,
            state_keywords::JOIN => StateType::Join,
            state_keywords::END => StateType::End,
            _ => StateType::Simple,
        };
        (id, stype)
    } else {
        (state_text.to_string(), StateType::Simple)
    };

    let state = State {
        id: state_id.clone(),
        display_name: None,
        state_type,
        substates: Vec::new(),
        concurrent_regions: Vec::new(),
    };
    states.insert(state_id, state.clone());
    Some(state)
}

/// Parse a transition line
fn parse_transition(line: &str, states: &mut HashMap<String, State>) -> Option<StateTransition> {
    // Find the arrow
    if !line.contains("-->") {
        return None;
    }

    let parts: Vec<&str> = line.split("-->").collect();
    if parts.len() != 2 {
        return None;
    }

    let from = parts[0].trim().to_string();
    let to_and_label = parts[1].trim();

    // Parse the target and optional label
    let (to, label) = if let Some(colon_pos) = to_and_label.find(':') {
        (
            to_and_label[..colon_pos].trim().to_string(),
            Some(to_and_label[colon_pos + 1..].trim().to_string()),
        )
    } else {
        (to_and_label.to_string(), None)
    };

    // Ensure states exist
    ensure_state_exists(states, &from);
    ensure_state_exists(states, &to);

    // Parse label into event, guard, and action
    let (event, guard, action) = if let Some(label_text) = label {
        parse_transition_label(&label_text)
    } else {
        (None, None, None)
    };

    Some(StateTransition {
        from,
        to,
        event,
        guard,
        action,
    })
}

/// Parse a transition label into event, guard, and action
fn parse_transition_label(label: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut event = None;
    let mut guard = None;
    let mut action = None;

    // Check for guard condition [condition]
    let main_label = if let Some(bracket_start) = label.find('[') {
        if let Some(bracket_end) = label.find(']') {
            guard = Some(label[bracket_start + 1..bracket_end].trim().to_string());
            let before = label[..bracket_start].trim();
            let after = label[bracket_end + 1..].trim();
            format!("{} {}", before, after).trim().to_string()
        } else {
            label.to_string()
        }
    } else {
        label.to_string()
    };

    // Check for event/action pattern: event / action
    if let Some(slash_pos) = main_label.find('/') {
        event = Some(main_label[..slash_pos].trim().to_string());
        action = Some(main_label[slash_pos + 1..].trim().to_string());
    } else if !main_label.is_empty() {
        event = Some(main_label);
    }

    (event, guard, action)
}

/// Parse a note statement
fn parse_note(line: &str) -> Option<StateNote> {
    let note_text = line.strip_prefix("note ").unwrap().trim();

    // Parse position
    let (position, rest) = if note_text.starts_with("left of ") {
        (
            StateNotePosition::LeftOf,
            note_text.strip_prefix("left of ").unwrap(),
        )
    } else if note_text.starts_with("right of ") {
        (
            StateNotePosition::RightOf,
            note_text.strip_prefix("right of ").unwrap(),
        )
    } else if note_text.starts_with("above ") {
        (
            StateNotePosition::Above,
            note_text.strip_prefix("above ").unwrap(),
        )
    } else if note_text.starts_with("below ") {
        (
            StateNotePosition::Below,
            note_text.strip_prefix("below ").unwrap(),
        )
    } else {
        return None;
    };

    // Find the target and text
    let (target, text) = if let Some(colon_pos) = rest.find(':') {
        (
            rest[..colon_pos].trim().to_string(),
            rest[colon_pos + 1..].trim().to_string(),
        )
    } else {
        // Handle case where there's no colon
        (rest.trim().to_string(), String::new())
    };

    Some(StateNote {
        position,
        target,
        text,
    })
}

/// Ensure a state exists in the diagram, creating it if necessary
fn ensure_state_exists(states: &mut HashMap<String, State>, state_id: &str) {
    if !states.contains_key(state_id) {
        let state_type = StateType::Simple;

        states.insert(
            state_id.to_string(),
            State {
                id: state_id.to_string(),
                display_name: None,
                state_type,
                substates: Vec::new(),
                concurrent_regions: Vec::new(),
            },
        );
    }
}
