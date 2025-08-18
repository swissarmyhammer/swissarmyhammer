//! Tests for CLI help generation and parameter group functionality
//!
//! This module tests the generation of help text for parameters, including
//! parameter grouping, validation rule documentation, and user-friendly
//! error messages.

use serde_json::json;
use std::collections::HashMap;
use swissarmyhammer::common::parameters::{
    CommonPatterns, Parameter, ParameterGroup, ParameterProvider, ParameterType, ValidationRules,
};

/// Mock implementation of ParameterProvider for testing
#[derive(Debug)]
struct MockWorkflow {
    parameters: Vec<Parameter>,
    groups: Option<Vec<ParameterGroup>>,
}

impl MockWorkflow {
    fn new(parameters: Vec<Parameter>) -> Self {
        Self {
            parameters,
            groups: None,
        }
    }

    fn with_groups(mut self, groups: Vec<ParameterGroup>) -> Self {
        self.groups = Some(groups);
        self
    }
}

impl ParameterProvider for MockWorkflow {
    fn get_parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn get_parameter_groups(&self) -> Option<&[ParameterGroup]> {
        self.groups.as_deref()
    }
}

/// Generate help text for a parameter (simplified version)
fn generate_parameter_help(param: &Parameter) -> String {
    let mut help = String::new();
    
    // Parameter name with CLI switch format
    let switch_name = param.name.replace('_', "-");
    help.push_str(&format!("--{switch_name}"));
    
    // Parameter type
    match param.parameter_type {
        ParameterType::String => help.push_str(" <STRING>"),
        ParameterType::Boolean => {}, // Boolean flags don't take values
        ParameterType::Number => help.push_str(" <NUMBER>"),
        ParameterType::Choice => help.push_str(" <CHOICE>"),
        ParameterType::MultiChoice => help.push_str(" <CHOICES>"),
    }
    
    // Required indicator
    if param.required {
        help.push_str(" (required)");
    }
    
    help.push('\n');
    
    // Description
    help.push_str(&format!("    {}", param.description));
    
    // Default value
    if let Some(default) = &param.default {
        match default {
            serde_json::Value::String(s) => help.push_str(&format!(" [default: {s}]")),
            serde_json::Value::Bool(b) => help.push_str(&format!(" [default: {b}]")),
            serde_json::Value::Number(n) => help.push_str(&format!(" [default: {n}]")),
            _ => help.push_str(" [default: complex]"),
        }
    }
    
    help.push('\n');
    
    // Choices
    if let Some(choices) = &param.choices {
        if !choices.is_empty() {
            help.push_str(&format!("    [possible values: {}]\n", choices.join(", ")));
        }
    }
    
    // Validation rules
    if let Some(validation) = &param.validation {
        let mut constraints = Vec::new();
        
        if let Some(min_len) = validation.min_length {
            constraints.push(format!("min length: {min_len}"));
        }
        if let Some(max_len) = validation.max_length {
            constraints.push(format!("max length: {max_len}"));
        }
        if let Some(pattern) = &validation.pattern {
            let description = CommonPatterns::description_for_pattern(pattern);
            constraints.push(format!("format: {description}"));
        }
        if let Some(min) = validation.min {
            constraints.push(format!("min: {min}"));
        }
        if let Some(max) = validation.max {
            constraints.push(format!("max: {max}"));
        }
        if let Some(step) = validation.step {
            constraints.push(format!("step: {step}"));
        }
        if let Some(min_sel) = validation.min_selections {
            constraints.push(format!("min selections: {min_sel}"));
        }
        if let Some(max_sel) = validation.max_selections {
            constraints.push(format!("max selections: {max_sel}"));
        }
        
        if !constraints.is_empty() {
            help.push_str(&format!("    [constraints: {}]\n", constraints.join(", ")));
        }
    }
    
    // Condition
    if let Some(condition) = &param.condition {
        help.push_str(&format!("    [when: {}]\n", condition.expression));
    }
    
    help
}

