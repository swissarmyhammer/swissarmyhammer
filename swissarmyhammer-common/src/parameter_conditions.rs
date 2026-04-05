//! Conditional parameter system for dynamic parameter requirements
//!
//! This module provides conditional parameter functionality that allows parameters
//! to be required or shown only when certain conditions are met based on other
//! parameter values.

// No imports needed from parameters module for this base functionality
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A condition that determines whether a parameter should be required or included
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterCondition {
    /// The condition expression (e.g., "deploy_env == 'prod'")
    pub expression: String,
    /// Optional explanation of when this condition applies
    pub description: Option<String>,
}

impl ParameterCondition {
    /// Create a new parameter condition
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            description: None,
        }
    }

    /// Add an optional description explaining when the condition applies
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Errors that can occur during condition evaluation
#[derive(Debug, thiserror::Error)]
pub enum ConditionError {
    /// The condition expression could not be parsed
    #[error("Failed to parse condition expression '{expression}': {details}")]
    ParseError {
        /// The condition expression that failed to parse
        expression: String,
        /// Detailed error message explaining the parsing failure
        details: String,
    },

    /// A parameter referenced in the condition is not available
    #[error("Parameter '{parameter}' referenced in condition is not available")]
    ParameterNotAvailable {
        /// The parameter name that is not available
        parameter: String,
    },

    /// The condition evaluation failed
    #[error("Failed to evaluate condition '{expression}': {details}")]
    EvaluationError {
        /// The condition expression that failed to evaluate
        expression: String,
        /// Detailed error message explaining the evaluation failure
        details: String,
    },
}

/// AST nodes for condition expressions
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionAst {
    /// Comparison operation (parameter == value, parameter != value, etc.)
    Comparison {
        /// The parameter name to compare
        parameter: String,
        /// The comparison operator to use
        operator: ComparisonOp,
        /// The value to compare the parameter against
        value: serde_json::Value,
    },
    /// Logical operation (condition && condition, condition || condition)
    Logical {
        /// The left-hand side condition
        left: Box<ConditionAst>,
        /// The logical operator (AND/OR)
        operator: LogicalOp,
        /// The right-hand side condition
        right: Box<ConditionAst>,
    },
    /// In operation (parameter in [value1, value2, ...])
    In {
        /// The parameter name to check
        parameter: String,
        /// The list of values to check membership in
        values: Vec<serde_json::Value>,
    },
    /// Contains operation (parameter contains "substring")
    Contains {
        /// The parameter name to check
        parameter: String,
        /// The substring to search for
        substring: String,
    },
}

/// Comparison operators supported in condition expressions
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    /// Equality operator (==)
    Equal,
    /// Inequality operator (!=)
    NotEqual,
    /// Less than operator (<)
    Less,
    /// Greater than operator (>)
    Greater,
    /// Less than or equal operator (<=)
    LessEq,
    /// Greater than or equal operator (>=)
    GreaterEq,
}

/// Logical operators supported in condition expressions
#[derive(Debug, Clone, PartialEq)]
pub enum LogicalOp {
    /// Logical AND operator (&&)
    And,
    /// Logical OR operator (||)
    Or,
}

/// Parser for condition expressions
pub struct ConditionParser;

impl ConditionParser {
    /// Parse a condition expression string into an AST
    pub fn parse(expression: &str) -> Result<ConditionAst, ConditionError> {
        // For now, implement a simple parser that handles basic cases
        // This will be extended to handle more complex expressions

        if expression.contains(" && ") {
            return Self::parse_logical(expression, LogicalOp::And);
        }

        if expression.contains(" || ") {
            return Self::parse_logical(expression, LogicalOp::Or);
        }

        if expression.contains(" in ") {
            return Self::parse_in_operation(expression);
        }

        if expression.contains(" contains ") {
            return Self::parse_contains_operation(expression);
        }

        // Handle simple comparison operations
        Self::parse_comparison(expression)
    }

