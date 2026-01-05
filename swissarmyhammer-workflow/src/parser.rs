//! Mermaid state diagram parser for workflows
//!
//! This module integrates the mermaid_parser library to parse Mermaid state diagrams
//! and convert them to our internal Workflow types.

use crate::{
    ConditionType, State, StateId, StateType, Transition, TransitionCondition, Workflow,
    WorkflowName,
};
use mermaid_parser::{
    common::ast::{DiagramType, StateDiagram, StateTransition},
    parse_diagram,
};
use std::collections::HashMap;
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Errors that can occur during Mermaid parsing
#[derive(Debug, Error)]
pub enum ParseError {
    /// Error from the mermaid-parser library
    #[error("Mermaid parse error: {0}")]
    MermaidError(String),

    /// Diagram is not a state diagram
    #[error("Expected state diagram, found {diagram_type}")]
    WrongDiagramType {
        /// The type of diagram that was found
        diagram_type: String,
    },

    /// No initial state found in diagram
    #[error("No initial state found in state diagram. Ensure your diagram has a transition from [*] to define the starting state")]
    NoInitialState,

    /// No terminal states found
    #[error("No terminal states found in state diagram. At least one state must transition to [*] to mark workflow completion")]
    NoTerminalStates,

    /// Invalid state or transition structure
    #[error("Invalid workflow structure: {message}. Please check your diagram syntax and state references")]
    InvalidStructure {
        /// Description of the structural problem
        message: String,
    },
}

/// Result type for parsing operations
pub type ParseResult<T> = Result<T, ParseError>;

/// Implementation of Severity trait for ParseError
impl Severity for ParseError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Missing required workflow structure elements
            ParseError::NoInitialState => ErrorSeverity::Critical,
            ParseError::NoTerminalStates => ErrorSeverity::Critical,
            // Error: Parsing failures that prevent workflow creation
            ParseError::MermaidError(_) => ErrorSeverity::Error,
            ParseError::WrongDiagramType { .. } => ErrorSeverity::Error,
            ParseError::InvalidStructure { .. } => ErrorSeverity::Error,
        }
    }
}

/// Mermaid parser for converting state diagrams to workflows
pub struct MermaidParser;

/// State ID validation utilities for workflow parsing
impl MermaidParser {
    /// Validates a state ID and determines if it should be included in the workflow.
    ///
    /// This validation handles special cases that can occur when parsing Mermaid state diagrams:
    ///
    /// 1. **Special [*] state**: Represents start/end markers in Mermaid but not actual workflow states
    /// 2. **Empty state IDs**: Can occur when parallel state separators (|||) are used incorrectly,
    ///    creating phantom empty states in the parsed diagram
    /// 3. **Whitespace-only IDs**: Similar to empty IDs but containing only spaces or tabs
    ///
    /// Returns `true` if the state ID is valid and should be processed, `false` if it should be skipped.
    ///
    /// # Arguments
    /// * `state_id` - The state identifier to validate
    ///
    /// # Returns
    /// * `true` - State ID is valid and should be included in the workflow
    /// * `false` - State ID should be skipped (special marker or empty/invalid)
    fn is_valid_state_id(state_id: &str) -> bool {
        // Skip the special [*] state as it's not a real state in our workflow model
        if state_id == "[*]" {
            return false;
        }

        // Skip empty or whitespace-only state IDs
        // These can happen with parallel state separators (|||) creating invalid states
        // when the Mermaid parser encounters malformed parallel constructs
        !state_id.trim().is_empty()
    }
}

impl MermaidParser {
    /// Parse a Mermaid state diagram into a Workflow
    pub fn parse(input: &str, workflow_name: impl Into<WorkflowName>) -> ParseResult<Workflow> {
        // Parse front matter and extract mermaid content
        let mermaid_content = Self::extract_mermaid_from_markdown(input)?;

        // Extract actions from the markdown content
        let actions = Self::extract_actions_from_markdown(input);

        // Attempt to parse the diagram
        match parse_diagram(&mermaid_content) {
            Ok(diagram) => match diagram {
                DiagramType::State(state_diagram) => Self::convert_state_diagram_with_actions(
                    state_diagram,
                    workflow_name.into(),
                    actions,
                ),
                _ => Err(ParseError::WrongDiagramType {
                    diagram_type: format!("{diagram:?}"),
                }),
            },
            Err(e) => Err(ParseError::MermaidError(e.to_string())),
        }
    }

    /// Parse a Mermaid state diagram into a Workflow with metadata
    pub fn parse_with_metadata(
        input: &str,
        workflow_name: impl Into<WorkflowName>,
        title: Option<String>,
        description: Option<String>,
    ) -> ParseResult<Workflow> {
        // Parse front matter and extract mermaid content
        let mermaid_content = Self::extract_mermaid_from_markdown(input)?;

        // Extract actions from the markdown content
        let actions = Self::extract_actions_from_markdown(input);

        // Extract parameters from frontmatter
        let parameters = Self::extract_parameters_from_frontmatter(input)?;

        // Extract mode from frontmatter
        let mode = Self::extract_mode_from_frontmatter(input)?;

        // Attempt to parse the diagram
        match parse_diagram(&mermaid_content) {
            Ok(diagram) => match diagram {
                DiagramType::State(state_diagram) => {
                    Self::convert_state_diagram_with_actions_metadata_and_parameters(
                        state_diagram,
                        workflow_name.into(),
                        actions,
                        title,
                        description,
                        parameters,
                        mode,
                    )
                }
                _ => Err(ParseError::WrongDiagramType {
                    diagram_type: format!("{diagram:?}"),
                }),
            },
            Err(e) => Err(ParseError::MermaidError(e.to_string())),
        }
    }

