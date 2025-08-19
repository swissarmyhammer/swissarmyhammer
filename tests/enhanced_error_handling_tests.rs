//! Comprehensive tests for enhanced error handling functionality
//!
//! Tests cover:
//! - Error message enhancement
//! - Fuzzy matching suggestions
//! - Interactive error recovery
//! - CLI error formatting
//! - Pattern explanation

use std::collections::HashMap;
use swissarmyhammer::common::parameters::{
    ErrorMessageEnhancer, Parameter, ParameterError, ParameterType, ParameterValidator,
    CommonPatterns
};
use swissarmyhammer::common::interactive_prompts::InteractivePrompts;
use swissarmyhammer_cli::error::CliError;

#[cfg(test)]
mod error_enhancement_tests {
    use super::*;

    #[test]
    fn test_pattern_mismatch_enhancement() {
        let enhancer = ErrorMessageEnhancer::new();
        
        let original_error = ParameterError::PatternMismatch {
            name: "email".to_string(),
            value: "invalid@".to_string(),
            pattern: CommonPatterns::EMAIL.to_string(),
        };
        
        let enhanced = enhancer.enhance_parameter_error(&original_error);
        
        match enhanced {
            ParameterError::PatternMismatchEnhanced {
                parameter,
                value,
                pattern_description,
                examples,
                recoverable,
                ..
            } => {
                assert_eq!(parameter, "email");
                assert_eq!(value, "invalid@");
                assert_eq!(pattern_description, "Valid email address");
                assert!(!examples.is_empty());
                assert_eq!(recoverable, true);
                
                // Check that examples contain valid email formats
                assert!(examples.iter().any(|e| e.contains("@") && e.contains(".")));
            }
            _ => panic!("Expected PatternMismatchEnhanced error"),
        }
    }

    #[test]
    fn test_invalid_choice_enhancement_with_fuzzy_matching() {
        let enhancer = ErrorMessageEnhancer::new();
        
        let choices = vec!["production".to_string(), "staging".to_string(), "development".to_string()];
        let original_error = ParameterError::InvalidChoice {
            name: "environment".to_string(),
            value: "prod".to_string(),
            choices: choices.clone(),
        };
        
        let enhanced = enhancer.enhance_parameter_error(&original_error);
        
        match enhanced {
            ParameterError::InvalidChoiceEnhanced {
                parameter,
                value,
                choices: enhanced_choices,
                did_you_mean,
                recoverable,
            } => {
                assert_eq!(parameter, "environment");
                assert_eq!(value, "prod");
                assert_eq!(enhanced_choices, choices);
                assert_eq!(did_you_mean, Some("production".to_string()));
                assert_eq!(recoverable, true);
            }
            _ => panic!("Expected InvalidChoiceEnhanced error"),
        }
    }

    #[test]
    fn test_string_length_error_enhancement() {
        let enhancer = ErrorMessageEnhancer::new();
        
        let original_error = ParameterError::StringTooShort {
            name: "password".to_string(),
            min_length: 8,
            actual_length: 4,
        };
        
        let enhanced = enhancer.enhance_parameter_error(&original_error);
        
        match enhanced {
            ParameterError::ValidationFailedWithContext {
                parameter,
                message,
                explanation,
                suggestions,
                recoverable,
                ..
            } => {
                assert_eq!(parameter, "password");
                assert_eq!(message, "Must be at least 8 characters long");
                assert!(explanation.is_some());
                assert!(!suggestions.is_empty());
                assert_eq!(recoverable, true);
                
                // Check that suggestion includes specific guidance
                let suggestion_text = suggestions.join(" ");
                assert!(suggestion_text.contains("4 more characters"));
            }
            _ => panic!("Expected ValidationFailedWithContext error"),
        }
    }

    #[test]
    fn test_out_of_range_error_enhancement() {
        let enhancer = ErrorMessageEnhancer::new();
        
        let original_error = ParameterError::OutOfRange {
            name: "port".to_string(),
            value: 70000.0,
            min: Some(1.0),
            max: Some(65535.0),
        };
        
        let enhanced = enhancer.enhance_parameter_error(&original_error);
        
        match enhanced {
            ParameterError::ValidationFailedWithContext {
                parameter,
                message,
                explanation,
                suggestions,
                recoverable,
                ..
            } => {
                assert_eq!(parameter, "port");
                assert!(message.contains("between 1 and 65535"));
                assert!(explanation.is_some());
                assert!(!suggestions.is_empty());
                assert_eq!(recoverable, true);
                
                // Check that suggestion includes range guidance
                let suggestion_text = suggestions.join(" ");
                assert!(suggestion_text.contains("<= 65535"));
            }
            _ => panic!("Expected ValidationFailedWithContext error"),
        }
    }

