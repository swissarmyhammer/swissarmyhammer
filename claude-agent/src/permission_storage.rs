//! Permission preference storage for "always" decisions
//!
//! This module provides in-memory storage for user permission preferences that
//! persist across tool calls within a single agent session, such as "allow-always"
//! and "reject-always" decisions.
//!
//! # Important Note
//!
//! Preferences are stored in-memory only and do not persist across agent restarts.
//! This is intentional for the current implementation to maintain session-level
//! preference isolation and avoid stale permissions accumulating over time.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::tools::PermissionOptionKind;

/// In-memory storage for user permission preferences
///
/// Stores permission preferences for the duration of the agent session.
/// Preferences are lost when the agent process ends. This design prevents
/// accumulation of stale permissions and ensures users make conscious decisions
/// for each agent session.
#[derive(Clone)]
pub struct PermissionStorage {
    /// Map of tool name to stored permission decision
    preferences: Arc<RwLock<HashMap<String, PermissionOptionKind>>>,
}

impl PermissionStorage {
    /// Create a new empty permission storage
    pub fn new() -> Self {
        Self {
            preferences: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if there is a stored preference for a tool
    ///
    /// # Arguments
    /// * `tool_name` - The name of the tool to check
    ///
    /// # Returns
    /// The stored permission kind if one exists, None otherwise
    pub async fn get_preference(&self, tool_name: &str) -> Option<PermissionOptionKind> {
        let prefs = self.preferences.read().await;
        prefs.get(tool_name).cloned()
    }

    /// Store a permission preference for a tool
    ///
    /// Only "always" decisions (AllowAlways, RejectAlways) should be stored.
    /// "Once" decisions should not be stored as they apply only to a single call.
    ///
    /// # Arguments
    /// * `tool_name` - The name of the tool
    /// * `kind` - The permission kind to store
    pub async fn store_preference(&self, tool_name: &str, kind: PermissionOptionKind) {
        match &kind {
            PermissionOptionKind::AllowAlways | PermissionOptionKind::RejectAlways => {
                let mut prefs = self.preferences.write().await;
                prefs.insert(tool_name.to_string(), kind.clone());
                tracing::info!(
                    "Stored permission preference for '{}': {:?}",
                    tool_name,
                    kind
                );
            }
            PermissionOptionKind::AllowOnce | PermissionOptionKind::RejectOnce => {
                tracing::debug!(
                    "Not storing 'once' permission for '{}': {:?}",
                    tool_name,
                    kind
                );
            }
        }
    }

    /// Clear all stored preferences
    ///
    /// This is primarily useful for testing or resetting user preferences.
    pub async fn clear_all(&self) {
        let mut prefs = self.preferences.write().await;
        prefs.clear();
        tracing::info!("Cleared all permission preferences");
    }

    /// Remove a specific tool's preference
    ///
    /// # Arguments
    /// * `tool_name` - The name of the tool whose preference should be removed
    pub async fn remove_preference(&self, tool_name: &str) -> bool {
        let mut prefs = self.preferences.write().await;
        let removed = prefs.remove(tool_name).is_some();
        if removed {
            tracing::info!("Removed permission preference for '{}'", tool_name);
        }
        removed
    }

    /// Get the number of stored preferences
    ///
    /// This method is part of the public API to support monitoring and debugging
    /// of permission state. It can be used by UI components to display the number
    /// of stored preferences or by external tools to verify storage behavior.
    pub async fn count(&self) -> usize {
        let prefs = self.preferences.read().await;
        prefs.len()
    }
}

impl Default for PermissionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_get_allow_always() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("test_tool", PermissionOptionKind::AllowAlways)
            .await;

        let result = storage.get_preference("test_tool").await;
        assert!(matches!(result, Some(PermissionOptionKind::AllowAlways)));
    }

    #[tokio::test]
    async fn test_store_and_get_reject_always() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("test_tool", PermissionOptionKind::RejectAlways)
            .await;

        let result = storage.get_preference("test_tool").await;
        assert!(matches!(result, Some(PermissionOptionKind::RejectAlways)));
    }

    #[tokio::test]
    async fn test_once_decisions_not_stored() {
        let storage = PermissionStorage::new();

        // Store "once" decisions
        storage
            .store_preference("tool1", PermissionOptionKind::AllowOnce)
            .await;
        storage
            .store_preference("tool2", PermissionOptionKind::RejectOnce)
            .await;

        // Verify they were not stored
        assert_eq!(storage.get_preference("tool1").await, None);
        assert_eq!(storage.get_preference("tool2").await, None);
        assert_eq!(storage.count().await, 0);
    }

    #[tokio::test]
    async fn test_clear_all() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("tool1", PermissionOptionKind::AllowAlways)
            .await;
        storage
            .store_preference("tool2", PermissionOptionKind::RejectAlways)
            .await;

        assert_eq!(storage.count().await, 2);

        storage.clear_all().await;

        assert_eq!(storage.count().await, 0);
        assert_eq!(storage.get_preference("tool1").await, None);
        assert_eq!(storage.get_preference("tool2").await, None);
    }

    #[tokio::test]
    async fn test_remove_preference() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("tool1", PermissionOptionKind::AllowAlways)
            .await;
        storage
            .store_preference("tool2", PermissionOptionKind::RejectAlways)
            .await;

        assert_eq!(storage.count().await, 2);

        let removed = storage.remove_preference("tool1").await;
        assert!(removed);

        assert_eq!(storage.count().await, 1);
        assert_eq!(storage.get_preference("tool1").await, None);
        assert!(storage.get_preference("tool2").await.is_some());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_preference() {
        let storage = PermissionStorage::new();

        let removed = storage.remove_preference("nonexistent").await;
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_overwrite_preference() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("test_tool", PermissionOptionKind::AllowAlways)
            .await;

        let result = storage.get_preference("test_tool").await;
        assert!(matches!(result, Some(PermissionOptionKind::AllowAlways)));

        // Overwrite with RejectAlways
        storage
            .store_preference("test_tool", PermissionOptionKind::RejectAlways)
            .await;

        let result = storage.get_preference("test_tool").await;
        assert!(matches!(result, Some(PermissionOptionKind::RejectAlways)));

        // Should still have only one entry
        assert_eq!(storage.count().await, 1);
    }
}