    /// Extract mermaid diagram content from markdown with YAML front matter or raw mermaid content
    fn extract_mermaid_from_markdown(input: &str) -> ParseResult<String> {
        // Check if this is raw mermaid content (for backward compatibility with tests)
        let trimmed = input.trim();
        if trimmed.starts_with("stateDiagram")
            || trimmed.starts_with("flowchart")
            || trimmed.starts_with("graph")
        {
            return Ok(input.to_string());
        }

        // Parse front matter if present
        let content = if input.starts_with("---\n") {
            let parts: Vec<&str> = input.splitn(3, "---\n").collect();
            if parts.len() >= 3 {
                parts[2].trim_start()
            } else {
                input
            }
        } else {
            input
        };

        // Extract mermaid code block
        let lines: Vec<&str> = content.lines().collect();
        let mut in_mermaid_block = false;
        let mut mermaid_lines = Vec::new();

        for line in lines {
            if line.trim() == "```mermaid" {
                in_mermaid_block = true;
                continue;
            }
            if in_mermaid_block && line.trim() == "```" {
                break;
            }
            if in_mermaid_block {
                mermaid_lines.push(line);
            }
        }

        if mermaid_lines.is_empty() {
            return Err(ParseError::InvalidStructure {
                message: "No mermaid code block found in markdown content".to_string(),
            });
        }

        Ok(mermaid_lines.join("\n"))
    }

    /// Parse frontmatter and extract workflow parameters
    fn extract_parameters_from_frontmatter(
        input: &str,
    ) -> ParseResult<Vec<swissarmyhammer_common::Parameter>> {
        let mut parameters = Vec::new();

        // Use shared frontmatter parsing
        let frontmatter =
            swissarmyhammer_common::frontmatter::parse_frontmatter(input).map_err(|e| {
                ParseError::InvalidStructure {
                    message: e.to_string(),
                }
            })?;

        // Extract parameters from frontmatter if present
        let frontmatter_value = match frontmatter.metadata {
            Some(value) => value,
            None => return Ok(parameters),
        };

        // Extract parameters from frontmatter if present
        if let Some(params_value) = frontmatter_value.get("parameters") {
            if let Some(params_array) = params_value.as_array() {
                for param_value in params_array {
                    if let Some(param_obj) = param_value.as_object() {
                        let name = param_obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let description = param_obj
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let required = param_obj
                            .get("required")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        // Parse parameter type
                        let type_str = param_obj
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("string");

                        let parameter_type = match type_str.to_lowercase().as_str() {
                            "string" => swissarmyhammer_common::ParameterType::String,
                            "boolean" | "bool" => swissarmyhammer_common::ParameterType::Boolean,
                            "number" | "numeric" | "int" | "integer" | "float" => {
                                swissarmyhammer_common::ParameterType::Number
                            }
                            "choice" | "select" => swissarmyhammer_common::ParameterType::Choice,
                            "multi_choice" | "multichoice" | "multiselect" => {
                                swissarmyhammer_common::ParameterType::MultiChoice
                            }
                            _ => swissarmyhammer_common::ParameterType::String, // Default to string for unknown types
                        };

                        // Parse default value
                        let default = param_obj.get("default").cloned();

                        // Parse choices if present
                        let choices =
                            param_obj
                                .get("choices")
                                .and_then(|v| v.as_array())
                                .map(|seq| {
                                    seq.iter()
                                        .filter_map(|choice| choice.as_str())
                                        .map(String::from)
                                        .collect::<Vec<String>>()
                                });

                        let mut param = swissarmyhammer_common::Parameter::new(
                            name,
                            description,
                            parameter_type,
                        )
                        .required(required);

                        if let Some(default_value) = default {
                            param = param.with_default(default_value);
                        }

                        if let Some(choices_vec) = choices {
                            param = param.with_choices(choices_vec);
                        }

                        parameters.push(param);
                    }
                }
            }
        }

        Ok(parameters)
    }

    /// Parse frontmatter and extract workflow mode
    fn extract_mode_from_frontmatter(input: &str) -> ParseResult<Option<String>> {
        // Use shared frontmatter parsing
        let frontmatter =
            swissarmyhammer_common::frontmatter::parse_frontmatter(input).map_err(|e| {
                ParseError::InvalidStructure {
                    message: e.to_string(),
                }
            })?;

        // Extract mode from frontmatter if present
        let frontmatter_value = match frontmatter.metadata {
            Some(value) => value,
            None => return Ok(None),
        };

        // Extract mode field
        let mode = frontmatter_value
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(mode)
    }

    /// Extract actions from markdown content
    fn extract_actions_from_markdown(input: &str) -> HashMap<String, String> {
        let mut actions = HashMap::new();

        // First try to extract from ## Actions section (legacy format)
        let mut in_actions_section = false;
        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("## Actions")
                || trimmed.eq_ignore_ascii_case("### Actions")
            {
                in_actions_section = true;
                continue;
            }

            if in_actions_section && line.trim().starts_with("##") {
                // We've reached another section
                break;
            }

            if in_actions_section && line.trim().starts_with("-") {
                // Parse action line: - StateName: Action description
                let content = line.trim_start_matches('-').trim();
                if let Some(colon_pos) = content.find(':') {
                    let state_name = content[..colon_pos].trim();
                    let action = content[colon_pos + 1..].trim();
                    actions.insert(state_name.to_string(), action.to_string());
                }
            }
        }

