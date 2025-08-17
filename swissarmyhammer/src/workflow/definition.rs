//! Main workflow type and validation

use crate::validation::{Validatable, ValidationIssue, ValidationLevel};
use crate::workflow::{State, StateId, Transition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur when creating workflow-related types
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Workflow name cannot be empty or whitespace only
    #[error("Workflow name cannot be empty or whitespace only")]
    EmptyWorkflowName,
}

/// Result type for workflow operations
pub type WorkflowResult<T> = Result<T, WorkflowError>;

/// Unique identifier for workflows
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowName(String);

impl WorkflowName {
    /// Create a new workflow name
    ///
    /// # Panics
    /// Panics if the name is empty or whitespace only. For non-panicking creation,
    /// use `try_new` instead.
    pub fn new(name: impl Into<String>) -> Self {
        Self::try_new(name).expect("Workflow name cannot be empty or whitespace only")
    }

    /// Create a new workflow name, returning an error for invalid input
    pub fn try_new(name: impl Into<String>) -> WorkflowResult<Self> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(WorkflowError::EmptyWorkflowName);
        }
        Ok(Self(name))
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkflowName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for WorkflowName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl std::fmt::Display for WorkflowName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Types of parameters supported in workflow schemas
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterType {
    /// String text input
    String,
    /// Boolean true/false values
    Boolean,
    /// Numeric values (integers and floats)
    Number,
    /// Selection from predefined options
    Choice,
    /// Multiple selections from predefined options
    MultiChoice,
}

/// Specification for a workflow parameter
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowParameter {
    /// The parameter name used in templates
    pub name: String,
    /// Human-readable description of the parameter's purpose
    pub description: String,
    /// Whether this parameter must be provided
    pub required: bool,
    /// The type of parameter value expected
    pub parameter_type: ParameterType,
    /// Default value to use if parameter is not provided
    pub default: Option<serde_json::Value>,
    /// Available choices for Choice and MultiChoice types
    pub choices: Option<Vec<String>>,
}

/// Main workflow representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    /// Workflow name
    pub name: WorkflowName,
    /// Workflow description
    pub description: String,
    /// Parameter schema for this workflow
    pub parameters: Vec<WorkflowParameter>,
    /// All states in the workflow
    pub states: HashMap<StateId, State>,
    /// All transitions in the workflow
    pub transitions: Vec<Transition>,
    /// Initial state ID
    pub initial_state: StateId,
    /// Metadata for debugging and monitoring
    pub metadata: HashMap<String, String>,
}

impl Workflow {
    /// Create a new workflow with basic validation
    pub fn new(name: WorkflowName, description: String, initial_state: StateId) -> Self {
        Self {
            name,
            description,
            parameters: Vec::new(),
            states: Default::default(),
            transitions: Vec::new(),
            initial_state,
            metadata: Default::default(),
        }
    }

