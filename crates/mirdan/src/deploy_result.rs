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
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeployResult {
    /// What action was taken.
    pub action: DeployAction,
    /// The path affected (file, directory, or symlink), if applicable.
    pub path: Option<PathBuf>,
    /// Human-readable description of what happened.
    pub message: String,
}

impl DeployResult {
    /// Create a result indicating a new file or directory was created.
    pub fn created(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Created,
            path: Some(path.into()),
            message: message.into(),
        }
    }

    /// Create a result indicating an existing file or directory was updated.
    pub fn updated(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Updated,
            path: Some(path.into()),
            message: message.into(),
        }
    }

    /// Create a result indicating a file, directory, or symlink was removed.
    pub fn removed(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Removed,
            path: Some(path.into()),
            message: message.into(),
        }
    }

    /// Create a result indicating a symlink was created.
    pub fn linked(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Linked,
            path: Some(path.into()),
            message: message.into(),
        }
    }

    /// Create a result indicating an operation was skipped.
    pub fn skipped(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Skipped,
            path: Some(path.into()),
            message: message.into(),
        }
    }

    /// Create a result indicating a non-fatal warning.
    pub fn warning(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            action: DeployAction::Warning,
            path: Some(path.into()),
            message: message.into(),
        }
    }

    /// Create a result with no specific path affected.
    pub fn message(action: DeployAction, message: impl Into<String>) -> Self {
        Self {
            action,
            path: None,
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
        assert_eq!(r.path, Some(PathBuf::from("/tmp/foo")));
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

    #[test]
    fn test_deploy_result_message_has_no_path() {
        let r = DeployResult::message(DeployAction::Created, "Installed from registry");
        assert_eq!(r.action, DeployAction::Created);
        assert_eq!(r.path, None);
        assert_eq!(r.message, "Installed from registry");
    }

    #[test]
    fn test_deploy_result_equality() {
        let a = DeployResult::created("/tmp/foo", "test");
        let b = DeployResult::created("/tmp/foo", "test");
        assert_eq!(a, b);

        let c = DeployResult::message(DeployAction::Created, "test");
        assert_ne!(a, c);
    }

    #[test]
    fn test_deploy_result_updated() {
        let r = DeployResult::updated("/tmp/bar", "Updated /tmp/bar");
        assert_eq!(r.action, DeployAction::Updated);
        assert_eq!(r.path, Some(PathBuf::from("/tmp/bar")));
        assert_eq!(r.message, "Updated /tmp/bar");
    }

    #[test]
    fn test_deploy_action_clone() {
        let actions = [
            DeployAction::Created,
            DeployAction::Updated,
            DeployAction::Removed,
            DeployAction::Linked,
            DeployAction::Skipped,
            DeployAction::Warning,
        ];
        for action in &actions {
            let cloned = action.clone();
            assert_eq!(*action, cloned);
        }
    }

    #[test]
    fn test_deploy_action_debug() {
        // Exercise Debug for every variant.
        assert_eq!(format!("{:?}", DeployAction::Created), "Created");
        assert_eq!(format!("{:?}", DeployAction::Updated), "Updated");
        assert_eq!(format!("{:?}", DeployAction::Removed), "Removed");
        assert_eq!(format!("{:?}", DeployAction::Linked), "Linked");
        assert_eq!(format!("{:?}", DeployAction::Skipped), "Skipped");
        assert_eq!(format!("{:?}", DeployAction::Warning), "Warning");
    }

    #[test]
    fn test_deploy_result_clone() {
        let original = DeployResult::updated("/tmp/baz", "cloned");
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_deploy_result_debug() {
        let r = DeployResult::updated("/tmp/dbg", "debug test");
        let dbg = format!("{:?}", r);
        assert!(dbg.contains("Updated"));
        assert!(dbg.contains("debug test"));
    }

    #[test]
    fn test_deploy_result_serializes_all_actions() {
        // Ensure every action variant serializes correctly.
        let cases = [
            (DeployResult::created("/a", "c"), "Created"),
            (DeployResult::updated("/a", "u"), "Updated"),
            (DeployResult::removed("/a", "r"), "Removed"),
            (DeployResult::linked("/a", "l"), "Linked"),
            (DeployResult::skipped("/a", "s"), "Skipped"),
            (DeployResult::warning("/a", "w"), "Warning"),
        ];
        for (result, expected_action) in &cases {
            let json = serde_json::to_string(result).unwrap();
            assert!(
                json.contains(&format!("\"action\":\"{}\"", expected_action)),
                "Expected action {} in JSON: {}",
                expected_action,
                json
            );
        }
    }

    #[test]
    fn test_deploy_result_message_all_actions() {
        // Exercise `message()` with every action variant (no path).
        let actions = [
            DeployAction::Created,
            DeployAction::Updated,
            DeployAction::Removed,
            DeployAction::Linked,
            DeployAction::Skipped,
            DeployAction::Warning,
        ];
        for action in actions {
            let r = DeployResult::message(action.clone(), "no path");
            assert_eq!(r.path, None);
            assert_eq!(r.message, "no path");
            assert_eq!(r.action, action);
        }
    }
}