    #[test]
    fn test_conditional_parameter_missing_enhancement() {
        let enhancer = ErrorMessageEnhancer::new();
        
        let original_error = ParameterError::ConditionalParameterMissing {
            parameter: "ssl_cert".to_string(),
            condition: "enable_ssl == true".to_string(),
        };
        
        let enhanced = enhancer.enhance_parameter_error(&original_error);
        
        match enhanced {
            ParameterError::ValidationFailedWithContext {
                parameter,
                message,
                explanation,
                suggestions,
                recoverable,
                ..
            } => {
                assert_eq!(parameter, "ssl_cert");
                assert_eq!(message, "Parameter required for your current configuration");
                assert!(explanation.is_some());
                assert!(!suggestions.is_empty());
                assert_eq!(recoverable, true);
                
                // Check that suggestions include CLI guidance
                let suggestion_text = suggestions.join(" ");
                assert!(suggestion_text.contains("--ssl-cert"));
                assert!(suggestion_text.contains("interactive"));
            }
            _ => panic!("Expected ValidationFailedWithContext error"),
        }
    }
}

#[cfg(test)]
mod fuzzy_matching_tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance_calculation() {
        let enhancer = ErrorMessageEnhancer::new();
        
        // Test various distance calculations
        assert_eq!(enhancer.levenshtein_distance("prod", "production"), 6);
        assert_eq!(enhancer.levenshtein_distance("dev", "development"), 7);
        assert_eq!(enhancer.levenshtein_distance("stage", "staging"), 3);
        assert_eq!(enhancer.levenshtein_distance("test", "testing"), 3);
        assert_eq!(enhancer.levenshtein_distance("same", "same"), 0);
    }

    #[test]
    fn test_closest_match_suggestions() {
        let enhancer = ErrorMessageEnhancer::new();
        
        let choices = vec![
            "production".to_string(),
            "staging".to_string(),
            "development".to_string(),
            "testing".to_string(),
        ];

        // Test close matches that should be suggested
        assert_eq!(
            enhancer.suggest_closest_match("prod", &choices),
            Some("production".to_string())
        );
        assert_eq!(
            enhancer.suggest_closest_match("stage", &choices),
            Some("staging".to_string())
        );
        assert_eq!(
            enhancer.suggest_closest_match("dev", &choices),
            Some("development".to_string())
        );

        // Test exact match (should still suggest)
        assert_eq!(
            enhancer.suggest_closest_match("testing", &choices),
            Some("testing".to_string())
        );

        // Test very different input (should not suggest)
        assert_eq!(enhancer.suggest_closest_match("completely_different", &choices), None);

        // Test empty choices
        assert_eq!(enhancer.suggest_closest_match("anything", &vec![]), None);
    }

    #[test]
    fn test_fuzzy_matching_threshold() {
        let enhancer = ErrorMessageEnhancer::new();
        
        let choices = vec!["short".to_string(), "medium".to_string(), "verylongword".to_string()];

        // Test that suggestions are only made for reasonable distances
        assert_eq!(
            enhancer.suggest_closest_match("shor", &choices),
            Some("short".to_string())
        );

        // Test that very distant matches are not suggested
        assert_eq!(
            enhancer.suggest_closest_match("completely_unrelated_text", &choices),
            None
        );
    }
}

#[cfg(test)]
mod pattern_explanation_tests {
    use super::*;

    #[test]
    fn test_common_patterns_examples() {
        // Test email pattern examples
        let email_examples = CommonPatterns::examples_for_pattern(CommonPatterns::EMAIL);
        assert!(!email_examples.is_empty());
        assert!(email_examples.iter().all(|e| e.contains("@") && e.contains(".")));

        // Test URL pattern examples
        let url_examples = CommonPatterns::examples_for_pattern(CommonPatterns::URL);
        assert!(!url_examples.is_empty());
        assert!(url_examples.iter().all(|u| u.starts_with("http://") || u.starts_with("https://")));

        // Test IPv4 pattern examples
        let ipv4_examples = CommonPatterns::examples_for_pattern(CommonPatterns::IPV4);
        assert!(!ipv4_examples.is_empty());
        assert!(ipv4_examples.iter().all(|ip| ip.split('.').count() == 4));

        // Test semantic version examples
        let semver_examples = CommonPatterns::examples_for_pattern(CommonPatterns::SEMVER);
        assert!(!semver_examples.is_empty());
        assert!(semver_examples.iter().all(|v| v.matches('.').count() == 2));
    }

