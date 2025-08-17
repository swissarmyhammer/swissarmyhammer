//! Interactive parameter prompting system
//!
//! This module provides interactive prompting capabilities for parameters using the dialoguer crate.
//! It handles different parameter types with appropriate UI controls and validation.

use crate::common::parameters::{Parameter, ParameterError, ParameterResult, ParameterType, ParameterValidator};
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
    pub fn prompt_for_parameters(
        &self,
        parameters: &[Parameter],
        existing_values: &HashMap<String, serde_json::Value>,
    ) -> ParameterResult<HashMap<String, serde_json::Value>> {
        let mut resolved = existing_values.clone();

        for param in parameters {
            if resolved.contains_key(&param.name) {
                // Parameter already provided, validate it
                if let Some(value) = resolved.get(&param.name) {
                    self.validator.validate_parameter(param, value)?;
                }
                continue;
            }

            // Parameter is missing, check if we need to prompt or use default
            if param.required {
                if self.non_interactive {
                    return Err(ParameterError::MissingRequired {
                        name: param.name.clone(),
                    });
                }
                // Prompt for required parameter
                let value = self.prompt_for_parameter(param)?;
                resolved.insert(param.name.clone(), value);
            } else if let Some(default) = &param.default {
                // Use default value for optional parameter
                resolved.insert(param.name.clone(), default.clone());
            }
            // Optional parameters without defaults are left unset
        }

        Ok(resolved)
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
                serde_json::Value::Number(
                    serde_json::Number::from_f64(value)
                        .ok_or_else(|| ParameterError::ValidationFailed {
                            message: format!("Invalid number value: {value}"),
                        })?,
                )
            }
            ParameterType::Choice => {
                let value = self.prompt_choice(param)?;
                serde_json::Value::String(value)
            }
            ParameterType::MultiChoice => {
                let values = self.prompt_multi_choice(param)?;
                serde_json::Value::Array(
                    values
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
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

            let input = input_prompt.interact().map_err(|e| {
                ParameterError::ValidationFailed {
                    message: format!("Failed to read input: {e}"),
                }
            })?;

            // Validate the input
            let value = serde_json::Value::String(input.clone());
            match self.validator.validate_parameter(param, &value) {
                Ok(_) => return Ok(input),
                Err(e) => {
                    println!("❌ {e}");
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

        confirm_prompt.interact().map_err(|e| {
            ParameterError::ValidationFailed {
                message: format!("Failed to read input: {e}"),
            }
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
            if let (Some(min), Some(max)) = (&param.min, &param.max) {
                prompt_text = format!("{prompt_text} [{min}-{max}]");
            } else if let Some(min) = &param.min {
                prompt_text = format!("{prompt_text} [>= {min}]");
            } else if let Some(max) = &param.max {
                prompt_text = format!("{prompt_text} [<= {max}]");
            }
            
            let mut input_prompt = Input::<String>::with_theme(&theme)
                .with_prompt(prompt_text);

            // Add default value if available
            if let Some(default) = &param.default {
                if let Some(default_num) = default.as_f64() {
                    input_prompt = input_prompt.default(default_num.to_string());
                }
            }

            let input = input_prompt.interact().map_err(|e| {
                ParameterError::ValidationFailed {
                    message: format!("Failed to read input: {e}"),
                }
            })?;

            // Parse the number
            match input.parse::<f64>() {
                Ok(num) => {
                    // Validate the input
                    let value = serde_json::Value::Number(
                        serde_json::Number::from_f64(num).ok_or_else(|| {
                            ParameterError::ValidationFailed {
                                message: format!("Invalid number value: {num}"),
                            }
                        })?,
                    );
                    
                    match self.validator.validate_parameter(param, &value) {
                        Ok(_) => return Ok(num),
                        Err(e) => {
                            println!("❌ {e}");
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

        let choices = param.choices.as_ref().ok_or_else(|| {
            ParameterError::ValidationFailed {
                message: format!("Choice parameter '{}' has no choices defined", param.name),
            }
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

        let selection = select_prompt.interact().map_err(|e| {
            ParameterError::ValidationFailed {
                message: format!("Failed to read selection: {e}"),
            }
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

        let choices = param.choices.as_ref().ok_or_else(|| {
            ParameterError::ValidationFailed {
                message: format!(
                    "MultiChoice parameter '{}' has no choices defined",
                    param.name
                ),
            }
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

        let selections = multi_select.interact().map_err(|e| {
            ParameterError::ValidationFailed {
                message: format!("Failed to read selections: {e}"),
            }
        })?;

        let selected_values: Vec<String> = selections
            .iter()
            .map(|&i| choices[i].clone())
            .collect();

        Ok(selected_values)
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

        let param = Parameter::new("test_param", "Test parameter", ParameterType::String)
            .required(true);
        let parameters = vec![param];

        let mut existing = HashMap::new();
        existing.insert("test_param".to_string(), serde_json::Value::String("existing_value".to_string()));

        let result = prompts.prompt_for_parameters(&parameters, &existing).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("test_param").unwrap(),
            &serde_json::Value::String("existing_value".to_string())
        );
    }

    #[test]
    fn test_prompt_for_parameters_with_defaults() {
        let prompts = InteractivePrompts::new(true);

        let param = Parameter::new("optional_param", "Optional parameter", ParameterType::String)
            .with_default(serde_json::Value::String("default_value".to_string()));
        let parameters = vec![param];

        let existing = HashMap::new();

        let result = prompts.prompt_for_parameters(&parameters, &existing).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("optional_param").unwrap(),
            &serde_json::Value::String("default_value".to_string())
        );
    }

    #[test]
    fn test_prompt_for_parameters_missing_required_non_interactive() {
        let prompts = InteractivePrompts::new(true);

        let param = Parameter::new("required_param", "Required parameter", ParameterType::String)
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
        existing.insert("choice_param".to_string(), serde_json::Value::String("invalid_choice".to_string()));

        let result = prompts.prompt_for_parameters(&parameters, &existing);
        assert!(result.is_err());

        if let Err(ParameterError::InvalidChoice { name, value, choices }) = result {
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
        let param = Parameter::new("bad_multi_choice", "Bad multi choice parameter", ParameterType::MultiChoice);
        
        let result = prompts.prompt_multi_choice(&param);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_choice_empty_choices() {
        let prompts = InteractivePrompts::new(true);
        let param = Parameter::new("empty_choice", "Empty choice parameter", ParameterType::Choice)
            .with_choices(vec![]);
        
        let result = prompts.prompt_choice(&param);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_multi_choice_empty_choices() {
        let prompts = InteractivePrompts::new(true);
        let param = Parameter::new("empty_multi_choice", "Empty multi choice parameter", ParameterType::MultiChoice)
            .with_choices(vec![]);
        
        let result = prompts.prompt_multi_choice(&param);
        assert!(result.is_err());
    }
}