/// Generate grouped help text
fn generate_grouped_help(provider: &dyn ParameterProvider) -> String {
    let mut help = String::new();
    let grouped = provider.get_parameters_by_group();
    
    for (group_name, params) in &grouped {
        // Find group description if available
        let group_description = provider
            .get_parameter_groups()
            .and_then(|groups| groups.iter().find(|g| &g.name == group_name))
            .map(|g| g.description.as_str())
            .unwrap_or("");
        
        if group_name == "general" {
            help.push_str("Options:\n");
        } else {
            help.push_str(&format!("{}:\n", group_name.replace('_', " ").to_uppercase()));
            if !group_description.is_empty() {
                help.push_str(&format!("  {group_description}\n"));
            }
        }
        
        for param in params {
            let param_help = generate_parameter_help(param);
            // Indent parameter help
            for line in param_help.lines() {
                help.push_str(&format!("  {line}\n"));
            }
        }
        
        help.push('\n');
    }
    
    help
}

#[cfg(test)]
mod parameter_help_generation_tests {
    use super::*;

    #[test]
    fn test_basic_parameter_help_generation() {
        let param = Parameter::new("username", "User name for authentication", ParameterType::String)
            .required(true);
        
        let help = generate_parameter_help(&param);
        
        assert!(help.contains("--username <STRING> (required)"));
        assert!(help.contains("User name for authentication"));
        assert!(!help.contains("[default:"));
    }

    #[test]
    fn test_parameter_help_with_default_values() {
        let test_cases = vec![
            (
                Parameter::new("port", "Server port", ParameterType::Number)
                    .with_default(json!(8080)),
                vec!["--port <NUMBER>", "Server port", "[default: 8080]"],
            ),
            (
                Parameter::new("debug", "Debug mode", ParameterType::Boolean)
                    .with_default(json!(false)),
                vec!["--debug", "Debug mode", "[default: false]"],
            ),
            (
                Parameter::new("environment", "Target environment", ParameterType::Choice)
                    .with_default(json!("development"))
                    .with_choices(vec!["development".to_string(), "staging".to_string(), "production".to_string()]),
                vec!["--environment <CHOICE>", "Target environment", "[default: development]", "[possible values: development, staging, production]"],
            ),
        ];

        for (param, expected_content) in test_cases {
            let help = generate_parameter_help(&param);
            
            for content in expected_content {
                assert!(
                    help.contains(content),
                    "Help should contain '{content}' for parameter {}: {help}",
                    param.name
                );
            }
        }
    }

    #[test]
    fn test_parameter_help_with_choices() {
        let param = Parameter::new("log_level", "Logging level", ParameterType::Choice)
            .required(true)
            .with_choices(vec![
                "error".to_string(),
                "warn".to_string(),
                "info".to_string(),
                "debug".to_string(),
                "trace".to_string(),
            ]);
        
        let help = generate_parameter_help(&param);
        
        assert!(help.contains("--log-level <CHOICE> (required)"));
        assert!(help.contains("Logging level"));
        assert!(help.contains("[possible values: error, warn, info, debug, trace]"));
    }

    #[test]
    fn test_parameter_help_with_validation_rules() {
        let param = Parameter::new("email", "Email address", ParameterType::String)
            .required(true)
            .with_pattern(CommonPatterns::EMAIL)
            .with_length_range(Some(5), Some(100));
        
        let help = generate_parameter_help(&param);
        
        assert!(help.contains("--email <STRING> (required)"));
        assert!(help.contains("Email address"));
        assert!(help.contains("min length: 5"));
        assert!(help.contains("max length: 100"));
        assert!(help.contains("format: Valid email address"));
    }

    #[test]
    fn test_parameter_help_with_numeric_validation() {
        let param = Parameter::new("percentage", "Percentage value", ParameterType::Number)
            .required(false)
            .with_range(Some(0.0), Some(100.0))
            .with_step(0.1)
            .with_default(json!(50.0));
        
        let help = generate_parameter_help(&param);
        
        assert!(help.contains("--percentage <NUMBER>"));
        assert!(help.contains("Percentage value"));
        assert!(help.contains("[default: 50]"));
        assert!(help.contains("min: 0"));
        assert!(help.contains("max: 100"));
        assert!(help.contains("step: 0.1"));
    }

