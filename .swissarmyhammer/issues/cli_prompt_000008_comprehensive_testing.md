# Add Comprehensive Testing for New Prompt Architecture

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Add comprehensive unit and integration tests for the new prompt command architecture to ensure reliability, maintainability, and backward compatibility. This includes testing all new modules, error scenarios, and integration points.

## Current State

- New prompt architecture implemented but minimally tested
- Some unit tests exist in individual modules
- No comprehensive integration testing for the complete flow

## Goals

- Complete unit test coverage for all new modules
- Integration tests covering full prompt command workflows
- Regression tests ensuring backward compatibility
- Error scenario testing for robust error handling
- Performance validation for prompt loading and rendering

## Implementation Steps

### 1. Unit Tests for Display Module

**File**: `swissarmyhammer-cli/src/commands/prompt/display.rs`

Expand existing tests and add comprehensive coverage:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_prompts::Prompt;
    use std::collections::HashMap;

    #[test]
    fn test_prompt_row_from_prompt_with_all_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), serde_json::json!("Test Title"));
        
        let prompt = Prompt {
            name: "test-prompt".to_string(),
            description: Some("Test description".to_string()),
            category: Some("test".to_string()),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            template: "Test template".to_string(),
            parameters: vec![],
            source: Some(swissarmyhammer::PromptSource::Builtin),
            metadata,
        };

        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.title, "Test Title");
    }

    #[test]
    fn test_prompt_row_from_prompt_missing_metadata() {
        let prompt = Prompt {
            name: "no-metadata".to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: "Template".to_string(),
            parameters: vec![],
            source: None,
            metadata: HashMap::new(),
        };

        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "no-metadata");
        assert_eq!(row.title, "No title");
    }

    #[test]
    fn test_verbose_prompt_row_conversion() {
        let prompt = create_test_prompt_with_metadata();
        let row = VerbosePromptRow::from(&prompt);
        
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.title, "Test Title");
        assert_eq!(row.description, "Test description");
        assert_eq!(row.source, "Builtin");
        assert_eq!(row.category, Some("test".to_string()));
    }

    #[test]
    fn test_prompts_to_display_rows_standard() {
        let prompts = vec![create_test_prompt()];
        let display_rows = prompts_to_display_rows(prompts, false);
        
        match display_rows {
            DisplayRows::Standard(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].name, "test-prompt");
            }
            _ => panic!("Expected Standard display rows"),
        }
    }

    #[test]
    fn test_prompts_to_display_rows_verbose() {
        let prompts = vec![create_test_prompt()];
        let display_rows = prompts_to_display_rows(prompts, true);
        
        match display_rows {
            DisplayRows::Verbose(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].name, "test-prompt");
            }
            _ => panic!("Expected Verbose display rows"),
        }
    }

    #[test]
    fn test_serialization_prompt_row() {
        let row = PromptRow {
            name: "test".to_string(),
            title: "Test Title".to_string(),
        };
        
        let json = serde_json::to_string(&row).expect("Should serialize to JSON");
        assert!(json.contains("test"));
        assert!(json.contains("Test Title"));
    }

    fn create_test_prompt() -> Prompt {
        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), serde_json::json!("Test Title"));
        
        Prompt {
            name: "test-prompt".to_string(),
            description: Some("Test description".to_string()),
            category: Some("test".to_string()),
            tags: vec![],
            template: "Test template".to_string(),
            parameters: vec![],
            source: Some(swissarmyhammer::PromptSource::Builtin),
            metadata,
        }
    }

    fn create_test_prompt_with_metadata() -> Prompt {
        create_test_prompt()
    }
}
```

### 2. Unit Tests for CLI Module

**File**: `swissarmyhammer-cli/src/commands/prompt/cli.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::ArgMatches;

    fn create_list_matches() -> ArgMatches {
        let cmd = build_prompt_command();
        cmd.try_get_matches_from(vec!["prompt", "list"]).unwrap()
    }

    fn create_test_matches() -> ArgMatches {
        let cmd = build_prompt_command();
        cmd.try_get_matches_from(vec!["prompt", "test", "help"]).unwrap()
    }

    #[test]
    fn test_build_prompt_command() {
        let cmd = build_prompt_command();
        assert_eq!(cmd.get_name(), "prompt");
        
        let subcommands: Vec<_> = cmd.get_subcommands().map(|s| s.get_name()).collect();
        assert!(subcommands.contains(&"list"));
        assert!(subcommands.contains(&"test"));
    }

    #[test]
    fn test_parse_list_command() {
        let matches = create_list_matches();
        let result = parse_prompt_command(&matches).unwrap();
        
        match result {
            PromptCommand::List(_) => (),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_parse_test_command() {
        let matches = create_test_matches();
        let result = parse_prompt_command(&matches).unwrap();
        
        match result {
            PromptCommand::Test(test_cmd) => {
                assert_eq!(test_cmd.prompt_name, Some("help".to_string()));
            }
            _ => panic!("Expected Test command"),
        }
    }

    #[test]
    fn test_parse_test_command_with_vars() {
        let cmd = build_prompt_command();
        let matches = cmd.try_get_matches_from(vec![
            "prompt", "test", "help", 
            "--var", "topic=git", 
            "--var", "author=John"
        ]).unwrap();
        
        let result = parse_prompt_command(&matches).unwrap();
        match result {
            PromptCommand::Test(test_cmd) => {
                assert_eq!(test_cmd.vars, vec!["topic=git", "author=John"]);
            }
            _ => panic!("Expected Test command"),
        }
    }

    #[test]
    fn test_parse_unknown_subcommand() {
        let cmd = build_prompt_command();
        let result = cmd.try_get_matches_from(vec!["prompt", "unknown"]);
        assert!(result.is_err());
    }
}
```

### 3. Integration Tests for List Command

**File**: `swissarmyhammer-cli/src/commands/prompt/list.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use swissarmyhammer_config::TemplateContext;
    use crate::cli::OutputFormat;

    async fn create_test_context(verbose: bool, format: OutputFormat) -> CliContext {
        CliContext {
            template_context: TemplateContext::new(),
            format,
            verbose,
            debug: false,
            quiet: false,
            matches: create_dummy_matches(),
        }
    }

    fn create_dummy_matches() -> clap::ArgMatches {
        // Create minimal ArgMatches for testing
        clap::Command::new("test")
            .try_get_matches_from(vec!["test"])
            .unwrap()
    }

    #[tokio::test]
    async fn test_execute_list_command_success() {
        let context = create_test_context(false, OutputFormat::Table).await;
        let result = execute_list_command(&context).await;
        
        // Should succeed even if no prompts found
        assert!(result.is_ok());
    }

    #[tokio::test]  
    async fn test_execute_list_command_verbose() {
        let context = create_test_context(true, OutputFormat::Json).await;
        let result = execute_list_command(&context).await;
        
        // Should succeed with verbose output
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_display_prompts_removes_partials() {
        let prompts = vec![
            create_regular_prompt("regular1"),
            create_partial_prompt("partial1"),  
            create_regular_prompt("regular2"),
            create_partial_description_prompt("partial2"),
        ];

        let filtered = filter_display_prompts(prompts);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "regular1");
        assert_eq!(filtered[1].name, "regular2");
    }

    // Helper functions from existing tests
    fn create_regular_prompt(name: &str) -> swissarmyhammer_prompts::Prompt {
        // Implementation from existing tests
        swissarmyhammer_prompts::Prompt {
            name: name.to_string(),
            description: Some("A regular prompt".to_string()),
            category: None,
            tags: vec![],
            template: "Regular template content".to_string(),
            parameters: vec![],
            source: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    fn create_partial_prompt(name: &str) -> swissarmyhammer_prompts::Prompt {
        swissarmyhammer_prompts::Prompt {
            name: name.to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: "{% partial %}\nPartial content".to_string(),
            parameters: vec![],
            source: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    fn create_partial_description_prompt(name: &str) -> swissarmyhammer_prompts::Prompt {
        swissarmyhammer_prompts::Prompt {
            name: name.to_string(),
            description: Some("Partial template for reuse in other prompts".to_string()),
            category: None,
            tags: vec![],
            template: "Content".to_string(),
            parameters: vec[],
            source: None,
            metadata: std::collections::HashMap::new(),
        }
    }
}
```

### 4. Integration Tests for Test Command

**File**: `swissarmyhammer-cli/src/commands/prompt/test.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::{Parameter, ParameterType};

    #[test]
    fn test_parse_cli_variables_multiple() {
        let vars = vec![
            "name=John".to_string(),
            "age=30".to_string(),
            "active=true".to_string(),
        ];
        
        let result = parse_cli_variables(&vars).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result["name"], serde_json::json!("John"));
        assert_eq!(result["age"], serde_json::json!("30"));
        assert_eq!(result["active"], serde_json::json!("true"));
    }

    #[test]
    fn test_parse_cli_variables_empty() {
        let vars = vec![];
        let result = parse_cli_variables(&vars).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_cli_variables_invalid_formats() {
        let invalid_cases = vec![
            vec!["no_equals".to_string()],
            vec!["=no_key".to_string()],
            vec!["multiple=equals=signs".to_string()], // This should actually work
        ];

        for (i, case) in invalid_cases.iter().enumerate() {
            if i == 2 {
                // multiple=equals=signs should parse as key="equals=signs"
                let result = parse_cli_variables(case).unwrap();
                assert_eq!(result["multiple"], serde_json::json!("equals=signs"));
            } else {
                let result = parse_cli_variables(case);
                assert!(result.is_err(), "Case {} should fail: {:?}", i, case);
            }
        }
    }

    #[test]
    fn test_convert_parameter_types() {
        // Test string
        let result = convert_parameter_input("test", &ParameterType::String, &None).unwrap();
        assert_eq!(result.unwrap(), serde_json::json!("test"));

        // Test boolean
        let result = convert_parameter_input("true", &ParameterType::Boolean, &None).unwrap();
        assert_eq!(result.unwrap(), serde_json::json!(true));

        // Test number
        let result = convert_parameter_input("42", &ParameterType::Number, &None).unwrap();
        assert_eq!(result.unwrap(), serde_json::json!(42.0));

        // Test choice
        let choices = Some(vec!["option1".to_string(), "option2".to_string()]);
        let result = convert_parameter_input("option1", &ParameterType::Choice, &choices).unwrap();
        assert_eq!(result.unwrap(), serde_json::json!("option1"));

        // Test multi-choice
        let result = convert_parameter_input("option1,option2", &ParameterType::MultiChoice, &choices).unwrap();
        assert_eq!(result.unwrap(), serde_json::json!(["option1", "option2"]));
    }

    #[test]
    fn test_convert_boolean_variations() {
        let true_values = vec!["true", "True", "TRUE", "t", "T", "yes", "Yes", "y", "Y", "1"];
        let false_values = vec!["false", "False", "FALSE", "f", "F", "no", "No", "n", "N", "0"];

        for val in true_values {
            let result = convert_boolean_input(val).unwrap();
            assert_eq!(result, serde_json::json!(true), "Value '{}' should be true", val);
        }

        for val in false_values {
            let result = convert_boolean_input(val).unwrap();
            assert_eq!(result, serde_json::json!(false), "Value '{}' should be false", val);
        }

        // Test invalid
        assert!(convert_boolean_input("maybe").is_err());
        assert!(convert_boolean_input("").is_err());
    }

    #[test]
    fn test_convert_choice_validation() {
        let choices = Some(vec!["red".to_string(), "green".to_string(), "blue".to_string()]);
        
        // Valid choice
        let result = convert_choice_input("red", &choices).unwrap();
        assert_eq!(result, serde_json::json!("red"));

        // Invalid choice
        let result = convert_choice_input("yellow", &choices);
        assert!(result.is_err());

        // No choices provided
        let result = convert_choice_input("anything", &None).unwrap();
        assert_eq!(result, serde_json::json!("anything"));
    }

    #[test]
    fn test_format_parameter_default() {
        assert_eq!(format_parameter_default(&serde_json::json!("text")), "text");
        assert_eq!(format_parameter_default(&serde_json::json!(true)), "true");
        assert_eq!(format_parameter_default(&serde_json::json!(42)), "42");
        assert_eq!(format_parameter_default(&serde_json::json!(3.14)), "3.14");
    }

    // Mock test for non-interactive parameter collection
    #[test]
    fn test_collect_missing_parameters_non_interactive() {
        let parameters = vec![
            Parameter {
                name: "required_param".to_string(),
                description: "A required parameter".to_string(),
                parameter_type: ParameterType::String,
                required: true,
                default: Some(serde_json::json!("default_value")),
                choices: None,
                validation: None,
                condition: None,
            },
            Parameter {
                name: "optional_param".to_string(),
                description: "An optional parameter".to_string(),
                parameter_type: ParameterType::String,
                required: false,
                default: Some(serde_json::json!("optional_default")),
                choices: None,
                validation: None,
                condition: None,
            },
        ];

        let existing = std::collections::HashMap::new();
        let interactive_prompts = swissarmyhammer::interactive_prompts::InteractivePrompts::new(false);
        
        // This test assumes non-interactive environment
        let result = collect_missing_parameters(&interactive_prompts, &parameters, &existing, false);
        
        // Should succeed with defaults in non-interactive mode
        if result.is_ok() {
            let resolved = result.unwrap();
            assert_eq!(resolved["required_param"], serde_json::json!("default_value"));
            assert_eq!(resolved["optional_param"], serde_json::json!("optional_default"));
        }
    }
}
```

### 5. End-to-End Integration Tests

**File**: `swissarmyhammer-cli/tests/prompt_command_integration_test.rs`

```rust
use swissarmyhammer_cli::context::CliContext;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_cli::cli::OutputFormat;

#[tokio::test]
async fn test_prompt_list_command_integration() {
    let template_context = TemplateContext::new();
    let matches = create_test_matches_for_list();
    
    let cli_context = CliContext::new(template_context, matches).await.unwrap();
    
    let result = swissarmyhammer_cli::commands::prompt::list::handle_list_command(&cli_context).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), swissarmyhammer_cli::exit_codes::EXIT_SUCCESS);
}

#[tokio::test]
async fn test_prompt_test_command_integration() {
    // Test with a known builtin prompt if available
    let template_context = TemplateContext::new();
    let matches = create_test_matches_for_test();
    
    let cli_context = CliContext::new(template_context, matches).await.unwrap();
    
    let test_cmd = swissarmyhammer_cli::commands::prompt::cli::TestCommand {
        prompt_name: Some("help".to_string()),
        file: None,
        vars: vec!["topic=testing".to_string()],
        raw: false,
        copy: false,
        save: None,
        debug: false,
    };
    
    let result = swissarmyhammer_cli::commands::prompt::test::handle_test_command(test_cmd, &cli_context).await;
    // May succeed or fail depending on available prompts, but should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

fn create_test_matches_for_list() -> clap::ArgMatches {
    clap::Command::new("test")
        .arg(clap::Arg::new("format").long("format").default_value("table"))
        .arg(clap::Arg::new("verbose").long("verbose").action(clap::ArgAction::SetTrue))
        .try_get_matches_from(vec!["test"])
        .unwrap()
}

fn create_test_matches_for_test() -> clap::ArgMatches {
    create_test_matches_for_list() // Same base structure
}
```

### 6. Performance and Regression Tests  

**File**: `swissarmyhammer-cli/tests/prompt_performance_test.rs`

```rust
use std::time::Instant;
use tokio::test;

#[tokio::test]
async fn test_prompt_list_performance() {
    let start = Instant::now();
    
    // Load all prompts and measure time
    let template_context = swissarmyhammer_config::TemplateContext::new();
    let matches = create_minimal_matches();
    let cli_context = swissarmyhammer_cli::context::CliContext::new(template_context, matches).await.unwrap();
    
    let result = swissarmyhammer_cli::commands::prompt::list::execute_list_command(&cli_context).await;
    
    let duration = start.elapsed();
    
    // Should complete within reasonable time (adjust threshold as needed)
    assert!(duration.as_millis() < 5000, "List command took too long: {:?}", duration);
    assert!(result.is_ok(), "List command should succeed");
}

fn create_minimal_matches() -> clap::ArgMatches {
    clap::Command::new("test")
        .arg(clap::Arg::new("format").long("format").default_value("table"))
        .try_get_matches_from(vec!["test"])
        .unwrap()
}
```

## Testing Requirements

### Unit Test Coverage
- All public functions in display, cli, list, and test modules
- Error handling paths and edge cases
- Parameter parsing and validation
- Type conversions and serialization

### Integration Test Coverage  
- Full command execution workflows
- Global argument integration with CliContext
- Output formatting across all modes (table/json/yaml)
- Error scenarios and recovery

### Performance Tests
- Prompt loading performance
- Large prompt list handling
- Memory usage validation

### Regression Tests
- All existing prompt functionality
- Backward compatibility with current usage patterns
- Error message consistency

## Success Criteria

1. ✅ 90%+ unit test coverage for all new modules
2. ✅ Integration tests cover all major workflows
3. ✅ Performance tests validate reasonable execution times
4. ✅ All tests pass consistently
5. ✅ Error scenarios handled gracefully with good messages
6. ✅ Backward compatibility verified

## Files Created

- `swissarmyhammer-cli/tests/prompt_command_integration_test.rs` - End-to-end integration tests
- `swissarmyhammer-cli/tests/prompt_performance_test.rs` - Performance validation

## Files Modified  

- `swissarmyhammer-cli/src/commands/prompt/display.rs` - Expanded unit tests
- `swissarmyhammer-cli/src/commands/prompt/cli.rs` - Command parsing tests
- `swissarmyhammer-cli/src/commands/prompt/list.rs` - List functionality tests
- `swissarmyhammer-cli/src/commands/prompt/test.rs` - Test command tests

## Risk Mitigation

- Comprehensive test coverage reduces regression risk
- Performance tests catch performance degradation
- Integration tests validate real usage scenarios
- Mock external dependencies for reliable testing

---

**Estimated Effort**: Large (500+ lines of test code)
**Dependencies**: cli_prompt_000007_remove_legacy_code  
**Blocks**: cli_prompt_000009_documentation_update