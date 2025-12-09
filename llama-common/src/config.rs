//! Configuration trait for validated, consistent configuration across crates

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Trait for configuration types that can be validated and have defaults
///
/// This trait provides a consistent interface for configuration validation
/// across all crates in the llama-agent workspace.
pub trait ValidatedConfig:
    Send + Sync + Clone + Debug + Serialize + for<'de> Deserialize<'de>
{
    type Error: std::error::Error + Send + Sync + 'static;

    /// Validate the configuration, returning an error if invalid
    fn validate(&self) -> Result<(), Self::Error>;

    /// Merge this configuration with defaults, preferring this config's values
    fn merge_with_defaults(self, defaults: Self) -> Self;

    /// Get a description of what this configuration controls
    fn description() -> &'static str;
}

/// Helper trait for configurations that can be created with sensible defaults
pub trait DefaultConfig: ValidatedConfig + Default {
    /// Create a validated default configuration
    fn validated_default() -> Result<Self, Self::Error> {
        let config = Self::default();
        config.validate()?;
        Ok(config)
    }
}

// Blanket implementation for any ValidatedConfig that also implements Default
impl<T> DefaultConfig for T where T: ValidatedConfig + Default {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use thiserror::Error;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestConfig {
        max_value: u32,
        name: String,
    }

    #[derive(Error, Debug)]
    enum TestConfigError {
        #[error("max_value must be greater than 0")]
        InvalidMaxValue,
        #[error("name cannot be empty")]
        EmptyName,
    }

    impl ValidatedConfig for TestConfig {
        type Error = TestConfigError;

        fn validate(&self) -> Result<(), Self::Error> {
            if self.max_value == 0 {
                return Err(TestConfigError::InvalidMaxValue);
            }
            if self.name.is_empty() {
                return Err(TestConfigError::EmptyName);
            }
            Ok(())
        }

        fn merge_with_defaults(self, defaults: Self) -> Self {
            Self {
                max_value: if self.max_value != 0 {
                    self.max_value
                } else {
                    defaults.max_value
                },
                name: if !self.name.is_empty() {
                    self.name
                } else {
                    defaults.name
                },
            }
        }

        fn description() -> &'static str {
            "Test configuration for validation"
        }
    }

    #[test]
    fn test_valid_config() {
        let config = TestConfig {
            max_value: 100,
            name: "test".to_string(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_max_value() {
        let config = TestConfig {
            max_value: 0,
            name: "test".to_string(),
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TestConfigError::InvalidMaxValue
        ));
    }

    #[test]
    fn test_empty_name() {
        let config = TestConfig {
            max_value: 100,
            name: String::new(),
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TestConfigError::EmptyName));
    }

    #[test]
    fn test_merge_with_defaults() {
        let partial = TestConfig {
            max_value: 0,
            name: "custom".to_string(),
        };
        let defaults = TestConfig {
            max_value: 50,
            name: "default".to_string(),
        };

        let merged = partial.merge_with_defaults(defaults);
        assert_eq!(merged.max_value, 50); // From defaults
        assert_eq!(merged.name, "custom"); // From original
    }

    #[test]
    fn test_validated_default() {
        // With a valid default implementation, this should succeed
        let result = TestConfig::validated_default();
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.max_value, 10);
        assert_eq!(config.name, "default");
    }

    impl Default for TestConfig {
        fn default() -> Self {
            Self {
                max_value: 10,
                name: "default".to_string(),
            }
        }
    }
}