    #[test]
    fn test_parameter_help_with_multi_choice_validation() {
        let param = Parameter::new("features", "Features to enable", ParameterType::MultiChoice)
            .required(false)
            .with_choices(vec![
                "auth".to_string(),
                "logging".to_string(),
                "metrics".to_string(),
                "caching".to_string(),
            ])
            .with_selection_range(Some(1), Some(3))
            .with_default(json!(["logging"]));
        
        let help = generate_parameter_help(&param);
        
        assert!(help.contains("--features <CHOICES>"));
        assert!(help.contains("Features to enable"));
        assert!(help.contains("[possible values: auth, logging, metrics, caching]"));
        assert!(help.contains("min selections: 1"));
        assert!(help.contains("max selections: 3"));
    }

    #[test]
    fn test_parameter_help_with_conditions() {
        let param = Parameter::new("ssl_cert", "SSL certificate path", ParameterType::String)
            .required(true)
            .when("enable_ssl == true");
        
        let help = generate_parameter_help(&param);
        
        assert!(help.contains("--ssl-cert <STRING> (required)"));
        assert!(help.contains("SSL certificate path"));
        assert!(help.contains("[when: enable_ssl == true]"));
    }

    #[test]
    fn test_parameter_name_conversion() {
        // Test that parameter names are converted to CLI-friendly formats
        let test_cases = vec![
            ("simple_name", "--simple-name"),
            ("camelCaseName", "--camelCaseName"), // No conversion for camelCase
            ("snake_case_name", "--snake-case-name"),
            ("UPPERCASE_NAME", "--UPPERCASE-NAME"),
            ("name_with_numbers_123", "--name-with-numbers-123"),
        ];

        for (param_name, expected_switch) in test_cases {
            let param = Parameter::new(param_name, "Test parameter", ParameterType::String);
            let help = generate_parameter_help(&param);
            
            assert!(
                help.contains(expected_switch),
                "Parameter '{param_name}' should generate switch '{expected_switch}' in: {help}"
            );
        }
    }

    #[test]
    fn test_parameter_help_edge_cases() {
        // Test edge cases in help generation
        
        // Empty description
        let param = Parameter::new("param", "", ParameterType::String);
        let help = generate_parameter_help(&param);
        assert!(help.contains("--param <STRING>"));
        
        // Very long description
        let long_desc = "A".repeat(200);
        let param = Parameter::new("param", &long_desc, ParameterType::String);
        let help = generate_parameter_help(&param);
        assert!(help.contains(&long_desc));
        
        // Many choices
        let many_choices: Vec<String> = (0..50).map(|i| format!("choice_{i}")).collect();
        let param = Parameter::new("param", "Many choices", ParameterType::Choice)
            .with_choices(many_choices.clone());
        let help = generate_parameter_help(&param);
        assert!(help.contains("[possible values:"));
        for choice in many_choices.iter().take(5) {
            assert!(help.contains(choice));
        }
    }
}

#[cfg(test)]
mod parameter_group_tests {
    use super::*;

    #[test]
    fn test_parameter_group_basic_functionality() {
        let group = ParameterGroup::new("authentication", "Authentication settings")
            .with_parameter("username")
            .with_parameter("password")
            .with_parameter("token");
        
        assert_eq!(group.name, "authentication");
        assert_eq!(group.description, "Authentication settings");
        assert_eq!(group.parameters, vec!["username", "password", "token"]);
        assert_eq!(group.collapsed, None);
        assert_eq!(group.condition, None);
    }