    /// Parse logical operations (AND, OR)
    fn parse_logical(expression: &str, op: LogicalOp) -> Result<ConditionAst, ConditionError> {
        let operator_str = match op {
            LogicalOp::And => " && ",
            LogicalOp::Or => " || ",
        };

        let parts: Vec<&str> = expression.splitn(2, operator_str).collect();
        if parts.len() != 2 {
            return Err(ConditionError::ParseError {
                expression: expression.to_string(),
                details: format!("Invalid logical expression, expected '{operator_str}'"),
            });
        }

        let left = Self::parse(parts[0].trim())?;
        let right = Self::parse(parts[1].trim())?;

        Ok(ConditionAst::Logical {
            left: Box::new(left),
            operator: op,
            right: Box::new(right),
        })
    }

    /// Parse 'in' operations (parameter in [value1, value2, ...])
    fn parse_in_operation(expression: &str) -> Result<ConditionAst, ConditionError> {
        let parts: Vec<&str> = expression.splitn(2, " in ").collect();
        if parts.len() != 2 {
            return Err(ConditionError::ParseError {
                expression: expression.to_string(),
                details: "Invalid 'in' expression".to_string(),
            });
        }

        let parameter = parts[0].trim().to_string();
        let values_str = parts[1].trim();

        // Parse the values list - expect format like ["value1", "value2"] or [value1, value2]
        if !values_str.starts_with('[') || !values_str.ends_with(']') {
            return Err(ConditionError::ParseError {
                expression: expression.to_string(),
                details: "Expected array format [value1, value2, ...] for 'in' operation"
                    .to_string(),
            });
        }

        let values_content = &values_str[1..values_str.len() - 1];
        let values: Vec<serde_json::Value> = values_content
            .split(',')
            .map(|v| {
                let trimmed = v.trim();
                // Try to parse as JSON value
                if trimmed.starts_with('"') && trimmed.ends_with('"') {
                    serde_json::Value::String(trimmed[1..trimmed.len() - 1].to_string())
                } else if let Ok(num) = trimmed.parse::<f64>() {
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(num)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    )
                } else if trimmed == "true" {
                    serde_json::Value::Bool(true)
                } else if trimmed == "false" {
                    serde_json::Value::Bool(false)
                } else {
                    // Default to string
                    serde_json::Value::String(trimmed.to_string())
                }
            })
            .collect();

        if values.is_empty() {
            return Err(ConditionError::ParseError {
                expression: expression.to_string(),
                details: "Empty values array not allowed for 'in' operation".to_string(),
            });
        }

        Ok(ConditionAst::In { parameter, values })
    }

    /// Parse 'contains' operations (parameter contains "substring")
    fn parse_contains_operation(expression: &str) -> Result<ConditionAst, ConditionError> {
        let parts: Vec<&str> = expression.splitn(2, " contains ").collect();
        if parts.len() != 2 {
            return Err(ConditionError::ParseError {
                expression: expression.to_string(),
                details: "Invalid 'contains' expression".to_string(),
            });
        }

        let parameter = parts[0].trim().to_string();
        let substring_part = parts[1].trim();

        // Remove quotes if present
        let substring = if substring_part.starts_with('"') && substring_part.ends_with('"') {
            substring_part[1..substring_part.len() - 1].to_string()
        } else {
            substring_part.to_string()
        };

        Ok(ConditionAst::Contains {
            parameter,
            substring,
        })
    }

    /// Parse comparison operations (==, !=, <, >, <=, >=)
    fn parse_comparison(expression: &str) -> Result<ConditionAst, ConditionError> {
        // Try different comparison operators in order of priority
        let operators = [
            ("<=", ComparisonOp::LessEq),
            (">=", ComparisonOp::GreaterEq),
            ("==", ComparisonOp::Equal),
            ("!=", ComparisonOp::NotEqual),
            ("<", ComparisonOp::Less),
            (">", ComparisonOp::Greater),
        ];

        for (op_str, op) in &operators {
            if let Some(pos) = expression.find(op_str) {
                let parameter = expression[..pos].trim().to_string();
                let value_str = expression[pos + op_str.len()..].trim();

                // Parse the value
                let value = Self::parse_value(value_str)?;

                return Ok(ConditionAst::Comparison {
                    parameter,
                    operator: op.clone(),
                    value,
                });
            }
        }

        Err(ConditionError::ParseError {
            expression: expression.to_string(),
            details: "No valid comparison operator found".to_string(),
        })
    }

    /// Parse a value from string into JSON value
    fn parse_value(value_str: &str) -> Result<serde_json::Value, ConditionError> {
        // Handle quoted strings
        if value_str.starts_with('\'') && value_str.ends_with('\'') {
            return Ok(serde_json::Value::String(
                value_str[1..value_str.len() - 1].to_string(),
            ));
        }

        if value_str.starts_with('"') && value_str.ends_with('"') {
            return Ok(serde_json::Value::String(
                value_str[1..value_str.len() - 1].to_string(),
            ));
        }

        // Handle booleans
        if value_str == "true" {
            return Ok(serde_json::Value::Bool(true));
        }

        if value_str == "false" {
            return Ok(serde_json::Value::Bool(false));
        }

        // Try to parse as number
        if let Ok(num) = value_str.parse::<f64>() {
            return Ok(serde_json::Value::Number(
                serde_json::Number::from_f64(num).unwrap_or_else(|| serde_json::Number::from(0)),
            ));
        }

        // Default to string (unquoted)
        Ok(serde_json::Value::String(value_str.to_string()))
    }
}

