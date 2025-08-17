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
        expression: String,
        details: String,
    },

    /// A parameter referenced in the condition is not available
    #[error("Parameter '{parameter}' referenced in condition is not available")]
    ParameterNotAvailable {
        parameter: String,
    },

    /// The condition evaluation failed
    #[error("Failed to evaluate condition '{expression}': {details}")]
    EvaluationError {
        expression: String,
        details: String,
    },
}

/// AST nodes for condition expressions
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionAst {
    /// Comparison operation (parameter == value, parameter != value, etc.)
    Comparison {
        parameter: String,
        operator: ComparisonOp,
        value: serde_json::Value,
    },
    /// Logical operation (condition && condition, condition || condition)
    Logical {
        left: Box<ConditionAst>,
        operator: LogicalOp,
        right: Box<ConditionAst>,
    },
    /// In operation (parameter in [value1, value2, ...])
    In {
        parameter: String,
        values: Vec<serde_json::Value>,
    },
    /// Contains operation (parameter contains "substring")
    Contains {
        parameter: String,
        substring: String,
    },
}

/// Comparison operators supported in condition expressions
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Equal,    // ==
    NotEqual, // !=
    Less,     // <
    Greater,  // >
    LessEq,   // <=
    GreaterEq,// >=
}

/// Logical operators supported in condition expressions
#[derive(Debug, Clone, PartialEq)]
pub enum LogicalOp {
    And, // &&
    Or,  // ||
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
                details: format!("Invalid logical expression, expected '{}'", operator_str),
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
                details: "Expected array format [value1, value2, ...] for 'in' operation".to_string(),
            });
        }
        
        let values_content = &values_str[1..values_str.len()-1];
        let values: Vec<serde_json::Value> = values_content
            .split(',')
            .map(|v| {
                let trimmed = v.trim();
                // Try to parse as JSON value
                if trimmed.starts_with('"') && trimmed.ends_with('"') {
                    serde_json::Value::String(trimmed[1..trimmed.len()-1].to_string())
                } else if let Ok(num) = trimmed.parse::<f64>() {
                    serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap_or_else(|| serde_json::Number::from(0)))
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
            substring_part[1..substring_part.len()-1].to_string()
        } else {
            substring_part.to_string()
        };
        
        Ok(ConditionAst::Contains { parameter, substring })
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
            return Ok(serde_json::Value::String(value_str[1..value_str.len()-1].to_string()));
        }
        
        if value_str.starts_with('"') && value_str.ends_with('"') {
            return Ok(serde_json::Value::String(value_str[1..value_str.len()-1].to_string()));
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
                serde_json::Number::from_f64(num).unwrap_or_else(|| serde_json::Number::from(0))
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
            ConditionAst::Comparison { parameter, operator, value } => {
                self.evaluate_comparison(parameter, operator, value)
            }
            ConditionAst::Logical { left, operator, right } => {
                let left_result = self.evaluate_ast(left)?;
                let right_result = self.evaluate_ast(right)?;
                
                match operator {
                    LogicalOp::And => Ok(left_result && right_result),
                    LogicalOp::Or => Ok(left_result || right_result),
                }
            }
            ConditionAst::In { parameter, values } => {
                self.evaluate_in(parameter, values)
            }
            ConditionAst::Contains { parameter, substring } => {
                self.evaluate_contains(parameter, substring)
            }
        }
    }
    
    /// Evaluate a comparison operation
    fn evaluate_comparison(&self, parameter: &str, operator: &ComparisonOp, expected: &serde_json::Value) -> Result<bool, ConditionError> {
        let actual = self.variables.get(parameter).ok_or_else(|| ConditionError::ParameterNotAvailable {
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
    fn compare_numbers<F>(&self, actual: &serde_json::Value, expected: &serde_json::Value, op: F) -> Result<bool, ConditionError> 
    where 
        F: Fn(f64, f64) -> bool,
    {
        let actual_num = actual.as_f64().ok_or_else(|| ConditionError::EvaluationError {
            expression: "numeric comparison".to_string(),
            details: format!("Left value is not a number: {actual}"),
        })?;
        
        let expected_num = expected.as_f64().ok_or_else(|| ConditionError::EvaluationError {
            expression: "numeric comparison".to_string(),
            details: format!("Right value is not a number: {expected}"),
        })?;
        
        Ok(op(actual_num, expected_num))
    }
    
    /// Evaluate an 'in' operation
    fn evaluate_in(&self, parameter: &str, values: &[serde_json::Value]) -> Result<bool, ConditionError> {
        let actual = self.variables.get(parameter).ok_or_else(|| ConditionError::ParameterNotAvailable {
            parameter: parameter.to_string(),
        })?;
        
        Ok(values.contains(actual))
    }
    
    /// Evaluate a 'contains' operation
    fn evaluate_contains(&self, parameter: &str, substring: &str) -> Result<bool, ConditionError> {
        let actual = self.variables.get(parameter).ok_or_else(|| ConditionError::ParameterNotAvailable {
            parameter: parameter.to_string(),
        })?;
        
        let actual_str = actual.as_str().ok_or_else(|| ConditionError::EvaluationError {
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
        assert_eq!(condition.description, Some("Production deployment confirmation".to_string()));
    }
    
    #[test]
    fn test_condition_parser_simple_equality() {
        let ast = ConditionParser::parse("deploy_env == 'prod'").unwrap();
        
        match ast {
            ConditionAst::Comparison { parameter, operator, value } => {
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
            ConditionAst::Comparison { parameter, operator, value } => {
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
            ConditionAst::Logical { left, operator, right } => {
                assert_eq!(operator, LogicalOp::And);
                // Verify left side
                if let ConditionAst::Comparison { parameter, operator, value } = left.as_ref() {
                    assert_eq!(parameter, "env");
                    assert_eq!(*operator, ComparisonOp::Equal);
                    assert_eq!(*value, serde_json::Value::String("prod".to_string()));
                }
                // Verify right side  
                if let ConditionAst::Comparison { parameter, operator, value } = right.as_ref() {
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
            ConditionAst::Contains { parameter, substring } => {
                assert_eq!(parameter, "name");
                assert_eq!(substring, "test");
            }
            _ => panic!("Expected contains AST"),
        }
    }
    
    #[test]
    fn test_condition_evaluator_simple_equality() {
        let mut variables = HashMap::new();
        variables.insert("deploy_env".to_string(), serde_json::Value::String("prod".to_string()));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("deploy_env == 'prod'").unwrap();
        
        assert!(result);
    }
    
    #[test]
    fn test_condition_evaluator_equality_false() {
        let mut variables = HashMap::new();
        variables.insert("deploy_env".to_string(), serde_json::Value::String("dev".to_string()));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("deploy_env == 'prod'").unwrap();
        
        assert!(!result);
    }
    
    #[test]
    fn test_condition_evaluator_numeric_comparison() {
        let mut variables = HashMap::new();
        variables.insert("port".to_string(), serde_json::Value::Number(serde_json::Number::from(8080)));
        
        let evaluator = ConditionEvaluator::new(variables);
        
        assert!(evaluator.evaluate("port > 1000").unwrap());
        assert!(!evaluator.evaluate("port < 1000").unwrap());
        assert!(evaluator.evaluate("port >= 8080").unwrap());
        assert!(evaluator.evaluate("port <= 8080").unwrap());
    }
    
    #[test]
    fn test_condition_evaluator_logical_and() {
        let mut variables = HashMap::new();
        variables.insert("env".to_string(), serde_json::Value::String("prod".to_string()));
        variables.insert("confirm".to_string(), serde_json::Value::Bool(true));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("env == 'prod' && confirm == true").unwrap();
        
        assert!(result);
    }
    
    #[test]
    fn test_condition_evaluator_logical_and_false() {
        let mut variables = HashMap::new();
        variables.insert("env".to_string(), serde_json::Value::String("prod".to_string()));
        variables.insert("confirm".to_string(), serde_json::Value::Bool(false));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("env == 'prod' && confirm == true").unwrap();
        
        assert!(!result);
    }
    
    #[test]
    fn test_condition_evaluator_logical_or() {
        let mut variables = HashMap::new();
        variables.insert("env".to_string(), serde_json::Value::String("staging".to_string()));
        variables.insert("urgent".to_string(), serde_json::Value::Bool(true));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("env == 'prod' || urgent == true").unwrap();
        
        assert!(result);
    }
    
    #[test]
    fn test_condition_evaluator_in_operation() {
        let mut variables = HashMap::new();
        variables.insert("env".to_string(), serde_json::Value::String("staging".to_string()));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("env in [\"dev\", \"staging\", \"prod\"]").unwrap();
        
        assert!(result);
    }
    
    #[test]
    fn test_condition_evaluator_in_operation_false() {
        let mut variables = HashMap::new();
        variables.insert("env".to_string(), serde_json::Value::String("test".to_string()));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("env in [\"dev\", \"staging\", \"prod\"]").unwrap();
        
        assert!(!result);
    }
    
    #[test]
    fn test_condition_evaluator_contains_operation() {
        let mut variables = HashMap::new();
        variables.insert("branch_name".to_string(), serde_json::Value::String("feature/new-auth".to_string()));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("branch_name contains \"feature\"").unwrap();
        
        assert!(result);
    }
    
    #[test]
    fn test_condition_evaluator_contains_operation_false() {
        let mut variables = HashMap::new();
        variables.insert("branch_name".to_string(), serde_json::Value::String("main".to_string()));
        
        let evaluator = ConditionEvaluator::new(variables);
        let result = evaluator.evaluate("branch_name contains \"feature\"").unwrap();
        
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
}