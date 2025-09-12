//! Test command handler for prompts
//!
//! Modern test command implementation that uses CliContext for prompt library access
//! and output formatting, with clean separation of concerns.

use crate::commands::prompt::cli::TestCommand;
use crate::context::CliContext;
use anyhow::Result;
use std::collections::HashMap;
use swissarmyhammer::interactive_prompts::InteractivePrompts;
use swissarmyhammer_common::{Parameter, ParameterError, ParameterProvider, ParameterType};

/// Execute the test command with the provided configuration
pub async fn execute_test_command(test_cmd: TestCommand, cli_context: &CliContext) -> Result<()> {
    // Determine debug mode from either test command or global context
    let debug_mode = test_cmd.debug || cli_context.debug;

    // Handle file-based prompt loading
    let (prompt_name, prompt) = if let Some(file_path) = &test_cmd.file {
        if debug_mode {
            println!("Loading prompt from file: {}", file_path);
        }

        // Use file name as prompt name for display
        let name = std::path::Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_path)
            .to_string();

        // Load prompt from file
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read prompt file '{}': {}", file_path, e))?;

        // Parse the prompt from the file content using PromptLoader
        let loader = swissarmyhammer_prompts::PromptLoader::new();
        let prompt = loader.load_from_string(&name, &content).map_err(|e| {
            anyhow::anyhow!("Failed to parse prompt from file '{}': {}", file_path, e)
        })?;

        (name, prompt)
    } else {
        // Use prompt name from library
        let prompt_name = test_cmd
            .prompt_name
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Either prompt name or file path is required"))?;

        if cli_context.verbose {
            println!("Testing prompt: {}", prompt_name);
        }

        // Get prompt library from CliContext
        let library = cli_context.get_prompt_library()?;

        // Get the specific prompt
        let prompt = library
            .get(&prompt_name)
            .map_err(|e| anyhow::anyhow!("Failed to get prompt '{}': {}", prompt_name, e))?;

        (prompt_name, prompt)
    };

    if debug_mode {
        println!("Prompt parameters: {:#?}", prompt.get_parameters());
    }

    // Collect parameters
    let parameters =
        collect_test_parameters(&test_cmd, prompt.get_parameters(), cli_context, debug_mode)?;

    // Render the prompt - handle both file and library cases
    let rendered = if test_cmd.file.is_some() {
        // For file-based prompts, render using templating engine directly
        let mut template_context = cli_context.template_context.clone();
        for (key, value) in &parameters {
            template_context.set(key.clone(), value.clone());
        }

        // Create template and render
        let template = swissarmyhammer_templating::Template::new(&prompt.template)
            .map_err(|e| anyhow::anyhow!("Failed to create template: {}", e))?;
        template
            .render_with_context(&template_context)
            .map_err(|e| anyhow::anyhow!("Failed to render prompt: {}", e))?
    } else {
        // For library prompts, use CliContext
        cli_context.render_prompt(&prompt_name, &parameters)?
    };

    // Output the result
    output_rendered_prompt(&rendered, &test_cmd, cli_context)?;

    Ok(())
}

