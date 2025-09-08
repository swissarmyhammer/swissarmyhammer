//! # SwissArmyHammer Shell Domain Crate
//!
//! This crate provides comprehensive shell command execution security, hardening, and performance
//! monitoring capabilities for the SwissArmyHammer ecosystem.
//!
//! ## Features
//!
//! - **Security Validation**: Command validation, blocked pattern detection, and security policies
//! - **Advanced Hardening**: Threat detection, behavioral analysis, and security assessments
//! - **Performance Monitoring**: Execution profiling, resource usage tracking, and performance metrics
//! - **Audit Logging**: Comprehensive audit trails for all shell command executions
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use swissarmyhammer_shell::{ShellSecurityValidator, ShellSecurityPolicy};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a security validator with default policy
//! let validator = ShellSecurityValidator::with_default_policy()?;
//!
//! // Validate a command
//! validator.validate_command("echo 'Hello, World!'")?;
//!
//! // The command is safe to execute
//! println!("Command passed security validation");
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

/// Shell command security validation and control system
pub mod security;

/// Advanced security hardening for shell command execution
pub mod hardening;

/// Performance monitoring and profiling for shell command execution
pub mod performance;

// Re-export core types for convenience
pub use security::{
    get_validator, log_shell_completion, log_shell_execution, ShellAuditEvent, ShellSecurityError,
    ShellSecurityPolicy, ShellSecurityValidator,
};

pub use hardening::{
    CommandContext, DetectedThreat, HardenedSecurityValidator, SecurityAssessment,
    SecurityHardeningConfig, SecurityMeasure, SecurityStatistics, ThreatDetector, ThreatLevel,
    ThreatType,
};

pub use performance::{
    PerformanceConfig, PerformanceStatistics, ShellPerformanceMetrics, ShellPerformanceProfiler,
};

// Re-export workflow validation functions that are heavily used by swissarmyhammer-tools
// These were previously in swissarmyhammer::workflow but are shell-specific
pub use security::{
    validate_command, validate_environment_variables_security, validate_working_directory_security,
};

/// Result type for shell operations
pub type Result<T> = std::result::Result<T, ShellSecurityError>;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
