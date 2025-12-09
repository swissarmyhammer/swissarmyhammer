//! Comprehensive diagram validation framework
//!
//! This module provides semantic validation for Mermaid diagrams beyond just syntax parsing.
//! It validates that diagrams are logically correct and well-formed according to their specific rules.
//!
//! # Example Usage
//!
//! ```rust
//! use mermaid_parser::common::validation::{DiagramValidator, FlowchartValidator};
//! use mermaid_parser::common::ast::DiagramType;
//! use mermaid_parser::parse_diagram;
//!
//! let input = "flowchart TD\n    A --> B\n    B --> C";
//! let diagram = parse_diagram(input).unwrap();
//!
//! if let DiagramType::Flowchart(flowchart) = diagram {
//!     let validator = FlowchartValidator::new();
//!     match validator.validate(&flowchart) {
//!         Ok(()) => println!("Diagram is valid"),
//!         Err(errors) => {
//!             for error in errors {
//!                 println!("Validation error: {}", error.message);
//!             }
//!         }
//!     }
//! }
//! ```

use crate::common::ast::*;
use std::collections::{HashMap, HashSet};

/// Location information for validation errors
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
    pub element_id: Option<String>,
}

impl Location {
    pub fn new(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            element_id: None,
        }
    }

    pub fn with_element(line: usize, column: usize, element_id: String) -> Self {
        Self {
            line,
            column,
            element_id: Some(element_id),
        }
    }
}

/// Severity level for validation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,    // Consider fixing
    Warning, // Should fix
    Error,   // Must fix
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "INFO"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Error => write!(f, "ERROR"),
        }
    }
}

/// Validation error with detailed information
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub rule: &'static str,
    pub message: String,
    pub severity: Severity,
    pub location: Option<Location>,
}

impl ValidationError {
    pub fn new(rule: &'static str, message: String, severity: Severity) -> Self {
        Self {
            rule,
            message,
            severity,
            location: None,
        }
    }

    pub fn with_location(
        rule: &'static str,
        message: String,
        severity: Severity,
        location: Location,
    ) -> Self {
        Self {
            rule,
            message,
            severity,
            location: Some(location),
        }
    }

    pub fn error(rule: &'static str, message: String) -> Self {
        Self::new(rule, message, Severity::Error)
    }

    pub fn warning(rule: &'static str, message: String) -> Self {
        Self::new(rule, message, Severity::Warning)
    }

    pub fn info(rule: &'static str, message: String) -> Self {
        Self::new(rule, message, Severity::Info)
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.rule, self.message)?;
        if let Some(location) = &self.location {
            if let Some(element_id) = &location.element_id {
                write!(
                    f,
                    " (at {}:{}, element: {})",
                    location.line, location.column, element_id
                )?;
            } else {
                write!(f, " (at {}:{})", location.line, location.column)?;
            }
        }
        Ok(())
    }
}

/// Core trait for diagram validation
pub trait DiagramValidator {
    type Diagram;
    type Error;

    fn validate(&self, diagram: &Self::Diagram) -> Result<(), Vec<Self::Error>>;
}

/// Configuration for validation behavior
#[derive(Debug)]
pub struct ValidationConfig {
    pub min_severity: Severity,
    pub ignore_rules: HashSet<&'static str>,
    pub custom_rules: Vec<Box<dyn CustomValidationRule>>,
}

impl Clone for ValidationConfig {
    fn clone(&self) -> Self {
        Self {
            min_severity: self.min_severity,
            ignore_rules: self.ignore_rules.clone(),
            custom_rules: Vec::new(), // Custom rules can't be cloned
        }
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            min_severity: Severity::Warning,
            ignore_rules: HashSet::new(),
            custom_rules: Vec::new(),
        }
    }
}

/// Trait for custom validation rules
pub trait CustomValidationRule: std::fmt::Debug {
    fn validate_flowchart(&self, _diagram: &FlowchartDiagram) -> Vec<ValidationError> {
        Vec::new()
    }

    fn validate_sequence(&self, _diagram: &SequenceDiagram) -> Vec<ValidationError> {
        Vec::new()
    }

    fn validate_class(&self, _diagram: &ClassDiagram) -> Vec<ValidationError> {
        Vec::new()
    }

