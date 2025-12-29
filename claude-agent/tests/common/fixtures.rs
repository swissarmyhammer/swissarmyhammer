//! Core test fixtures for common test setup patterns

use claude_agent::permissions::{FilePermissionStorage, PermissionPolicyEngine};
use claude_agent::session::SessionManager;
use claude_agent::tools::ToolPermissions;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// Create test permission engine with temporary file storage
pub fn permission_engine() -> Arc<PermissionPolicyEngine> {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    Arc::new(PermissionPolicyEngine::new(Box::new(storage)))
}

/// Create test session manager
pub fn session_manager() -> Arc<SessionManager> {
    Arc::new(SessionManager::new())
}

/// Create temporary directory for testing
/// Returns both the TempDir (to keep it alive) and the path
pub fn temp_storage() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    (dir, path)
}

/// Create default test tool permissions with auto-approved test tool
pub fn tool_permissions() -> ToolPermissions {
    ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec!["test_tool".to_string()],
        forbidden_paths: vec![],
    }
}

/// Create test tool permissions with custom settings
pub fn tool_permissions_with(
    require_permission_for: Vec<String>,
    auto_approved: Vec<String>,
    forbidden_paths: Vec<String>,
) -> ToolPermissions {
    ToolPermissions {
        require_permission_for,
        auto_approved,
        forbidden_paths,
    }
}

/// Create test session ID with ACP-compliant format
pub fn session_id(id: &str) -> agent_client_protocol::SessionId {
    agent_client_protocol::SessionId::new(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_engine_creation() {
        let engine = permission_engine();
        assert!(Arc::strong_count(&engine) == 1);
    }

    #[test]
    fn test_session_manager_creation() {
        let manager = session_manager();
        assert!(Arc::strong_count(&manager) == 1);
    }

    #[test]
    fn test_temp_storage_creation() {
        let (_temp_dir, path) = temp_storage();
        assert!(path.exists());
        assert!(path.is_dir());
    }

    #[test]
    fn test_tool_permissions_defaults() {
        let perms = tool_permissions();
        assert!(perms.require_permission_for.is_empty());
        assert_eq!(perms.auto_approved, vec!["test_tool".to_string()]);
        assert!(perms.forbidden_paths.is_empty());
    }

    #[test]
    fn test_tool_permissions_custom() {
        let perms = tool_permissions_with(
            vec!["sensitive_tool".to_string()],
            vec!["safe_tool".to_string()],
            vec!["/etc".to_string()],
        );
        assert_eq!(perms.require_permission_for.len(), 1);
        assert_eq!(perms.auto_approved.len(), 1);
        assert_eq!(perms.forbidden_paths.len(), 1);
    }

    #[test]
    fn test_session_id_creation() {
        let id = session_id("test_session_123");
        assert_eq!(id.0.as_ref(), "test_session_123");
    }
}