    #[test]
    fn test_parameter_group_with_options() {
        let group = ParameterGroup::new("advanced", "Advanced configuration options")
            .with_parameters(vec!["debug".to_string(), "verbose".to_string(), "timeout".to_string()])
            .collapsed(true)
            .with_condition("mode == 'expert'");
        
        assert_eq!(group.name, "advanced");
        assert_eq!(group.description, "Advanced configuration options");
        assert_eq!(group.parameters, vec!["debug", "verbose", "timeout"]);
        assert_eq!(group.collapsed, Some(true));
        assert_eq!(group.condition, Some("mode == 'expert'".to_string()));
    }

    #[test]
    fn test_parameter_provider_with_groups() {
        let parameters = vec![
            Parameter::new("username", "Username", ParameterType::String)
                .required(true),
            Parameter::new("password", "Password", ParameterType::String)
                .required(true),
            Parameter::new("server", "Server address", ParameterType::String)
                .required(true),
            Parameter::new("port", "Server port", ParameterType::Number)
                .with_default(json!(443)),
            Parameter::new("debug", "Debug mode", ParameterType::Boolean)
                .with_default(json!(false)),
            Parameter::new("timeout", "Request timeout", ParameterType::Number)
                .with_default(json!(30)),
        ];

        let groups = vec![
            ParameterGroup::new("authentication", "Authentication credentials")
                .with_parameters(vec!["username".to_string(), "password".to_string()]),
            ParameterGroup::new("connection", "Connection settings")
                .with_parameters(vec!["server".to_string(), "port".to_string()]),
            ParameterGroup::new("advanced", "Advanced options")
                .with_parameters(vec!["debug".to_string(), "timeout".to_string()]),
        ];

        let workflow = MockWorkflow::new(parameters).with_groups(groups);
        
        // Test parameter grouping
        let grouped = workflow.get_parameters_by_group();
        assert_eq!(grouped.len(), 3);
        
        // Test authentication group
        let auth_params = grouped.get("authentication").unwrap();
        assert_eq!(auth_params.len(), 2);
        assert!(auth_params.iter().any(|p| p.name == "username"));
        assert!(auth_params.iter().any(|p| p.name == "password"));
        
        // Test connection group
        let conn_params = grouped.get("connection").unwrap();
        assert_eq!(conn_params.len(), 2);
        assert!(conn_params.iter().any(|p| p.name == "server"));
        assert!(conn_params.iter().any(|p| p.name == "port"));
        
        // Test advanced group
        let adv_params = grouped.get("advanced").unwrap();
        assert_eq!(adv_params.len(), 2);
        assert!(adv_params.iter().any(|p| p.name == "debug"));
        assert!(adv_params.iter().any(|p| p.name == "timeout"));
    }

    #[test]
    fn test_parameter_provider_with_ungrouped_parameters() {
        let parameters = vec![
            Parameter::new("grouped1", "Grouped parameter 1", ParameterType::String),
            Parameter::new("grouped2", "Grouped parameter 2", ParameterType::String),
            Parameter::new("ungrouped1", "Ungrouped parameter 1", ParameterType::String),
            Parameter::new("ungrouped2", "Ungrouped parameter 2", ParameterType::String),
        ];

        let groups = vec![
            ParameterGroup::new("main", "Main group")
                .with_parameters(vec!["grouped1".to_string(), "grouped2".to_string()]),
        ];

        let workflow = MockWorkflow::new(parameters).with_groups(groups);
        let grouped = workflow.get_parameters_by_group();
        
        assert_eq!(grouped.len(), 2); // main + general
        
        // Test main group
        let main_params = grouped.get("main").unwrap();
        assert_eq!(main_params.len(), 2);
        
        // Test general group (ungrouped parameters)
        let general_params = grouped.get("general").unwrap();
        assert_eq!(general_params.len(), 2);
        assert!(general_params.iter().any(|p| p.name == "ungrouped1"));
        assert!(general_params.iter().any(|p| p.name == "ungrouped2"));
    }

