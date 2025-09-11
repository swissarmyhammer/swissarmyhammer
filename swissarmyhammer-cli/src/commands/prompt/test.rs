//! Test command handler for prompts
//!
//! Modern test command implementation that uses CliContext for prompt library access
//! and output formatting, with clean separation of concerns.

use crate::context::CliContext;
use crate::commands::prompt::cli::TestCommand;
use swissarmyhammer::interactive_prompts::InteractivePrompts;
use swissarmyhammer_common::{Parameter, ParameterError, ParameterProvider, ParameterType};
use std::collections::HashMap;
use anyhow::Result;

/// Execute the test command with the provided configuration
pub async fn execute_test_command(
    test_cmd: TestCommand,
    cli_context: &CliContext,
) -> Result<()> {
    let prompt_name = test_cmd.prompt_name.clone()
        .ok_or_else(|| anyhow::anyhow!("Prompt name is required"))?;

    if cli_context.verbose {
        println!("Testing prompt: {}", prompt_name);
    }

    // Get prompt library from CliContext
    let library = cli_context.get_prompt_library()?;
    
    // Get the specific prompt
    let prompt = library
        .get(&prompt_name)
        .map_err(|e| anyhow::anyhow!("Failed to get prompt '{}': {}", prompt_name, e))?;

    if cli_context.debug {
        println!("Prompt parameters: {:#?}", prompt.get_parameters());
    }

    // Collect parameters
    let parameters = collect_test_parameters(&test_cmd, prompt.get_parameters(), cli_context)?;
    
    // Render the prompt using CliContext
    let rendered = cli_context.render_prompt(&prompt_name, &parameters)?;

    // Output the result
    output_rendered_prompt(&rendered, &test_cmd, cli_context)?;
    
    Ok(())
}

/// Collect parameters for the test, combining CLI args with interactive prompts
fn collect_test_parameters(
    test_cmd: &TestCommand,
    prompt_parameters: &[Parameter],
    cli_context: &CliContext,
) -> Result<HashMap<String, serde_json::Value>> {
    // Parse CLI variables
    let cli_parameters = parse_cli_variables(&test_cmd.vars)?;
    
    if cli_context.verbose && !cli_parameters.is_empty() {
        println!("CLI parameters: {:#?}", cli_parameters);
    }

    // Use InteractivePrompts to collect missing parameters
    let interactive_prompts = InteractivePrompts::with_max_attempts(false, 3);
    let all_parameters = collect_missing_parameters(
        &interactive_prompts,
        prompt_parameters,
        &cli_parameters,
        cli_context.verbose,
    )?;

    if cli_context.verbose {
        println!("Final parameters: {:#?}", all_parameters);
    }

    Ok(all_parameters)
}

/// Parse CLI variable arguments (key=value format) into a HashMap
fn parse_cli_variables(vars: &[String]) -> Result<HashMap<String, serde_json::Value>> {
    let mut cli_arguments = HashMap::new();
    for var in vars {
        let parts: Vec<&str> = var.splitn(2, '=').collect();
        if parts.len() == 2 {
            cli_arguments.insert(
                parts[0].to_string(),
                serde_json::Value::String(parts[1].to_string()),
            );
        }
    }
    Ok(cli_arguments)
}

/// Collect missing parameters using interactive prompts when in terminal mode
///
/// # Arguments
/// * `_interactive_prompts` - InteractivePrompts instance (unused in current implementation)
/// * `parameters` - Slice of parameters required by the prompt
/// * `existing_values` - HashMap of already provided parameter values from CLI
/// * `verbose` - Whether to output verbose logging information
fn collect_missing_parameters(
    _interactive_prompts: &InteractivePrompts,
    parameters: &[Parameter],
    existing_values: &HashMap<String, serde_json::Value>,
    verbose: bool,
) -> Result<HashMap<String, serde_json::Value>, ParameterError> {
    use std::io::{self, IsTerminal, Write};

    let mut resolved = existing_values.clone();

    // Check if we're in a terminal - if not, just use defaults
    if !io::stdin().is_terminal() {
        // Non-interactive mode - use defaults for optional parameters
        for param in parameters {
            if !resolved.contains_key(&param.name) {
                if let Some(default) = &param.default {
                    resolved.insert(param.name.clone(), default.clone());
                } else if param.required {
                    return Err(ParameterError::MissingRequired {
                        name: param.name.clone(),
                    });
                }
            }
        }
        return Ok(resolved);
    }

    for param in parameters {
        if resolved.contains_key(&param.name) {
            // Parameter already provided via CLI, skip
            continue;
        }

        // For CLI testing, prompt for parameters (both required and optional)
        // Show a simple prompt with default value if available
        let prompt_text = if let Some(default) = &param.default {
            let default_str = match default {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => default.to_string(),
            };
            format!(
                "Enter {} ({}) [{}]: ",
                param.name, param.description, default_str
            )
        } else {
            format!("Enter {} ({}): ", param.name, param.description)
        };

        print!("{}", prompt_text);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| ParameterError::ValidationFailed {
                message: format!("Failed to read input: {}", e),
            })?;

        let input = input.trim();

        let value = if input.is_empty() {
            // Use default if available
            if let Some(default) = &param.default {
                default.clone()
            } else if param.required {
                return Err(ParameterError::MissingRequired {
                    name: param.name.clone(),
                });
            } else {
                // Optional parameter with no default - skip
                continue;
            }
        } else {
            // Convert input to appropriate type based on parameter type
            convert_input_to_parameter_value(input, &param.parameter_type, &param.choices)?
        };

        resolved.insert(param.name.clone(), value);
    }

    if verbose {
        println!("Collected parameters: {:#?}", resolved);
    }

    Ok(resolved)
}