    fn validate_state(&self, _diagram: &StateDiagram) -> Vec<ValidationError> {
        Vec::new()
    }
}

/// Comprehensive validator for all diagram types
#[derive(Debug)]
pub struct UniversalValidator {
    config: ValidationConfig,
}

impl UniversalValidator {
    pub fn new() -> Self {
        Self {
            config: ValidationConfig::default(),
        }
    }

    pub fn with_config(config: ValidationConfig) -> Self {
        Self { config }
    }

    pub fn validate_any(&self, diagram: &DiagramType) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        match diagram {
            DiagramType::Flowchart(d) => {
                let validator = FlowchartValidator::with_config(self.config.clone());
                if let Err(flowchart_errors) = validator.validate(d) {
                    errors.extend(flowchart_errors);
                }
            }
            DiagramType::Sequence(d) => {
                let validator = SequenceValidator::with_config(self.config.clone());
                if let Err(sequence_errors) = validator.validate(d) {
                    errors.extend(sequence_errors);
                }
            }
            DiagramType::Class(d) => {
                let validator = ClassValidator::with_config(self.config.clone());
                if let Err(class_errors) = validator.validate(d) {
                    errors.extend(class_errors);
                }
            }
            DiagramType::State(d) => {
                let validator = StateValidator::with_config(self.config.clone());
                if let Err(state_errors) = validator.validate(d) {
                    errors.extend(state_errors);
                }
            }
            _ => {
                // Other diagram types can be added here as needed
            }
        }

        // Apply custom rules
        for rule in &self.config.custom_rules {
            match diagram {
                DiagramType::Flowchart(d) => errors.extend(rule.validate_flowchart(d)),
                DiagramType::Sequence(d) => errors.extend(rule.validate_sequence(d)),
                DiagramType::Class(d) => errors.extend(rule.validate_class(d)),
                DiagramType::State(d) => errors.extend(rule.validate_state(d)),
                _ => {}
            }
        }

        // Filter by severity and ignored rules
        errors.retain(|error| {
            error.severity >= self.config.min_severity
                && !self.config.ignore_rules.contains(error.rule)
        });

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for UniversalValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Flowchart-specific validator
#[derive(Debug)]
pub struct FlowchartValidator {
    config: ValidationConfig,
}

impl FlowchartValidator {
    pub fn new() -> Self {
        Self {
            config: ValidationConfig::default(),
        }
    }

    pub fn with_config(config: ValidationConfig) -> Self {
        Self { config }
    }

    fn validate_node_connectivity(&self, diagram: &FlowchartDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut connected_nodes = HashSet::new();

        // Collect all nodes that have connections
        for edge in &diagram.edges {
            connected_nodes.insert(&edge.from);
            connected_nodes.insert(&edge.to);
        }

        // Check for isolated nodes (warning)
        for node_id in diagram.nodes.keys() {
            if !connected_nodes.contains(node_id) {
                errors.push(ValidationError::warning(
                    "isolated_node",
                    format!("Node '{}' has no connections", node_id),
                ));
            }
        }

        errors
    }

    fn validate_edge_references(&self, diagram: &FlowchartDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for edge in &diagram.edges {
            if !diagram.nodes.contains_key(&edge.from) {
                errors.push(ValidationError::error(
                    "undefined_node_reference",
                    format!("Edge references undefined node '{}'", edge.from),
                ));
            }
            if !diagram.nodes.contains_key(&edge.to) {
                errors.push(ValidationError::error(
                    "undefined_node_reference",
                    format!("Edge references undefined node '{}'", edge.to),
                ));
            }
        }

        errors
    }

    fn validate_subgraphs(&self, diagram: &FlowchartDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut subgraph_names = HashSet::new();

        for subgraph in &diagram.subgraphs {
            if let Some(name) = &subgraph.title {
                if subgraph_names.contains(name) {
                    errors.push(ValidationError::error(
                        "duplicate_subgraph_name",
                        format!("Subgraph name '{}' is not unique", name),
                    ));
                } else {
                    subgraph_names.insert(name.clone());
                }
            }
        }

        errors
    }

