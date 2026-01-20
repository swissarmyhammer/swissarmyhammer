//! Health check framework for SwissArmyHammer components
//!
//! This module provides a trait-based system for components to report their health status.
//! Components can implement the `Doctorable` trait to provide diagnostic information
//! to the `sah doctor` command.

/// Status of a health check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Check passed - component is healthy
    Ok,
    /// Check passed with warnings - component may have issues
    Warning,
    /// Check failed - component has errors
    Error,
}

/// Result of a health check
#[derive(Debug, Clone)]
pub struct HealthCheck {
    /// Name of the check
    pub name: String,
    /// Status of the check
    pub status: HealthStatus,
    /// Human-readable message describing the check result
    pub message: String,
    /// Optional suggestion for fixing the issue
    pub fix: Option<String>,
    /// Category this check belongs to (e.g., "system", "tools", "configuration")
    pub category: String,
}

impl HealthCheck {
    /// Create a new health check with OK status
    pub fn ok(name: impl Into<String>, message: impl Into<String>, category: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Ok,
            message: message.into(),
            fix: None,
            category: category.into(),
        }
    }

    /// Create a new health check with Warning status
    pub fn warning(
        name: impl Into<String>,
        message: impl Into<String>,
        fix: Option<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Warning,
            message: message.into(),
            fix,
            category: category.into(),
        }
    }

    /// Create a new health check with Error status
    pub fn error(
        name: impl Into<String>,
        message: impl Into<String>,
        fix: Option<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Error,
            message: message.into(),
            fix,
            category: category.into(),
        }
    }
}

/// Trait for components that can report their health status
///
/// Implement this trait for any component that needs to be checked by `sah doctor`.
/// Components can be MCP tools, system utilities, integrations, or any other
/// part of the system that has health requirements.
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_common::health::{Doctorable, HealthCheck};
///
/// struct MyTool;
///
/// impl Doctorable for MyTool {
///     fn name(&self) -> &str {
///         "My Tool"
///     }
///
///     fn category(&self) -> &str {
///         "tools"
///     }
///
///     fn run_health_checks(&self) -> Vec<HealthCheck> {
///         vec![
///             HealthCheck::ok(
///                 "Tool Configuration",
///                 "Configuration is valid",
///                 self.category()
///             ),
///         ]
///     }
/// }
/// ```
pub trait Doctorable {
    /// Name of the component being checked
    ///
    /// This should be a human-readable name that identifies the component.
    fn name(&self) -> &str;

    /// Category this component belongs to
    ///
    /// Common categories: "system", "tools", "integrations", "configuration"
    fn category(&self) -> &str;

    /// Run health checks and return results
    ///
    /// Implementations should:
    /// - Check all relevant health conditions
    /// - Return one HealthCheck per condition
    /// - Provide clear messages and actionable fixes
    /// - Avoid blocking or expensive operations
    fn run_health_checks(&self) -> Vec<HealthCheck>;

    /// Optional: Check if this component is available/applicable
    ///
    /// Return false to skip health checks for this component entirely.
    /// Useful for optional dependencies or platform-specific components.
    fn is_applicable(&self) -> bool {
        true
    }
}

/// Registry for Doctorable components
///
/// Collects all components that implement Doctorable and provides
/// a unified interface for running health checks across the system.
pub struct HealthCheckRegistry {
    components: Vec<Box<dyn Doctorable>>,
}

impl HealthCheckRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    /// Register a component for health checking
    pub fn register<T: Doctorable + 'static>(&mut self, component: T) {
        self.components.push(Box::new(component));
    }

    /// Run health checks for all registered components
    ///
    /// Returns a flattened list of all health checks from all components.
    pub fn run_all_checks(&self) -> Vec<HealthCheck> {
        self.components
            .iter()
            .filter(|c| c.is_applicable())
            .flat_map(|c| c.run_health_checks())
            .collect()
    }

    /// Get all registered component names
    pub fn component_names(&self) -> Vec<&str> {
        self.components
            .iter()
            .filter(|c| c.is_applicable())
            .map(|c| c.name())
            .collect()
    }

    /// Get number of registered components
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

impl Default for HealthCheckRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestComponent {
        name: String,
        should_fail: bool,
    }

    impl Doctorable for TestComponent {
        fn name(&self) -> &str {
            &self.name
        }

        fn category(&self) -> &str {
            "test"
        }

        fn run_health_checks(&self) -> Vec<HealthCheck> {
            if self.should_fail {
                vec![HealthCheck::error(
                    "Test Check",
                    "Test failed",
                    Some("Fix the test".to_string()),
                    self.category(),
                )]
            } else {
                vec![HealthCheck::ok(
                    "Test Check",
                    "Test passed",
                    self.category(),
                )]
            }
        }
    }

    #[test]
    fn test_health_check_creation() {
        let ok_check = HealthCheck::ok("test", "message", "category");
        assert_eq!(ok_check.status, HealthStatus::Ok);
        assert_eq!(ok_check.name, "test");

        let warn_check = HealthCheck::warning("test", "message", Some("fix".to_string()), "category");
        assert_eq!(warn_check.status, HealthStatus::Warning);
        assert!(warn_check.fix.is_some());

        let error_check = HealthCheck::error("test", "message", None, "category");
        assert_eq!(error_check.status, HealthStatus::Error);
    }

    #[test]
    fn test_registry_basic() {
        let mut registry = HealthCheckRegistry::new();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());

        registry.register(TestComponent {
            name: "Test 1".to_string(),
            should_fail: false,
        });
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let names = registry.component_names();
        assert_eq!(names, vec!["Test 1"]);
    }

    #[test]
    fn test_registry_run_checks() {
        let mut registry = HealthCheckRegistry::new();

        registry.register(TestComponent {
            name: "Passing Component".to_string(),
            should_fail: false,
        });

        registry.register(TestComponent {
            name: "Failing Component".to_string(),
            should_fail: true,
        });

        let checks = registry.run_all_checks();
        assert_eq!(checks.len(), 2);

        let ok_checks: Vec<_> = checks.iter().filter(|c| c.status == HealthStatus::Ok).collect();
        let error_checks: Vec<_> = checks.iter().filter(|c| c.status == HealthStatus::Error).collect();

        assert_eq!(ok_checks.len(), 1);
        assert_eq!(error_checks.len(), 1);
    }

    #[test]
    fn test_is_applicable() {
        struct ConditionalComponent {
            applicable: bool,
        }

        impl Doctorable for ConditionalComponent {
            fn name(&self) -> &str {
                "Conditional"
            }

            fn category(&self) -> &str {
                "test"
            }

            fn run_health_checks(&self) -> Vec<HealthCheck> {
                vec![HealthCheck::ok("Check", "OK", self.category())]
            }

            fn is_applicable(&self) -> bool {
                self.applicable
            }
        }

        let mut registry = HealthCheckRegistry::new();
        registry.register(ConditionalComponent { applicable: false });
        registry.register(ConditionalComponent { applicable: true });

        // Only the applicable component should run
        let checks = registry.run_all_checks();
        assert_eq!(checks.len(), 1);

        // But both should be in the registry
        assert_eq!(registry.len(), 2);
    }
}