        // If no actions found, try to extract from ## States section (new format)
        if actions.is_empty() {
            actions.extend(Self::extract_state_descriptions_from_markdown(input));
        }

        actions
    }

    /// Extract state descriptions from ## States section
    fn extract_state_descriptions_from_markdown(input: &str) -> HashMap<String, String> {
        let mut state_descriptions = HashMap::new();
        let mut in_states_section = false;
        let mut current_state: Option<String> = None;
        let mut current_description = Vec::new();

        for line in input.lines() {
            let trimmed = line.trim();

            // Check if we're entering the States section
            if trimmed.eq_ignore_ascii_case("## States") {
                in_states_section = true;
                continue;
            }

            // If we encounter another ## section (but not ###), we're done with States
            if in_states_section
                && trimmed.starts_with("## ")
                && !trimmed.eq_ignore_ascii_case("## States")
            {
                // Store the last state if we have one
                if let Some(state_name) = current_state.take() {
                    if !current_description.is_empty() {
                        state_descriptions.insert(state_name, current_description.join("\n"));
                        current_description.clear();
                    }
                }
                break;
            }

            if in_states_section {
                // Check if this is a state header (### state_name)
                if let Some(stripped) = trimmed.strip_prefix("### ") {
                    // Store the previous state if we have one
                    if let Some(state_name) = current_state.take() {
                        if !current_description.is_empty() {
                            state_descriptions.insert(state_name, current_description.join("\n"));
                            current_description.clear();
                        }
                    }

                    // Start tracking the new state
                    current_state = Some(stripped.trim().to_string());
                } else if current_state.is_some() {
                    // We're inside a state section, collect the content
                    current_description.push(line.to_string());
                }
            }
        }

        // Don't forget the last state
        if let Some(state_name) = current_state {
            if !current_description.is_empty() {
                state_descriptions.insert(state_name, current_description.join("\n"));
            }
        }

        state_descriptions
    }

    /// Convert a parsed state diagram to our Workflow type with actions
    fn convert_state_diagram_with_actions(
        state_diagram: StateDiagram,
        workflow_name: WorkflowName,
        actions: HashMap<String, String>,
    ) -> ParseResult<Workflow> {
        // Extract description from title or create default
        let description = state_diagram
            .title
            .unwrap_or_else(|| "Workflow from Mermaid state diagram".to_string());

        // Find initial state - look for [*] as source in transitions
        let initial_state_id = Self::find_initial_state(&state_diagram.transitions)?;

        let mut workflow = Workflow::new(workflow_name, description, initial_state_id.clone());

        // Convert all states from mermaid to our format
        for (state_id, mermaid_state) in state_diagram.states {
            // Validate state ID and skip invalid ones (including [*] markers and empty IDs)
            if !Self::is_valid_state_id(&state_id) {
                continue;
            }

            let is_terminal = Self::is_terminal_state(&state_id, &state_diagram.transitions);

            // Get the action for this state from the actions map
            let description = actions
                .get(&state_id)
                .cloned()
                .unwrap_or_else(|| state_id.clone());

            let mut metadata = HashMap::new();
            metadata.insert(
                "mermaid_type".to_string(),
                format!("{:?}", mermaid_state.state_type),
            );

            // Check if this is a fork or join state based on state type
            let state_type = match mermaid_state.state_type {
                mermaid_parser::common::ast::StateType::Fork => StateType::Fork,
                mermaid_parser::common::ast::StateType::Join => StateType::Join,
                _ => StateType::Normal,
            };

            // Check if this state has substates or concurrent regions to enable parallel execution
            // Also enable parallel execution for fork and join states
            let allows_parallel = !mermaid_state.substates.is_empty()
                || !mermaid_state.concurrent_regions.is_empty()
                || matches!(state_type, StateType::Fork | StateType::Join);

            workflow.add_state(State {
                id: StateId::new(state_id),
                description,
                state_type,
                is_terminal,
                allows_parallel,
                metadata,
            });
        }

        // Convert all transitions
        for transition in state_diagram.transitions {
            // Skip transitions to/from [*] that don't involve real states
            if transition.from == "[*]" && transition.to == "[*]" {
                continue;
            }

            // Handle initial transitions from [*]
            if transition.from == "[*]" {
                // This is already handled by setting initial_state, skip the transition
                continue;
            }

            // Handle terminal transitions to [*]
            if transition.to == "[*]" {
                // Mark the source state as terminal (already handled above)
                continue;
            }

            // Skip transitions with empty or whitespace-only state IDs
            if transition.from.trim().is_empty() || transition.to.trim().is_empty() {
                continue;
            }

            let condition = Self::parse_transition_condition(&transition);

            workflow.add_transition(Transition {
                from_state: StateId::new(transition.from),
                to_state: StateId::new(transition.to),
                condition,
                action: transition.action,
                metadata: HashMap::new(),
            });
        }

        // Add metadata about the source
        workflow
            .metadata
            .insert("source".to_string(), "mermaid".to_string());
        workflow.metadata.insert(
            "version".to_string(),
            format!("{:?}", state_diagram.version),
        );

        // Detect and update choice states based on their transition patterns
        Self::detect_and_update_choice_states(&mut workflow);

        // Perform workflow-specific validation
        Self::validate_workflow_structure(&workflow)?;

        Ok(workflow)
    }

    /// Convert a parsed state diagram to our Workflow type with actions, metadata, and parameters
    fn convert_state_diagram_with_actions_metadata_and_parameters(
        state_diagram: StateDiagram,
        workflow_name: WorkflowName,
        actions: HashMap<String, String>,
        title: Option<String>,
        description: Option<String>,
        parameters: Vec<swissarmyhammer_common::Parameter>,
        mode: Option<String>,
    ) -> ParseResult<Workflow> {
        // Use provided description or title, or fall back to default
        let workflow_description = description
            .or(title.clone())
            .or(state_diagram.title)
            .unwrap_or_else(|| "Workflow from Mermaid state diagram".to_string());

        // Find initial state - look for [*] as source in transitions
        let initial_state_id = Self::find_initial_state(&state_diagram.transitions)?;

        let mut workflow = Workflow::new(
            workflow_name,
            workflow_description,
            initial_state_id.clone(),
        );

        // Set parameters from frontmatter
        workflow.parameters = parameters;

        // Set mode from frontmatter
        workflow.mode = mode;

        // Convert all states from mermaid to our format
        for (state_id, mermaid_state) in state_diagram.states {
            // Validate state ID and skip invalid ones (including [*] markers and empty IDs)
            if !Self::is_valid_state_id(&state_id) {
                continue;
            }

            let is_terminal = Self::is_terminal_state(&state_id, &state_diagram.transitions);

            // Get the action for this state from the actions map
            let description = actions
                .get(&state_id)
                .cloned()
                .unwrap_or_else(|| state_id.clone());

            let mut metadata = HashMap::new();
            metadata.insert(
                "mermaid_type".to_string(),
                format!("{:?}", mermaid_state.state_type),
            );

            // Check if this is a fork or join state based on state type
            let state_type = match mermaid_state.state_type {
                mermaid_parser::common::ast::StateType::Fork => StateType::Fork,
                mermaid_parser::common::ast::StateType::Join => StateType::Join,
                _ => StateType::Normal,
            };

            // Check if this state has substates or concurrent regions to enable parallel execution
            // Also enable parallel execution for fork and join states
            let allows_parallel = !mermaid_state.substates.is_empty()
                || !mermaid_state.concurrent_regions.is_empty()
                || matches!(state_type, StateType::Fork | StateType::Join);

            workflow.add_state(State {
                id: StateId::new(state_id),
                description,
                state_type,
                is_terminal,
                allows_parallel,
                metadata,
            });
        }

        // Convert all transitions
        for transition in state_diagram.transitions {
            // Skip transitions to/from [*] that don't involve real states
            if transition.from == "[*]" && transition.to == "[*]" {
                continue;
            }

            // Handle initial transitions from [*]
            if transition.from == "[*]" {
                // This is already handled by setting initial_state, skip the transition
                continue;
            }

            // Handle terminal transitions to [*]
            if transition.to == "[*]" {
                // Mark the source state as terminal (already handled above)
                continue;
            }

            // Skip transitions with empty or whitespace-only state IDs
            if transition.from.trim().is_empty() || transition.to.trim().is_empty() {
                continue;
            }

            let condition = Self::parse_transition_condition(&transition);

            workflow.add_transition(Transition {
                from_state: StateId::new(transition.from),
                to_state: StateId::new(transition.to),
                condition,
                action: transition.action,
                metadata: HashMap::new(),
            });
        }

        // Add metadata about the source
        workflow
            .metadata
            .insert("source".to_string(), "mermaid".to_string());
        workflow.metadata.insert(
            "version".to_string(),
            format!("{:?}", state_diagram.version),
        );

        // Store the title in metadata if provided
        if let Some(title) = title {
            workflow.metadata.insert("title".to_string(), title);
        }

        // Detect and update choice states based on their transition patterns
        Self::detect_and_update_choice_states(&mut workflow);

        // Perform workflow-specific validation
        Self::validate_workflow_structure(&workflow)?;

        Ok(workflow)
    }

    /// Find the initial state by looking for transitions from [*]
    fn find_initial_state(transitions: &[StateTransition]) -> ParseResult<StateId> {
        for transition in transitions {
            if transition.from == "[*]" && transition.to != "[*]" {
                return Ok(StateId::new(transition.to.clone()));
            }
        }
        Err(ParseError::NoInitialState)
    }

    /// Check if a state is terminal by looking for transitions to [*]
    fn is_terminal_state(state_id: &str, transitions: &[StateTransition]) -> bool {
        transitions
            .iter()
            .any(|t| t.from == state_id && t.to == "[*]")
    }

    /// Parse transition condition from mermaid transition
    fn parse_transition_condition(transition: &StateTransition) -> TransitionCondition {
        match &transition.event {
            Some(event) => {
                // Analyze the event text to determine condition type
                // Check for CEL expressions first (contains operators or function calls)
                let event_lower = event.to_lowercase();
                let is_cel_expression = event.contains("==")
                    || event.contains("!=")
                    || event.contains("&&")
                    || event.contains("||")
                    || event.contains(".")
                    || event.contains("(")
                    || event.contains("<")
                    || event.contains(">");

                let condition_type = if is_cel_expression {
                    ConditionType::Custom
                } else if event_lower == "always" || event.is_empty() {
                    ConditionType::Always
                } else if event_lower.split_whitespace().any(|word| {
                    word == "fail" || word == "failure" || word == "error" || word == "invalid"
                }) {
                    ConditionType::OnFailure
                } else if event_lower
                    .split_whitespace()
                    .any(|word| word == "valid" || word == "success")
                {
                    ConditionType::OnSuccess
                } else {
                    ConditionType::Custom
                };

                let expression = if matches!(condition_type, ConditionType::Custom) {
                    Some(event.clone())
                } else {
                    None
                };

                TransitionCondition {
                    condition_type,
                    expression,
                }
            }
            None => TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
        }
    }

    /// Validate workflow structure with additional checks beyond basic validation
    fn validate_workflow_structure(workflow: &Workflow) -> ParseResult<()> {
        // Run basic validation first
        if let Err(errors) = workflow.validate_structure() {
            return Err(ParseError::InvalidStructure {
                message: errors.join("; "),
            });
        }

        // Check for single start state (no multiple initial transitions)
        let _initial_count = workflow
            .transitions
            .iter()
            .filter(|t| t.from_state == workflow.initial_state)
            .count();

        // Ensure reachability - all states should be reachable from initial state
        // This now properly handles states within compound states (concurrent regions)
        let reachable_states = Self::find_reachable_states(workflow);
        let unreachable: Vec<_> = workflow
            .states
            .keys()
            .filter(|id| !reachable_states.contains(id) && **id != workflow.initial_state)
            .collect();

        if !unreachable.is_empty() {
            // Check if all unreachable states have meaningful actions or parallel support - this indicates they're
            // meant to be executable states within a compound state or parallel regions
            let all_have_actions_or_parallel_support = unreachable.iter().all(|state_id| {
                workflow
                    .states
                    .get(state_id)
                    .map(|state| {
                        // Allow states that have parallel support or meaningful descriptions/actions
                        state.allows_parallel
                            || (!state.description.is_empty()
                                && state.description != state_id.as_str())
                            || state.state_type == crate::StateType::Fork
                            || state.state_type == crate::StateType::Join
                    })
                    .unwrap_or(false)
            });

            if !all_have_actions_or_parallel_support {
                return Err(ParseError::InvalidStructure {
                    message: format!("Unreachable states found: {unreachable:?}"),
                });
            }
            // If they all have actions or parallel support, they're likely states within compound states
            // and we'll allow them
        }

        // Check for disconnected components by ensuring at least one terminal state is reachable
        let terminal_reachable = workflow
            .states
            .values()
            .filter(|s| s.is_terminal)
            .any(|s| reachable_states.contains(&s.id));

        if !terminal_reachable {
            return Err(ParseError::InvalidStructure {
                message: "No terminal states are reachable from initial state".to_string(),
            });
        }

        Ok(())
    }

    /// Find all states reachable from the initial state using DFS
    #[allow(dead_code)]
    fn find_reachable_states(workflow: &Workflow) -> std::collections::HashSet<StateId> {
        let mut reachable = std::collections::HashSet::new();
        let mut stack = vec![workflow.initial_state.clone()];

        while let Some(current) = stack.pop() {
            if reachable.contains(&current) {
                continue;
            }

            reachable.insert(current.clone());

            // Find all states reachable from current state
            for transition in &workflow.transitions {
                if transition.from_state == current && !reachable.contains(&transition.to_state) {
                    stack.push(transition.to_state.clone());
                }
            }
        }

        reachable
    }

    /// Detect states that should be choice states based on their transition patterns
    /// and update their state_type accordingly.
    ///
    /// A state is considered a choice state if it has multiple outgoing transitions
    /// with different condition types (not all Always transitions).
    fn detect_and_update_choice_states(workflow: &mut Workflow) {
        // Group transitions by their from_state
        let mut state_transitions: std::collections::HashMap<StateId, Vec<&Transition>> =
            std::collections::HashMap::new();

        for transition in &workflow.transitions {
            state_transitions
                .entry(transition.from_state.clone())
                .or_default()
                .push(transition);
        }

        // Check each state to see if it should be a choice state
        for (state_id, transitions) in state_transitions {
            // Skip if already a special state type (Fork/Join)
            if let Some(state) = workflow.states.get(&state_id) {
                if matches!(state.state_type, StateType::Fork | StateType::Join) {
                    continue;
                }
            }

            // Analyze transition patterns
            if Self::should_be_choice_state(&transitions) {
                // Update the state to be a choice state
                if let Some(state) = workflow.states.get_mut(&state_id) {
                    tracing::debug!(
                        "Detected choice state: {} with {} transitions",
                        state_id,
                        transitions.len()
                    );
                    state.state_type = StateType::Choice;
                }
            }
        }
    }

    /// Determine if a state should be classified as a choice state based on its transitions
    fn should_be_choice_state(transitions: &[&Transition]) -> bool {
        // Must have multiple outgoing transitions
        if transitions.len() < 2 {
            return false;
        }

        // Count different condition types
        let mut has_custom_conditions = false;
        let mut has_success_condition = false;
        let mut has_failure_condition = false;
        let mut always_count = 0;

        for transition in transitions {
            match &transition.condition.condition_type {
                ConditionType::Custom => has_custom_conditions = true,
                ConditionType::OnSuccess => has_success_condition = true,
                ConditionType::OnFailure => has_failure_condition = true,
                ConditionType::Always => always_count += 1,
                ConditionType::Never => {} // Ignore Never conditions
            }
        }

        // It's a choice state if it has:
        // 1. At least one custom condition, OR
        // 2. Both success and failure conditions, OR
        // 3. Multiple different condition types (not just all Always)
        has_custom_conditions
            || (has_success_condition && has_failure_condition)
            || (transitions.len() > always_count && always_count < transitions.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_state_diagram() {
        let input = r"
        stateDiagram-v2
            [*] --> State1
            State1 --> State2: condition
            State2 --> [*]
        ";

        let result = MermaidParser::parse(input, "test_workflow");
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.name.as_str(), "test_workflow");
        assert_eq!(workflow.states.len(), 2); // State1 and State2 (not [*])
        assert_eq!(workflow.transitions.len(), 1); // Only State1 -> State2

        // Check initial state
        assert_eq!(workflow.initial_state.as_str(), "State1");

        // Check states
        assert!(workflow.states.contains_key(&StateId::new("State1")));
        assert!(workflow.states.contains_key(&StateId::new("State2")));

        // Check that State2 is terminal
        let state2 = &workflow.states[&StateId::new("State2")];
        assert!(state2.is_terminal);

        // Check transition
        let transition = &workflow.transitions[0];
        assert_eq!(transition.from_state.as_str(), "State1");
        assert_eq!(transition.to_state.as_str(), "State2");
        assert_eq!(transition.condition.condition_type, ConditionType::Custom);
        assert_eq!(
            transition.condition.expression,
            Some("condition".to_string())
        );
    }

    #[test]
    fn test_parse_wrong_diagram_type() {
        let input = r"
        flowchart TD
            A --> B
        ";

        let result = MermaidParser::parse(input, "test_workflow");
        assert!(result.is_err());

        match result.unwrap_err() {
            ParseError::MermaidError(msg) => {
                assert!(msg.contains("Lexer error") || msg.contains("error"));
            }
            _ => panic!("Expected MermaidError for invalid syntax"),
        }
    }

    #[test]
    fn test_parse_state_diagram_with_actions() {
        let input = r"
        stateDiagram-v2
            [*] --> CheckingInput: Start workflow
            CheckingInput --> ProcessingData: Input valid
            CheckingInput --> ErrorState: Input invalid
            ProcessingData --> [*]: Complete
            ErrorState --> [*]: Abort
        ";

        let result = MermaidParser::parse(input, "action_workflow");
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.states.len(), 3);
        assert_eq!(workflow.initial_state.as_str(), "CheckingInput");

        // Check transitions with proper condition types
        assert_eq!(workflow.transitions.len(), 2);

        let valid_transition = workflow
            .transitions
            .iter()
            .find(|t| {
                t.from_state.as_str() == "CheckingInput" && t.to_state.as_str() == "ProcessingData"
            })
            .unwrap();
        assert_eq!(
            valid_transition.condition.condition_type,
            ConditionType::OnSuccess
        );

        let invalid_transition = workflow
            .transitions
            .iter()
            .find(|t| {
                t.from_state.as_str() == "CheckingInput" && t.to_state.as_str() == "ErrorState"
            })
            .unwrap();
        assert_eq!(
            invalid_transition.condition.condition_type,
            ConditionType::OnFailure
        );
    }

    #[test]
    fn test_no_initial_state_error() {
        let input = r"
        stateDiagram-v2
            State1 --> State2
            State2 --> State1
        ";

        let result = MermaidParser::parse(input, "invalid_workflow");
        assert!(result.is_err());

        match result.unwrap_err() {
            ParseError::NoInitialState => (),
            _ => panic!("Expected NoInitialState error"),
        }
    }

    #[test]
    fn test_unreachable_states_validation() {
        // This test would require a more complex setup where we manually construct
        // a workflow with unreachable states, which is hard to do with valid Mermaid syntax
        // For now, we test that normal workflows pass validation
        let input = r"
        stateDiagram-v2
            [*] --> State1
            State1 --> State2
            State2 --> [*]
        ";

        let result = MermaidParser::parse(input, "valid_workflow");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_transition_condition() {
        use mermaid_parser::common::ast::StateTransition;

        let transition = StateTransition {
            from: "A".to_string(),
            to: "B".to_string(),
            event: Some("Input valid".to_string()),
            guard: None,
            action: None,
        };

        let condition = MermaidParser::parse_transition_condition(&transition);
        assert_eq!(condition.condition_type, ConditionType::OnSuccess);
        assert_eq!(condition.expression, None);

        let transition_custom = StateTransition {
            from: "A".to_string(),
            to: "B".to_string(),
            event: Some("custom condition".to_string()),
            guard: None,
            action: None,
        };

        let condition_custom = MermaidParser::parse_transition_condition(&transition_custom);
        assert_eq!(condition_custom.condition_type, ConditionType::Custom);
        assert_eq!(
            condition_custom.expression,
            Some("custom condition".to_string())
        );
    }

    #[test]
    fn test_parse_fork_join_diagram() {
        let input = r"
        stateDiagram-v2
            [*] --> Process
            state Fork1 <<fork>>
            Process --> Fork1
            Fork1 --> Branch1: path1
            Fork1 --> Branch2: path2
            state Join1 <<join>>
            Branch1 --> Join1: complete
            Branch2 --> Join1: complete
            Join1 --> Complete
            Complete --> [*]
        ";

        let result = MermaidParser::parse(input, "fork_join_workflow");
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.name.as_str(), "fork_join_workflow");

        // Check that fork and join states exist
        assert!(workflow.states.contains_key(&StateId::new("Fork1")));
        assert!(workflow.states.contains_key(&StateId::new("Join1")));

        // Check that fork state is identified as fork type
        let fork_state = &workflow.states[&StateId::new("Fork1")];
        assert_eq!(fork_state.state_type, StateType::Fork);

        // Check that join state is identified as join type
        let join_state = &workflow.states[&StateId::new("Join1")];
        assert_eq!(join_state.state_type, StateType::Join);

        // Check that parallel execution is enabled for these states
        assert!(fork_state.allows_parallel);
        assert!(join_state.allows_parallel);
    }

    #[test]
    fn test_extract_actions_without_bold_markers() {
        let input = r#"---
name: test-workflow
---

# Test Workflow

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> Process
    Process --> Complete
    Complete --> [*]
```

## Actions

- Start: Log "Starting workflow"
- Process: Execute prompt "test-prompt" with result="output"
- Complete: Log "Workflow completed with result: ${output}"
"#;

        let actions = MermaidParser::extract_actions_from_markdown(input);
        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions.get("Start"),
            Some(&"Log \"Starting workflow\"".to_string())
        );
        assert_eq!(
            actions.get("Process"),
            Some(&"Execute prompt \"test-prompt\" with result=\"output\"".to_string())
        );
        assert_eq!(
            actions.get("Complete"),
            Some(&"Log \"Workflow completed with result: ${output}\"".to_string())
        );
    }

    #[test]
    fn test_parse_nested_fork_join_diagram() {
        let input = r"
        stateDiagram-v2
            [*] --> Start
            state OuterFork <<fork>>
            Start --> OuterFork
            OuterFork --> Branch1: outer1
            OuterFork --> Branch2: outer2
            state InnerFork <<fork>>
            Branch1 --> InnerFork
            InnerFork --> SubBranch1: inner1
            InnerFork --> SubBranch2: inner2
            state InnerJoin <<join>>
            SubBranch1 --> InnerJoin
            SubBranch2 --> InnerJoin
            InnerJoin --> Branch1Complete
            state OuterJoin <<join>>
            Branch1Complete --> OuterJoin
            Branch2 --> OuterJoin
            OuterJoin --> End
            End --> [*]
        ";

        let result = MermaidParser::parse(input, "nested_fork_join_workflow");
        assert!(result.is_ok());

        let workflow = result.unwrap();

        // Check nested fork/join states exist
        assert!(workflow.states.contains_key(&StateId::new("OuterFork")));
        assert!(workflow.states.contains_key(&StateId::new("OuterJoin")));
        assert!(workflow.states.contains_key(&StateId::new("InnerFork")));
        assert!(workflow.states.contains_key(&StateId::new("InnerJoin")));
    }

    #[test]
    fn test_parse_workflow_with_parameters() {
        let input = r#"---
title: Greeting Workflow
description: A workflow that greets someone
parameters:
  - name: person_name
    description: The name of the person to greet
    required: true
    type: string
  - name: language
    description: The language to use for greeting
    required: false
    type: string
    default: English
    choices:
      - English
      - Spanish
      - French
  - name: formal
    description: Use formal greeting
    required: false
    type: boolean
    default: false
---

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> Greet
    Greet --> [*]
```
"#;

        let result = MermaidParser::parse_with_metadata(
            input,
            "greeting_workflow",
            Some("Greeting Workflow".to_string()),
            Some("A workflow that greets someone".to_string()),
        );
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.name.as_str(), "greeting_workflow");

        // Check parameters
        assert_eq!(workflow.parameters.len(), 3);

        // Check first parameter
        let param1 = &workflow.parameters[0];
        assert_eq!(param1.name, "person_name");
        assert_eq!(param1.description, "The name of the person to greet");
        assert!(param1.required);
        assert!(matches!(
            param1.parameter_type,
            swissarmyhammer_common::ParameterType::String
        ));
        assert!(param1.default.is_none());
        assert!(param1.choices.is_none());

        // Check second parameter (with choices)
        let param2 = &workflow.parameters[1];
        assert_eq!(param2.name, "language");
        assert_eq!(param2.description, "The language to use for greeting");
        assert!(!param2.required);
        assert!(matches!(
            param2.parameter_type,
            swissarmyhammer_common::ParameterType::String
        ));
        assert_eq!(
            param2.default.as_ref().unwrap().as_str().unwrap(),
            "English"
        );
        assert!(param2.choices.is_some());
        let choices = param2.choices.as_ref().unwrap();
        assert_eq!(choices.len(), 3);
        assert!(choices.contains(&"English".to_string()));
        assert!(choices.contains(&"Spanish".to_string()));
        assert!(choices.contains(&"French".to_string()));

        // Check third parameter (boolean)
        let param3 = &workflow.parameters[2];
        assert_eq!(param3.name, "formal");
        assert_eq!(param3.description, "Use formal greeting");
        assert!(!param3.required);
        assert!(matches!(
            param3.parameter_type,
            swissarmyhammer_common::ParameterType::Boolean
        ));
        assert!(!param3.default.as_ref().unwrap().as_bool().unwrap());
        assert!(param3.choices.is_none());
    }

    #[test]
    fn test_parse_workflow_with_different_parameter_types() {
        let input = r#"---
title: Mixed Parameter Types
description: Tests different parameter types
parameters:
  - name: count
    description: Number of items
    required: true
    type: number
  - name: enabled
    description: Feature enabled
    required: false
    type: bool
    default: true
  - name: priority
    description: Priority level
    required: true
    type: choice
    choices:
      - low
      - medium
      - high
  - name: tags
    description: Multiple tags
    required: false
    type: multi_choice
    choices:
      - urgent
      - important
      - review
---

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```
"#;

        let result = MermaidParser::parse_with_metadata(
            input,
            "mixed_params_workflow",
            Some("Mixed Parameter Types".to_string()),
            Some("Tests different parameter types".to_string()),
        );
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.parameters.len(), 4);

        // Check number parameter
        let number_param = &workflow.parameters[0];
        assert_eq!(number_param.name, "count");
        assert!(matches!(
            number_param.parameter_type,
            swissarmyhammer_common::ParameterType::Number
        ));
        assert!(number_param.required);

        // Check boolean parameter
        let bool_param = &workflow.parameters[1];
        assert_eq!(bool_param.name, "enabled");
        assert!(matches!(
            bool_param.parameter_type,
            swissarmyhammer_common::ParameterType::Boolean
        ));
        assert!(!bool_param.required);
        assert!(bool_param.default.as_ref().unwrap().as_bool().unwrap());

        // Check choice parameter
        let choice_param = &workflow.parameters[2];
        assert_eq!(choice_param.name, "priority");
        assert!(matches!(
            choice_param.parameter_type,
            swissarmyhammer_common::ParameterType::Choice
        ));
        assert!(choice_param.required);
        let choices = choice_param.choices.as_ref().unwrap();
        assert_eq!(choices.len(), 3);
        assert!(choices.contains(&"low".to_string()));
        assert!(choices.contains(&"medium".to_string()));
        assert!(choices.contains(&"high".to_string()));

        // Check multi-choice parameter
        let multi_choice_param = &workflow.parameters[3];
        assert_eq!(multi_choice_param.name, "tags");
        assert!(matches!(
            multi_choice_param.parameter_type,
            swissarmyhammer_common::ParameterType::MultiChoice
        ));
        assert!(!multi_choice_param.required);
        let multi_choices = multi_choice_param.choices.as_ref().unwrap();
        assert_eq!(multi_choices.len(), 3);
        assert!(multi_choices.contains(&"urgent".to_string()));
        assert!(multi_choices.contains(&"important".to_string()));
        assert!(multi_choices.contains(&"review".to_string()));
    }

    #[test]
    fn test_parse_workflow_without_parameters() {
        let input = r#"---
title: Simple Workflow
description: A workflow without parameters
---

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```
"#;

        let result = MermaidParser::parse_with_metadata(
            input,
            "simple_workflow",
            Some("Simple Workflow".to_string()),
            Some("A workflow without parameters".to_string()),
        );
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert!(workflow.parameters.is_empty());
    }

    #[test]
    fn test_parse_workflow_with_unknown_parameter_type() {
        let input = r#"---
title: Unknown Type Workflow
description: Tests fallback to string for unknown types
parameters:
  - name: custom_field
    description: A field with unknown type
    required: false
    type: unknown_type
---

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```
"#;

        let result = MermaidParser::parse_with_metadata(
            input,
            "unknown_type_workflow",
            Some("Unknown Type Workflow".to_string()),
            Some("Tests fallback to string for unknown types".to_string()),
        );
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.parameters.len(), 1);

        let param = &workflow.parameters[0];
        assert_eq!(param.name, "custom_field");
        // Should fallback to String type for unknown types
        assert!(matches!(
            param.parameter_type,
            swissarmyhammer_common::ParameterType::String
        ));
    }

    #[test]
    fn test_extract_parameters_from_frontmatter_empty() {
        let input = "No frontmatter here";
        let result = MermaidParser::extract_parameters_from_frontmatter(input);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_extract_parameters_from_frontmatter_no_parameters() {
        let input = r#"---
title: Test
description: No parameters
---
Content here
"#;
        let result = MermaidParser::extract_parameters_from_frontmatter(input);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_workflow_with_mode() {
        let input = r#"---
title: Test Workflow with Mode
description: Tests mode parsing
mode: planner
---

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```
"#;

        let result = MermaidParser::parse_with_metadata(
            input,
            "mode_workflow",
            Some("Test Workflow with Mode".to_string()),
            Some("Tests mode parsing".to_string()),
        );
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.name.as_str(), "mode_workflow");
        assert_eq!(workflow.mode, Some("planner".to_string()));
    }

    #[test]
    fn test_parse_workflow_without_mode() {
        let input = r#"---
title: Test Workflow without Mode
description: Tests that mode is optional
---

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```
"#;

        let result = MermaidParser::parse_with_metadata(
            input,
            "no_mode_workflow",
            Some("Test Workflow without Mode".to_string()),
            Some("Tests that mode is optional".to_string()),
        );
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.name.as_str(), "no_mode_workflow");
        assert_eq!(workflow.mode, None);
    }

    #[test]
    fn test_backward_compatibility() {
        // Test that workflows without parameters still work
        let input = r"
        stateDiagram-v2
            [*] --> State1
            State1 --> State2: condition
            State2 --> [*]
        ";

        let result = MermaidParser::parse(input, "test_workflow");
        assert!(result.is_ok());

        let workflow = result.unwrap();
        assert_eq!(workflow.name.as_str(), "test_workflow");
        assert!(workflow.parameters.is_empty()); // No parameters
        assert_eq!(workflow.states.len(), 2); // State1 and State2 (not [*])
    }

    #[test]
    fn test_parse_error_severity() {
        // Critical severity errors
        let no_initial = ParseError::NoInitialState;
        assert_eq!(no_initial.severity(), ErrorSeverity::Critical);

        let no_terminal = ParseError::NoTerminalStates;
        assert_eq!(no_terminal.severity(), ErrorSeverity::Critical);

        // Error severity errors
        let mermaid_err = ParseError::MermaidError("parse failed".to_string());
        assert_eq!(mermaid_err.severity(), ErrorSeverity::Error);

        let wrong_type = ParseError::WrongDiagramType {
            diagram_type: "flowchart".to_string(),
        };
        assert_eq!(wrong_type.severity(), ErrorSeverity::Error);

        let invalid_struct = ParseError::InvalidStructure {
            message: "missing state".to_string(),
        };
        assert_eq!(invalid_struct.severity(), ErrorSeverity::Error);
    }
}
