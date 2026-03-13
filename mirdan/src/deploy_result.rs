//! Structured result types for deploy/uninstall operations.
//!
//! Instead of printing directly, install and uninstall functions collect
//! `DeployResult` entries. The CLI or GUI layer is then responsible for
//! formatting them into user-facing output.

use serde::Serialize;
use std::path::PathBuf;

/// What action was performed on a path during deploy/uninstall.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum DeployAction {
    /// Created a new file or directory.
    Created,
    /// Updated an existing file or directory.
    Updated,
    /// Removed a file, directory, or symlink.
    Removed,
    /// Symlinked from target to source.
    Linked,
    /// Skipped (already up to date, or not applicable).
    Skipped,
    /// A non-fatal warning occurred.
    Warning,
}

/// Structured result of a single deploy/uninstall operation.
#[derive(Debug, Clone, Serialize)]
pub struct DeployResult {
    /// What action was taken.
    pub action: DeployAction,
    /// The path affected (file, directory, or symlink).
    pub path: PathBuf,
    /// Human-readable description of what happened.
    pub message: String,
}

impl DeployResult {
    /// Create a result indicating a new file or directory was created.
    pub fn created(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Created,
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a result indicating an existing file or directory was updated.
    pub fn updated(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Updated,
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a result indicating a file, directory, or symlink was removed.
    pub fn removed(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Removed,
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a result indicating a symlink was created.
    pub fn linked(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Linked,
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a result indicating an operation was skipped.
    pub fn skipped(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Skipped,
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a result indicating a non-fatal warning.
    pub fn warning(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Warning,
            path: path.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deploy_result_created() {
        let r = DeployResult::created("/tmp/foo", "Stored in /tmp/foo");
        assert_eq!(r.action, DeployAction::Created);
        assert_eq!(r.path, PathBuf::from("/tmp/foo"));
        assert_eq!(r.message, "Stored in /tmp/foo");
    }

    #[test]
    fn test_deploy_result_linked() {
        let r = DeployResult::linked("/tmp/link", "Linked /tmp/link -> /tmp/target");
        assert_eq!(r.action, DeployAction::Linked);
    }

    #[test]
    fn test_deploy_result_removed() {
        let r = DeployResult::removed("/tmp/gone", "Removed from /tmp/gone");
        assert_eq!(r.action, DeployAction::Removed);
    }

    #[test]
    fn test_deploy_result_skipped() {
        let r = DeployResult::skipped("/tmp/skip", "Skipped agent (no MCP support)");
        assert_eq!(r.action, DeployAction::Skipped);
    }

    #[test]
    fn test_deploy_result_warning() {
        let r = DeployResult::warning("/tmp/warn", "Warning: something odd");
        assert_eq!(r.action, DeployAction::Warning);
    }

    #[test]
    fn test_deploy_action_equality() {
        assert_eq!(DeployAction::Created, DeployAction::Created);
        assert_ne!(DeployAction::Created, DeployAction::Removed);
    }

    #[test]
    fn test_deploy_result_serializes() {
        let r = DeployResult::created("/tmp/foo", "test");
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"action\":\"Created\""));
        assert!(json.contains("\"message\":\"test\""));
    }
}
