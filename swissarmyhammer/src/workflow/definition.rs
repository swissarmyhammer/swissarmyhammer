//! Main workflow type and validation

use crate::common::{Parameter, ParameterGroup, ParameterProvider};
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

impl WorkflowParameter {
    /// Convert this WorkflowParameter to the shared Parameter type
    pub fn to_parameter(&self) -> Parameter {
        let shared_type = match self.parameter_type {
            ParameterType::String => crate::common::ParameterType::String,
            ParameterType::Boolean => crate::common::ParameterType::Boolean,
            ParameterType::Number => crate::common::ParameterType::Number,
            ParameterType::Choice => crate::common::ParameterType::Choice,
            ParameterType::MultiChoice => crate::common::ParameterType::MultiChoice,
        };

        let mut param =
            Parameter::new(&self.name, &self.description, shared_type).required(self.required);

        if let Some(default) = &self.default {
            param = param.with_default(default.clone());
        }

        if let Some(choices) = &self.choices {
            param = param.with_choices(choices.clone());
        }

        param
    }
}

impl From<Parameter> for WorkflowParameter {
    /// Convert a shared Parameter back to WorkflowParameter for backward compatibility
    fn from(param: Parameter) -> Self {
        let workflow_type = match param.parameter_type {
            crate::common::ParameterType::String => ParameterType::String,
            crate::common::ParameterType::Boolean => ParameterType::Boolean,
            crate::common::ParameterType::Number => ParameterType::Number,
            crate::common::ParameterType::Choice => ParameterType::Choice,
            crate::common::ParameterType::MultiChoice => ParameterType::MultiChoice,
        };

        Self {
            name: param.name,
            description: param.description,
            required: param.required,
            parameter_type: workflow_type,
            default: param.default,
            choices: param.choices,
        }
    }
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
    /// Parameter groups for organizing parameters (optional)
    pub parameter_groups: Option<Vec<ParameterGroup>>,
    /// Cached shared parameters converted from WorkflowParameters.
    ///
    /// This field is lazily populated to provide efficient access to the shared
    /// parameter system without breaking backward compatibility.
    #[serde(skip)]
    cached_parameters: std::sync::OnceLock<Vec<Parameter>>,
    /// Cached parameter groups for efficient access
    #[serde(skip)]
    cached_parameter_groups: std::sync::OnceLock<Vec<ParameterGroup>>,
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
            parameter_groups: None,
            cached_parameters: std::sync::OnceLock::new(),
            cached_parameter_groups: std::sync::OnceLock::new(),
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

        // Validate parameter groups
        if let Err(group_errors) = self.validate_parameter_groups() {
            errors.extend(group_errors);
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
                errors.push(format!(
                    "Parameter '{}' must have a description",
                    parameter.name
                ));
            }