    #[test]
    fn test_parameter_provider_without_groups() {
        let parameters = vec![
            Parameter::new("param1", "Parameter 1", ParameterType::String),
            Parameter::new("param2", "Parameter 2", ParameterType::Boolean),
            Parameter::new("param3", "Parameter 3", ParameterType::Number),
        ];

        let workflow = MockWorkflow::new(parameters);
        let grouped = workflow.get_parameters_by_group();
        
        // All parameters should be in general group
        assert_eq!(grouped.len(), 1);
        let general_params = grouped.get("general").unwrap();
        assert_eq!(general_params.len(), 3);
    }

    #[test]
    fn test_is_parameter_in_any_group() {
        let parameters = vec![
            Parameter::new("grouped", "Grouped parameter", ParameterType::String),
            Parameter::new("ungrouped", "Ungrouped parameter", ParameterType::String),
        ];

        let groups = vec![
            ParameterGroup::new("test_group", "Test group")
                .with_parameter("grouped"),
        ];

        let workflow = MockWorkflow::new(parameters).with_groups(groups);
        
        assert!(workflow.is_parameter_in_any_group("grouped"));
        assert!(!workflow.is_parameter_in_any_group("ungrouped"));
        assert!(!workflow.is_parameter_in_any_group("nonexistent"));
    }
}

#[cfg(test)]
mod grouped_help_generation_tests {
    use super::*;

    #[test]
    fn test_grouped_help_generation() {
        let parameters = vec![
            Parameter::new("username", "Username for authentication", ParameterType::String)
                .required(true),
            Parameter::new("password", "Password for authentication", ParameterType::String)
                .required(true),
            Parameter::new("server", "Server hostname", ParameterType::String)
                .required(true),
            Parameter::new("port", "Server port", ParameterType::Number)
                .with_default(json!(443)),
            Parameter::new("debug", "Enable debug logging", ParameterType::Boolean)
                .with_default(json!(false)),
        ];

        let groups = vec![
            ParameterGroup::new("authentication", "User credentials")
                .with_parameters(vec!["username".to_string(), "password".to_string()]),
            ParameterGroup::new("connection", "Server connection settings")
                .with_parameters(vec!["server".to_string(), "port".to_string()]),
        ];

        let workflow = MockWorkflow::new(parameters).with_groups(groups);
        let help = generate_grouped_help(&workflow);
        
        // Should contain group headers
        assert!(help.contains("AUTHENTICATION:"));
        assert!(help.contains("CONNECTION:"));
        assert!(help.contains("Options:")); // General group
        
        // Should contain group descriptions
        assert!(help.contains("User credentials"));
        assert!(help.contains("Server connection settings"));
        
        // Should contain parameters under correct groups
        let auth_section = help.split("CONNECTION:").next().unwrap();
        assert!(auth_section.contains("--username"));
        assert!(auth_section.contains("--password"));
        
        let conn_section = help.split("Options:").next().unwrap().split("AUTHENTICATION:").skip(1).next().unwrap();
        assert!(conn_section.contains("--server"));
        assert!(conn_section.contains("--port"));
        
        // Should contain ungrouped parameters in Options section
        let options_section = help.split("Options:").skip(1).next().unwrap();
        assert!(options_section.contains("--debug"));
    }