/// Condition evaluator that evaluates parsed condition ASTs against parameter values
pub struct ConditionEvaluator {
    /// Available parameter values for condition evaluation
    variables: HashMap<String, serde_json::Value>,
}

impl ConditionEvaluator {
    /// Create a new condition evaluator with the given parameter values
    pub fn new(variables: HashMap<String, serde_json::Value>) -> Self {
        Self { variables }
    }

    /// Evaluate a condition expression against the current parameter values
    pub fn evaluate(&self, expression: &str) -> Result<bool, ConditionError> {
        let ast = ConditionParser::parse(expression)?;
        self.evaluate_ast(&ast)
    }

    /// Evaluate a parsed condition AST
    pub fn evaluate_ast(&self, ast: &ConditionAst) -> Result<bool, ConditionError> {
        match ast {
            ConditionAst::Comparison {
                parameter,
                operator,
                value,
            } => self.evaluate_comparison(parameter, operator, value),
            ConditionAst::Logical {
                left,
                operator,
                right,
            } => {
                let left_result = self.evaluate_ast(left)?;
                let right_result = self.evaluate_ast(right)?;

                match operator {
                    LogicalOp::And => Ok(left_result && right_result),
                    LogicalOp::Or => Ok(left_result || right_result),
                }
            }
            ConditionAst::In { parameter, values } => self.evaluate_in(parameter, values),
            ConditionAst::Contains {
                parameter,
                substring,
            } => self.evaluate_contains(parameter, substring),
        }
    }

    /// Evaluate a comparison operation
    fn evaluate_comparison(
        &self,
        parameter: &str,
        operator: &ComparisonOp,
        expected: &serde_json::Value,
    ) -> Result<bool, ConditionError> {
        let actual =
            self.variables
                .get(parameter)
                .ok_or_else(|| ConditionError::ParameterNotAvailable {
                    parameter: parameter.to_string(),
                })?;

        match operator {
            ComparisonOp::Equal => Ok(actual == expected),
            ComparisonOp::NotEqual => Ok(actual != expected),
            ComparisonOp::Less => self.compare_numbers(actual, expected, |a, b| a < b),
            ComparisonOp::Greater => self.compare_numbers(actual, expected, |a, b| a > b),
            ComparisonOp::LessEq => self.compare_numbers(actual, expected, |a, b| a <= b),
            ComparisonOp::GreaterEq => self.compare_numbers(actual, expected, |a, b| a >= b),
        }
    }

    /// Compare two JSON values as numbers
    fn compare_numbers<F>(
        &self,
        actual: &serde_json::Value,
        expected: &serde_json::Value,
        op: F,
    ) -> Result<bool, ConditionError>
    where
        F: Fn(f64, f64) -> bool,
    {
        let actual_num = actual
            .as_f64()
            .ok_or_else(|| ConditionError::EvaluationError {
                expression: "numeric comparison".to_string(),
                details: format!("Left value is not a number: {actual}"),
            })?;

        let expected_num = expected
            .as_f64()
            .ok_or_else(|| ConditionError::EvaluationError {
                expression: "numeric comparison".to_string(),
                details: format!("Right value is not a number: {expected}"),
            })?;

        Ok(op(actual_num, expected_num))
    }

