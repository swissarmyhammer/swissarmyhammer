//! Severity levels for rule violations
//!
//! # Example
//!
//! ```
//! use swissarmyhammer_rules::Severity;
//!
//! let severity = Severity::Error;
//! assert_eq!(severity.to_string(), "error");
//!
//! let parsed: Severity = "warning".parse().unwrap();
//! assert_eq!(parsed, Severity::Warning);
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Severity level for rule violations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Error severity - must be fixed
    Error,
    /// Warning severity - should be fixed
    Warning,
    /// Info severity - informational
    Info,
    /// Hint severity - suggestion
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
            Severity::Hint => write!(f, "hint"),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(Severity::Error),
            "warning" => Ok(Severity::Warning),
            "info" => Ok(Severity::Info),
            "hint" => Ok(Severity::Hint),
            _ => Err(format!("Invalid severity: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Info.to_string(), "info");
        assert_eq!(Severity::Hint.to_string(), "hint");
    }

    #[test]
    fn test_severity_from_str() {
        assert_eq!("error".parse::<Severity>().unwrap(), Severity::Error);
        assert_eq!("warning".parse::<Severity>().unwrap(), Severity::Warning);
        assert_eq!("info".parse::<Severity>().unwrap(), Severity::Info);
        assert_eq!("hint".parse::<Severity>().unwrap(), Severity::Hint);
        assert_eq!("ERROR".parse::<Severity>().unwrap(), Severity::Error);
        assert!("invalid".parse::<Severity>().is_err());
    }

    #[test]
    fn test_severity_serde() {
        let severity = Severity::Error;
        let json = serde_json::to_string(&severity).unwrap();
        assert_eq!(json, "\"error\"");
        let deserialized: Severity = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Severity::Error);
    }
}