    #[test]
    fn test_grouped_help_with_complex_parameters() {
        let parameters = vec![
            Parameter::new("email", "User email address", ParameterType::String)
                .required(true)
                .with_pattern(CommonPatterns::EMAIL)
                .with_length_range(Some(5), Some(100)),
            Parameter::new("api_key", "API authentication key", ParameterType::String)
                .required(true)
                .with_pattern(CommonPatterns::UUID),
            Parameter::new("environment", "Deployment environment", ParameterType::Choice)
                .required(true)
                .with_choices(vec!["dev".to_string(), "staging".to_string(), "prod".to_string()])
                .with_default(json!("dev")),
            Parameter::new("features", "Features to enable", ParameterType::MultiChoice)
                .with_choices(vec!["auth".to_string(), "metrics".to_string(), "logging".to_string()])
                .with_selection_range(Some(1), Some(3)),
            Parameter::new("ssl_cert", "SSL certificate path", ParameterType::String)
                .when("environment == 'prod'")
                .required(true),
        ];

        let groups = vec![
            ParameterGroup::new("authentication", "Authentication settings")
                .with_parameters(vec!["email".to_string(), "api_key".to_string()]),
            ParameterGroup::new("deployment", "Deployment configuration")
                .with_parameters(vec!["environment".to_string(), "features".to_string(), "ssl_cert".to_string()]),
        ];

        let workflow = MockWorkflow::new(parameters).with_groups(groups);
        let help = generate_grouped_help(&workflow);
        
        // Should contain all validation information
        assert!(help.contains("format: Valid email address"));
        assert!(help.contains("format: Valid UUID v4 identifier"));
        assert!(help.contains("[possible values: dev, staging, prod]"));
        assert!(help.contains("[possible values: auth, metrics, logging]"));
        assert!(help.contains("[when: environment == 'prod']"));
        
        // Should contain constraint information
        assert!(help.contains("min length: 5"));
        assert!(help.contains("max length: 100"));
        assert!(help.contains("min selections: 1"));
        assert!(help.contains("max selections: 3"));
    }

    #[test]
    fn test_grouped_help_empty_groups() {
        let parameters = vec![
            Parameter::new("param1", "Parameter 1", ParameterType::String),
            Parameter::new("param2", "Parameter 2", ParameterType::String),
        ];

        let groups = vec![]; // Empty groups

        let workflow = MockWorkflow::new(parameters).with_groups(groups);
        let help = generate_grouped_help(&workflow);
        
        // Should have only the general Options section
        assert!(help.contains("Options:"));
        assert!(help.contains("--param1"));
        assert!(help.contains("--param2"));
        
        // Should not contain any named groups
        assert!(!help.contains("::\n")); // Group separator pattern
    }

    #[test]
    fn test_grouped_help_with_no_groups() {
        let parameters = vec![
            Parameter::new("simple", "Simple parameter", ParameterType::String)
                .required(true),
            Parameter::new("optional", "Optional parameter", ParameterType::Boolean)
                .with_default(json!(false)),
        ];

        let workflow = MockWorkflow::new(parameters); // No groups
        let help = generate_grouped_help(&workflow);
        
        // Should have only the general Options section
        assert!(help.contains("Options:"));
        assert!(help.contains("--simple"));
        assert!(help.contains("--optional"));
        
        // Should not contain any uppercase group headers
        let lines: Vec<&str> = help.lines().collect();
        for line in lines {
            if line.ends_with(':') && line != "Options:" {
                assert!(!line.chars().any(|c| c.is_uppercase() && c != 'O'), 
                    "Found unexpected group header: {line}");
            }
        }
    }

    #[test]
    fn test_help_formatting_consistency() {
        let param = Parameter::new("test_parameter", "A test parameter with many features", ParameterType::String)
            .required(true)
            .with_pattern(CommonPatterns::EMAIL)
            .with_length_range(Some(10), Some(50))
            .with_default(json!("test@example.com"));

        let help = generate_parameter_help(&param);
        
        // Check formatting consistency
        let lines: Vec<&str> = help.lines().collect();
        assert!(!lines.is_empty(), "Help should contain at least one line");
        
        // First line should contain the parameter switch
        assert!(lines[0].contains("--test-parameter"));
        
        // Description lines should be indented
        let description_lines: Vec<&str> = lines.iter().skip(1).filter(|line| line.contains("A test parameter")).cloned().collect();
        for line in description_lines {
            assert!(line.starts_with("    "), "Description line should be indented: '{line}'");
        }
        
        // Constraint lines should be indented
        let constraint_lines: Vec<&str> = lines.iter().filter(|line| line.contains("[constraints:") || line.contains("[when:")).cloned().collect();
        for line in constraint_lines {
            assert!(line.starts_with("    "), "Constraint line should be indented: '{line}'");
        }
    }
}