    /// Evaluate an 'in' operation
    fn evaluate_in(
        &self,
        parameter: &str,
        values: &[serde_json::Value],
    ) -> Result<bool, ConditionError> {
        let actual =
            self.variables
                .get(parameter)
                .ok_or_else(|| ConditionError::ParameterNotAvailable {
                    parameter: parameter.to_string(),
                })?;

        Ok(values.contains(actual))
    }

    /// Evaluate a 'contains' operation
    fn evaluate_contains(&self, parameter: &str, substring: &str) -> Result<bool, ConditionError> {
        let actual =
            self.variables
                .get(parameter)
                .ok_or_else(|| ConditionError::ParameterNotAvailable {
                    parameter: parameter.to_string(),
                })?;

        let actual_str = actual
            .as_str()
            .ok_or_else(|| ConditionError::EvaluationError {
                expression: "contains operation".to_string(),
                details: format!("Parameter '{parameter}' is not a string: {actual}"),
            })?;

        Ok(actual_str.contains(substring))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_condition_creation() {
        let condition = ParameterCondition::new("deploy_env == 'prod'")
            .with_description("Production deployment confirmation");

        assert_eq!(condition.expression, "deploy_env == 'prod'");
        assert_eq!(
            condition.description,
            Some("Production deployment confirmation".to_string())
        );
    }

    #[test]
    fn test_condition_parser_simple_equality() {
        let ast = ConditionParser::parse("deploy_env == 'prod'").unwrap();

        match ast {
            ConditionAst::Comparison {
                parameter,
                operator,
                value,
            } => {
                assert_eq!(parameter, "deploy_env");
                assert_eq!(operator, ComparisonOp::Equal);
                assert_eq!(value, serde_json::Value::String("prod".to_string()));
            }
            _ => panic!("Expected comparison AST"),
        }
    }

    #[test]
    fn test_condition_parser_numeric_comparison() {
        let ast = ConditionParser::parse("port >= 1024").unwrap();

        match ast {
            ConditionAst::Comparison {
                parameter,
                operator,
                value,
            } => {
                assert_eq!(parameter, "port");
                assert_eq!(operator, ComparisonOp::GreaterEq);
                // Check that the value is 1024 as a number, allowing for f64 representation
                if let serde_json::Value::Number(num) = value {
                    assert_eq!(num.as_f64().unwrap(), 1024.0);
                } else {
                    panic!("Expected number value");
                }
            }
            _ => panic!("Expected comparison AST"),
        }
    }

    #[test]
    fn test_condition_parser_logical_and() {
        let ast = ConditionParser::parse("env == 'prod' && confirm == true").unwrap();

        match ast {
            ConditionAst::Logical {
                left,
                operator,
                right,
            } => {
                assert_eq!(operator, LogicalOp::And);
                // Verify left side
                if let ConditionAst::Comparison {
                    parameter,
                    operator,
                    value,
                } = left.as_ref()
                {
                    assert_eq!(parameter, "env");
                    assert_eq!(*operator, ComparisonOp::Equal);
                    assert_eq!(*value, serde_json::Value::String("prod".to_string()));
                }
                // Verify right side
                if let ConditionAst::Comparison {
                    parameter,
                    operator,
                    value,
                } = right.as_ref()
                {
                    assert_eq!(parameter, "confirm");
                    assert_eq!(*operator, ComparisonOp::Equal);
                    assert_eq!(*value, serde_json::Value::Bool(true));
                }
            }
            _ => panic!("Expected logical AST"),
        }
    }

    #[test]
    fn test_condition_parser_in_operation() {
        let ast = ConditionParser::parse("env in [\"dev\", \"staging\", \"prod\"]").unwrap();

        match ast {
            ConditionAst::In { parameter, values } => {
                assert_eq!(parameter, "env");
                assert_eq!(values.len(), 3);
                assert_eq!(values[0], serde_json::Value::String("dev".to_string()));
                assert_eq!(values[1], serde_json::Value::String("staging".to_string()));
                assert_eq!(values[2], serde_json::Value::String("prod".to_string()));
            }
            _ => panic!("Expected in AST"),
        }
    }

    #[test]
    fn test_condition_parser_contains_operation() {
        let ast = ConditionParser::parse("name contains \"test\"").unwrap();

        match ast {
            ConditionAst::Contains {
                parameter,
                substring,
            } => {
                assert_eq!(parameter, "name");
                assert_eq!(substring, "test");
            }
            _ => panic!("Expected contains AST"),
        }
    }

    #[test]
    fn test_condition_evaluator_simple_equality() {
        let mut variables = HashMap::new();
        variables.insert(
            "deploy_env".to_string(),
            serde_json::Value::String("prod".to_string()),
        );

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("deploy_env == 'prod'").unwrap();

        assert!(result);
    }

    #[test]
    fn test_condition_evaluator_equality_false() {
        let mut variables = HashMap::new();
        variables.insert(
            "deploy_env".to_string(),
            serde_json::Value::String("dev".to_string()),
        );

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("deploy_env == 'prod'").unwrap();

        assert!(!result);
    }

    #[test]
    fn test_condition_evaluator_numeric_comparison() {
        let mut variables = HashMap::new();
        variables.insert(
            "port".to_string(),
            serde_json::Value::Number(serde_json::Number::from(8080)),
        );

        let evaluator = ConditionEvaluator::new(variables);

        assert!(evaluator.evaluate("port > 1000").unwrap());
        assert!(!evaluator.evaluate("port < 1000").unwrap());
        assert!(evaluator.evaluate("port >= 8080").unwrap());
        assert!(evaluator.evaluate("port <= 8080").unwrap());
    }

    #[test]
    fn test_condition_evaluator_logical_and() {
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("prod".to_string()),
        );
        variables.insert("confirm".to_string(), serde_json::Value::Bool(true));

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("env == 'prod' && confirm == true")
            .unwrap();

        assert!(result);
    }