    fn validate_style_classes(&self, diagram: &FlowchartDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let defined_classes: HashSet<_> = diagram.class_defs.keys().collect();

        // Check if referenced style classes are defined
        for node in diagram.nodes.values() {
            for class_name in &node.classes {
                if !defined_classes.contains(class_name) {
                    errors.push(ValidationError::warning(
                        "undefined_style_class",
                        format!("Node references undefined style class '{}'", class_name),
                    ));
                }
            }
        }

        errors
    }
}

impl DiagramValidator for FlowchartValidator {
    type Diagram = FlowchartDiagram;
    type Error = ValidationError;

    fn validate(&self, diagram: &Self::Diagram) -> Result<(), Vec<Self::Error>> {
        let mut errors = Vec::new();

        errors.extend(self.validate_node_connectivity(diagram));
        errors.extend(self.validate_edge_references(diagram));
        errors.extend(self.validate_subgraphs(diagram));
        errors.extend(self.validate_style_classes(diagram));

        // Filter by severity and ignored rules
        errors.retain(|error| {
            error.severity >= self.config.min_severity
                && !self.config.ignore_rules.contains(error.rule)
        });

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for FlowchartValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Sequence diagram validator
#[derive(Debug)]
pub struct SequenceValidator {
    config: ValidationConfig,
}

impl SequenceValidator {
    pub fn new() -> Self {
        Self {
            config: ValidationConfig::default(),
        }
    }

    pub fn with_config(config: ValidationConfig) -> Self {
        Self { config }
    }

    fn validate_participant_references(&self, diagram: &SequenceDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let participant_names: HashSet<_> = diagram.participants.iter().map(|p| &p.actor).collect();

        for statement in &diagram.statements {
            check_statement_participants(statement, &participant_names, &mut errors);
        }

        errors
    }

    fn validate_activation_blocks(&self, diagram: &SequenceDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut activation_stack = Vec::new();

        for statement in &diagram.statements {
            check_activation_balance(statement, &mut activation_stack, &mut errors);
        }

        // Check if any activations are left unbalanced
        if !activation_stack.is_empty() {
            errors.push(ValidationError::error(
                "unbalanced_activation",
                format!(
                    "Found {} unbalanced activation blocks",
                    activation_stack.len()
                ),
            ));
        }

        errors
    }
}

fn check_statement_participants(
    statement: &SequenceStatement,
    participants: &HashSet<&String>,
    errors: &mut Vec<ValidationError>,
) {
    match statement {
        SequenceStatement::Message(msg) => {
            if !participants.contains(&msg.from) {
                errors.push(ValidationError::error(
                    "undefined_participant",
                    format!("Message references undefined participant '{}'", msg.from),
                ));
            }
            if !participants.contains(&msg.to) {
                errors.push(ValidationError::error(
                    "undefined_participant",
                    format!("Message references undefined participant '{}'", msg.to),
                ));
            }
        }
        SequenceStatement::Loop(loop_stmt) => {
            for stmt in &loop_stmt.statements {
                check_statement_participants(stmt, participants, errors);
            }
        }
        SequenceStatement::Alt(alt) => {
            for stmt in &alt.statements {
                check_statement_participants(stmt, participants, errors);
            }
            if let Some(else_branch) = &alt.else_branch {
                for stmt in &else_branch.statements {
                    check_statement_participants(stmt, participants, errors);
                }
            }
        }
        SequenceStatement::Opt(opt) => {
            for stmt in &opt.statements {
                check_statement_participants(stmt, participants, errors);
            }
        }
        SequenceStatement::Par(par) => {
            for branch in &par.branches {
                for stmt in &branch.statements {
                    check_statement_participants(stmt, participants, errors);
                }
            }
        }
        SequenceStatement::Critical(critical) => {
            for stmt in &critical.statements {
                check_statement_participants(stmt, participants, errors);
            }
            for option in &critical.options {
                for stmt in &option.statements {
                    check_statement_participants(stmt, participants, errors);
                }
            }
        }
        _ => {} // Other statement types
    }
}

fn check_activation_balance(
    statement: &SequenceStatement,
    activation_stack: &mut Vec<String>,
    errors: &mut Vec<ValidationError>,
) {
    match statement {
        SequenceStatement::Activate(participant) => {
            activation_stack.push(participant.clone());
        }
        SequenceStatement::Deactivate(participant) => {
            if let Some(last_activated) = activation_stack.pop() {
                if last_activated != *participant {
                    errors.push(ValidationError::warning(
                        "mismatched_activation",
                        format!(
                            "Deactivate '{}' does not match last activate '{}'",
                            participant, last_activated
                        ),
                    ));
                }
            } else {
                errors.push(ValidationError::error(
                    "unmatched_deactivate",
                    format!("Deactivate '{}' without matching activate", participant),
                ));
            }
        }
        // Recursively check nested statements
        SequenceStatement::Loop(loop_stmt) => {
            for stmt in &loop_stmt.statements {
                check_activation_balance(stmt, activation_stack, errors);
            }
        }
        SequenceStatement::Alt(alt) => {
            for stmt in &alt.statements {
                check_activation_balance(stmt, activation_stack, errors);
            }
            if let Some(else_branch) = &alt.else_branch {
                for stmt in &else_branch.statements {
                    check_activation_balance(stmt, activation_stack, errors);
                }
            }
        }
        _ => {} // Other statement types
    }
}

impl DiagramValidator for SequenceValidator {
    type Diagram = SequenceDiagram;
    type Error = ValidationError;

    fn validate(&self, diagram: &Self::Diagram) -> Result<(), Vec<Self::Error>> {
        let mut errors = Vec::new();

        errors.extend(self.validate_participant_references(diagram));
        errors.extend(self.validate_activation_blocks(diagram));

        // Filter by severity and ignored rules
        errors.retain(|error| {
            error.severity >= self.config.min_severity
                && !self.config.ignore_rules.contains(error.rule)
        });

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for SequenceValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Class diagram validator
#[derive(Debug)]
pub struct ClassValidator {
    config: ValidationConfig,
}

impl ClassValidator {
    pub fn new() -> Self {
        Self {
            config: ValidationConfig::default(),
        }
    }

    pub fn with_config(config: ValidationConfig) -> Self {
        Self { config }
    }

    fn validate_inheritance_cycles(&self, diagram: &ClassDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut inheritance_graph = HashMap::new();

        // Build inheritance graph
        for relationship in &diagram.relationships {
            if matches!(
                relationship.relationship_type,
                ClassRelationshipType::Inheritance
            ) {
                inheritance_graph
                    .entry(relationship.from.clone())
                    .or_insert_with(Vec::new)
                    .push(relationship.to.clone());
            }
        }

        // Check for cycles using DFS
        for class_name in diagram.classes.keys() {
            if has_inheritance_cycle(&inheritance_graph, class_name, &mut HashSet::new()) {
                errors.push(ValidationError::error(
                    "circular_inheritance",
                    format!(
                        "Circular inheritance detected involving class '{}'",
                        class_name
                    ),
                ));
            }
        }

        errors
    }

    fn validate_relationship_references(&self, diagram: &ClassDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for relationship in &diagram.relationships {
            if !diagram.classes.contains_key(&relationship.from) {
                errors.push(ValidationError::error(
                    "undefined_class_reference",
                    format!(
                        "Relationship references undefined class '{}'",
                        relationship.from
                    ),
                ));
            }
            if !diagram.classes.contains_key(&relationship.to) {
                errors.push(ValidationError::error(
                    "undefined_class_reference",
                    format!(
                        "Relationship references undefined class '{}'",
                        relationship.to
                    ),
                ));
            }
        }

        errors
    }

    fn validate_duplicate_members(&self, diagram: &ClassDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for (class_name, class) in &diagram.classes {
            let mut member_names = HashSet::new();

            for member in &class.members {
                let (member_name, member_type_str) = match member {
                    ClassMember::Property(prop) => (&prop.name, "property"),
                    ClassMember::Method(method) => (&method.name, "method"),
                };

                let member_key = format!("{}:{}", member_name, member_type_str);
                if member_names.contains(&member_key) {
                    errors.push(ValidationError::error(
                        "duplicate_member",
                        format!(
                            "Class '{}' has duplicate {} '{}'",
                            class_name, member_type_str, member_name
                        ),
                    ));
                } else {
                    member_names.insert(member_key);
                }
            }
        }

        errors
    }
}

fn has_inheritance_cycle(
    graph: &HashMap<String, Vec<String>>,
    class: &str,
    visited: &mut HashSet<String>,
) -> bool {
    if visited.contains(class) {
        return true;
    }

    visited.insert(class.to_string());

    if let Some(parents) = graph.get(class) {
        for parent in parents {
            if has_inheritance_cycle(graph, parent, visited) {
                return true;
            }
        }
    }

    visited.remove(class);
    false
}

impl DiagramValidator for ClassValidator {
    type Diagram = ClassDiagram;
    type Error = ValidationError;

    fn validate(&self, diagram: &Self::Diagram) -> Result<(), Vec<Self::Error>> {
        let mut errors = Vec::new();

        errors.extend(self.validate_inheritance_cycles(diagram));
        errors.extend(self.validate_relationship_references(diagram));
        errors.extend(self.validate_duplicate_members(diagram));

        // Filter by severity and ignored rules
        errors.retain(|error| {
            error.severity >= self.config.min_severity
                && !self.config.ignore_rules.contains(error.rule)
        });

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for ClassValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// State diagram validator
#[derive(Debug)]
pub struct StateValidator {
    config: ValidationConfig,
}

impl StateValidator {
    pub fn new() -> Self {
        Self {
            config: ValidationConfig::default(),
        }
    }

    pub fn with_config(config: ValidationConfig) -> Self {
        Self { config }
    }

    fn validate_start_state(&self, diagram: &StateDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let has_start_state = diagram
            .states
            .values()
            .any(|state| matches!(state.state_type, StateType::Start));

        if !has_start_state {
            errors.push(ValidationError::warning(
                "missing_start_state",
                "State diagram should have a start state".to_string(),
            ));
        }

        errors
    }

    fn validate_unreachable_states(&self, diagram: &StateDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut reachable_states = HashSet::new();

        // Find start states
        let start_states: Vec<_> = diagram
            .states
            .iter()
            .filter(|(_, state)| matches!(state.state_type, StateType::Start))
            .map(|(id, _)| id.clone())
            .collect();

        // DFS to find all reachable states
        for start_state in &start_states {
            mark_reachable_states(diagram, start_state, &mut reachable_states);
        }

        // Check for unreachable states
        for state_id in diagram.states.keys() {
            if !reachable_states.contains(state_id) && !start_states.contains(state_id) {
                errors.push(ValidationError::warning(
                    "unreachable_state",
                    format!("State '{}' is unreachable", state_id),
                ));
            }
        }

        errors
    }

    fn validate_transition_references(&self, diagram: &StateDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for transition in &diagram.transitions {
            if !diagram.states.contains_key(&transition.from) {
                errors.push(ValidationError::error(
                    "undefined_state_reference",
                    format!(
                        "Transition references undefined state '{}'",
                        transition.from
                    ),
                ));
            }
            if !diagram.states.contains_key(&transition.to) {
                errors.push(ValidationError::error(
                    "undefined_state_reference",
                    format!("Transition references undefined state '{}'", transition.to),
                ));
            }
        }

        errors
    }

    fn validate_end_state_transitions(&self, diagram: &StateDiagram) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for (state_id, state) in &diagram.states {
            if matches!(state.state_type, StateType::End) {
                // Check if this end state has any outgoing transitions
                for transition in &diagram.transitions {
                    if transition.from == *state_id {
                        errors.push(ValidationError::warning(
                            "end_state_with_outgoing_transition",
                            format!("End state '{}' has outgoing transition", state_id),
                        ));
                    }
                }
            }
        }

        errors
    }
}

fn mark_reachable_states(diagram: &StateDiagram, state_id: &str, reachable: &mut HashSet<String>) {
    if reachable.contains(state_id) {
        return;
    }

    reachable.insert(state_id.to_string());

    // Find all transitions from this state
    for transition in &diagram.transitions {
        if transition.from == state_id {
            mark_reachable_states(diagram, &transition.to, reachable);
        }
    }
}

impl DiagramValidator for StateValidator {
    type Diagram = StateDiagram;
    type Error = ValidationError;

    fn validate(&self, diagram: &Self::Diagram) -> Result<(), Vec<Self::Error>> {
        let mut errors = Vec::new();

        errors.extend(self.validate_start_state(diagram));
        errors.extend(self.validate_unreachable_states(diagram));
        errors.extend(self.validate_transition_references(diagram));
        errors.extend(self.validate_end_state_transitions(diagram));

        // Filter by severity and ignored rules
        errors.retain(|error| {
            error.severity >= self.config.min_severity
                && !self.config.ignore_rules.contains(error.rule)
        });

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for StateValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_creation() {
        let error = ValidationError::error("test_rule", "Test message".to_string());
        assert_eq!(error.rule, "test_rule");
        assert_eq!(error.message, "Test message");
        assert_eq!(error.severity, Severity::Error);
        assert!(error.location.is_none());
    }

    #[test]
    fn test_validation_error_with_location() {
        let location = Location::new(1, 5);
        let error = ValidationError::with_location(
            "test_rule",
            "Test message".to_string(),
            Severity::Warning,
            location,
        );
        assert!(error.location.is_some());
        assert_eq!(error.location.as_ref().unwrap().line, 1);
        assert_eq!(error.location.as_ref().unwrap().column, 5);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
        assert!(Severity::Info < Severity::Error);
    }

    #[test]
    fn test_validation_config_default() {
        let config = ValidationConfig::default();
        assert_eq!(config.min_severity, Severity::Warning);
        assert!(config.ignore_rules.is_empty());
    }

    #[test]
    fn test_flowchart_validator_isolated_nodes() {
        use std::collections::HashMap;

        let mut nodes = HashMap::new();
        nodes.insert(
            "A".to_string(),
            FlowNode {
                id: "A".to_string(),
                text: Some("Node A".to_string()),
                shape: NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );
        nodes.insert(
            "B".to_string(),
            FlowNode {
                id: "B".to_string(),
                text: Some("Node B".to_string()),
                shape: NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );

        let diagram = FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: FlowDirection::TD,
            nodes,
            edges: vec![FlowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            }],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        };

        let validator = FlowchartValidator::new();
        let result = validator.validate(&diagram);
        assert!(result.is_ok());
    }

    #[test]
    fn test_flowchart_validator_undefined_reference() {
        use std::collections::HashMap;

        let mut nodes = HashMap::new();
        nodes.insert(
            "A".to_string(),
            FlowNode {
                id: "A".to_string(),
                text: Some("Node A".to_string()),
                shape: NodeShape::Rectangle,
                classes: vec![],
                icon: None,
            },
        );

        let diagram = FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: FlowDirection::TD,
            nodes,
            edges: vec![FlowEdge {
                from: "A".to_string(),
                to: "UNDEFINED".to_string(),
                edge_type: EdgeType::Arrow,
                label: None,
                min_length: None,
            }],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        };

        let validator = FlowchartValidator::new();
        let result = validator.validate(&diagram);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "undefined_node_reference");
        assert_eq!(errors[0].severity, Severity::Error);
    }

    #[test]
    fn test_sequence_validator_undefined_participant() {
        let diagram = SequenceDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            participants: vec![Participant {
                actor: "Alice".to_string(),
                alias: None,
                participant_type: ParticipantType::Actor,
            }],
            statements: vec![SequenceStatement::Message(Message {
                from: "Alice".to_string(),
                to: "Bob".to_string(), // Undefined participant
                text: "Hello".to_string(),
                arrow_type: ArrowType::SolidOpen,
            })],
            autonumber: None,
        };

        let validator = SequenceValidator::new();
        let result = validator.validate(&diagram);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "undefined_participant");
        assert_eq!(errors[0].severity, Severity::Error);
    }

    #[test]
    fn test_validation_error_display() {
        let error = ValidationError::error("test_rule", "Test message".to_string());
        let display = format!("{}", error);
        assert!(display.contains("ERROR"));
        assert!(display.contains("test_rule"));
        assert!(display.contains("Test message"));
    }

    #[test]
    fn test_universal_validator() {
        let validator = UniversalValidator::new();

        // Test with a simple valid flowchart
        let diagram = DiagramType::Flowchart(FlowchartDiagram {
            title: None,
            accessibility: AccessibilityInfo::default(),
            direction: FlowDirection::TD,
            nodes: HashMap::new(),
            edges: vec![],
            subgraphs: vec![],
            styles: vec![],
            class_defs: HashMap::new(),
            clicks: vec![],
        });

        let result = validator.validate_any(&diagram);
        assert!(result.is_ok());
    }
}