/// Collect parameters for the test, combining CLI args with interactive prompts
fn collect_test_parameters(
    test_cmd: &TestCommand,
    prompt_parameters: &[Parameter],
    cli_context: &CliContext,
    debug_mode: bool,
) -> Result<HashMap<String, serde_json::Value>> {
    // Parse CLI variables
    let cli_parameters = parse_cli_variables(&test_cmd.vars)?;

    if (cli_context.verbose || debug_mode) && !cli_parameters.is_empty() {
        println!("CLI parameters: {:#?}", cli_parameters);
    }

    // Use InteractivePrompts to collect missing parameters
    let interactive_prompts = InteractivePrompts::with_max_attempts(false, 3);
    let all_parameters = collect_missing_parameters(
        &interactive_prompts,
        prompt_parameters,
        &cli_parameters,
        cli_context.verbose || debug_mode,
    )?;

    if cli_context.verbose || debug_mode {
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
            Ok(serde_json::Value::Number(
                serde_json::Number::from_f64(num).unwrap(),
            ))
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
        let result =
            convert_input_to_parameter_value("hello", &ParameterType::String, &None).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn test_convert_input_to_parameter_value_boolean_true() {
        let test_cases = vec!["true", "t", "yes", "y", "1"];
        for input in test_cases {
            let result =
                convert_input_to_parameter_value(input, &ParameterType::Boolean, &None).unwrap();
            assert_eq!(result, json!(true), "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_convert_input_to_parameter_value_boolean_false() {
        let test_cases = vec!["false", "f", "no", "n", "0"];
        for input in test_cases {
            let result =
                convert_input_to_parameter_value(input, &ParameterType::Boolean, &None).unwrap();
            assert_eq!(result, json!(false), "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_convert_input_to_parameter_value_boolean_invalid() {
        let result = convert_input_to_parameter_value("invalid", &ParameterType::Boolean, &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_input_to_parameter_value_number() {
        let result =
            convert_input_to_parameter_value("42.5", &ParameterType::Number, &None).unwrap();
        assert_eq!(result, json!(42.5));
    }

    #[test]
    fn test_convert_input_to_parameter_value_number_invalid() {
        let result =
            convert_input_to_parameter_value("not_a_number", &ParameterType::Number, &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_input_to_parameter_value_choice_valid() {
        let choices = Some(vec!["option1".to_string(), "option2".to_string()]);
        let result =
            convert_input_to_parameter_value("option1", &ParameterType::Choice, &choices).unwrap();
        assert_eq!(result, json!("option1"));
    }

    #[test]
    fn test_convert_input_to_parameter_value_choice_invalid() {
        let choices = Some(vec!["option1".to_string(), "option2".to_string()]);
        let result =
            convert_input_to_parameter_value("invalid_option", &ParameterType::Choice, &choices);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_input_to_parameter_value_multichoice() {
        let choices = Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        let result =
            convert_input_to_parameter_value("a,b", &ParameterType::MultiChoice, &choices).unwrap();
        assert_eq!(result, json!(["a", "b"]));
    }

    #[test]
    fn test_convert_input_to_parameter_value_multichoice_invalid() {
        let choices = Some(vec!["a".to_string(), "b".to_string()]);
        let result =
            convert_input_to_parameter_value("a,invalid", &ParameterType::MultiChoice, &choices);
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_missing_parameters_non_interactive_with_defaults() {
        let interactive_prompts = InteractivePrompts::new(false);

        // Test parameters with defaults
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

        let existing_values = HashMap::new();
        let result =
            collect_missing_parameters(&interactive_prompts, &parameters, &existing_values, false);

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
        let result =
            collect_missing_parameters(&interactive_prompts, &parameters, &existing_values, false);

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

        let result =
            collect_missing_parameters(&interactive_prompts, &parameters, &existing_values, false);

        assert!(result.is_ok());
        let resolved = result.unwrap();
        // Should preserve existing value, not use default
        assert_eq!(resolved.get("greeting").unwrap(), &json!("Hi there!"));
    }

    // Additional integration-style tests
    #[tokio::test]
    async fn test_execute_test_command_file_not_found() {
        use crate::context::CliContextBuilder;
        use swissarmyhammer_config::TemplateContext;

        let test_cmd = super::TestCommand {
            prompt_name: None,
            file: Some("/nonexistent/file.md".to_string()),
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        };

        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(false)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let result = super::execute_test_command(test_cmd, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read prompt file"));
    }

    #[tokio::test]
    async fn test_execute_test_command_missing_prompt_and_file() {
        use crate::context::CliContextBuilder;
        use swissarmyhammer_config::TemplateContext;

        let test_cmd = super::TestCommand {
            prompt_name: None,
            file: None,
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        };

        let template_context = TemplateContext::new();
        let matches = clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap();
        let context = CliContextBuilder::default()
            .template_context(template_context)
            .format(crate::cli::OutputFormat::Table)
            .format_option(None)
            .verbose(false)
            .debug(false)
            .quiet(false)
            .matches(matches)
            .build_async()
            .await
            .unwrap();

        let result = super::execute_test_command(test_cmd, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Either prompt name or file path is required"));
    }

    #[test]
    fn test_parse_cli_variables_with_equals_in_value() {
        let vars = vec![
            "equation=x=y+1".to_string(),
            "url=https://example.com".to_string(),
        ];

        let result = parse_cli_variables(&vars).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("equation").unwrap(), &json!("x=y+1"));
        assert_eq!(result.get("url").unwrap(), &json!("https://example.com"));
    }

    #[test]
    fn test_parse_cli_variables_edge_cases() {
        // Test edge cases with empty keys or values
        let vars = vec![
            "key=".to_string(),      // Empty value - valid (2 parts)
            "no_equals".to_string(), // Invalid - only 1 part, should be ignored
        ];

        let result = parse_cli_variables(&vars).unwrap();

        // Should have 1 entry - only "key=" gets parsed since splitn(2, '=') creates exactly 2 parts
        // "no_equals" gets ignored as it only has 1 part after split
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("key").unwrap(), &json!(""));
    }

    #[test]
    fn test_parse_cli_variables_empty_key() {
        // Test empty key case separately
        let vars = vec![
            "=value".to_string(), // Empty key - valid (2 parts)
        ];

        let result = parse_cli_variables(&vars).unwrap();

        // Should have 1 entry with empty key
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("").unwrap(), &json!("value"));
    }

    #[test]
    fn test_convert_input_to_parameter_value_multichoice_spaces() {
        let choices = Some(vec![
            "option 1".to_string(),
            "option 2".to_string(),
            "option 3".to_string(),
        ]);
        let result = convert_input_to_parameter_value(
            "option 1, option 2",
            &ParameterType::MultiChoice,
            &choices,
        )
        .unwrap();
        assert_eq!(result, json!(["option 1", "option 2"]));
    }

    #[test]
    fn test_convert_input_to_parameter_value_number_edge_cases() {
        // Test various number formats
        let test_cases = vec![
            ("0", 0.0),
            ("42", 42.0),
            ("-17", -17.0),
            ("3.14159", 3.14159_f64), // Parse the actual string value
            ("1e6", 1000000.0),
            ("1.5e-3", 0.0015),
        ];

        for (input, expected) in test_cases {
            let result =
                convert_input_to_parameter_value(input, &ParameterType::Number, &None).unwrap();
            if let serde_json::Value::Number(n) = result {
                assert!(
                    (n.as_f64().unwrap() - expected).abs() < f64::EPSILON,
                    "Failed for input '{}': expected {}, got {}",
                    input,
                    expected,
                    n
                );
            } else {
                panic!("Expected number value for input '{}'", input);
            }
        }
    }

    #[test]
    fn test_parse_boolean_input_case_sensitivity() {
        let true_variations = vec!["TRUE", "True", "tRuE", "T", "YES", "Yes", "Y"];
        let false_variations = vec!["FALSE", "False", "fAlSe", "F", "NO", "No", "N"];

        for input in true_variations {
            let result = parse_boolean_input(input).unwrap();
            assert_eq!(result, json!(true), "Failed for input: {}", input);
        }

        for input in false_variations {
            let result = parse_boolean_input(input).unwrap();
            assert_eq!(result, json!(false), "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_collect_test_parameters_integration() {
        use swissarmyhammer_common::{Parameter, ParameterType};

        // This is a synchronous test that tests the parameter collection logic
        let test_cmd = super::TestCommand {
            prompt_name: Some("test".to_string()),
            file: None,
            vars: vec!["name=Alice".to_string(), "age=30".to_string()],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        };

        let parameters = vec![
            Parameter {
                name: "name".to_string(),
                description: "User name".to_string(),
                parameter_type: ParameterType::String,
                required: true,
                default: None,
                choices: None,
                validation: None,
                condition: None,
            },
            Parameter {
                name: "age".to_string(),
                description: "User age".to_string(),
                parameter_type: ParameterType::Number,
                required: false,
                default: Some(json!(25)),
                choices: None,
                validation: None,
                condition: None,
            },
        ];

        // Test CLI variable parsing
        let cli_params = super::parse_cli_variables(&test_cmd.vars).unwrap();
        assert_eq!(cli_params.len(), 2);
        assert_eq!(cli_params.get("name").unwrap(), &json!("Alice"));
        assert_eq!(cli_params.get("age").unwrap(), &json!("30"));

        // Test parameter collection in non-interactive mode
        let interactive_prompts = InteractivePrompts::new(false);
        let all_params = super::collect_missing_parameters(
            &interactive_prompts,
            &parameters,
            &cli_params,
            false,
        )
        .unwrap();

        assert_eq!(all_params.len(), 2);
        assert_eq!(all_params.get("name").unwrap(), &json!("Alice"));
        assert_eq!(all_params.get("age").unwrap(), &json!("30")); // Should use CLI value, not default
    }

    #[test]
    fn test_parameter_type_validation_comprehensive() {
        // Test all parameter types with various valid inputs

        // String type
        let result =
            convert_input_to_parameter_value("any string", &ParameterType::String, &None).unwrap();
        assert_eq!(result, json!("any string"));

        // Boolean type variations
        let bool_inputs = vec![("true", true), ("false", false), ("1", true), ("0", false)];
        for (input, expected) in bool_inputs {
            let result =
                convert_input_to_parameter_value(input, &ParameterType::Boolean, &None).unwrap();
            assert_eq!(result, json!(expected));
        }

        // Number type
        let result =
            convert_input_to_parameter_value("123.45", &ParameterType::Number, &None).unwrap();
        assert_eq!(result, json!(123.45));

        // Choice type with valid choices
        let choices = Some(vec![
            "red".to_string(),
            "green".to_string(),
            "blue".to_string(),
        ]);
        let result =
            convert_input_to_parameter_value("green", &ParameterType::Choice, &choices).unwrap();
        assert_eq!(result, json!("green"));

        // MultiChoice type
        let result =
            convert_input_to_parameter_value("red,blue", &ParameterType::MultiChoice, &choices)
                .unwrap();
        assert_eq!(result, json!(["red", "blue"]));
    }

    #[test]
    fn test_error_scenarios_comprehensive() {
        // Test various error scenarios

        // Invalid boolean
        let result = convert_input_to_parameter_value("maybe", &ParameterType::Boolean, &None);
        assert!(result.is_err());

        // Invalid number
        let result =
            convert_input_to_parameter_value("not-a-number", &ParameterType::Number, &None);
        assert!(result.is_err());

        // Invalid choice
        let choices = Some(vec!["valid1".to_string(), "valid2".to_string()]);
        let result = convert_input_to_parameter_value("invalid", &ParameterType::Choice, &choices);
        assert!(result.is_err());

        // Invalid multichoice
        let result = convert_input_to_parameter_value(
            "valid1,invalid",
            &ParameterType::MultiChoice,
            &choices,
        );
        assert!(result.is_err());
    }
}