    #[test]
    fn test_condition_evaluator_logical_and_false() {
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("prod".to_string()),
        );
        variables.insert("confirm".to_string(), serde_json::Value::Bool(false));

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("env == 'prod' && confirm == true")
            .unwrap();

        assert!(!result);
    }

    #[test]
    fn test_condition_evaluator_logical_or() {
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("staging".to_string()),
        );
        variables.insert("urgent".to_string(), serde_json::Value::Bool(true));

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("env == 'prod' || urgent == true")
            .unwrap();

        assert!(result);
    }

    #[test]
    fn test_condition_evaluator_in_operation() {
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("staging".to_string()),
        );

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("env in [\"dev\", \"staging\", \"prod\"]")
            .unwrap();

        assert!(result);
    }

    #[test]
    fn test_condition_evaluator_in_operation_false() {
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("test".to_string()),
        );

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("env in [\"dev\", \"staging\", \"prod\"]")
            .unwrap();

        assert!(!result);
    }

    #[test]
    fn test_condition_evaluator_contains_operation() {
        let mut variables = HashMap::new();
        variables.insert(
            "branch_name".to_string(),
            serde_json::Value::String("feature/new-auth".to_string()),
        );

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("branch_name contains \"feature\"")
            .unwrap();

        assert!(result);
    }

    #[test]
    fn test_condition_evaluator_contains_operation_false() {
        let mut variables = HashMap::new();
        variables.insert(
            "branch_name".to_string(),
            serde_json::Value::String("main".to_string()),
        );

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("branch_name contains \"feature\"")
            .unwrap();

        assert!(!result);
    }

    #[test]
    fn test_condition_evaluator_parameter_not_available() {
        let variables = HashMap::new(); // Empty

        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("missing_param == 'value'");

        assert!(result.is_err());
        match result.unwrap_err() {
            ConditionError::ParameterNotAvailable { parameter } => {
                assert_eq!(parameter, "missing_param");
            }
            _ => panic!("Expected ParameterNotAvailable error"),
        }
    }

    #[test]
    fn test_condition_parser_invalid_expression() {
        let result = ConditionParser::parse("invalid expression without operator");
        assert!(result.is_err());

        match result.unwrap_err() {
            ConditionError::ParseError { expression, .. } => {
                assert_eq!(expression, "invalid expression without operator");
            }
            _ => panic!("Expected ParseError"),
        }
    }

    // --- Tests for uncovered condition evaluation ---

    #[test]
    fn test_in_operation_non_array_format_returns_error() {
        // Covers lines 197-200: 'in' with non-bracket value
        let result = ConditionParser::parse("env in dev, staging");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConditionError::ParseError { details, .. } => {
                assert!(details.contains("Expected array format"));
            }
            _ => panic!("Expected ParseError about array format"),
        }
    }

    #[test]
    fn test_in_operation_with_numeric_values() {
        // Covers lines 212-216: numeric values inside 'in' array
        let ast = ConditionParser::parse("code in [200, 404, 500]").unwrap();
        match ast {
            ConditionAst::In { parameter, values } => {
                assert_eq!(parameter, "code");
                assert_eq!(values.len(), 3);
                assert_eq!(values[0].as_f64().unwrap(), 200.0);
                assert_eq!(values[1].as_f64().unwrap(), 404.0);
                assert_eq!(values[2].as_f64().unwrap(), 500.0);
            }
            _ => panic!("Expected In AST"),
        }
    }

    #[test]
    fn test_in_operation_with_boolean_values() {
        // Covers lines 217-220: true/false inside 'in' array
        let ast = ConditionParser::parse("flag in [true, false]").unwrap();
        match ast {
            ConditionAst::In { parameter, values } => {
                assert_eq!(parameter, "flag");
                assert_eq!(values.len(), 2);
                assert_eq!(values[0], serde_json::Value::Bool(true));
                assert_eq!(values[1], serde_json::Value::Bool(false));
            }
            _ => panic!("Expected In AST"),
        }
    }

    #[test]
    fn test_in_operation_with_unquoted_string_values() {
        // Covers lines 222-223: bare (unquoted) string values in 'in' array
        let ast = ConditionParser::parse("env in [dev, staging]").unwrap();
        match ast {
            ConditionAst::In { parameter, values } => {
                assert_eq!(parameter, "env");
                assert_eq!(values[0], serde_json::Value::String("dev".to_string()));
                assert_eq!(values[1], serde_json::Value::String("staging".to_string()));
            }
            _ => panic!("Expected In AST"),
        }
    }

    #[test]
    fn test_contains_without_quotes() {
        // Covers line 255: contains with unquoted substring
        let ast = ConditionParser::parse("name contains hello").unwrap();
        match ast {
            ConditionAst::Contains {
                parameter,
                substring,
            } => {
                assert_eq!(parameter, "name");
                assert_eq!(substring, "hello");
            }
            _ => panic!("Expected Contains AST"),
        }
    }

    #[test]
    fn test_parse_value_double_quoted_string() {
        // Covers lines 307-309: double-quoted values in comparisons
        let ast = ConditionParser::parse("env == \"prod\"").unwrap();
        match ast {
            ConditionAst::Comparison {
                parameter, value, ..
            } => {
                assert_eq!(parameter, "env");
                assert_eq!(value, serde_json::Value::String("prod".to_string()));
            }
            _ => panic!("Expected Comparison AST"),
        }
    }

    #[test]
    fn test_parse_value_boolean_false() {
        // Covers line 319: parse_value with "false"
        let ast = ConditionParser::parse("flag == false").unwrap();
        match ast {
            ConditionAst::Comparison {
                parameter, value, ..
            } => {
                assert_eq!(parameter, "flag");
                assert_eq!(value, serde_json::Value::Bool(false));
            }
            _ => panic!("Expected Comparison AST"),
        }
    }

    #[test]
    fn test_parse_value_number() {
        // Covers lines 323-326: parse_value with numeric string
        let ast = ConditionParser::parse("count == 42").unwrap();
        match ast {
            ConditionAst::Comparison {
                parameter, value, ..
            } => {
                assert_eq!(parameter, "count");
                assert_eq!(value.as_f64().unwrap(), 42.0);
            }
            _ => panic!("Expected Comparison AST"),
        }
    }

    #[test]
    fn test_parse_value_unquoted_string() {
        // Covers line 330: parse_value falls through to bare string
        let ast = ConditionParser::parse("status == active").unwrap();
        match ast {
            ConditionAst::Comparison {
                parameter, value, ..
            } => {
                assert_eq!(parameter, "status");
                assert_eq!(value, serde_json::Value::String("active".to_string()));
            }
            _ => panic!("Expected Comparison AST"),
        }
    }

    #[test]
    fn test_compare_numbers_left_not_numeric() {
        // Covers lines 417-420: non-numeric actual value in numeric comparison
        let mut variables = HashMap::new();
        variables.insert(
            "val".to_string(),
            serde_json::Value::String("not_a_number".to_string()),
        );
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("val > 10");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConditionError::EvaluationError { details, .. } => {
                assert!(details.contains("not a number"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    fn test_compare_numbers_right_not_numeric() {
        // Covers lines 424-426: non-numeric expected value in numeric comparison
        let mut variables = HashMap::new();
        variables.insert(
            "val".to_string(),
            serde_json::Value::Number(serde_json::Number::from(10)),
        );
        let evaluator = ConditionEvaluator::new(variables);
        // Use a comparison with a bare string that parses to non-numeric value
        // Since parse_value would make "abc" a string, we test via AST directly
        let ast = ConditionAst::Comparison {
            parameter: "val".to_string(),
            operator: ComparisonOp::Greater,
            value: serde_json::Value::String("abc".to_string()),
        };
        let result = evaluator.evaluate_ast(&ast);
        assert!(result.is_err());
        match result.unwrap_err() {
            ConditionError::EvaluationError { details, .. } => {
                assert!(details.contains("not a number"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    fn test_evaluate_not_equal() {
        // Covers line 397: NotEqual comparison operator
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("dev".to_string()),
        );
        let evaluator = ConditionEvaluator::new(variables);
        assert!(evaluator.evaluate("env != 'prod'").unwrap());
    }

    #[test]
    fn test_evaluate_not_equal_false() {
        // NotEqual returning false when values are equal
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("prod".to_string()),
        );
        let evaluator = ConditionEvaluator::new(variables);
        assert!(!evaluator.evaluate("env != 'prod'").unwrap());
    }

    #[test]
    fn test_evaluate_less_than() {
        // Covers line 398: Less comparison operator
        let mut variables = HashMap::new();
        variables.insert(
            "count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)),
        );
        let evaluator = ConditionEvaluator::new(variables);
        assert!(evaluator.evaluate("count < 10").unwrap());
        assert!(!evaluator.evaluate("count < 3").unwrap());
    }

    #[test]
    fn test_evaluate_less_eq() {
        // Covers line 400: LessEq comparison operator
        let mut variables = HashMap::new();
        variables.insert(
            "count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(10)),
        );
        let evaluator = ConditionEvaluator::new(variables);
        assert!(evaluator.evaluate("count <= 10").unwrap());
        assert!(!evaluator.evaluate("count <= 9").unwrap());
    }

    #[test]
    fn test_in_operation_missing_parameter() {
        // Covers line 442: ParameterNotAvailable in evaluate_in
        let variables = HashMap::new();
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("missing in [\"a\", \"b\"]");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConditionError::ParameterNotAvailable { parameter } => {
                assert_eq!(parameter, "missing");
            }
            _ => panic!("Expected ParameterNotAvailable"),
        }
    }

    #[test]
    fn test_contains_missing_parameter() {
        // Covers lines 453-455: ParameterNotAvailable in evaluate_contains
        let variables = HashMap::new();
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("missing contains \"test\"");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConditionError::ParameterNotAvailable { parameter } => {
                assert_eq!(parameter, "missing");
            }
            _ => panic!("Expected ParameterNotAvailable"),
        }
    }

    #[test]
    fn test_contains_on_non_string_value() {
        // Covers lines 459-462: contains on a non-string parameter
        let mut variables = HashMap::new();
        variables.insert(
            "count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(42)),
        );
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("count contains \"4\"");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConditionError::EvaluationError { details, .. } => {
                assert!(details.contains("not a string"));
            }
            _ => panic!("Expected EvaluationError"),
        }
    }

    #[test]
    fn test_logical_or_both_false() {
        // Covers OR evaluation returning false
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("dev".to_string()),
        );
        variables.insert("urgent".to_string(), serde_json::Value::Bool(false));
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("env == 'prod' || urgent == true")
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn test_logical_and_both_true() {
        // Covers AND evaluation where both sides are true
        let mut variables = HashMap::new();
        variables.insert(
            "env".to_string(),
            serde_json::Value::String("prod".to_string()),
        );
        variables.insert("confirm".to_string(), serde_json::Value::Bool(true));
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator
            .evaluate("env == 'prod' && confirm == true")
            .unwrap();
        assert!(result);
    }

    #[test]
    fn test_parameter_condition_serialization_roundtrip() {
        // Covers ParameterCondition serde derive
        let condition =
            ParameterCondition::new("env == 'prod'").with_description("Only for production");
        let json = serde_json::to_string(&condition).unwrap();
        let deserialized: ParameterCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(condition, deserialized);
    }

    #[test]
    fn test_parameter_condition_serialization_without_description() {
        // Covers ParameterCondition serde with None description
        let condition = ParameterCondition::new("flag == true");
        let json = serde_json::to_string(&condition).unwrap();
        let deserialized: ParameterCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(condition, deserialized);
        assert_eq!(deserialized.description, None);
    }

    #[test]
    fn test_parser_inequality_operators() {
        // Covers parsing of != operator
        let ast = ConditionParser::parse("status != 'error'").unwrap();
        match ast {
            ConditionAst::Comparison {
                parameter,
                operator,
                ..
            } => {
                assert_eq!(parameter, "status");
                assert_eq!(operator, ComparisonOp::NotEqual);
            }
            _ => panic!("Expected Comparison AST"),
        }
    }

    #[test]
    fn test_parser_less_than_operator() {
        // Covers parsing of < operator
        let ast = ConditionParser::parse("age < 18").unwrap();
        match ast {
            ConditionAst::Comparison {
                parameter,
                operator,
                ..
            } => {
                assert_eq!(parameter, "age");
                assert_eq!(operator, ComparisonOp::Less);
            }
            _ => panic!("Expected Comparison AST"),
        }
    }

    #[test]
    fn test_parser_greater_than_operator() {
        // Covers parsing of > operator
        let ast = ConditionParser::parse("score > 90").unwrap();
        match ast {
            ConditionAst::Comparison {
                parameter,
                operator,
                ..
            } => {
                assert_eq!(parameter, "score");
                assert_eq!(operator, ComparisonOp::Greater);
            }
            _ => panic!("Expected Comparison AST"),
        }
    }

    #[test]
    fn test_parser_less_eq_operator() {
        // Covers parsing of <= operator
        let ast = ConditionParser::parse("retries <= 3").unwrap();
        match ast {
            ConditionAst::Comparison {
                parameter,
                operator,
                ..
            } => {
                assert_eq!(parameter, "retries");
                assert_eq!(operator, ComparisonOp::LessEq);
            }
            _ => panic!("Expected Comparison AST"),
        }
    }

    #[test]
    fn test_evaluate_in_numeric_match() {
        // Evaluate 'in' with numeric values
        let mut variables = HashMap::new();
        variables.insert(
            "code".to_string(),
            serde_json::Value::Number(serde_json::Number::from(404)),
        );
        let evaluator = ConditionEvaluator::new(variables);
        // Build AST directly since parsing numbers in 'in' arrays produces f64
        let ast = ConditionAst::In {
            parameter: "code".to_string(),
            values: vec![
                serde_json::json!(200),
                serde_json::json!(404),
                serde_json::json!(500),
            ],
        };
        assert!(evaluator.evaluate_ast(&ast).unwrap());
    }

    #[test]
    fn test_evaluate_in_boolean_match() {
        // Evaluate 'in' with boolean values
        let mut variables = HashMap::new();
        variables.insert("flag".to_string(), serde_json::Value::Bool(true));
        let evaluator = ConditionEvaluator::new(variables);
        let ast = ConditionAst::In {
            parameter: "flag".to_string(),
            values: vec![
                serde_json::Value::Bool(true),
                serde_json::Value::Bool(false),
            ],
        };
        assert!(evaluator.evaluate_ast(&ast).unwrap());
    }

    #[test]
    fn test_logical_or_parse() {
        // Covers parsing of || operator
        let ast = ConditionParser::parse("a == 1 || b == 2").unwrap();
        match ast {
            ConditionAst::Logical { operator, .. } => {
                assert_eq!(operator, LogicalOp::Or);
            }
            _ => panic!("Expected Logical AST"),
        }
    }

    #[test]
    fn test_condition_error_display() {
        // Covers Display impls via error messages
        let err = ConditionError::ParseError {
            expression: "bad expr".to_string(),
            details: "no operator".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("bad expr"));
        assert!(msg.contains("no operator"));

        let err = ConditionError::ParameterNotAvailable {
            parameter: "missing".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("missing"));

        let err = ConditionError::EvaluationError {
            expression: "x > 1".to_string(),
            details: "not a number".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("x > 1"));
        assert!(msg.contains("not a number"));
    }
}