            // Validate choices for Choice and MultiChoice types
            match parameter.parameter_type {
                ParameterType::Choice | ParameterType::MultiChoice => {
                    if parameter.choices.is_none() || parameter.choices.as_ref().unwrap().is_empty()
                    {
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
                    if parameter.choices.is_some()
                        && !parameter.choices.as_ref().unwrap().is_empty()
                    {
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

    /// Validate parameter groups
    pub fn validate_parameter_groups(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Some(ref groups) = self.parameter_groups {
            let mut assigned_params = std::collections::HashSet::<String>::new();

            for group in groups {
                // Validate group name is not empty
                if group.name.trim().is_empty() {
                    errors.push("Parameter group name cannot be empty".to_string());
                }

                // Validate group description is not empty
                if group.description.trim().is_empty() {
                    errors.push(format!(
                        "Parameter group '{}' must have a description",
                        group.name
                    ));
                }

                // Check for duplicate parameter assignments across groups
                for param_name in &group.parameters {
                    if assigned_params.contains(param_name) {
                        errors.push(format!(
                            "Parameter '{param_name}' is assigned to multiple groups"
                        ));
                    } else {
                        assigned_params.insert(param_name.clone());
                    }

                    // Verify parameter exists in the workflow
                    if !self.parameters.iter().any(|p| &p.name == param_name) {
                        errors.push(format!(
                            "Parameter group '{}' references non-existent parameter '{}'",
                            group.name, param_name
                        ));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl ParameterProvider for Workflow {
    /// Get the parameters for this workflow by converting from WorkflowParameter
    fn get_parameters(&self) -> &[Parameter] {
        self.cached_parameters.get_or_init(|| {
            self.parameters
                .iter()
                .map(|workflow_param| workflow_param.to_parameter())
                .collect()
        })
    }

    /// Get the parameter groups for this workflow
    fn get_parameter_groups(&self) -> Option<&[ParameterGroup]> {
        if let Some(ref groups) = self.parameter_groups {
            Some(groups.as_slice())
        } else {
            None
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
        assert!(errors
            .iter()
            .any(|e| e.contains("Parameter name cannot be empty")));
        assert!(errors.iter().any(|e| e.contains("must have a description")));
        assert!(errors
            .iter()
            .any(|e| e.contains("must have choices defined")));
        assert!(errors
            .iter()
            .any(|e| e.contains("should not have choices defined")));
        assert!(errors
            .iter()
            .any(|e| e.contains("default value does not match parameter type")));
        assert!(errors
            .iter()
            .any(|e| e.contains("Duplicate parameter name")));
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
        assert!(errors
            .iter()
            .any(|e| e.contains("must have choices defined")));
    }

    #[test]
    fn test_shared_parameter_system_integration() {
        use crate::common::ParameterProvider;
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();

        // Add workflow parameters
        workflow.parameters.push(WorkflowParameter {
            name: "input_file".to_string(),
            description: "Input file path".to_string(),
            required: true,
            parameter_type: ParameterType::String,
            default: None,
            choices: None,
        });

        workflow.parameters.push(WorkflowParameter {
            name: "mode".to_string(),
            description: "Processing mode".to_string(),
            required: false,
            parameter_type: ParameterType::Choice,
            default: Some(serde_json::Value::String("fast".to_string())),
            choices: Some(vec!["fast".to_string(), "thorough".to_string()]),
        });

        // Test that ParameterProvider trait works
        let parameters = workflow.get_parameters();
        assert_eq!(parameters.len(), 2);

        // Check first parameter
        assert_eq!(parameters[0].name, "input_file");
        assert_eq!(parameters[0].description, "Input file path");
        assert!(parameters[0].required);
        assert_eq!(parameters[0].parameter_type.as_str(), "string");

        // Check second parameter
        assert_eq!(parameters[1].name, "mode");
        assert_eq!(parameters[1].description, "Processing mode");
        assert!(!parameters[1].required);
        assert_eq!(parameters[1].parameter_type.as_str(), "choice");
        assert_eq!(
            parameters[1].choices,
            Some(vec!["fast".to_string(), "thorough".to_string()])
        );
        assert_eq!(
            parameters[1].default,
            Some(serde_json::Value::String("fast".to_string()))
        );
    }

    #[test]
    fn test_workflow_parameter_groups_validation_success() {
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();

        // Add valid parameters
        workflow.parameters = vec![
            WorkflowParameter {
                name: "deploy_env".to_string(),
                description: "Deployment environment".to_string(),
                required: true,
                parameter_type: ParameterType::Choice,
                default: None,
                choices: Some(vec!["dev".to_string(), "prod".to_string()]),
            },
            WorkflowParameter {
                name: "region".to_string(),
                description: "AWS region".to_string(),
                required: true,
                parameter_type: ParameterType::String,
                default: Some(serde_json::Value::String("us-east-1".to_string())),
                choices: None,
            },
            WorkflowParameter {
                name: "enable_ssl".to_string(),
                description: "Enable SSL".to_string(),
                required: false,
                parameter_type: ParameterType::Boolean,
                default: Some(serde_json::Value::Bool(true)),
                choices: None,
            },
        ];

        // Add valid parameter groups
        workflow.parameter_groups = Some(vec![
            ParameterGroup::new("deployment", "Deployment configuration")
                .with_parameters(vec!["deploy_env".to_string(), "region".to_string()]),
            ParameterGroup::new("security", "Security settings").with_parameter("enable_ssl"),
        ]);

        // Should pass validation
        let result = workflow.validate_parameter_groups();
        assert!(result.is_ok());

        // Should also pass overall structure validation
        let result = workflow.validate_structure();
        assert!(result.is_ok());
    }

    #[test]
    fn test_workflow_parameter_groups_validation_errors() {
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();

        // Add valid parameters
        workflow.parameters = vec![
            WorkflowParameter {
                name: "param1".to_string(),
                description: "Parameter 1".to_string(),
                required: true,
                parameter_type: ParameterType::String,
                default: None,
                choices: None,
            },
            WorkflowParameter {
                name: "param2".to_string(),
                description: "Parameter 2".to_string(),
                required: false,
                parameter_type: ParameterType::Boolean,
                default: Some(serde_json::Value::Bool(false)),
                choices: None,
            },
        ];

        // Add invalid parameter groups
        workflow.parameter_groups = Some(vec![
            // Empty group name
            ParameterGroup::new("", "Empty name group").with_parameter("param1"),
            // Empty group description
            ParameterGroup::new("group2", "").with_parameter("param2"),
            // Reference non-existent parameter
            ParameterGroup::new("group3", "Group with non-existent param")
                .with_parameter("nonexistent"),
            // Duplicate parameter assignment
            ParameterGroup::new("group4", "Group with duplicate param").with_parameter("param1"), // param1 already in first group
        ]);

        let result = workflow.validate_parameter_groups();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("Parameter group name cannot be empty")));
        assert!(errors.iter().any(|e| e.contains("must have a description")));
        assert!(errors
            .iter()
            .any(|e| e.contains("references non-existent parameter 'nonexistent'")));
        assert!(errors
            .iter()
            .any(|e| e.contains("Parameter 'param1' is assigned to multiple groups")));
    }

    #[test]
    fn test_workflow_parameter_groups_integration_with_parameter_provider() {
        use crate::common::ParameterProvider;
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();

        // Add parameters
        workflow.parameters = vec![
            WorkflowParameter {
                name: "database_url".to_string(),
                description: "Database connection URL".to_string(),
                required: true,
                parameter_type: ParameterType::String,
                default: None,
                choices: None,
            },
            WorkflowParameter {
                name: "cache_enabled".to_string(),
                description: "Enable caching".to_string(),
                required: false,
                parameter_type: ParameterType::Boolean,
                default: Some(serde_json::Value::Bool(true)),
                choices: None,
            },
            WorkflowParameter {
                name: "log_level".to_string(),
                description: "Logging level".to_string(),
                required: false,
                parameter_type: ParameterType::Choice,
                default: Some(serde_json::Value::String("info".to_string())),
                choices: Some(vec![
                    "debug".to_string(),
                    "info".to_string(),
                    "warn".to_string(),
                    "error".to_string(),
                ]),
            },
            WorkflowParameter {
                name: "timeout".to_string(),
                description: "Request timeout".to_string(),
                required: false,
                parameter_type: ParameterType::Number,
                default: Some(serde_json::Value::Number(serde_json::Number::from(30))),
                choices: None,
            },
        ];

        // Add parameter groups
        workflow.parameter_groups = Some(vec![
            ParameterGroup::new("database", "Database configuration")
                .with_parameter("database_url")
                .with_parameter("cache_enabled"),
            ParameterGroup::new("logging", "Logging and monitoring").with_parameter("log_level"),
            // timeout parameter intentionally left ungrouped
        ]);

        // Test ParameterProvider trait implementation
        let parameters = workflow.get_parameters();
        assert_eq!(parameters.len(), 4);

        let groups = workflow.get_parameter_groups().unwrap();
        assert_eq!(groups.len(), 2);

        // Test parameter grouping
        let grouped = workflow.get_parameters_by_group();
        assert_eq!(grouped.len(), 3); // database, logging, general

        // Check database group
        let database_params = grouped.get("database").unwrap();
        assert_eq!(database_params.len(), 2);
        let param_names: Vec<&str> = database_params.iter().map(|p| p.name.as_str()).collect();
        assert!(param_names.contains(&"database_url"));
        assert!(param_names.contains(&"cache_enabled"));

        // Check logging group
        let logging_params = grouped.get("logging").unwrap();
        assert_eq!(logging_params.len(), 1);
        assert_eq!(logging_params[0].name, "log_level");

        // Check general group (ungrouped parameters)
        let general_params = grouped.get("general").unwrap();
        assert_eq!(general_params.len(), 1);
        assert_eq!(general_params[0].name, "timeout");

        // Test parameter membership checks
        assert!(workflow.is_parameter_in_any_group("database_url"));
        assert!(workflow.is_parameter_in_any_group("log_level"));
        assert!(!workflow.is_parameter_in_any_group("timeout")); // Not in any explicit group
        assert!(!workflow.is_parameter_in_any_group("nonexistent"));
    }

    #[test]
    fn test_workflow_parameter_groups_without_groups() {
        use crate::common::ParameterProvider;
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();

        // Add parameters but no groups
        workflow.parameters = vec![
            WorkflowParameter {
                name: "param1".to_string(),
                description: "Parameter 1".to_string(),
                required: true,
                parameter_type: ParameterType::String,
                default: None,
                choices: None,
            },
            WorkflowParameter {
                name: "param2".to_string(),
                description: "Parameter 2".to_string(),
                required: false,
                parameter_type: ParameterType::Boolean,
                default: Some(serde_json::Value::Bool(false)),
                choices: None,
            },
        ];

        workflow.parameter_groups = None;

        // Test ParameterProvider trait implementation
        assert!(workflow.get_parameter_groups().is_none());

        // All parameters should be in general group
        let grouped = workflow.get_parameters_by_group();
        assert_eq!(grouped.len(), 1);

        let general_params = grouped.get("general").unwrap();
        assert_eq!(general_params.len(), 2);

        // No parameters should be in explicit groups
        assert!(!workflow.is_parameter_in_any_group("param1"));
        assert!(!workflow.is_parameter_in_any_group("param2"));

        // Validation should still pass
        let result = workflow.validate_parameter_groups();
        assert!(result.is_ok());
    }

    #[test]
    fn test_workflow_validation_includes_parameter_groups() {
        use crate::workflow::test_helpers::*;

        let mut workflow = create_basic_workflow();

        // Add valid parameter
        workflow.parameters = vec![WorkflowParameter {
            name: "test_param".to_string(),
            description: "Test parameter".to_string(),
            required: true,
            parameter_type: ParameterType::String,
            default: None,
            choices: None,
        }];

        // Add invalid parameter group
        workflow.parameter_groups = Some(vec![
            ParameterGroup::new("invalid_group", "") // Empty description
                .with_parameter("nonexistent"), // Non-existent parameter
        ]);

        // Workflow structure validation should include parameter group validation
        let result = workflow.validate_structure();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("must have a description")));
        assert!(errors
            .iter()
            .any(|e| e.contains("references non-existent parameter")));
    }
}