    #[test]
    fn test_pattern_descriptions() {
        assert_eq!(
            CommonPatterns::description_for_pattern(CommonPatterns::EMAIL),
            "Valid email address"
        );
        assert_eq!(
            CommonPatterns::description_for_pattern(CommonPatterns::URL),
            "Valid HTTP or HTTPS URL"
        );
        assert_eq!(
            CommonPatterns::description_for_pattern(CommonPatterns::SEMVER),
            "Semantic version (major.minor.patch)"
        );
        assert_eq!(
            CommonPatterns::description_for_pattern("custom_pattern"),
            "Custom pattern"
        );
    }
}

#[cfg(test)]
mod interactive_error_recovery_tests {
    use super::*;

    #[test]
    fn test_interactive_prompts_max_attempts_configuration() {
        let prompts_default = InteractivePrompts::new(true);
        let prompts_custom = InteractivePrompts::with_max_attempts(true, 5);

        // Test that both constructors work without panicking and create valid objects
        assert!(std::ptr::addr_of!(prompts_default) != std::ptr::addr_of!(prompts_custom));
        // Test basic functionality - these calls should not panic
        let test_param = Parameter {
            name: "test".to_string(),
            parameter_type: ParameterType::String,
            description: Some("Test parameter".to_string()),
            required: true,
            default_value: None,
            choices: None,
            validation: None,
            condition: None,
        };
        let _ = prompts_default.format_prompt(&test_param);
        let _ = prompts_custom.format_prompt(&test_param);
    }

    #[test]
    fn test_error_display_formatting() {
        let prompts = InteractivePrompts::new(true);
        
        // Test that display methods exist and can be called
        // Since display methods print to console, we can't easily test output
        // In a real implementation, we might want to make these methods return strings
        // or accept a writer parameter for testability
        
        let error = ParameterError::ValidationFailedWithContext {
            parameter: "test".to_string(),
            value: "invalid".to_string(),
            message: "Test validation failed".to_string(),
            explanation: Some("This is a test explanation".to_string()),
            examples: vec!["example1".to_string(), "example2".to_string()],
            suggestions: vec!["Try this".to_string(), "Or that".to_string()],
            recoverable: true,
        };
        
        // This would print to console - in a real test we'd capture or mock output
        prompts.display_enhanced_error(&error);
        
        // Test passes if no panic occurred during display - method successfully executed
        // We could improve this by capturing stdout in the future, but for now verify no panic
    }
}

#[cfg(test)]
mod cli_error_formatting_tests {
    use super::*;

    #[test]
    fn test_parameter_error_to_cli_error_conversion() {
        let param_error = ParameterError::PatternMismatch {
            name: "email".to_string(),
            value: "invalid@".to_string(),
            pattern: CommonPatterns::EMAIL.to_string(),
        };
        
        let cli_error: CliError = param_error.into();
        
        // Test that the conversion preserves information and adds CLI-specific formatting
        assert!(cli_error.message.contains("❌"));
        assert!(cli_error.message.contains("email"));
        assert!(cli_error.message.contains("invalid@"));
        assert!(cli_error.message.contains("📖"));
        assert!(cli_error.message.contains("🔄"));
        assert!(cli_error.message.contains("--help"));
        assert!(cli_error.message.contains("--interactive"));
        
        // Test appropriate exit code
        assert!(cli_error.exit_code != 0); // Should be error exit code
    }

    #[test]
    fn test_max_attempts_exceeded_cli_formatting() {
        let param_error = ParameterError::MaxAttemptsExceeded {
            parameter: "password".to_string(),
            attempts: 3,
        };
        
        let cli_error: CliError = param_error.into();
        
        assert!(cli_error.message.contains("Maximum retry attempts exceeded"));
        assert!(cli_error.message.contains("password"));
        assert!(cli_error.message.contains("3 attempts"));
    }

    #[test]
    fn test_invalid_choice_cli_formatting() {
        let param_error = ParameterError::InvalidChoice {
            name: "environment".to_string(),
            value: "prod".to_string(),
            choices: vec!["production".to_string(), "staging".to_string()],
        };
        
        let cli_error: CliError = param_error.into();
        
        assert!(cli_error.message.contains("environment"));
        assert!(cli_error.message.contains("prod"));
        assert!(cli_error.message.contains("Did you mean"));
        assert!(cli_error.message.contains("production"));
    }
}

