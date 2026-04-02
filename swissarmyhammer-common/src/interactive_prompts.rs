//! Interactive parameter prompting system
//!
//! This module provides interactive prompting capabilities for parameters using the dialoguer crate.
//! It handles different parameter types with appropriate UI controls and validation.

use crate::parameters::{
    ErrorMessageEnhancer, Parameter, ParameterError, ParameterResult, ParameterType,
    ParameterValidator,
};
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, MultiSelect};
use std::collections::HashMap;
use std::io::{self, IsTerminal};

/// Interactive prompting system for parameters
pub struct InteractivePrompts {
    /// Whether to disable interactive prompts (for testing or non-TTY environments)
    non_interactive: bool,
    /// Parameter validator for input validation
    validator: ParameterValidator,
    /// Error message enhancer for user-friendly error messages
    error_enhancer: ErrorMessageEnhancer,
    /// Maximum retry attempts for error recovery
    max_attempts: u32,
}

impl InteractivePrompts {
    /// Create a new interactive prompts instance
    pub fn new(non_interactive: bool) -> Self {
        Self {
            non_interactive: non_interactive || !io::stdin().is_terminal(),
            validator: ParameterValidator::new(),
            error_enhancer: ErrorMessageEnhancer::new(),
            max_attempts: 3,
        }
    }

    /// Create a new interactive prompts instance with custom max attempts
    pub fn with_max_attempts(non_interactive: bool, max_attempts: u32) -> Self {
        Self {
            non_interactive: non_interactive || !io::stdin().is_terminal(),
            validator: ParameterValidator::new(),
            error_enhancer: ErrorMessageEnhancer::new(),
            max_attempts,
        }
    }

    /// Prompt for missing parameters interactively
    ///
    /// This method will prompt for any required parameters that are missing from existing_values.
    /// Optional parameters with defaults are automatically resolved.
    /// Supports conditional parameters by evaluating conditions dynamically.
    pub fn prompt_for_parameters(
        &self,
        parameters: &[Parameter],
        existing_values: &HashMap<String, serde_json::Value>,
    ) -> ParameterResult<HashMap<String, serde_json::Value>> {
        let resolved = existing_values.clone();

        // Handle conditional parameters with iterative approach
        self.prompt_conditional_parameters(parameters, resolved)
    }

    /// Prompt for parameters in a simple flat list
    ///
    /// This method prompts for all parameters without grouping.
    pub fn prompt_parameters_by_groups(
        &self,
        provider: &dyn crate::parameters::ParameterProvider,
        existing_values: &HashMap<String, serde_json::Value>,
    ) -> ParameterResult<HashMap<String, serde_json::Value>> {
        let parameters = provider.get_parameters();
        self.prompt_conditional_parameters(parameters, existing_values.clone())
    }

    /// Prompt for conditional parameters using iterative resolution
    pub fn prompt_conditional_parameters(
        &self,
        parameters: &[Parameter],
        mut resolved: HashMap<String, serde_json::Value>,
    ) -> ParameterResult<HashMap<String, serde_json::Value>> {
        use crate::parameter_conditions::ConditionEvaluator;

        let mut changed = true;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 100; // Prevent infinite loops

        while changed && iterations < MAX_ITERATIONS {
            changed = false;
            iterations += 1;

            for param in parameters {
                if resolved.contains_key(&param.name) {
                    // Parameter already provided, validate it
                    if let Some(value) = resolved.get(&param.name) {
                        self.validator.validate_parameter(param, value)?;
                    }
                    continue;
                }

                // Check if this parameter should be included based on its condition
                let should_include = if let Some(condition) = &param.condition {
                    let evaluator = ConditionEvaluator::new(resolved.clone());
                    match evaluator.evaluate(&condition.expression) {
                        Ok(result) => result,
                        Err(_) => {
                            // Condition references parameters we don't have yet, skip for now
                            continue;
                        }
                    }
                } else {
                    true // No condition means always include
                };

                if should_include {
                    // Check if we can use a default value first, regardless of whether it's required
                    if let Some(default) = &param.default {
                        // Use default value for parameters when condition is met
                        resolved.insert(param.name.clone(), default.clone());
                        changed = true;
                    } else if param.required && !self.non_interactive {
                        // Only prompt for required parameters without defaults
                        // Show conditional explanation if available
                        if let Some(condition) = &param.condition {
                            println!(
                                "📋 {} (required because: {})",
                                param.description,
                                self.format_condition_explanation(condition)
                            );
                        }

                        let value = self.prompt_for_parameter(param)?;
                        resolved.insert(param.name.clone(), value);
                        changed = true;
                    } else if param.required && self.non_interactive {
                        return Err(ParameterError::MissingRequired {
                            name: param.name.clone(),
                        });
                    }
                    // If it's not required and has no default, we simply don't include it
                }
            }
        }

        if iterations >= MAX_ITERATIONS {
            return Err(ParameterError::ValidationFailed {
                message: "Too many iterations resolving conditional parameters - possible circular dependency".to_string(),
            });
        }

        Ok(resolved)
    }