    /// Validate the workflow structure
    pub fn validate_structure(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Check if workflow name is not empty
        if self.name.as_str().trim().is_empty() {
            errors.push("Workflow name cannot be empty".to_string());
        }

        // Check if initial state exists
        if !self.states.contains_key(&self.initial_state) {
            errors.push(format!(
                "Initial state '{}' not found in workflow states. Available states: {:?}",
                self.initial_state,
                self.states.keys().map(|k| k.as_str()).collect::<Vec<_>>()
            ));
        }

        // Check if all transitions reference existing states
        for transition in &self.transitions {
            // Check for empty state IDs in transitions
            if transition.from_state.as_str().trim().is_empty() {
                errors.push(format!("Transition #{} has empty source state ID. All transitions must have valid non-empty state IDs", self.transitions.iter().position(|t| t == transition).unwrap_or(0)));
            }
            if transition.to_state.as_str().trim().is_empty() {
                errors.push(format!("Transition #{} has empty target state ID. All transitions must have valid non-empty state IDs", self.transitions.iter().position(|t| t == transition).unwrap_or(0)));
            }

            if !self.states.contains_key(&transition.from_state) {
                errors.push(format!(
                    "Transition references non-existent source state: '{}'",
                    transition.from_state
                ));
            }
            if !self.states.contains_key(&transition.to_state) {
                errors.push(format!(
                    "Transition references non-existent target state: '{}'",
                    transition.to_state
                ));
            }
        }

        // Check for at least one terminal state
        let has_terminal = self.states.values().any(|s| s.is_terminal);
        if !has_terminal {
            errors.push("Workflow must have at least one terminal state. Add 'is_terminal: true' to at least one state or create a transition to [*]".to_string());
        }

        // Validate parameters
        if let Err(param_errors) = self.validate_parameters() {
            errors.extend(param_errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Add a state to the workflow
    pub fn add_state(&mut self, state: State) {
        self.states.insert(state.id.clone(), state);
    }

    /// Add a transition to the workflow
    pub fn add_transition(&mut self, transition: Transition) {
        self.transitions.push(transition);
    }

    /// Validate workflow parameters
    pub fn validate_parameters(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for parameter in &self.parameters {
            // Check parameter name is not empty
            if parameter.name.trim().is_empty() {
                errors.push("Parameter name cannot be empty".to_string());
                continue;
            }

            // Check parameter description is not empty
            if parameter.description.trim().is_empty() {
                errors.push(format!("Parameter '{}' must have a description", parameter.name));
            }

            // Validate choices for Choice and MultiChoice types
            match parameter.parameter_type {
                ParameterType::Choice | ParameterType::MultiChoice => {
                    if parameter.choices.is_none() || parameter.choices.as_ref().unwrap().is_empty() {
                        errors.push(format!(
                            "Parameter '{}' with type {:?} must have choices defined",
                            parameter.name, parameter.parameter_type
                        ));
                    }
                }
                ParameterType::String => {
                    // String parameters can optionally have choices for UI hints
                    // No validation needed - choices are optional
                }
                ParameterType::Boolean | ParameterType::Number => {
                    // For Boolean and Number types, choices should not be defined
                    if parameter.choices.is_some() && !parameter.choices.as_ref().unwrap().is_empty() {
                        errors.push(format!(
                            "Parameter '{}' with type {:?} should not have choices defined",
                            parameter.name, parameter.parameter_type
                        ));
                    }
                }
            }

            // Validate default value type matches parameter type
            if let Some(default_value) = &parameter.default {
                let type_matches = match parameter.parameter_type {
                    ParameterType::String => {
                        // For string types, check if it's a valid string
                        if !default_value.is_string() {
                            false
                        } else if let Some(choices) = &parameter.choices {
                            // If choices are provided, default must be in the choices
                            if let Some(default_str) = default_value.as_str() {
                                choices.contains(&default_str.to_string())
                            } else {
                                false
                            }
                        } else {
                            true // String without choices is valid
                        }
                    }
                    ParameterType::Boolean => default_value.is_boolean(),
                    ParameterType::Number => default_value.is_number(),
                    ParameterType::Choice => {
                        // For choice, default must be a string and in the choices list
                        if let Some(default_str) = default_value.as_str() {
                            if let Some(choices) = &parameter.choices {
                                choices.contains(&default_str.to_string())
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    ParameterType::MultiChoice => {
                        // For multi-choice, default must be an array of strings from choices
                        if let Some(default_array) = default_value.as_array() {
                            if let Some(choices) = &parameter.choices {
                                default_array.iter().all(|v| {
                                    v.as_str().is_some_and(|s| choices.contains(&s.to_string()))
                                })
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                };

                if !type_matches {
                    errors.push(format!(
                        "Parameter '{}' default value does not match parameter type {:?}",
                        parameter.name, parameter.parameter_type
                    ));
                }
            }
        }

        // Check for duplicate parameter names
        let mut seen_names = std::collections::HashSet::new();
        for parameter in &self.parameters {
            if !seen_names.insert(&parameter.name) {
                errors.push(format!("Duplicate parameter name: '{}'", parameter.name));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Validatable for Workflow {
    fn validate(&self, source_path: Option<&Path>) -> Vec<ValidationIssue> {
        match self.validate_structure() {
            Ok(()) => Vec::new(),
            Err(error_messages) => {
                let workflow_path = source_path.map(|p| p.to_path_buf()).unwrap_or_else(|| {
                    std::path::PathBuf::from(format!("workflow:{}", self.name.as_str()))
                });

                error_messages
                    .into_iter()
                    .map(|message| ValidationIssue {
                        level: ValidationLevel::Error,
                        file_path: workflow_path.clone(),
                        content_title: Some(self.name.to_string()),
                        line: None,
                        column: None,
                        message,
                        suggestion: None,
                    })
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::test_helpers::*;

    #[test]
    fn test_workflow_validation_success() {
        let workflow = create_basic_workflow();
        assert!(workflow.validate_structure().is_ok());
    }

    #[test]
    fn test_workflow_validation_missing_initial_state() {
        let workflow = Workflow::new(
            WorkflowName::new("Test Workflow"),
            "A test workflow".to_string(),
            StateId::new("start"),
        );

        let result = workflow.validate_structure();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Initial state")));
    }

    #[test]
    fn test_workflow_validation_no_terminal_state() {
        let mut workflow = Workflow::new(
            WorkflowName::new("Test Workflow"),
            "A test workflow".to_string(),
            StateId::new("start"),
        );

        workflow.add_state(create_state("start", "Start state", false));

        let result = workflow.validate_structure();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("terminal state")));
    }

    #[test]
    fn test_workflow_parameter_validation() {
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();
        
        // Add valid parameters
        workflow.parameters.push(WorkflowParameter {
            name: "valid_string".to_string(),
            description: "A valid string parameter".to_string(),
            required: true,
            parameter_type: ParameterType::String,
            default: None,
            choices: None,
        });

        workflow.parameters.push(WorkflowParameter {
            name: "valid_choice".to_string(),
            description: "A valid choice parameter".to_string(),
            required: false,
            parameter_type: ParameterType::Choice,
            default: Some(serde_json::Value::String("option1".to_string())),
            choices: Some(vec!["option1".to_string(), "option2".to_string()]),
        });

        // Should pass validation
        let result = workflow.validate_parameters();
        assert!(result.is_ok());
    }

    #[test]
    fn test_workflow_parameter_validation_errors() {
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();
        
        // Add invalid parameters
        workflow.parameters.push(WorkflowParameter {
            name: "".to_string(), // Empty name
            description: "Parameter with empty name".to_string(),
            required: true,
            parameter_type: ParameterType::String,
            default: None,
            choices: None,
        });

        workflow.parameters.push(WorkflowParameter {
            name: "no_description".to_string(),
            description: "".to_string(), // Empty description
            required: true,
            parameter_type: ParameterType::String,
            default: None,
            choices: None,
        });

        workflow.parameters.push(WorkflowParameter {
            name: "choice_without_choices".to_string(),
            description: "Choice parameter without choices".to_string(),
            required: true,
            parameter_type: ParameterType::Choice,
            default: None,
            choices: None, // No choices for choice type
        });

        workflow.parameters.push(WorkflowParameter {
            name: "boolean_with_choices".to_string(),
            description: "Boolean parameter with choices".to_string(),
            required: false,
            parameter_type: ParameterType::Boolean,
            default: None,
            choices: Some(vec!["choice1".to_string()]), // Boolean should not have choices
        });

        workflow.parameters.push(WorkflowParameter {
            name: "wrong_default_type".to_string(),
            description: "Boolean with string default".to_string(),
            required: false,
            parameter_type: ParameterType::Boolean,
            default: Some(serde_json::Value::String("not_a_bool".to_string())), // Wrong type
            choices: None,
        });

        // Add duplicate parameter name
        workflow.parameters.push(WorkflowParameter {
            name: "boolean_with_choices".to_string(), // Duplicate name
            description: "Duplicate parameter name".to_string(),
            required: false,
            parameter_type: ParameterType::String,
            default: None,
            choices: None,
        });

        let result = workflow.validate_parameters();
        assert!(result.is_err());
        
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Parameter name cannot be empty")));
        assert!(errors.iter().any(|e| e.contains("must have a description")));
        assert!(errors.iter().any(|e| e.contains("must have choices defined")));
        assert!(errors.iter().any(|e| e.contains("should not have choices defined")));
        assert!(errors.iter().any(|e| e.contains("default value does not match parameter type")));
        assert!(errors.iter().any(|e| e.contains("Duplicate parameter name")));
    }

    #[test]
    fn test_workflow_validation_includes_parameters() {
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();
        
        // Add invalid parameter that should cause workflow validation to fail
        workflow.parameters.push(WorkflowParameter {
            name: "invalid".to_string(),
            description: "".to_string(), // Empty description
            required: true,
            parameter_type: ParameterType::Choice,
            default: None,
            choices: None, // No choices
        });

        let result = workflow.validate_structure();
        assert!(result.is_err());
        
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("must have a description")));
        assert!(errors.iter().any(|e| e.contains("must have choices defined")));
    }
}