/// Parse boolean input string into a JSON boolean value
///
/// Accepts various boolean representations: true/false, yes/no, t/f, y/n, 1/0
fn parse_boolean_input(input: &str) -> Result<serde_json::Value, ParameterError> {
    let bool_val = input.to_lowercase();
    match bool_val.as_str() {
        "true" | "t" | "yes" | "y" | "1" => Ok(serde_json::Value::Bool(true)),
        "false" | "f" | "no" | "n" | "0" => Ok(serde_json::Value::Bool(false)),
        _ => Err(ParameterError::ValidationFailed {
            message: format!(
                "Invalid boolean value: '{}'. Use true/false, yes/no, or 1/0.",
                input
            ),
        }),
    }
}

/// Convert user input string to appropriate JSON value based on parameter type
///
/// # Arguments
/// * `input` - User input string to convert
/// * `parameter_type` - The expected parameter type for validation and conversion
/// * `choices` - Optional list of valid choices for Choice/MultiChoice parameters
fn convert_input_to_parameter_value(
    input: &str,
    parameter_type: &ParameterType,
    choices: &Option<Vec<String>>,
) -> Result<serde_json::Value, ParameterError> {
    match parameter_type {
        ParameterType::String => Ok(serde_json::Value::String(input.to_string())),
        ParameterType::Boolean => parse_boolean_input(input),
        ParameterType::Number => {
            let num: f64 = input
                .parse()
                .map_err(|_| ParameterError::ValidationFailed {
                    message: format!("Invalid number: '{}'", input),
                })?;
            Ok(serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap()))
        }
        ParameterType::Choice => {
            if let Some(valid_choices) = choices {
                if valid_choices.contains(&input.to_string()) {
                    Ok(serde_json::Value::String(input.to_string()))
                } else {
                    Err(ParameterError::ValidationFailed {
                        message: format!(
                            "Invalid choice '{}'. Valid options: {}",
                            input,
                            valid_choices.join(", ")
                        ),
                    })
                }
            } else {
                Ok(serde_json::Value::String(input.to_string()))
            }
        }
        ParameterType::MultiChoice => {
            // For simplicity, accept comma-separated values
            let selected: Vec<String> = input.split(',').map(|s| s.trim().to_string()).collect();
            if let Some(valid_choices) = choices {
                for choice in &selected {
                    if !valid_choices.contains(choice) {
                        return Err(ParameterError::ValidationFailed {
                            message: format!(
                                "Invalid choice '{}'. Valid options: {}",
                                choice,
                                valid_choices.join(", ")
                            ),
                        });
                    }
                }
            }
            Ok(serde_json::Value::Array(
                selected
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ))
        }
    }
}