    /// Format a condition explanation for user display
    fn format_condition_explanation(
        &self,
        condition: &crate::parameter_conditions::ParameterCondition,
    ) -> String {
        if let Some(desc) = &condition.description {
            desc.clone()
        } else {
            format!("condition '{}' is met", condition.expression)
        }
    }

    /// Prompt for a single parameter based on its type with error recovery
    fn prompt_for_parameter(&self, param: &Parameter) -> ParameterResult<serde_json::Value> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

        self.prompt_with_error_recovery(param)
    }

    /// Prompt for a parameter with enhanced error recovery and retry logic
    pub fn prompt_with_error_recovery(
        &self,
        param: &Parameter,
    ) -> ParameterResult<serde_json::Value> {
        let mut attempts = 0;

        loop {
            attempts += 1;

            let result = match param.parameter_type {
                ParameterType::String => {
                    let value = self.prompt_string_single_attempt(param)?;
                    serde_json::Value::String(value)
                }
                ParameterType::Boolean => {
                    let value = self.prompt_boolean(param)?;
                    serde_json::Value::Bool(value)
                }
                ParameterType::Number => {
                    let value = self.prompt_number_single_attempt(param)?;
                    serde_json::Value::Number(serde_json::Number::from_f64(value).ok_or_else(
                        || ParameterError::ValidationFailed {
                            message: format!("Invalid number value: {value}"),
                        },
                    )?)
                }
                ParameterType::Choice => {
                    let value = self.prompt_choice_single_attempt(param)?;
                    serde_json::Value::String(value)
                }
                ParameterType::MultiChoice => {
                    let values = self.prompt_multi_choice_single_attempt(param)?;
                    serde_json::Value::Array(
                        values.into_iter().map(serde_json::Value::String).collect(),
                    )
                }
            };

            // Validate the input
            match self.validator.validate_parameter(param, &result) {
                Ok(_) => return Ok(result),
                Err(error) => {
                    if attempts >= self.max_attempts {
                        println!("✗ Maximum attempts reached. Use --help for parameter details.");
                        return Err(ParameterError::MaxAttemptsExceeded {
                            parameter: param.name.clone(),
                            attempts,
                        });
                    }

                    // Enhance and display the error
                    let enhanced_error = self.error_enhancer.enhance_parameter_error(&error);
                    self.display_enhanced_error(&enhanced_error);

                    println!("Please try again ({}/{}):", attempts, self.max_attempts);
                }
            }
        }
    }

    /// Display an enhanced error message with context and suggestions
    fn display_enhanced_error(&self, error: &ParameterError) {
        match error {
            ParameterError::ValidationFailedWithContext { details, .. } => {
                println!("✗ {}", details.message);

                if let Some(explanation) = &details.explanation {
                    println!("   {explanation}");
                }

                if !details.examples.is_empty() {
                    println!("   Examples: {}", details.examples.join(", "));
                }

                for suggestion in &details.suggestions {
                    println!("💡 {suggestion}");
                }
            }

            ParameterError::PatternMismatchEnhanced {
                parameter, details, ..
            } => {
                println!(
                    "✗ Parameter '{parameter}' format is invalid: '{}'",
                    details.value
                );
                println!("   {}", details.pattern_description);

                if !details.examples.is_empty() && details.examples.len() <= 3 {
                    println!("   Examples: {}", details.examples.join(", "));
                } else if !details.examples.is_empty() {
                    println!("   Examples: {}", details.examples[..2].join(", "));
                }
            }

            ParameterError::InvalidChoiceEnhanced {
                parameter, details, ..
            } => {
                println!(
                    "✗ Parameter '{parameter}' has invalid value: '{}'",
                    details.value
                );

                if let Some(suggestion) = &details.did_you_mean {
                    println!("💡 Did you mean '{suggestion}'?");
                } else if details.choices.len() <= 5 {
                    println!("💡 Valid options: {}", details.choices.join(", "));
                } else {
                    println!("💡 {} options available", details.choices.len());
                }
            }

            _ => {
                // Fallback to basic error display
                println!("✗ {error}");
                self.print_validation_hints_for_error(error);
            }
        }
    }

    /// Print validation hints for errors that don't have enhanced context
    fn print_validation_hints_for_error(&self, error: &ParameterError) {
        match error {
            ParameterError::StringTooShort { min_length, .. } => {
                println!("💡 Must be at least {min_length} characters long");
            }
            ParameterError::StringTooLong { max_length, .. } => {
                println!("💡 Must be at most {max_length} characters long");
            }
            ParameterError::OutOfRange { min, max, .. } => {
                if let (Some(min_val), Some(max_val)) = (min, max) {
                    println!("💡 Value must be between {min_val} and {max_val}");
                } else if let Some(min_val) = min {
                    println!("💡 Value must be at least {min_val}");
                } else if let Some(max_val) = max {
                    println!("💡 Value must be at most {max_val}");
                }
            }
            _ => {}
        }
    }

    /// Prompt for a string parameter with validation
    pub fn prompt_string(&self, param: &Parameter) -> ParameterResult<String> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

        self.prompt_string_single_attempt(param)
    }

    /// Prompt for a string parameter without retry logic (single attempt)
    fn prompt_string_single_attempt(&self, param: &Parameter) -> ParameterResult<String> {
        let theme = ColorfulTheme::default();

        let mut input_prompt = Input::<String>::with_theme(&theme)
            .with_prompt(format!("Enter {} ({})", param.name, param.description));

        // Add default value if available
        if let Some(default) = &param.default {
            if let Some(default_str) = default.as_str() {
                input_prompt = input_prompt.default(default_str.to_string());
            }
        }

        let input = input_prompt
            .interact()
            .map_err(|e| ParameterError::ValidationFailed {
                message: format!("Failed to read input: {e}"),
            })?;

        Ok(input)
    }

    /// Prompt for a boolean parameter
    pub fn prompt_boolean(&self, param: &Parameter) -> ParameterResult<bool> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

        let theme = ColorfulTheme::default();
        let mut confirm_prompt = Confirm::with_theme(&theme)
            .with_prompt(format!("{} ({})", param.name, param.description));

        // Set default value if available
        if let Some(default) = &param.default {
            if let Some(default_bool) = default.as_bool() {
                confirm_prompt = confirm_prompt.default(default_bool);
            }
        }

        confirm_prompt
            .interact()
            .map_err(|e| ParameterError::ValidationFailed {
                message: format!("Failed to read input: {e}"),
            })
    }

    /// Prompt for a numeric parameter with validation
    pub fn prompt_number(&self, param: &Parameter) -> ParameterResult<f64> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

        self.prompt_number_single_attempt(param)
    }

    /// Prompt for a numeric parameter without retry logic (single attempt)
    fn prompt_number_single_attempt(&self, param: &Parameter) -> ParameterResult<f64> {
        let theme = ColorfulTheme::default();

        let mut prompt_text = format!("Enter {} ({})", param.name, param.description);
        if let Some(validation) = &param.validation {
            if let (Some(min), Some(max)) = (validation.min, validation.max) {
                prompt_text = format!("{prompt_text} [{min}-{max}]");
            } else if let Some(min) = validation.min {
                prompt_text = format!("{prompt_text} [>= {min}]");
            } else if let Some(max) = validation.max {
                prompt_text = format!("{prompt_text} [<= {max}]");
            }
        }

        let mut input_prompt = Input::<String>::with_theme(&theme).with_prompt(prompt_text);

        // Add default value if available
        if let Some(default) = &param.default {
            if let Some(default_num) = default.as_f64() {
                input_prompt = input_prompt.default(default_num.to_string());
            }
        }

        let input = input_prompt
            .interact()
            .map_err(|e| ParameterError::ValidationFailed {
                message: format!("Failed to read input: {e}"),
            })?;

        // Parse the number
        input
            .parse::<f64>()
            .map_err(|_| ParameterError::TypeMismatch {
                name: param.name.clone(),
                expected_type: "number".to_string(),
                actual_type: "invalid number format".to_string(),
            })
    }

    /// Prompt for a choice parameter using fuzzy selection
    pub fn prompt_choice(&self, param: &Parameter) -> ParameterResult<String> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

        self.prompt_choice_single_attempt(param)
    }

    /// Prompt for a choice parameter without retry logic (single attempt)
    fn prompt_choice_single_attempt(&self, param: &Parameter) -> ParameterResult<String> {
        let choices = param
            .choices
            .as_ref()
            .ok_or_else(|| ParameterError::ValidationFailed {
                message: format!("Choice parameter '{}' has no choices defined", param.name),
            })?;

        if choices.is_empty() {
            return Err(ParameterError::ValidationFailed {
                message: format!("Choice parameter '{}' has empty choices list", param.name),
            });
        }

        let theme = ColorfulTheme::default();
        let mut select_prompt = FuzzySelect::with_theme(&theme)
            .with_prompt(format!("Select {} ({})", param.name, param.description))
            .items(choices);

        // Set default selection if available
        if let Some(default) = &param.default {
            if let Some(default_str) = default.as_str() {
                if let Some(index) = choices.iter().position(|x| x == default_str) {
                    select_prompt = select_prompt.default(index);
                }
            }
        }

        let selection = select_prompt
            .interact()
            .map_err(|e| ParameterError::ValidationFailed {
                message: format!("Failed to read selection: {e}"),
            })?;

        Ok(choices[selection].clone())
    }

    /// Prompt for multiple choice selection
    pub fn prompt_multi_choice(&self, param: &Parameter) -> ParameterResult<Vec<String>> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

        self.prompt_multi_choice_single_attempt(param)
    }

    /// Prompt for multiple choice selection without retry logic (single attempt)
    fn prompt_multi_choice_single_attempt(
        &self,
        param: &Parameter,
    ) -> ParameterResult<Vec<String>> {
        let choices = param
            .choices
            .as_ref()
            .ok_or_else(|| ParameterError::ValidationFailed {
                message: format!(
                    "MultiChoice parameter '{}' has no choices defined",
                    param.name
                ),
            })?;

        if choices.is_empty() {
            return Err(ParameterError::ValidationFailed {
                message: format!(
                    "MultiChoice parameter '{}' has empty choices list",
                    param.name
                ),
            });
        }

        let theme = ColorfulTheme::default();
        let mut multi_select = MultiSelect::with_theme(&theme)
            .with_prompt(format!(
                "Select {} (use space to select, enter to confirm) ({})",
                param.name, param.description
            ))
            .items(choices);

        // Set default selections if available
        if let Some(default) = &param.default {
            if let Some(default_array) = default.as_array() {
                let mut defaults = vec![false; choices.len()];
                for default_item in default_array {
                    if let Some(default_str) = default_item.as_str() {
                        if let Some(index) = choices.iter().position(|x| x == default_str) {
                            defaults[index] = true;
                        }
                    }
                }
                multi_select = multi_select.defaults(&defaults);
            }
        }

        let selections = multi_select
            .interact()
            .map_err(|e| ParameterError::ValidationFailed {
                message: format!("Failed to read selections: {e}"),
            })?;

        let selected_values: Vec<String> = selections.iter().map(|&i| choices[i].clone()).collect();

        Ok(selected_values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parameters::{
        InvalidChoiceDetails, PatternMismatchDetails, ValidationFailedDetails,
    };

    #[test]
    fn test_new_interactive_prompts_non_interactive() {
        let prompts = InteractivePrompts::new(true);
        assert!(prompts.non_interactive);
    }

    #[test]
    fn test_prompt_for_parameters_with_existing_values() {
        let prompts = InteractivePrompts::new(true);

        let param =
            Parameter::new("test_param", "Test parameter", ParameterType::String).required(true);
        let parameters = vec![param];

        let mut existing = HashMap::new();
        existing.insert(
            "test_param".to_string(),
            serde_json::Value::String("existing_value".to_string()),
        );

        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("test_param").unwrap(),
            &serde_json::Value::String("existing_value".to_string())
        );
    }

    #[test]
    fn test_prompt_for_parameters_with_defaults() {
        let prompts = InteractivePrompts::new(true);

        let param = Parameter::new(
            "optional_param",
            "Optional parameter",
            ParameterType::String,
        )
        .with_default(serde_json::Value::String("default_value".to_string()));
        let parameters = vec![param];

        let existing = HashMap::new();

        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("optional_param").unwrap(),
            &serde_json::Value::String("default_value".to_string())
        );
    }

    #[test]
    fn test_prompt_for_parameters_missing_required_non_interactive() {
        let prompts = InteractivePrompts::new(true);

        let param = Parameter::new(
            "required_param",
            "Required parameter",
            ParameterType::String,
        )
        .required(true);
        let parameters = vec![param];

        let existing = HashMap::new();

        let result = prompts.prompt_for_parameters(&parameters, &existing);
        assert!(result.is_err());

        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "required_param");
        } else {
            panic!("Expected MissingRequired error");
        }
    }

    #[test]
    fn test_prompt_for_parameters_validation_error() {
        let prompts = InteractivePrompts::new(true);

        let param = Parameter::new("choice_param", "Choice parameter", ParameterType::Choice)
            .with_choices(vec!["option1".to_string(), "option2".to_string()]);
        let parameters = vec![param];

        let mut existing = HashMap::new();
        existing.insert(
            "choice_param".to_string(),
            serde_json::Value::String("invalid_choice".to_string()),
        );

        let result = prompts.prompt_for_parameters(&parameters, &existing);
        assert!(result.is_err());

        if let Err(ParameterError::InvalidChoice {
            name,
            value,
            choices,
        }) = result
        {
            assert_eq!(name, "choice_param");
            assert_eq!(value, "invalid_choice");
            assert_eq!(choices, vec!["option1", "option2"]);
        } else {
            panic!("Expected InvalidChoice error");
        }
    }

    #[test]
    fn test_prompt_choice_no_choices() {
        let prompts = InteractivePrompts::new(true);
        let param = Parameter::new("bad_choice", "Bad choice parameter", ParameterType::Choice);

        let result = prompts.prompt_choice(&param);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_multi_choice_no_choices() {
        let prompts = InteractivePrompts::new(true);
        let param = Parameter::new(
            "bad_multi_choice",
            "Bad multi choice parameter",
            ParameterType::MultiChoice,
        );

        let result = prompts.prompt_multi_choice(&param);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_choice_empty_choices() {
        let prompts = InteractivePrompts::new(true);
        let param = Parameter::new(
            "empty_choice",
            "Empty choice parameter",
            ParameterType::Choice,
        )
        .with_choices(vec![]);

        let result = prompts.prompt_choice(&param);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_multi_choice_empty_choices() {
        let prompts = InteractivePrompts::new(true);
        let param = Parameter::new(
            "empty_multi_choice",
            "Empty multi choice parameter",
            ParameterType::MultiChoice,
        )
        .with_choices(vec![]);

        let result = prompts.prompt_multi_choice(&param);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_conditional_parameters_basic() {
        use crate::parameter_conditions::ParameterCondition;

        let prompts = InteractivePrompts::new(true);

        // Create conditional parameters
        let deploy_env = Parameter::new(
            "deploy_env",
            "Deployment environment",
            crate::parameters::ParameterType::String,
        )
        .required(true);

        let prod_confirmation = Parameter::new(
            "prod_confirmation",
            "Production confirmation",
            crate::parameters::ParameterType::Boolean,
        )
        .required(true)
        .with_condition(ParameterCondition::new("deploy_env == 'prod'"));

        let parameters = vec![deploy_env, prod_confirmation];

        // Test with existing deploy_env = dev (should not require prod_confirmation)
        let mut existing = HashMap::new();
        existing.insert(
            "deploy_env".to_string(),
            serde_json::Value::String("dev".to_string()),
        );

        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("deploy_env").unwrap(),
            &serde_json::Value::String("dev".to_string())
        );
        assert!(!result.contains_key("prod_confirmation"));
    }

    #[test]
    fn test_prompt_conditional_parameters_with_defaults() {
        let prompts = InteractivePrompts::new(true);

        let enable_ssl = Parameter::new(
            "enable_ssl",
            "Enable SSL",
            crate::parameters::ParameterType::Boolean,
        )
        .with_default(serde_json::json!(false))
        .required(false);

        let cert_path = Parameter::new(
            "cert_path",
            "SSL certificate path",
            crate::parameters::ParameterType::String,
        )
        .required(true)
        .when("enable_ssl == true")
        .with_default(serde_json::json!("/etc/ssl/cert.pem"));

        let parameters = vec![enable_ssl, cert_path];

        // Test 1: No existing values, should use defaults and not require cert_path
        let existing = HashMap::new();
        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("enable_ssl").unwrap(), &serde_json::json!(false));
        assert!(!result.contains_key("cert_path"));

        // Test 2: enable_ssl = true provided, should use cert_path default
        let mut existing = HashMap::new();
        existing.insert("enable_ssl".to_string(), serde_json::Value::Bool(true));

        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("enable_ssl").unwrap(), &serde_json::json!(true));
        assert_eq!(
            result.get("cert_path").unwrap(),
            &serde_json::json!("/etc/ssl/cert.pem")
        );
    }

    #[test]
    fn test_format_condition_explanation() {
        use crate::parameter_conditions::ParameterCondition;

        let prompts = InteractivePrompts::new(true);

        // Test with custom description
        let condition_with_desc = ParameterCondition::new("env == 'prod'")
            .with_description("production environment is selected");
        let explanation = prompts.format_condition_explanation(&condition_with_desc);
        assert_eq!(explanation, "production environment is selected");

        // Test without description
        let condition_without_desc = ParameterCondition::new("enable_ssl == true");
        let explanation = prompts.format_condition_explanation(&condition_without_desc);
        assert_eq!(explanation, "condition 'enable_ssl == true' is met");
    }

    #[test]
    fn test_with_max_attempts_constructor() {
        let prompts = InteractivePrompts::with_max_attempts(true, 5);
        assert!(prompts.non_interactive);
        assert_eq!(prompts.max_attempts, 5);
    }

    #[test]
    fn test_with_max_attempts_zero() {
        let prompts = InteractivePrompts::with_max_attempts(true, 0);
        assert_eq!(prompts.max_attempts, 0);
    }

    #[test]
    fn test_prompt_for_parameter_non_interactive_returns_missing_required() {
        let prompts = InteractivePrompts::new(true);

        let param =
            Parameter::new("my_param", "A test param", ParameterType::String).required(true);

        let result = prompts.prompt_for_parameter(&param);
        assert!(result.is_err());
        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "my_param");
        } else {
            panic!("Expected MissingRequired error");
        }
    }

    #[test]
    fn test_prompt_string_non_interactive() {
        let prompts = InteractivePrompts::new(true);
        let param =
            Parameter::new("str_param", "A string param", ParameterType::String).required(true);

        let result = prompts.prompt_string(&param);
        assert!(result.is_err());
        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "str_param");
        } else {
            panic!("Expected MissingRequired error");
        }
    }

    #[test]
    fn test_prompt_boolean_non_interactive() {
        let prompts = InteractivePrompts::new(true);
        let param =
            Parameter::new("bool_param", "A boolean param", ParameterType::Boolean).required(true);

        let result = prompts.prompt_boolean(&param);
        assert!(result.is_err());
        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "bool_param");
        } else {
            panic!("Expected MissingRequired error");
        }
    }

    #[test]
    fn test_prompt_number_non_interactive() {
        let prompts = InteractivePrompts::new(true);
        let param =
            Parameter::new("num_param", "A number param", ParameterType::Number).required(true);

        let result = prompts.prompt_number(&param);
        assert!(result.is_err());
        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "num_param");
        } else {
            panic!("Expected MissingRequired error");
        }
    }

    #[test]
    fn test_prompt_choice_non_interactive() {
        let prompts = InteractivePrompts::new(true);
        let param = Parameter::new("choice_param", "A choice param", ParameterType::Choice)
            .with_choices(vec!["a".to_string(), "b".to_string()])
            .required(true);

        let result = prompts.prompt_choice(&param);
        assert!(result.is_err());
        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "choice_param");
        } else {
            panic!("Expected MissingRequired error");
        }
    }

    #[test]
    fn test_prompt_multi_choice_non_interactive() {
        let prompts = InteractivePrompts::new(true);
        let param = Parameter::new(
            "multi_param",
            "A multi-choice param",
            ParameterType::MultiChoice,
        )
        .with_choices(vec!["x".to_string(), "y".to_string()])
        .required(true);

        let result = prompts.prompt_multi_choice(&param);
        assert!(result.is_err());
        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "multi_param");
        } else {
            panic!("Expected MissingRequired error");
        }
    }

    #[test]
    fn test_prompt_with_error_recovery_non_interactive_string() {
        // prompt_with_error_recovery dispatches based on parameter_type; in non-interactive
        // mode the inner prompt_*_single_attempt methods will fail trying to interact.
        // Since prompt_with_error_recovery is pub, we test the non-interactive guard indirectly
        // via prompt_for_parameter which calls it after checking non_interactive.
        let prompts = InteractivePrompts::new(true);
        let param =
            Parameter::new("recovery_param", "Recovery test", ParameterType::String).required(true);

        let result = prompts.prompt_for_parameter(&param);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParameterError::MissingRequired { .. }
        ));
    }

    #[test]
    fn test_display_enhanced_error_validation_failed_with_context() {
        let prompts = InteractivePrompts::new(true);

        // ValidationFailedWithContext — verify it does not panic
        let error = ParameterError::ValidationFailedWithContext {
            parameter: "email".to_string(),
            details: Box::new(ValidationFailedDetails {
                value: "bad".to_string(),
                message: "Invalid email format".to_string(),
                explanation: Some("Must contain @ and domain".to_string()),
                examples: vec!["user@example.com".to_string()],
                suggestions: vec!["Add an @ symbol".to_string()],
            }),
            recoverable: true,
        };
        // Should not panic
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_display_enhanced_error_validation_failed_with_context_no_explanation() {
        let prompts = InteractivePrompts::new(true);

        let error = ParameterError::ValidationFailedWithContext {
            parameter: "name".to_string(),
            details: Box::new(ValidationFailedDetails {
                value: "".to_string(),
                message: "Name cannot be empty".to_string(),
                explanation: None,
                examples: vec![],
                suggestions: vec![],
            }),
            recoverable: true,
        };
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_display_enhanced_error_pattern_mismatch() {
        let prompts = InteractivePrompts::new(true);

        // PatternMismatchEnhanced with few examples
        let error = ParameterError::PatternMismatchEnhanced {
            parameter: "version".to_string(),
            details: Box::new(PatternMismatchDetails {
                value: "abc".to_string(),
                pattern: r"^\d+\.\d+\.\d+$".to_string(),
                pattern_description: "Semantic version (X.Y.Z)".to_string(),
                examples: vec!["1.0.0".to_string(), "2.3.4".to_string()],
            }),
            recoverable: true,
        };
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_display_enhanced_error_pattern_mismatch_many_examples() {
        let prompts = InteractivePrompts::new(true);

        // PatternMismatchEnhanced with more than 3 examples (triggers truncation branch)
        let error = ParameterError::PatternMismatchEnhanced {
            parameter: "code".to_string(),
            details: Box::new(PatternMismatchDetails {
                value: "bad".to_string(),
                pattern: r"^[A-Z]{3}$".to_string(),
                pattern_description: "Three uppercase letters".to_string(),
                examples: vec![
                    "ABC".to_string(),
                    "DEF".to_string(),
                    "GHI".to_string(),
                    "JKL".to_string(),
                ],
            }),
            recoverable: true,
        };
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_display_enhanced_error_pattern_mismatch_empty_examples() {
        let prompts = InteractivePrompts::new(true);

        let error = ParameterError::PatternMismatchEnhanced {
            parameter: "field".to_string(),
            details: Box::new(PatternMismatchDetails {
                value: "x".to_string(),
                pattern: r"^\d+$".to_string(),
                pattern_description: "Digits only".to_string(),
                examples: vec![],
            }),
            recoverable: true,
        };
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_display_enhanced_error_invalid_choice_with_suggestion() {
        let prompts = InteractivePrompts::new(true);

        let error = ParameterError::InvalidChoiceEnhanced {
            parameter: "env".to_string(),
            details: Box::new(InvalidChoiceDetails {
                value: "prodution".to_string(),
                choices: vec!["production".to_string(), "staging".to_string()],
                did_you_mean: Some("production".to_string()),
            }),
            recoverable: true,
        };
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_display_enhanced_error_invalid_choice_few_options() {
        let prompts = InteractivePrompts::new(true);

        // No did_you_mean, 5 or fewer choices => prints all options
        let error = ParameterError::InvalidChoiceEnhanced {
            parameter: "size".to_string(),
            details: Box::new(InvalidChoiceDetails {
                value: "huge".to_string(),
                choices: vec![
                    "small".to_string(),
                    "medium".to_string(),
                    "large".to_string(),
                ],
                did_you_mean: None,
            }),
            recoverable: true,
        };
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_display_enhanced_error_invalid_choice_many_options() {
        let prompts = InteractivePrompts::new(true);

        // No did_you_mean, more than 5 choices => prints count
        let error = ParameterError::InvalidChoiceEnhanced {
            parameter: "color".to_string(),
            details: Box::new(InvalidChoiceDetails {
                value: "purple".to_string(),
                choices: vec![
                    "red".to_string(),
                    "green".to_string(),
                    "blue".to_string(),
                    "yellow".to_string(),
                    "orange".to_string(),
                    "pink".to_string(),
                ],
                did_you_mean: None,
            }),
            recoverable: true,
        };
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_display_enhanced_error_fallback() {
        let prompts = InteractivePrompts::new(true);

        // A basic error variant triggers the fallback branch
        let error = ParameterError::ValidationFailed {
            message: "something went wrong".to_string(),
        };
        prompts.display_enhanced_error(&error);
    }

    #[test]
    fn test_print_validation_hints_string_too_short() {
        let prompts = InteractivePrompts::new(true);

        let error = ParameterError::StringTooShort {
            name: "password".to_string(),
            min_length: 8,
            actual_length: 3,
        };
        // Should print hint without panicking
        prompts.print_validation_hints_for_error(&error);
    }

    #[test]
    fn test_print_validation_hints_string_too_long() {
        let prompts = InteractivePrompts::new(true);

        let error = ParameterError::StringTooLong {
            name: "username".to_string(),
            max_length: 20,
            actual_length: 25,
        };
        prompts.print_validation_hints_for_error(&error);
    }

    #[test]
    fn test_print_validation_hints_out_of_range_both_bounds() {
        let prompts = InteractivePrompts::new(true);

        let error = ParameterError::OutOfRange {
            name: "age".to_string(),
            value: 200.0,
            min: Some(0.0),
            max: Some(150.0),
        };
        prompts.print_validation_hints_for_error(&error);
    }

    #[test]
    fn test_print_validation_hints_out_of_range_min_only() {
        let prompts = InteractivePrompts::new(true);

        let error = ParameterError::OutOfRange {
            name: "count".to_string(),
            value: -1.0,
            min: Some(0.0),
            max: None,
        };
        prompts.print_validation_hints_for_error(&error);
    }

    #[test]
    fn test_print_validation_hints_out_of_range_max_only() {
        let prompts = InteractivePrompts::new(true);

        let error = ParameterError::OutOfRange {
            name: "discount".to_string(),
            value: 101.0,
            min: None,
            max: Some(100.0),
        };
        prompts.print_validation_hints_for_error(&error);
    }

    #[test]
    fn test_print_validation_hints_other_error_no_panic() {
        let prompts = InteractivePrompts::new(true);

        // The catch-all branch should do nothing
        let error = ParameterError::MissingRequired {
            name: "foo".to_string(),
        };
        prompts.print_validation_hints_for_error(&error);
    }

    #[test]
    fn test_prompt_for_parameters_optional_no_default_skipped() {
        let prompts = InteractivePrompts::new(true);

        // An optional parameter with no default should simply not appear in results
        let param = Parameter::new("optional", "Optional param", ParameterType::String);
        let parameters = vec![param];

        let existing = HashMap::new();
        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn test_prompt_for_parameters_multiple_defaults_resolved() {
        let prompts = InteractivePrompts::new(true);

        let param1 = Parameter::new("host", "Hostname", ParameterType::String)
            .with_default(serde_json::json!("localhost"));
        let param2 = Parameter::new("port", "Port number", ParameterType::Number)
            .with_default(serde_json::json!(8080));

        let parameters = vec![param1, param2];
        let existing = HashMap::new();

        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("host").unwrap(), &serde_json::json!("localhost"));
        assert_eq!(result.get("port").unwrap(), &serde_json::json!(8080));
    }

    #[test]
    fn test_prompt_for_parameters_existing_overrides_default() {
        let prompts = InteractivePrompts::new(true);

        let param = Parameter::new("host", "Hostname", ParameterType::String)
            .with_default(serde_json::json!("localhost"));
        let parameters = vec![param];

        let mut existing = HashMap::new();
        existing.insert("host".to_string(), serde_json::json!("example.com"));

        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("host").unwrap(),
            &serde_json::json!("example.com")
        );
    }

    #[test]
    fn test_prompt_conditional_parameters_condition_not_met_skips() {
        use crate::parameter_conditions::ParameterCondition;

        let prompts = InteractivePrompts::new(true);

        let mode = Parameter::new("mode", "Mode", ParameterType::String)
            .with_default(serde_json::json!("simple"));

        // This parameter has a condition that won't be met
        let advanced_opt = Parameter::new("advanced_opt", "Advanced option", ParameterType::String)
            .required(true)
            .with_condition(ParameterCondition::new("mode == 'advanced'"));

        let parameters = vec![mode, advanced_opt];
        let existing = HashMap::new();

        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("mode").unwrap(), &serde_json::json!("simple"));
        assert!(!result.contains_key("advanced_opt"));
    }

    #[test]
    fn test_prompt_conditional_parameters_condition_met_uses_default() {
        use crate::parameter_conditions::ParameterCondition;

        let prompts = InteractivePrompts::new(true);

        let mode = Parameter::new("mode", "Mode", ParameterType::String);
        let advanced_opt = Parameter::new("advanced_opt", "Advanced option", ParameterType::String)
            .required(true)
            .with_condition(ParameterCondition::new("mode == 'advanced'"))
            .with_default(serde_json::json!("default_advanced"));

        let parameters = vec![mode, advanced_opt];

        let mut existing = HashMap::new();
        existing.insert("mode".to_string(), serde_json::json!("advanced"));

        let result = prompts
            .prompt_for_parameters(&parameters, &existing)
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(
            result.get("advanced_opt").unwrap(),
            &serde_json::json!("default_advanced")
        );
    }

    #[test]
    fn test_prompt_conditional_parameters_condition_met_no_default_non_interactive_errors() {
        use crate::parameter_conditions::ParameterCondition;

        let prompts = InteractivePrompts::new(true);

        let mode = Parameter::new("mode", "Mode", ParameterType::String);
        let required_opt = Parameter::new("required_opt", "Required option", ParameterType::String)
            .required(true)
            .with_condition(ParameterCondition::new("mode == 'advanced'"));

        let parameters = vec![mode, required_opt];

        let mut existing = HashMap::new();
        existing.insert("mode".to_string(), serde_json::json!("advanced"));

        let result = prompts.prompt_for_parameters(&parameters, &existing);
        assert!(result.is_err());
        if let Err(ParameterError::MissingRequired { name }) = result {
            assert_eq!(name, "required_opt");
        } else {
            panic!("Expected MissingRequired error for required_opt");
        }
    }

    #[test]
    fn test_prompt_parameters_by_groups_delegates_correctly() {
        use crate::parameters::ParameterProvider;

        struct TestProvider {
            params: Vec<Parameter>,
        }

        impl ParameterProvider for TestProvider {
            fn get_parameters(&self) -> &[Parameter] {
                &self.params
            }
        }

        let prompts = InteractivePrompts::new(true);
        let provider = TestProvider {
            params: vec![Parameter::new("a", "Param A", ParameterType::String)
                .with_default(serde_json::json!("alpha"))],
        };

        let existing = HashMap::new();
        let result = prompts
            .prompt_parameters_by_groups(&provider, &existing)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("a").unwrap(), &serde_json::json!("alpha"));
    }

    #[test]
    fn test_prompt_choice_single_attempt_no_choices_returns_error() {
        let prompts = InteractivePrompts::new(false); // interactive=true but still errors on no choices

        let param = Parameter::new("pick", "Pick one", ParameterType::Choice);

        let result = prompts.prompt_choice_single_attempt(&param);
        assert!(result.is_err());
        if let Err(ParameterError::ValidationFailed { message }) = &result {
            assert!(message.contains("no choices defined"));
        } else {
            panic!("Expected ValidationFailed error about no choices");
        }
    }

    #[test]
    fn test_prompt_choice_single_attempt_empty_choices_returns_error() {
        let prompts = InteractivePrompts::new(false);

        let param = Parameter::new("pick", "Pick one", ParameterType::Choice).with_choices(vec![]);

        let result = prompts.prompt_choice_single_attempt(&param);
        assert!(result.is_err());
        if let Err(ParameterError::ValidationFailed { message }) = &result {
            assert!(message.contains("empty choices list"));
        } else {
            panic!("Expected ValidationFailed error about empty choices");
        }
    }

    #[test]
    fn test_prompt_multi_choice_single_attempt_no_choices_returns_error() {
        let prompts = InteractivePrompts::new(false);

        let param = Parameter::new("multi", "Multi pick", ParameterType::MultiChoice);

        let result = prompts.prompt_multi_choice_single_attempt(&param);
        assert!(result.is_err());
        if let Err(ParameterError::ValidationFailed { message }) = &result {
            assert!(message.contains("no choices defined"));
        } else {
            panic!("Expected ValidationFailed error about no choices");
        }
    }

    #[test]
    fn test_prompt_multi_choice_single_attempt_empty_choices_returns_error() {
        let prompts = InteractivePrompts::new(false);

        let param =
            Parameter::new("multi", "Multi pick", ParameterType::MultiChoice).with_choices(vec![]);

        let result = prompts.prompt_multi_choice_single_attempt(&param);
        assert!(result.is_err());
        if let Err(ParameterError::ValidationFailed { message }) = &result {
            assert!(message.contains("empty choices list"));
        } else {
            panic!("Expected ValidationFailed error about empty choices");
        }
    }
}