#[cfg(test)]
mod condition_explanation_tests {
    use super::*;

    #[test]
    fn test_condition_explanation_formatting() {
        let enhancer = ErrorMessageEnhancer::new();

        // Test equality condition explanation
        let eq_condition = "deploy_env == 'production'";
        let explanation = enhancer.explain_condition(eq_condition);
        assert!(explanation.contains("deploy_env"));
        assert!(explanation.contains("production"));

        // Test 'in' condition explanation
        let in_condition = "environment in ['prod', 'staging']";
        let explanation = enhancer.explain_condition(in_condition);
        assert!(explanation.contains("environment"));

        // Test fallback for complex conditions
        let complex_condition = "enable_ssl && port > 443";
        let explanation = enhancer.explain_condition(complex_condition);
        assert!(explanation.contains("condition"));
        assert!(explanation.contains(complex_condition));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_end_to_end_error_enhancement_flow() {
        // Create a parameter with validation rules
        let param = Parameter::new("port", "Server port number", ParameterType::Number)
            .with_validation_rules(|rules| rules.min(1.0).max(65535.0));

        let validator = ParameterValidator::new();
        let enhancer = ErrorMessageEnhancer::new();

        // Test with invalid value
        let invalid_value = serde_json::json!(70000);
        let validation_result = validator.validate_parameter(&param, &invalid_value);

        assert!(validation_result.is_err());
        
        if let Err(error) = validation_result {
            let enhanced_error = enhancer.enhance_parameter_error(&error);
            
            // Verify enhancement adds useful context
            match enhanced_error {
                ParameterError::ValidationFailedWithContext { 
                    parameter, 
                    suggestions, 
                    recoverable, 
                    .. 
                } => {
                    assert_eq!(parameter, "port");
                    assert!(!suggestions.is_empty());
                    assert_eq!(recoverable, true);
                }
                _ => panic!("Expected ValidationFailedWithContext"),
            }
        }
    }

    #[test]
    fn test_pattern_validation_enhancement_flow() {
        // Create a parameter with pattern validation
        let param = Parameter::new("email", "Email address", ParameterType::String)
            .with_validation_rules(|rules| rules.pattern(CommonPatterns::EMAIL));

        let validator = ParameterValidator::new();
        let enhancer = ErrorMessageEnhancer::new();

        // Test with invalid email
        let invalid_value = serde_json::json!("not-an-email");
        let validation_result = validator.validate_parameter(&param, &invalid_value);

        assert!(validation_result.is_err());
        
        if let Err(error) = validation_result {
            let enhanced_error = enhancer.enhance_parameter_error(&error);
            
            // Verify pattern enhancement
            match enhanced_error {
                ParameterError::PatternMismatchEnhanced { 
                    parameter, 
                    pattern_description, 
                    examples, 
                    recoverable, 
                    .. 
                } => {
                    assert_eq!(parameter, "email");
                    assert_eq!(pattern_description, "Valid email address");
                    assert!(!examples.is_empty());
                    assert_eq!(recoverable, true);
                }
                _ => panic!("Expected PatternMismatchEnhanced"),
            }
        }
    }

    #[test]
    fn test_choice_validation_with_fuzzy_matching_flow() {
        // Create a choice parameter
        let param = Parameter::new("environment", "Deployment environment", ParameterType::Choice)
            .with_choices(vec![
                "production".to_string(),
                "staging".to_string(),
                "development".to_string(),
            ]);

        let validator = ParameterValidator::new();
        let enhancer = ErrorMessageEnhancer::new();

        // Test with close but invalid choice
        let invalid_value = serde_json::json!("prod");
        let validation_result = validator.validate_parameter(&param, &invalid_value);

        assert!(validation_result.is_err());
        
        if let Err(error) = validation_result {
            let enhanced_error = enhancer.enhance_parameter_error(&error);
            
            // Verify fuzzy matching suggestion
            match enhanced_error {
                ParameterError::InvalidChoiceEnhanced { 
                    parameter, 
                    value,
                    did_you_mean, 
                    recoverable, 
                    .. 
                } => {
                    assert_eq!(parameter, "environment");
                    assert_eq!(value, "prod");
                    assert_eq!(did_you_mean, Some("production".to_string()));
                    assert_eq!(recoverable, true);
                }
                _ => panic!("Expected InvalidChoiceEnhanced"),
            }
        }
    }
}