/// Output the rendered prompt according to test command options
fn output_rendered_prompt(
    rendered: &str,
    test_cmd: &TestCommand,
    cli_context: &CliContext,
) -> Result<()> {
    // Handle file output if specified
    if let Some(save_path) = &test_cmd.save {
        std::fs::write(save_path, rendered)
            .map_err(|e| anyhow::anyhow!("Failed to write file '{}': {}", save_path, e))?;
        if cli_context.verbose {
            println!("Saved rendered prompt to: {}", save_path);
        }
    }

    // Handle copy to clipboard if specified
    if test_cmd.copy {
        eprintln!("Warning: Clipboard feature not available. Use --save to write to file instead.");
    }

    // Always output to stdout unless explicitly suppressed
    if !cli_context.quiet {
        if test_cmd.raw {
            print!("{}", rendered);
        } else {
            println!("{}", rendered);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_cli_variables() {
        let vars = vec![
            "name=John".to_string(),
            "age=25".to_string(),
            "city=New York".to_string(),
        ];

        let result = parse_cli_variables(&vars).unwrap();
        
        assert_eq!(result.len(), 3);
        assert_eq!(result.get("name").unwrap(), &json!("John"));
        assert_eq!(result.get("age").unwrap(), &json!("25"));
        assert_eq!(result.get("city").unwrap(), &json!("New York"));
    }

    #[test]
    fn test_parse_cli_variables_empty() {
        let vars = vec![];
        let result = parse_cli_variables(&vars).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_cli_variables_invalid_format() {
        let vars = vec!["invalid".to_string()];
        let result = parse_cli_variables(&vars).unwrap();
        // Invalid format should be ignored
        assert!(result.is_empty());
    }

    #[test]
    fn test_convert_input_to_parameter_value_string() {
        let result = convert_input_to_parameter_value(
            "hello", 
            &ParameterType::String, 
            &None
        ).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn test_convert_input_to_parameter_value_boolean_true() {
        let test_cases = vec!["true", "t", "yes", "y", "1"];
        for input in test_cases {
            let result = convert_input_to_parameter_value(
                input, 
                &ParameterType::Boolean, 
                &None
            ).unwrap();
            assert_eq!(result, json!(true), "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_convert_input_to_parameter_value_boolean_false() {
        let test_cases = vec!["false", "f", "no", "n", "0"];
        for input in test_cases {
            let result = convert_input_to_parameter_value(
                input, 
                &ParameterType::Boolean, 
                &None
            ).unwrap();
            assert_eq!(result, json!(false), "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_convert_input_to_parameter_value_boolean_invalid() {
        let result = convert_input_to_parameter_value(
            "invalid", 
            &ParameterType::Boolean, 
            &None
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_input_to_parameter_value_number() {
        let result = convert_input_to_parameter_value(
            "42.5", 
            &ParameterType::Number, 
            &None
        ).unwrap();
        assert_eq!(result, json!(42.5));
    }

    #[test]
    fn test_convert_input_to_parameter_value_number_invalid() {
        let result = convert_input_to_parameter_value(
            "not_a_number", 
            &ParameterType::Number, 
            &None
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_input_to_parameter_value_choice_valid() {
        let choices = Some(vec!["option1".to_string(), "option2".to_string()]);
        let result = convert_input_to_parameter_value(
            "option1", 
            &ParameterType::Choice, 
            &choices
        ).unwrap();
        assert_eq!(result, json!("option1"));
    }

    #[test]
    fn test_convert_input_to_parameter_value_choice_invalid() {
        let choices = Some(vec!["option1".to_string(), "option2".to_string()]);
        let result = convert_input_to_parameter_value(
            "invalid_option", 
            &ParameterType::Choice, 
            &choices
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_input_to_parameter_value_multichoice() {
        let choices = Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        let result = convert_input_to_parameter_value(
            "a,b", 
            &ParameterType::MultiChoice, 
            &choices
        ).unwrap();
        assert_eq!(result, json!(["a", "b"]));
    }

    #[test]
    fn test_convert_input_to_parameter_value_multichoice_invalid() {
        let choices = Some(vec!["a".to_string(), "b".to_string()]);
        let result = convert_input_to_parameter_value(
            "a,invalid", 
            &ParameterType::MultiChoice, 
            &choices
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_missing_parameters_non_interactive_with_defaults() {
        let interactive_prompts = InteractivePrompts::new(false);

        // Test parameters with defaults
        let parameters = vec![
            Parameter {
                name: "greeting".to_string(),
                description: "A greeting message".to_string(),
                parameter_type: ParameterType::String,
                required: false,
                default: Some(json!("Hello")),
                choices: None,
                validation: None,
                condition: None,
            },
        ];

        let existing_values = HashMap::new();
        let result = collect_missing_parameters(
            &interactive_prompts, 
            &parameters, 
            &existing_values,
            false
        );

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.get("greeting").unwrap(), &json!("Hello"));
    }

    #[test]
    fn test_collect_missing_parameters_non_interactive_missing_required() {
        let interactive_prompts = InteractivePrompts::new(false);

        // Test with required parameter without default
        let parameters = vec![Parameter {
            name: "required_param".to_string(),
            description: "A required parameter".to_string(),
            parameter_type: ParameterType::String,
            required: true,
            default: None,
            choices: None,
            validation: None,
            condition: None,
        }];

        let existing_values = HashMap::new();
        let result = collect_missing_parameters(
            &interactive_prompts, 
            &parameters, 
            &existing_values,
            false
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::MissingRequired { name } => {
                assert_eq!(name, "required_param");
            }
            _ => panic!("Expected MissingRequired error"),
        }
    }

    #[test]
    fn test_collect_missing_parameters_existing_values_preserved() {
        let interactive_prompts = InteractivePrompts::new(false);

        let parameters = vec![Parameter {
            name: "greeting".to_string(),
            description: "A greeting message".to_string(),
            parameter_type: ParameterType::String,
            required: false,
            default: Some(json!("Hello")),
            choices: None,
            validation: None,
            condition: None,
        }];

        let mut existing_values = HashMap::new();
        existing_values.insert("greeting".to_string(), json!("Hi there!"));

        let result = collect_missing_parameters(
            &interactive_prompts, 
            &parameters, 
            &existing_values,
            false
        );

        assert!(result.is_ok());
        let resolved = result.unwrap();
        // Should preserve existing value, not use default
        assert_eq!(resolved.get("greeting").unwrap(), &json!("Hi there!"));
    }
}