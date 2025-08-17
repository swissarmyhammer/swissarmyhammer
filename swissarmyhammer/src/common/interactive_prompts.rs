//! Interactive parameter prompting system
//!
//! This module provides interactive prompting capabilities for parameters using the dialoguer crate.
//! It handles different parameter types with appropriate UI controls and validation.

use crate::common::parameters::{
    CommonPatterns, Parameter, ParameterError, ParameterResult, ParameterType, ParameterValidator,
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
}

impl InteractivePrompts {
    /// Create a new interactive prompts instance
    pub fn new(non_interactive: bool) -> Self {
        Self {
            non_interactive: non_interactive || !io::stdin().is_terminal(),
            validator: ParameterValidator::new(),
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

    /// Prompt for conditional parameters using iterative resolution
    pub fn prompt_conditional_parameters(
        &self,
        parameters: &[Parameter],
        mut resolved: HashMap<String, serde_json::Value>,
    ) -> ParameterResult<HashMap<String, serde_json::Value>> {
        use crate::common::parameter_conditions::ConditionEvaluator;
        
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
                            println!("📋 {} (required because: {})", 
                                param.description, 
                                self.format_condition_explanation(condition));
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
    fn format_condition_explanation(&self, condition: &crate::common::parameter_conditions::ParameterCondition) -> String {
        if let Some(desc) = &condition.description {
            desc.clone()
        } else {
            format!("condition '{}' is met", condition.expression)
        }
    }

    /// Prompt for a single parameter based on its type
    fn prompt_for_parameter(&self, param: &Parameter) -> ParameterResult<serde_json::Value> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

        let result = match param.parameter_type {
            ParameterType::String => {
                let value = self.prompt_string(param)?;
                serde_json::Value::String(value)
            }
            ParameterType::Boolean => {
                let value = self.prompt_boolean(param)?;
                serde_json::Value::Bool(value)
            }
            ParameterType::Number => {
                let value = self.prompt_number(param)?;
                serde_json::Value::Number(serde_json::Number::from_f64(value).ok_or_else(|| {
                    ParameterError::ValidationFailed {
                        message: format!("Invalid number value: {value}"),
                    }
                })?)
            }
            ParameterType::Choice => {
                let value = self.prompt_choice(param)?;
                serde_json::Value::String(value)
            }
            ParameterType::MultiChoice => {
                let values = self.prompt_multi_choice(param)?;
                serde_json::Value::Array(
                    values.into_iter().map(serde_json::Value::String).collect(),
                )
            }
        };

        // Validate the input before returning
        self.validator.validate_parameter(param, &result)?;
        Ok(result)
    }

    /// Prompt for a string parameter with validation
    pub fn prompt_string(&self, param: &Parameter) -> ParameterResult<String> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

        let theme = ColorfulTheme::default();

        loop {
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

            // Validate the input
            let value = serde_json::Value::String(input.clone());
            match self.validator.validate_parameter(param, &value) {
                Ok(_) => return Ok(input),
                Err(e) => {
                    println!("❌ {e}");
                    self.print_validation_hints(param);
                    println!("Please try again.");
                }
            }
        }
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

        let theme = ColorfulTheme::default();

        loop {
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
            match input.parse::<f64>() {
                Ok(num) => {
                    // Validate the input
                    let value =
                        serde_json::Value::Number(serde_json::Number::from_f64(num).ok_or_else(
                            || ParameterError::ValidationFailed {
                                message: format!("Invalid number value: {num}"),
                            },
                        )?);

                    match self.validator.validate_parameter(param, &value) {
                        Ok(_) => return Ok(num),
                        Err(e) => {
                            println!("❌ {e}");
                            self.print_validation_hints(param);
                            println!("Please try again.");
                        }
                    }
                }
                Err(_) => {
                    println!("❌ Please enter a valid number.");
                    println!("Please try again.");
                }
            }
        }
    }

    /// Prompt for a choice parameter using fuzzy selection
    pub fn prompt_choice(&self, param: &Parameter) -> ParameterResult<String> {
        if self.non_interactive {
            return Err(ParameterError::MissingRequired {
                name: param.name.clone(),
            });
        }

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

    /// Print helpful validation hints for a parameter
    fn print_validation_hints(&self, param: &Parameter) {
        if let Some(validation) = &param.validation {
            match param.parameter_type {
                ParameterType::String => {
                    if let Some(pattern) = &validation.pattern {
                        let hint = CommonPatterns::hint_for_pattern(pattern);
                        let description = CommonPatterns::description_for_pattern(pattern);
                        println!("💡 Expected format: {hint} ({description})");
                    }

                    if let (Some(min), Some(max)) = (validation.min_length, validation.max_length) {
                        println!("💡 Length must be between {min} and {max} characters");
                    } else if let Some(min) = validation.min_length {
                        println!("💡 Must be at least {min} characters long");
                    } else if let Some(max) = validation.max_length {
                        println!("💡 Must be at most {max} characters long");
                    }
                }
                ParameterType::Number => {
                    if let (Some(min), Some(max)) = (validation.min, validation.max) {
                        println!("💡 Value must be between {min} and {max}");
                    } else if let Some(min) = validation.min {
                        println!("💡 Value must be at least {min}");
                    } else if let Some(max) = validation.max {
                        println!("💡 Value must be at most {max}");
                    }

                    if let Some(step) = validation.step {
                        println!("💡 Value must be a multiple of {step}");
                    }
                }
                ParameterType::MultiChoice => {
                    if let (Some(min), Some(max)) =
                        (validation.min_selections, validation.max_selections)
                    {
                        println!("💡 Must select between {min} and {max} options");
                    } else if let Some(min) = validation.min_selections {
                        println!("💡 Must select at least {min} options");
                    } else if let Some(max) = validation.max_selections {
                        println!("💡 Must select at most {max} options");
                    }
                }
                _ => {}
            }
        }

        // Show choices hint for choice/multichoice parameters
        if let Some(choices) = &param.choices {
            if !choices.is_empty() && choices.len() <= 5 {
                println!("💡 Available options: {}", choices.join(", "));
            } else if !choices.is_empty() {
                println!("💡 {} available options", choices.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        use crate::common::parameter_conditions::ParameterCondition;
        
        let prompts = InteractivePrompts::new(true);
        
        // Create conditional parameters
        let deploy_env = Parameter::new("deploy_env", "Deployment environment", crate::common::parameters::ParameterType::String)
            .required(true);
            
        let prod_confirmation = Parameter::new("prod_confirmation", "Production confirmation", crate::common::parameters::ParameterType::Boolean)
            .required(true)
            .with_condition(ParameterCondition::new("deploy_env == 'prod'"));
            
        let parameters = vec![deploy_env, prod_confirmation];

        // Test with existing deploy_env = dev (should not require prod_confirmation)
        let mut existing = HashMap::new();
        existing.insert("deploy_env".to_string(), serde_json::Value::String("dev".to_string()));

        let result = prompts.prompt_for_parameters(&parameters, &existing).unwrap();
        
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("deploy_env").unwrap(), &serde_json::Value::String("dev".to_string()));
        assert!(!result.contains_key("prod_confirmation"));
    }

    #[test]
    fn test_prompt_conditional_parameters_with_defaults() {
        
        let prompts = InteractivePrompts::new(true);
        
        let enable_ssl = Parameter::new("enable_ssl", "Enable SSL", crate::common::parameters::ParameterType::Boolean)
            .with_default(serde_json::json!(false))
            .required(false);
            
        let cert_path = Parameter::new("cert_path", "SSL certificate path", crate::common::parameters::ParameterType::String)
            .required(true)
            .when("enable_ssl == true")
            .with_default(serde_json::json!("/etc/ssl/cert.pem"));
            
        let parameters = vec![enable_ssl, cert_path];

        // Test 1: No existing values, should use defaults and not require cert_path
        let existing = HashMap::new();
        let result = prompts.prompt_for_parameters(&parameters, &existing).unwrap();
        
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("enable_ssl").unwrap(), &serde_json::json!(false));
        assert!(!result.contains_key("cert_path"));

        // Test 2: enable_ssl = true provided, should use cert_path default
        let mut existing = HashMap::new();
        existing.insert("enable_ssl".to_string(), serde_json::Value::Bool(true));

        let result = prompts.prompt_for_parameters(&parameters, &existing).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("enable_ssl").unwrap(), &serde_json::json!(true));
        assert_eq!(result.get("cert_path").unwrap(), &serde_json::json!("/etc/ssl/cert.pem"));
    }

    #[test]
    fn test_format_condition_explanation() {
        use crate::common::parameter_conditions::ParameterCondition;
        
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
}
