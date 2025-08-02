use crate::sah_config::{loader::ConfigurationError, ConfigValue, Configuration};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Validation errors for sah.toml configuration
#[derive(Error, Debug)]
pub enum ValidationError {
    /// Error occurred while loading configuration for validation
    #[error("Configuration loading error: {0}")]
    LoadError(#[from] ConfigurationError),

    /// Variable name is invalid according to naming rules
    #[error("Invalid variable name '{name}': {reason}")]
    InvalidVariableName {
        /// The invalid variable name
        name: String,
        /// Reason why the name is invalid
        reason: String,
    },

    /// Variable name is reserved and cannot be used
    #[error("Variable name '{name}' is reserved and cannot be used")]
    ReservedVariableName {
        /// The reserved variable name
        name: String,
    },

    /// String value exceeds maximum allowed length
    #[error("String value too long: {length} characters (max: {max_length})")]
    StringTooLong {
        /// Actual string length
        length: usize,
        /// Maximum allowed length
        max_length: usize,
    },

    /// Array exceeds maximum allowed number of elements
    #[error("Array too large: {length} elements (max: {max_elements})")]
    ArrayTooLarge {
        /// Actual number of elements
        length: usize,
        /// Maximum allowed elements
        max_elements: usize,
    },

    /// Configuration has too many levels of nesting
    #[error("Configuration nesting too deep: {depth} levels (max: {max_depth})")]
    NestingTooDeep {
        /// Actual nesting depth
        depth: usize,
        /// Maximum allowed depth
        max_depth: usize,
    },

    /// Configuration has too many variables
    #[error("Too many configuration variables: {count} (max: {max_count})")]
    TooManyVariables {
        /// Actual number of variables
        count: usize,
        /// Maximum allowed variables
        max_count: usize,
    },

    /// Custom validation rule failed
    #[error("Validation rule failed: {rule} - {message}")]
    RuleFailed {
        /// Name of the failed rule
        rule: String,
        /// Error message from the rule
        message: String,
    },
}

/// Validation rules that can be applied to configurations
#[derive(Debug, Clone)]
pub enum ValidationRule {
    /// Maximum string length for any string value
    MaxStringLength(usize),
    /// Maximum number of elements in any array
    MaxArrayElements(usize),
    /// Maximum nesting depth for tables
    MaxNestingDepth(usize),
    /// Maximum total number of configuration variables
    MaxVariableCount(usize),
    /// Require specific variable names to be present
    RequiredVariables(Vec<String>),
    /// Forbid specific variable names
    ForbiddenVariables(Vec<String>),
    /// Custom validation function
    Custom {
        /// Name of the custom validation rule
        name: String,
        /// Validation function that takes a configuration and returns success or error message
        validator: fn(&Configuration) -> Result<(), String>,
    },
}

/// Configuration validator with customizable rules
pub struct Validator {
    rules: Vec<ValidationRule>,
    reserved_names: HashSet<String>,
}

impl Validator {
    /// Create a new validator with default rules
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
            reserved_names: Self::default_reserved_names(),
        }
    }

    /// Create a validator with custom rules
    pub fn with_rules(rules: Vec<ValidationRule>) -> Self {
        Self {
            rules,
            reserved_names: Self::default_reserved_names(),
        }
    }

    /// Create a validator with additional reserved names
    pub fn with_reserved_names(mut self, names: Vec<String>) -> Self {
        for name in names {
            self.reserved_names.insert(name);
        }
        self
    }

    /// Add a validation rule
    pub fn add_rule(&mut self, rule: ValidationRule) {
        self.rules.push(rule);
    }

    /// Default validation rules for sah.toml files
    fn default_rules() -> Vec<ValidationRule> {
        vec![
            ValidationRule::MaxStringLength(10_240), // 10KB string limit
            ValidationRule::MaxArrayElements(1000),  // 1000 element array limit
            ValidationRule::MaxNestingDepth(10),     // 10 level nesting limit
            ValidationRule::MaxVariableCount(1000),  // 1000 variable limit
        ]
    }

    /// Default reserved variable names that cannot be used in sah.toml
    fn default_reserved_names() -> HashSet<String> {
        let mut reserved = HashSet::new();

        // Liquid template system reserved words
        reserved.insert("for".to_string());
        reserved.insert("endfor".to_string());
        reserved.insert("if".to_string());
        reserved.insert("endif".to_string());
        reserved.insert("else".to_string());
        reserved.insert("elsif".to_string());
        reserved.insert("elseif".to_string());
        reserved.insert("unless".to_string());
        reserved.insert("endunless".to_string());
        reserved.insert("case".to_string());
        reserved.insert("endcase".to_string());
        reserved.insert("when".to_string());
        reserved.insert("assign".to_string());
        reserved.insert("capture".to_string());
        reserved.insert("endcapture".to_string());
        reserved.insert("include".to_string());
        reserved.insert("layout".to_string());
        reserved.insert("block".to_string());
        reserved.insert("endblock".to_string());

        // SwissArmyHammer internal variables
        reserved.insert("_template_vars".to_string());
        reserved.insert("_workflow_state".to_string());
        reserved.insert("_current_action".to_string());
        reserved.insert("_execution_context".to_string());

        // Common system variables that might cause conflicts
        reserved.insert("env".to_string());
        reserved.insert("system".to_string());
        reserved.insert("config".to_string());
        reserved.insert("settings".to_string());

        reserved
    }

    /// Validate a configuration against all rules
    pub fn validate(&self, config: &Configuration) -> Result<(), ValidationError> {
        // First, validate variable names
        self.validate_variable_names(config)?;

        // Then apply all validation rules
        for rule in &self.rules {
            self.apply_rule(rule, config)?;
        }

        Ok(())
    }

    /// Validate variable names are valid liquid identifiers and not reserved
    fn validate_variable_names(&self, config: &Configuration) -> Result<(), ValidationError> {
        for key in config.values().keys() {
            // Check if name is reserved
            if self.reserved_names.contains(key) {
                return Err(ValidationError::ReservedVariableName { name: key.clone() });
            }

            // Check if name is a valid liquid identifier
            if !Self::is_valid_liquid_identifier(key) {
                return Err(ValidationError::InvalidVariableName {
                    name: key.clone(),
                    reason: "Must be a valid liquid identifier (letters, numbers, underscores, start with letter/underscore)".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Check if a string is a valid liquid template identifier
    fn is_valid_liquid_identifier(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // Must start with letter or underscore
        let first_char = name.chars().next().unwrap();
        if !first_char.is_alphabetic() && first_char != '_' {
            return false;
        }

        // Rest must be alphanumeric or underscore
        name.chars().all(|c| c.is_alphanumeric() || c == '_')
    }

    /// Apply a single validation rule
    fn apply_rule(
        &self,
        rule: &ValidationRule,
        config: &Configuration,
    ) -> Result<(), ValidationError> {
        match rule {
            ValidationRule::MaxStringLength(max_len) => {
                self.validate_string_lengths(config, *max_len)
            }
            ValidationRule::MaxArrayElements(max_elements) => {
                self.validate_array_sizes(config, *max_elements)
            }
            ValidationRule::MaxNestingDepth(max_depth) => {
                self.validate_nesting_depth(config, *max_depth)
            }
            ValidationRule::MaxVariableCount(max_count) => {
                self.validate_variable_count(config, *max_count)
            }
            ValidationRule::RequiredVariables(required) => {
                self.validate_required_variables(config, required)
            }
            ValidationRule::ForbiddenVariables(forbidden) => {
                self.validate_forbidden_variables(config, forbidden)
            }
            ValidationRule::Custom { name, validator } => match validator(config) {
                Ok(()) => Ok(()),
                Err(message) => Err(ValidationError::RuleFailed {
                    rule: name.clone(),
                    message,
                }),
            },
        }
    }

    /// Validate that all string values are within the maximum length
    fn validate_string_lengths(
        &self,
        config: &Configuration,
        max_length: usize,
    ) -> Result<(), ValidationError> {
        self.validate_string_lengths_recursive(config.values(), max_length)
    }

    /// Recursively validate string lengths in nested structures
    fn validate_string_lengths_recursive(
        &self,
        values: &HashMap<String, ConfigValue>,
        max_length: usize,
    ) -> Result<(), ValidationError> {
        for value in values.values() {
            match value {
                ConfigValue::String(s) => {
                    if s.len() > max_length {
                        return Err(ValidationError::StringTooLong {
                            length: s.len(),
                            max_length,
                        });
                    }
                }
                ConfigValue::Array(arr) => {
                    for item in arr {
                        if let ConfigValue::String(s) = item {
                            if s.len() > max_length {
                                return Err(ValidationError::StringTooLong {
                                    length: s.len(),
                                    max_length,
                                });
                            }
                        } else if let ConfigValue::Table(table) = item {
                            self.validate_string_lengths_recursive(table, max_length)?;
                        }
                    }
                }
                ConfigValue::Table(table) => {
                    self.validate_string_lengths_recursive(table, max_length)?;
                }
                _ => {} // Other types don't need string length validation
            }
        }
        Ok(())
    }

    /// Validate that all arrays are within the maximum element count
    fn validate_array_sizes(
        &self,
        config: &Configuration,
        max_elements: usize,
    ) -> Result<(), ValidationError> {
        self.validate_array_sizes_recursive(config.values(), max_elements)
    }

    /// Recursively validate array sizes in nested structures
    fn validate_array_sizes_recursive(
        &self,
        values: &HashMap<String, ConfigValue>,
        max_elements: usize,
    ) -> Result<(), ValidationError> {
        for value in values.values() {
            match value {
                ConfigValue::Array(arr) => {
                    if arr.len() > max_elements {
                        return Err(ValidationError::ArrayTooLarge {
                            length: arr.len(),
                            max_elements,
                        });
                    }
                    // Recursively check nested arrays
                    for item in arr {
                        if let ConfigValue::Table(table) = item {
                            self.validate_array_sizes_recursive(table, max_elements)?;
                        }
                    }
                }
                ConfigValue::Table(table) => {
                    self.validate_array_sizes_recursive(table, max_elements)?;
                }
                _ => {} // Other types don't need array size validation
            }
        }
        Ok(())
    }

    /// Validate that table nesting doesn't exceed the maximum depth
    fn validate_nesting_depth(
        &self,
        config: &Configuration,
        max_depth: usize,
    ) -> Result<(), ValidationError> {
        for value in config.values().values() {
            let depth = self.calculate_nesting_depth(value);
            if depth > max_depth {
                return Err(ValidationError::NestingTooDeep { depth, max_depth });
            }
        }
        Ok(())
    }

    /// Calculate the maximum nesting depth of a ConfigValue
    fn calculate_nesting_depth(&self, value: &ConfigValue) -> usize {
        match value {
            ConfigValue::Table(table) => {
                let max_child_depth = table
                    .values()
                    .map(|v| self.calculate_nesting_depth(v))
                    .max()
                    .unwrap_or(0);
                1 + max_child_depth
            }
            ConfigValue::Array(arr) => {
                let max_child_depth = arr
                    .iter()
                    .map(|v| self.calculate_nesting_depth(v))
                    .max()
                    .unwrap_or(0);
                max_child_depth // Arrays don't add to nesting depth for tables
            }
            _ => 0, // Scalar values have no nesting
        }
    }

    /// Validate that the total number of variables doesn't exceed the limit
    fn validate_variable_count(
        &self,
        config: &Configuration,
        max_count: usize,
    ) -> Result<(), ValidationError> {
        let count = self.count_total_variables(config.values());
        if count > max_count {
            return Err(ValidationError::TooManyVariables { count, max_count });
        }
        Ok(())
    }

    /// Count the total number of variables (including nested ones)
    fn count_total_variables(&self, values: &HashMap<String, ConfigValue>) -> usize {
        let mut count = values.len();
        for value in values.values() {
            match value {
                ConfigValue::Table(table) => {
                    count += self.count_total_variables(table);
                }
                ConfigValue::Array(arr) => {
                    for item in arr {
                        if let ConfigValue::Table(table) = item {
                            count += self.count_total_variables(table);
                        }
                    }
                }
                _ => {} // Scalar values already counted
            }
        }
        count
    }

    /// Validate that all required variables are present
    fn validate_required_variables(
        &self,
        config: &Configuration,
        required: &[String],
    ) -> Result<(), ValidationError> {
        for var_name in required {
            if !config.values().contains_key(var_name) {
                return Err(ValidationError::RuleFailed {
                    rule: "RequiredVariables".to_string(),
                    message: format!("Required variable '{}' is missing", var_name),
                });
            }
        }
        Ok(())
    }

    /// Validate that no forbidden variables are present
    fn validate_forbidden_variables(
        &self,
        config: &Configuration,
        forbidden: &[String],
    ) -> Result<(), ValidationError> {
        for var_name in forbidden {
            if config.values().contains_key(var_name) {
                return Err(ValidationError::RuleFailed {
                    rule: "ForbiddenVariables".to_string(),
                    message: format!("Forbidden variable '{}' is present", var_name),
                });
            }
        }
        Ok(())
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_valid_liquid_identifier() {
        assert!(Validator::is_valid_liquid_identifier("valid_name"));
        assert!(Validator::is_valid_liquid_identifier("_underscore"));
        assert!(Validator::is_valid_liquid_identifier("name123"));
        assert!(Validator::is_valid_liquid_identifier("CamelCase"));

        assert!(!Validator::is_valid_liquid_identifier("123invalid"));
        assert!(!Validator::is_valid_liquid_identifier("invalid-name"));
        assert!(!Validator::is_valid_liquid_identifier("invalid.name"));
        assert!(!Validator::is_valid_liquid_identifier(""));
        assert!(!Validator::is_valid_liquid_identifier("invalid name"));
    }

    #[test]
    fn test_reserved_variable_names() {
        let validator = Validator::new();
        let mut config = Configuration::new();
        config.insert("for".to_string(), ConfigValue::String("test".to_string()));

        let result = validator.validate(&config);
        assert!(matches!(
            result,
            Err(ValidationError::ReservedVariableName { .. })
        ));
    }

    #[test]
    fn test_invalid_variable_names() {
        let validator = Validator::new();
        let mut config = Configuration::new();
        config.insert(
            "123invalid".to_string(),
            ConfigValue::String("test".to_string()),
        );

        let result = validator.validate(&config);
        assert!(matches!(
            result,
            Err(ValidationError::InvalidVariableName { .. })
        ));
    }

    #[test]
    fn test_string_length_validation() {
        let validator = Validator::with_rules(vec![ValidationRule::MaxStringLength(10)]);
        let mut config = Configuration::new();
        config.insert("short".to_string(), ConfigValue::String("ok".to_string()));
        config.insert(
            "long".to_string(),
            ConfigValue::String("this_is_too_long".to_string()),
        );

        let result = validator.validate(&config);
        assert!(matches!(result, Err(ValidationError::StringTooLong { .. })));
    }

    #[test]
    fn test_array_size_validation() {
        let validator = Validator::with_rules(vec![ValidationRule::MaxArrayElements(2)]);
        let mut config = Configuration::new();
        let large_array = vec![
            ConfigValue::String("1".to_string()),
            ConfigValue::String("2".to_string()),
            ConfigValue::String("3".to_string()), // Too many elements
        ];
        config.insert("array".to_string(), ConfigValue::Array(large_array));

        let result = validator.validate(&config);
        assert!(matches!(result, Err(ValidationError::ArrayTooLarge { .. })));
    }

    #[test]
    fn test_nesting_depth_validation() {
        let validator = Validator::with_rules(vec![ValidationRule::MaxNestingDepth(2)]);
        let mut config = Configuration::new();

        // Create deeply nested structure
        let mut level3 = HashMap::new();
        level3.insert("deep".to_string(), ConfigValue::String("value".to_string()));

        let mut level2 = HashMap::new();
        level2.insert("level2".to_string(), ConfigValue::Table(level3));

        let mut level1 = HashMap::new();
        level1.insert("level1".to_string(), ConfigValue::Table(level2));

        config.insert("root".to_string(), ConfigValue::Table(level1)); // This exceeds depth of 2

        let result = validator.validate(&config);
        assert!(matches!(
            result,
            Err(ValidationError::NestingTooDeep { .. })
        ));
    }

    #[test]
    fn test_variable_count_validation() {
        let validator = Validator::with_rules(vec![ValidationRule::MaxVariableCount(2)]);
        let mut config = Configuration::new();
        config.insert(
            "var1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        config.insert(
            "var2".to_string(),
            ConfigValue::String("value2".to_string()),
        );
        config.insert(
            "var3".to_string(),
            ConfigValue::String("value3".to_string()),
        ); // Too many

        let result = validator.validate(&config);
        assert!(matches!(
            result,
            Err(ValidationError::TooManyVariables { .. })
        ));
    }

    #[test]
    fn test_required_variables_validation() {
        let validator = Validator::with_rules(vec![ValidationRule::RequiredVariables(vec![
            "project_name".to_string(),
            "version".to_string(),
        ])]);
        let mut config = Configuration::new();
        config.insert(
            "project_name".to_string(),
            ConfigValue::String("MyProject".to_string()),
        );
        // Missing "version"

        let result = validator.validate(&config);
        assert!(matches!(result, Err(ValidationError::RuleFailed { .. })));
    }

    #[test]
    fn test_forbidden_variables_validation() {
        let validator = Validator::with_rules(vec![ValidationRule::ForbiddenVariables(vec![
            "password".to_string(),
            "secret".to_string(),
        ])]);
        let mut config = Configuration::new();
        config.insert(
            "project_name".to_string(),
            ConfigValue::String("MyProject".to_string()),
        );
        config.insert(
            "password".to_string(),
            ConfigValue::String("secret123".to_string()),
        ); // Forbidden

        let result = validator.validate(&config);
        assert!(matches!(result, Err(ValidationError::RuleFailed { .. })));
    }

    #[test]
    fn test_valid_configuration() {
        let validator = Validator::new();
        let mut config = Configuration::new();
        config.insert(
            "project_name".to_string(),
            ConfigValue::String("MyProject".to_string()),
        );
        config.insert("version".to_string(), ConfigValue::Integer(1));
        config.insert("debug".to_string(), ConfigValue::Boolean(true));

        let result = validator.validate(&config);
        assert!(result.is_ok());
    }
}
