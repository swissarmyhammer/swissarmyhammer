//! Advanced permission system for tool call authorization
//!
//! Implements ACP-compliant permission management with:
//! - Permission persistence for "always" decisions
//! - Policy-based permission evaluation
//! - Tool pattern matching and context awareness
//! - Permission expiration and cleanup
//! - Storage backend abstraction

use crate::error::{AgentError, Result};
use crate::tools::{PermissionOption, PermissionOptionKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tracing::{debug, error, info, warn};

/// A stored permission decision for future reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPermission {
    /// Tool name or pattern this permission applies to
    pub tool_pattern: String,
    /// The permission decision (allow/deny)
    pub decision: PermissionDecision,
    /// When this permission was granted
    pub granted_at: u64,
    /// When this permission expires (None for no expiration)
    pub expires_at: Option<u64>,
    /// Additional context for the permission
    pub context: HashMap<String, String>,
}

/// The actual permission decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionDecision {
    /// Allow all future calls to this tool
    AllowAlways,
    /// Allow once (for test compatibility)
    #[allow(dead_code)]
    AllowOnce,
    /// Allow (for test compatibility)
    #[allow(dead_code)]
    Allow,
    /// Deny all future calls to this tool
    DenyAlways,
    /// Deny once (for test compatibility)
    #[allow(dead_code)]
    DenyOnce,
    /// Deny (for test compatibility)
    #[allow(dead_code)]
    Deny,
}

/// Permission policy rule for evaluating tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    /// Tool name pattern (supports wildcards)
    pub tool_pattern: String,
    /// Default action for this pattern
    pub default_action: PolicyAction,
    /// Whether user consent is required
    pub require_user_consent: bool,
    /// Whether "allow always" option should be offered
    pub allow_always_option: bool,
    /// Risk level of tools matching this pattern
    pub risk_level: RiskLevel,
}

/// Policy action for permission evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyAction {
    /// Allow the tool call automatically
    Allow,
    /// Deny the tool call automatically  
    Deny,
    /// Ask the user for permission
    AskUser,
}

/// Risk level for permission evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Safe operations that don't modify system state
    Low,
    /// Operations that modify files or settings
    Medium,
    /// Operations that could be dangerous (terminal, network)
    High,
    /// Operations with potential security implications
    Critical,
}

/// Result of permission policy evaluation
#[derive(Debug, Clone)]
pub enum PolicyEvaluation {
    /// Tool call is allowed without user consent
    Allowed,
    /// Tool call is denied
    Denied { reason: String },
    /// User consent is required with these options
    RequireUserConsent { options: Vec<PermissionOption> },
}

/// Permission storage backend trait
#[async_trait::async_trait]
pub trait PermissionStorage: Send + Sync {
    /// Store a permission decision
    async fn store_permission(&self, permission: StoredPermission) -> Result<()>;

    /// Lookup stored permission for a tool
    async fn lookup_permission(&self, tool_name: &str) -> Result<Option<StoredPermission>>;

    /// List all stored permissions
    async fn list_permissions(&self) -> Result<Vec<StoredPermission>>;

    /// Remove expired permissions
    async fn cleanup_expired(&self) -> Result<usize>;

    /// Remove a specific permission
    async fn remove_permission(&self, tool_pattern: &str) -> Result<bool>;

    /// Clear all permissions
    async fn clear_all(&self) -> Result<()>;
}

/// File-based permission storage implementation
#[derive(Debug, Clone)]
pub struct FilePermissionStorage {
    storage_path: PathBuf,
}

impl FilePermissionStorage {
    /// Create new file-based permission storage
    pub fn new(storage_path: PathBuf) -> Self {
        Self { storage_path }
    }

    /// Get the path to the permissions file
    fn permissions_file_path(&self) -> PathBuf {
        self.storage_path.join("permissions.json")
    }

    /// Load permissions from disk
    async fn load_permissions(&self) -> Result<HashMap<String, StoredPermission>> {
        let file_path = self.permissions_file_path();

        if !file_path.exists() {
            debug!("Permissions file does not exist, starting with empty storage");
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&file_path).await.map_err(|e| {
            error!("Failed to read permissions file: {}", e);
            AgentError::Config(format!("Failed to read permissions file: {}", e))
        })?;

        let permissions: HashMap<String, StoredPermission> = serde_json::from_str(&content)
            .map_err(|e| {
                error!("Failed to parse permissions file: {}", e);
                AgentError::Config(format!("Failed to parse permissions file: {}", e))
            })?;

        debug!("Loaded {} permissions from storage", permissions.len());
        Ok(permissions)
    }

    /// Save permissions to disk
    async fn save_permissions(
        &self,
        permissions: &HashMap<String, StoredPermission>,
    ) -> Result<()> {
        // Ensure the storage directory exists
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                error!("Failed to create storage directory: {}", e);
                AgentError::Config(format!("Failed to create storage directory: {}", e))
            })?;
        }
        fs::create_dir_all(&self.storage_path).await.map_err(|e| {
            error!("Failed to create storage directory: {}", e);
            AgentError::Config(format!("Failed to create storage directory: {}", e))
        })?;

        let file_path = self.permissions_file_path();
        let content = serde_json::to_string_pretty(permissions).map_err(|e| {
            error!("Failed to serialize permissions: {}", e);
            AgentError::Config(format!("Failed to serialize permissions: {}", e))
        })?;

        fs::write(&file_path, content).await.map_err(|e| {
            error!("Failed to write permissions file: {}", e);
            AgentError::Config(format!("Failed to write permissions file: {}", e))
        })?;

        debug!("Saved {} permissions to storage", permissions.len());
        Ok(())
    }
}

#[async_trait::async_trait]
impl PermissionStorage for FilePermissionStorage {
    async fn store_permission(&self, permission: StoredPermission) -> Result<()> {
        let mut permissions = self.load_permissions().await?;
        permissions.insert(permission.tool_pattern.clone(), permission);
        self.save_permissions(&permissions).await?;
        info!(
            "Stored permission for tool pattern: {}",
            permissions.keys().last().unwrap()
        );
        Ok(())
    }

    async fn lookup_permission(&self, tool_name: &str) -> Result<Option<StoredPermission>> {
        let permissions = self.load_permissions().await?;

        // First try exact match
        if let Some(permission) = permissions.get(tool_name) {
            return Ok(Some(permission.clone()));
        }

        // Try pattern matching
        for (pattern, permission) in permissions {
            if matches_tool_pattern(&pattern, tool_name) {
                debug!(
                    "Found matching permission pattern '{}' for tool '{}'",
                    pattern, tool_name
                );
                return Ok(Some(permission));
            }
        }

        debug!("No stored permission found for tool: {}", tool_name);
        Ok(None)
    }

    async fn list_permissions(&self) -> Result<Vec<StoredPermission>> {
        let permissions = self.load_permissions().await?;
        Ok(permissions.into_values().collect())
    }

    async fn cleanup_expired(&self) -> Result<usize> {
        let mut permissions = self.load_permissions().await?;
        let now = current_timestamp();
        let original_count = permissions.len();

        permissions.retain(|_, perm| {
            if let Some(expires_at) = perm.expires_at {
                expires_at > now
            } else {
                true
            }
        });

        let removed_count = original_count - permissions.len();
        if removed_count > 0 {
            self.save_permissions(&permissions).await?;
            info!("Cleaned up {} expired permissions", removed_count);
        }

        Ok(removed_count)
    }

    async fn remove_permission(&self, tool_pattern: &str) -> Result<bool> {
        let mut permissions = self.load_permissions().await?;
        let removed = permissions.remove(tool_pattern).is_some();
        if removed {
            self.save_permissions(&permissions).await?;
            info!("Removed permission for tool pattern: {}", tool_pattern);
        }
        Ok(removed)
    }

    async fn clear_all(&self) -> Result<()> {
        let file_path = self.permissions_file_path();
        if file_path.exists() {
            fs::remove_file(&file_path).await.map_err(|e| {
                error!("Failed to remove permissions file: {}", e);
                AgentError::Config(format!("Failed to remove permissions file: {}", e))
            })?;
            info!("Cleared all permissions");
        }
        Ok(())
    }
}

/// Permission policy engine for evaluating tool call permissions
pub struct PermissionPolicyEngine {
    storage: Box<dyn PermissionStorage>,
    policies: Vec<PermissionPolicy>,
}

impl PermissionPolicyEngine {
    /// Create new permission policy engine with storage backend
    pub fn new(storage: Box<dyn PermissionStorage>) -> Self {
        let policies = default_permission_policies();
        Self { storage, policies }
    }

    /// Create with custom policies
    pub fn with_policies(
        storage: Box<dyn PermissionStorage>,
        policies: Vec<PermissionPolicy>,
    ) -> Self {
        Self { storage, policies }
    }

    /// Evaluate a tool call against stored permissions and policies
    pub async fn evaluate_tool_call(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Result<PolicyEvaluation> {
        // First check if we have a stored permission for this tool
        if let Some(stored) = self.storage.lookup_permission(tool_name).await? {
            // Check if stored permission is still valid
            if let Some(expires_at) = stored.expires_at {
                if current_timestamp() >= expires_at {
                    warn!("Stored permission for '{}' has expired", tool_name);
                } else {
                    // For "once" permissions, remove them after first use
                    let result = match stored.decision {
                        PermissionDecision::AllowAlways => PolicyEvaluation::Allowed,
                        PermissionDecision::AllowOnce | PermissionDecision::Allow => {
                            // Remove the permission so it's only used once
                            let _ = self.remove_permission(tool_name).await;
                            PolicyEvaluation::Allowed
                        }
                        PermissionDecision::DenyAlways => PolicyEvaluation::Denied {
                            reason: "Tool access denied by stored permission".to_string(),
                        },
                        PermissionDecision::DenyOnce | PermissionDecision::Deny => {
                            // Remove the permission so it's only used once
                            let _ = self.remove_permission(tool_name).await;
                            PolicyEvaluation::Denied {
                                reason: "Tool access denied by stored permission".to_string(),
                            }
                        }
                    };
                    return Ok(result);
                }
            } else {
                // For "once" permissions, remove them after first use
                let result = match stored.decision {
                    PermissionDecision::AllowAlways => PolicyEvaluation::Allowed,
                    PermissionDecision::AllowOnce | PermissionDecision::Allow => {
                        // Remove the permission so it's only used once
                        let _ = self.remove_permission(tool_name).await;
                        PolicyEvaluation::Allowed
                    }
                    PermissionDecision::DenyAlways => PolicyEvaluation::Denied {
                        reason: "Tool access denied by stored permission".to_string(),
                    },
                    PermissionDecision::DenyOnce | PermissionDecision::Deny => {
                        // Remove the permission so it's only used once
                        let _ = self.remove_permission(tool_name).await;
                        PolicyEvaluation::Denied {
                            reason: "Tool access denied by stored permission".to_string(),
                        }
                    }
                };
                return Ok(result);
            }
        }

        // Evaluate against policies
        for policy in &self.policies {
            if matches_tool_pattern(&policy.tool_pattern, tool_name) {
                debug!(
                    "Applying policy '{}' to tool '{}'",
                    policy.tool_pattern, tool_name
                );
                return Ok(self.apply_policy(policy, tool_name, args));
            }
        }

        // Default policy: require user consent for unknown tools
        debug!(
            "No matching policy found for '{}', requiring user consent",
            tool_name
        );
        Ok(PolicyEvaluation::RequireUserConsent {
            options: self.generate_permission_options(tool_name, RiskLevel::Medium),
        })
    }

    /// Store a permission decision
    pub async fn store_permission_decision(
        &self,
        tool_name: &str,
        decision: PermissionDecision,
        expires_in: Option<Duration>,
    ) -> Result<()> {
        let now = current_timestamp();
        let expires_at = expires_in.map(|d| now + d.as_secs());

        let stored_permission = StoredPermission {
            tool_pattern: tool_name.to_string(),
            decision,
            granted_at: now,
            expires_at,
            context: HashMap::new(),
        };

        self.storage.store_permission(stored_permission).await
    }

    /// Check if a tool call is allowed by stored permissions
    pub async fn is_tool_allowed(&self, tool_name: &str) -> Result<bool> {
        if let Some(stored) = self.storage.lookup_permission(tool_name).await? {
            // Check if stored permission is still valid
            if let Some(expires_at) = stored.expires_at {
                if current_timestamp() >= expires_at {
                    return Ok(false);
                }
            }

            Ok(matches!(stored.decision, PermissionDecision::AllowAlways))
        } else {
            Ok(false)
        }
    }

    /// Check if a tool call is denied by stored permissions
    pub async fn is_tool_denied(&self, tool_name: &str) -> Result<bool> {
        if let Some(stored) = self.storage.lookup_permission(tool_name).await? {
            // Check if stored permission is still valid
            if let Some(expires_at) = stored.expires_at {
                if current_timestamp() >= expires_at {
                    return Ok(false);
                }
            }

            Ok(matches!(stored.decision, PermissionDecision::DenyAlways))
        } else {
            Ok(false)
        }
    }

    /// Lookup a stored permission for a tool
    pub async fn lookup_permission(&self, tool_name: &str) -> Result<Option<StoredPermission>> {
        self.storage.lookup_permission(tool_name).await
    }

    /// List all stored permissions
    pub async fn list_permissions(&self) -> Result<Vec<StoredPermission>> {
        self.storage.list_permissions().await
    }

    /// Remove a specific permission
    pub async fn remove_permission(&self, tool_pattern: &str) -> Result<bool> {
        self.storage.remove_permission(tool_pattern).await
    }

    /// Clear all stored permissions
    pub async fn clear_all_permissions(&self) -> Result<()> {
        self.storage.clear_all().await
    }

    /// Remove expired permissions and return the count of removed items
    pub async fn cleanup_expired_permissions(&self) -> Result<usize> {
        self.storage.cleanup_expired().await
    }

    /// Apply a specific policy to a tool call
    fn apply_policy(
        &self,
        policy: &PermissionPolicy,
        tool_name: &str,
        _args: &serde_json::Value,
    ) -> PolicyEvaluation {
        match policy.default_action {
            PolicyAction::Allow => PolicyEvaluation::Allowed,
            PolicyAction::Deny => PolicyEvaluation::Denied {
                reason: format!("Tool '{}' is denied by policy", tool_name),
            },
            PolicyAction::AskUser => {
                if policy.require_user_consent {
                    PolicyEvaluation::RequireUserConsent {
                        options: self
                            .generate_permission_options(tool_name, policy.risk_level.clone()),
                    }
                } else {
                    PolicyEvaluation::Allowed
                }
            }
        }
    }

    /// Generate permission options based on tool and risk level
    fn generate_permission_options(
        &self,
        tool_name: &str,
        risk_level: RiskLevel,
    ) -> Vec<PermissionOption> {
        let mut options = vec![
            PermissionOption {
                option_id: "allow-once".to_string(),
                name: "Allow once".to_string(),
                kind: PermissionOptionKind::AllowOnce,
            },
            PermissionOption {
                option_id: "reject-once".to_string(),
                name: "Reject".to_string(),
                kind: PermissionOptionKind::RejectOnce,
            },
        ];

        // Add "always" options based on risk level
        match risk_level {
            RiskLevel::Low => {
                // Low risk tools can have allow always
                options.insert(
                    1,
                    PermissionOption {
                        option_id: "allow-always".to_string(),
                        name: "Allow always".to_string(),
                        kind: PermissionOptionKind::AllowAlways,
                    },
                );
            }
            RiskLevel::Medium => {
                // Medium risk tools get both always options but with warnings
                options.insert(
                    1,
                    PermissionOption {
                        option_id: "allow-always".to_string(),
                        name: format!("Allow always ({})", tool_name),
                        kind: PermissionOptionKind::AllowAlways,
                    },
                );
                options.push(PermissionOption {
                    option_id: "reject-always".to_string(),
                    name: format!("Reject always ({})", tool_name),
                    kind: PermissionOptionKind::RejectAlways,
                });
            }
            RiskLevel::High | RiskLevel::Critical => {
                // High risk tools only get reject always option, no allow always
                options.push(PermissionOption {
                    option_id: "reject-always".to_string(),
                    name: format!("Reject always ({})", tool_name),
                    kind: PermissionOptionKind::RejectAlways,
                });
            }
        }

        options
    }
}

/// Check if a tool pattern matches a tool name (supports basic wildcards)
fn matches_tool_pattern(pattern: &str, tool_name: &str) -> bool {
    if pattern == "*" || pattern == tool_name {
        return true;
    }

    // Support prefix wildcards like "fs_*"
    if let Some(prefix) = pattern.strip_suffix('*') {
        return tool_name.starts_with(prefix);
    }

    // Support suffix wildcards like "*_read"
    if let Some(suffix) = pattern.strip_prefix('*') {
        return tool_name.ends_with(suffix);
    }

    false
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Default permission policies for common tool patterns
fn default_permission_policies() -> Vec<PermissionPolicy> {
    vec![
        // File system read operations - low risk
        PermissionPolicy {
            tool_pattern: "fs_read*".to_string(),
            default_action: PolicyAction::Allow,
            require_user_consent: false,
            allow_always_option: true,
            risk_level: RiskLevel::Low,
        },
        // File system write operations - medium risk
        PermissionPolicy {
            tool_pattern: "fs_write*".to_string(),
            default_action: PolicyAction::AskUser,
            require_user_consent: true,
            allow_always_option: true,
            risk_level: RiskLevel::Medium,
        },
        // Terminal operations - high risk
        PermissionPolicy {
            tool_pattern: "terminal*".to_string(),
            default_action: PolicyAction::AskUser,
            require_user_consent: true,
            allow_always_option: false,
            risk_level: RiskLevel::High,
        },
        // Network operations - high risk
        PermissionPolicy {
            tool_pattern: "http*".to_string(),
            default_action: PolicyAction::AskUser,
            require_user_consent: true,
            allow_always_option: false,
            risk_level: RiskLevel::High,
        },
        // Default for unknown tools - medium risk
        PermissionPolicy {
            tool_pattern: "*".to_string(),
            default_action: PolicyAction::AskUser,
            require_user_consent: true,
            allow_always_option: true,
            risk_level: RiskLevel::Medium,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_storage() -> FilePermissionStorage {
        let temp_dir = tempdir().unwrap();
        FilePermissionStorage::new(temp_dir.path().to_path_buf())
    }

    #[tokio::test]
    async fn test_file_storage_store_and_lookup() {
        let storage = create_test_storage();

        let permission = StoredPermission {
            tool_pattern: "test_tool".to_string(),
            decision: PermissionDecision::AllowAlways,
            granted_at: current_timestamp(),
            expires_at: None,
            context: HashMap::new(),
        };

        storage.store_permission(permission.clone()).await.unwrap();

        let retrieved = storage.lookup_permission("test_tool").await.unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.tool_pattern, "test_tool");
        assert!(matches!(
            retrieved.decision,
            PermissionDecision::AllowAlways
        ));
    }

    #[tokio::test]
    async fn test_pattern_matching() {
        assert!(matches_tool_pattern("*", "any_tool"));
        assert!(matches_tool_pattern("fs_*", "fs_read"));
        assert!(matches_tool_pattern("fs_*", "fs_write"));
        assert!(matches_tool_pattern("*_read", "fs_read"));
        assert!(matches_tool_pattern("exact_match", "exact_match"));

        assert!(!matches_tool_pattern("fs_*", "terminal_create"));
        assert!(!matches_tool_pattern("*_read", "fs_write"));
    }

    #[tokio::test]
    async fn test_permission_cleanup() {
        let storage = create_test_storage();

        // Add expired permission
        let expired_permission = StoredPermission {
            tool_pattern: "expired_tool".to_string(),
            decision: PermissionDecision::AllowAlways,
            granted_at: current_timestamp() - 3600, // 1 hour ago
            expires_at: Some(current_timestamp() - 1800), // expired 30 min ago
            context: HashMap::new(),
        };

        // Add valid permission
        let valid_permission = StoredPermission {
            tool_pattern: "valid_tool".to_string(),
            decision: PermissionDecision::AllowAlways,
            granted_at: current_timestamp(),
            expires_at: Some(current_timestamp() + 3600), // expires in 1 hour
            context: HashMap::new(),
        };

        storage.store_permission(expired_permission).await.unwrap();
        storage.store_permission(valid_permission).await.unwrap();

        let removed_count = storage.cleanup_expired().await.unwrap();
        assert_eq!(removed_count, 1);

        // Verify expired permission is gone
        let result = storage.lookup_permission("expired_tool").await.unwrap();
        assert!(result.is_none());

        // Verify valid permission remains
        let result = storage.lookup_permission("valid_tool").await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_policy_engine_evaluation() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Test fs_read (should be allowed by default policy)
        let result = engine
            .evaluate_tool_call("fs_read_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Test fs_write (should require user consent)
        let result = engine
            .evaluate_tool_call("fs_write_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));

        // Test terminal (should require user consent)
        let result = engine
            .evaluate_tool_call("terminal_create", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));
    }

    #[tokio::test]
    async fn test_stored_permission_override() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store a deny-always permission for fs_write
        engine
            .store_permission_decision("fs_write_file", PermissionDecision::DenyAlways, None)
            .await
            .unwrap();

        // Should be denied even though policy would ask user
        let result = engine
            .evaluate_tool_call("fs_write_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Denied { .. }));
    }

    #[tokio::test]
    async fn test_is_tool_allowed() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Initially, tool should not be allowed
        let result = engine.is_tool_allowed("test_tool").await.unwrap();
        assert!(!result);

        // Store an allow-always permission
        engine
            .store_permission_decision("test_tool", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // Now tool should be allowed
        let result = engine.is_tool_allowed("test_tool").await.unwrap();
        assert!(result);

        // Store a deny-always permission
        engine
            .store_permission_decision("test_tool", PermissionDecision::DenyAlways, None)
            .await
            .unwrap();

        // Now tool should not be allowed
        let result = engine.is_tool_allowed("test_tool").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_is_tool_denied() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Initially, tool should not be denied
        let result = engine.is_tool_denied("test_tool").await.unwrap();
        assert!(!result);

        // Store a deny-always permission
        engine
            .store_permission_decision("test_tool", PermissionDecision::DenyAlways, None)
            .await
            .unwrap();

        // Now tool should be denied
        let result = engine.is_tool_denied("test_tool").await.unwrap();
        assert!(result);

        // Store an allow-always permission
        engine
            .store_permission_decision("test_tool", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // Now tool should not be denied
        let result = engine.is_tool_denied("test_tool").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_is_tool_allowed_with_expiration() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store an expired allow-always permission
        engine
            .store_permission_decision(
                "test_tool",
                PermissionDecision::AllowAlways,
                Some(Duration::from_secs(0)), // Already expired
            )
            .await
            .unwrap();

        // Tool should not be allowed due to expiration
        let result = engine.is_tool_allowed("test_tool").await.unwrap();
        assert!(!result);

        // Store a valid allow-always permission
        engine
            .store_permission_decision(
                "test_tool",
                PermissionDecision::AllowAlways,
                Some(Duration::from_secs(3600)), // Expires in 1 hour
            )
            .await
            .unwrap();

        // Now tool should be allowed
        let result = engine.is_tool_allowed("test_tool").await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_lookup_permission() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Initially, no permission should be found
        let result = engine.lookup_permission("test_tool").await.unwrap();
        assert!(result.is_none());

        // Store a permission
        engine
            .store_permission_decision("test_tool", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // Now permission should be found
        let result = engine.lookup_permission("test_tool").await.unwrap();
        assert!(result.is_some());
        let perm = result.unwrap();
        assert_eq!(perm.tool_pattern, "test_tool");
        assert!(matches!(perm.decision, PermissionDecision::AllowAlways));
    }

    #[tokio::test]
    async fn test_list_permissions() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Initially, list should be empty
        let result = engine.list_permissions().await.unwrap();
        assert_eq!(result.len(), 0);

        // Store multiple permissions
        engine
            .store_permission_decision("tool1", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();
        engine
            .store_permission_decision("tool2", PermissionDecision::DenyAlways, None)
            .await
            .unwrap();

        // List should now have 2 items
        let result = engine.list_permissions().await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_remove_permission() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store a permission
        engine
            .store_permission_decision("test_tool", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // Verify it exists
        let result = engine.lookup_permission("test_tool").await.unwrap();
        assert!(result.is_some());

        // Remove it
        let removed = engine.remove_permission("test_tool").await.unwrap();
        assert!(removed);

        // Verify it's gone
        let result = engine.lookup_permission("test_tool").await.unwrap();
        assert!(result.is_none());

        // Try to remove again, should return false
        let removed = engine.remove_permission("test_tool").await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_clear_all_permissions() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store multiple permissions
        engine
            .store_permission_decision("tool1", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();
        engine
            .store_permission_decision("tool2", PermissionDecision::DenyAlways, None)
            .await
            .unwrap();

        // Verify they exist
        let result = engine.list_permissions().await.unwrap();
        assert_eq!(result.len(), 2);

        // Clear all
        engine.clear_all_permissions().await.unwrap();

        // Verify they're all gone
        let result = engine.list_permissions().await.unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_cleanup_expired_permissions() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store an expired permission
        engine
            .store_permission_decision(
                "expired_tool",
                PermissionDecision::AllowAlways,
                Some(Duration::from_secs(0)), // Already expired
            )
            .await
            .unwrap();

        // Store a valid permission
        engine
            .store_permission_decision(
                "valid_tool",
                PermissionDecision::AllowAlways,
                Some(Duration::from_secs(3600)), // Expires in 1 hour
            )
            .await
            .unwrap();

        // Cleanup expired permissions
        let removed_count = engine.cleanup_expired_permissions().await.unwrap();
        assert_eq!(removed_count, 1);

        // Verify expired permission is gone
        let result = engine.lookup_permission("expired_tool").await.unwrap();
        assert!(result.is_none());

        // Verify valid permission remains
        let result = engine.lookup_permission("valid_tool").await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_evaluate_tool_call_with_custom_policies() {
        let storage = create_test_storage();

        // Create custom policies
        let custom_policies = vec![
            PermissionPolicy {
                tool_pattern: "custom_*".to_string(),
                default_action: PolicyAction::Allow,
                require_user_consent: false,
                allow_always_option: true,
                risk_level: RiskLevel::Low,
            },
            PermissionPolicy {
                tool_pattern: "dangerous_*".to_string(),
                default_action: PolicyAction::Deny,
                require_user_consent: false,
                allow_always_option: false,
                risk_level: RiskLevel::Critical,
            },
        ];

        let engine = PermissionPolicyEngine::with_policies(Box::new(storage), custom_policies);

        // Test custom allow policy
        let result = engine
            .evaluate_tool_call("custom_read", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Test custom deny policy
        let result = engine
            .evaluate_tool_call("dangerous_operation", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Denied { .. }));

        // Test tool not matching any custom policy (should default to requiring user consent)
        let result = engine
            .evaluate_tool_call("unknown_tool", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));
    }

    #[tokio::test]
    async fn test_evaluate_tool_call_risk_levels() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Test different tools and check their permission options based on risk level

        // fs_read should be allowed (Low risk)
        let result = engine
            .evaluate_tool_call("fs_read_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // fs_write should require consent (Medium risk)
        let result = engine
            .evaluate_tool_call("fs_write_file", &serde_json::json!({}))
            .await
            .unwrap();
        if let PolicyEvaluation::RequireUserConsent { options } = result {
            // Medium risk should have: allow-once, allow-always, reject-once, reject-always
            assert_eq!(options.len(), 4);
            assert!(options.iter().any(|o| o.option_id == "allow-once"));
            assert!(options.iter().any(|o| o.option_id == "allow-always"));
            assert!(options.iter().any(|o| o.option_id == "reject-once"));
            assert!(options.iter().any(|o| o.option_id == "reject-always"));
        } else {
            panic!("Expected RequireUserConsent for fs_write");
        }

        // terminal should require consent (High risk)
        let result = engine
            .evaluate_tool_call("terminal_create", &serde_json::json!({}))
            .await
            .unwrap();
        if let PolicyEvaluation::RequireUserConsent { options } = result {
            // High risk should not have allow-always option
            assert!(!options.iter().any(|o| o.option_id == "allow-always"));
            assert!(options.iter().any(|o| o.option_id == "allow-once"));
            assert!(options.iter().any(|o| o.option_id == "reject-once"));
            assert!(options.iter().any(|o| o.option_id == "reject-always"));
        } else {
            panic!("Expected RequireUserConsent for terminal");
        }

        // http should require consent (High risk)
        let result = engine
            .evaluate_tool_call("http_request", &serde_json::json!({}))
            .await
            .unwrap();
        if let PolicyEvaluation::RequireUserConsent { options } = result {
            // High risk should not have allow-always option
            assert!(!options.iter().any(|o| o.option_id == "allow-always"));
        } else {
            panic!("Expected RequireUserConsent for http");
        }
    }

    #[tokio::test]
    async fn test_evaluate_tool_call_policy_matching_priority() {
        let storage = create_test_storage();

        // Create policies where order matters
        let policies = vec![
            // More specific policy first
            PermissionPolicy {
                tool_pattern: "fs_read_secure".to_string(),
                default_action: PolicyAction::AskUser,
                require_user_consent: true,
                allow_always_option: false,
                risk_level: RiskLevel::High,
            },
            // More general policy second
            PermissionPolicy {
                tool_pattern: "fs_read*".to_string(),
                default_action: PolicyAction::Allow,
                require_user_consent: false,
                allow_always_option: true,
                risk_level: RiskLevel::Low,
            },
        ];

        let engine = PermissionPolicyEngine::with_policies(Box::new(storage), policies);

        // Test that specific policy is matched first
        let result = engine
            .evaluate_tool_call("fs_read_secure", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));

        // Test that general policy is still matched for other tools
        let result = engine
            .evaluate_tool_call("fs_read_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));
    }

    #[tokio::test]
    async fn test_evaluate_tool_call_with_expired_stored_permission() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store an expired permission
        engine
            .store_permission_decision(
                "fs_write_file",
                PermissionDecision::AllowAlways,
                Some(Duration::from_secs(0)), // Already expired
            )
            .await
            .unwrap();

        // Should fall through to policy evaluation since permission is expired
        let result = engine
            .evaluate_tool_call("fs_write_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));
    }

    #[tokio::test]
    async fn test_evaluate_tool_call_pattern_matching_variants() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Test prefix wildcard matching
        let result = engine
            .evaluate_tool_call("fs_read_anything", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Test prefix wildcard for different operations
        let result = engine
            .evaluate_tool_call("fs_write_anything", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));

        let result = engine
            .evaluate_tool_call("terminal_anything", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));

        let result = engine
            .evaluate_tool_call("http_anything", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));
    }

    #[tokio::test]
    async fn test_evaluate_tool_call_stored_permission_takes_precedence() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store an allow permission for a tool that would normally be denied
        engine
            .store_permission_decision("terminal_create", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // Should be allowed by stored permission, overriding policy
        let result = engine
            .evaluate_tool_call("terminal_create", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Store a deny permission for a tool that would normally be allowed
        engine
            .store_permission_decision("fs_read_file", PermissionDecision::DenyAlways, None)
            .await
            .unwrap();

        // Should be denied by stored permission, overriding policy
        let result = engine
            .evaluate_tool_call("fs_read_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Denied { .. }));
    }

    #[tokio::test]
    async fn test_evaluate_tool_call_wildcard_policy() {
        let storage = create_test_storage();

        // Create policies with wildcard as catch-all
        let policies = vec![
            PermissionPolicy {
                tool_pattern: "safe_*".to_string(),
                default_action: PolicyAction::Allow,
                require_user_consent: false,
                allow_always_option: true,
                risk_level: RiskLevel::Low,
            },
            PermissionPolicy {
                tool_pattern: "*".to_string(),
                default_action: PolicyAction::AskUser,
                require_user_consent: true,
                allow_always_option: true,
                risk_level: RiskLevel::Medium,
            },
        ];

        let engine = PermissionPolicyEngine::with_policies(Box::new(storage), policies);

        // Test specific policy match
        let result = engine
            .evaluate_tool_call("safe_operation", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Test wildcard catch-all
        let result = engine
            .evaluate_tool_call("random_tool", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));
    }

    #[tokio::test]
    async fn test_permission_options_for_different_risk_levels() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Test Low risk - should have allow-always
        let options = engine.generate_permission_options("test_tool", RiskLevel::Low);
        assert_eq!(options.len(), 3); // allow-once, allow-always, reject-once
        assert!(options.iter().any(|o| o.option_id == "allow-always"));
        assert!(!options.iter().any(|o| o.option_id == "reject-always"));

        // Test Medium risk - should have both always options
        let options = engine.generate_permission_options("test_tool", RiskLevel::Medium);
        assert_eq!(options.len(), 4); // allow-once, allow-always, reject-once, reject-always
        assert!(options.iter().any(|o| o.option_id == "allow-always"));
        assert!(options.iter().any(|o| o.option_id == "reject-always"));

        // Test High risk - should not have allow-always
        let options = engine.generate_permission_options("test_tool", RiskLevel::High);
        assert_eq!(options.len(), 3); // allow-once, reject-once, reject-always
        assert!(!options.iter().any(|o| o.option_id == "allow-always"));
        assert!(options.iter().any(|o| o.option_id == "reject-always"));

        // Test Critical risk - should not have allow-always
        let options = engine.generate_permission_options("test_tool", RiskLevel::Critical);
        assert_eq!(options.len(), 3); // allow-once, reject-once, reject-always
        assert!(!options.iter().any(|o| o.option_id == "allow-always"));
        assert!(options.iter().any(|o| o.option_id == "reject-always"));
    }

    #[tokio::test]
    async fn test_permission_decision_caching_allow() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // First evaluation should require user consent (fs_write is medium risk)
        let result = engine
            .evaluate_tool_call("fs_write_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));

        // Store an allow-always decision (simulating user granting permission)
        engine
            .store_permission_decision("fs_write_file", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // Second evaluation should use cached decision and allow without prompting
        let result = engine
            .evaluate_tool_call("fs_write_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Third evaluation should also use cached decision
        let result = engine
            .evaluate_tool_call("fs_write_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Verify the cached decision is retrievable
        let cached = engine.lookup_permission("fs_write_file").await.unwrap();
        assert!(cached.is_some());
        assert!(matches!(
            cached.unwrap().decision,
            PermissionDecision::AllowAlways
        ));
    }

    #[tokio::test]
    async fn test_permission_decision_caching_deny() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // First evaluation should require user consent
        let result = engine
            .evaluate_tool_call("terminal_exec", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));

        // Store a deny-always decision (simulating user denying permission)
        engine
            .store_permission_decision("terminal_exec", PermissionDecision::DenyAlways, None)
            .await
            .unwrap();

        // Second evaluation should use cached decision and deny
        let result = engine
            .evaluate_tool_call("terminal_exec", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Denied { .. }));

        // Third evaluation should also use cached decision
        let result = engine
            .evaluate_tool_call("terminal_exec", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Denied { .. }));

        // Verify the cached decision is retrievable
        let cached = engine.lookup_permission("terminal_exec").await.unwrap();
        assert!(cached.is_some());
        assert!(matches!(
            cached.unwrap().decision,
            PermissionDecision::DenyAlways
        ));
    }

    #[tokio::test]
    async fn test_permission_decision_caching_with_expiration() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store a permission with very short expiration
        engine
            .store_permission_decision(
                "test_tool",
                PermissionDecision::AllowAlways,
                Some(Duration::from_secs(1)),
            )
            .await
            .unwrap();

        // Should be allowed immediately
        let result = engine
            .evaluate_tool_call("test_tool", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Wait for expiration
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should now require consent again since cached decision expired
        let result = engine
            .evaluate_tool_call("test_tool", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result,
            PolicyEvaluation::RequireUserConsent { .. }
        ));
    }

    #[tokio::test]
    async fn test_permission_decision_caching_pattern_matching() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store a permission with wildcard pattern
        engine
            .store_permission_decision("fs_write*", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // All fs_write* tools should use the cached decision
        let result = engine
            .evaluate_tool_call("fs_write_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        let result = engine
            .evaluate_tool_call("fs_write_dir", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        let result = engine
            .evaluate_tool_call("fs_write_anything", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));

        // Non-matching tool should not use the cached decision
        let result = engine
            .evaluate_tool_call("fs_read_file", &serde_json::json!({}))
            .await
            .unwrap();
        // fs_read is allowed by default policy, not by cached decision
        assert!(matches!(result, PolicyEvaluation::Allowed));
    }

    #[tokio::test]
    async fn test_permission_decision_caching_overrides_policy() {
        let storage = create_test_storage();

        // Create a policy that denies a tool
        let policies = vec![PermissionPolicy {
            tool_pattern: "dangerous_tool".to_string(),
            default_action: PolicyAction::Deny,
            require_user_consent: false,
            allow_always_option: false,
            risk_level: RiskLevel::Critical,
        }];

        let engine = PermissionPolicyEngine::with_policies(Box::new(storage), policies);

        // First evaluation should be denied by policy
        let result = engine
            .evaluate_tool_call("dangerous_tool", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Denied { .. }));

        // Store an allow decision (overriding the deny policy)
        engine
            .store_permission_decision("dangerous_tool", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // Cached decision should override the deny policy
        let result = engine
            .evaluate_tool_call("dangerous_tool", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result, PolicyEvaluation::Allowed));
    }

    #[tokio::test]
    async fn test_permission_decision_caching_multiple_tools() {
        let storage = create_test_storage();
        let engine = PermissionPolicyEngine::new(Box::new(storage));

        // Store decisions for multiple tools
        engine
            .store_permission_decision("tool_a", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();
        engine
            .store_permission_decision("tool_b", PermissionDecision::DenyAlways, None)
            .await
            .unwrap();
        engine
            .store_permission_decision("tool_c", PermissionDecision::AllowAlways, None)
            .await
            .unwrap();

        // Each tool should use its own cached decision
        let result_a = engine
            .evaluate_tool_call("tool_a", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result_a, PolicyEvaluation::Allowed));

        let result_b = engine
            .evaluate_tool_call("tool_b", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result_b, PolicyEvaluation::Denied { .. }));

        let result_c = engine
            .evaluate_tool_call("tool_c", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(result_c, PolicyEvaluation::Allowed));

        // Uncached tool should require consent
        let result_d = engine
            .evaluate_tool_call("tool_d", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(
            result_d,
            PolicyEvaluation::RequireUserConsent { .. }
        ));
    }

    #[tokio::test]
    async fn test_permission_decision_caching_persistence() {
        let temp_dir = tempdir().unwrap();
        let storage_path = temp_dir.path().to_path_buf();

        // Create engine and store a decision
        {
            let storage = FilePermissionStorage::new(storage_path.clone());
            let engine = PermissionPolicyEngine::new(Box::new(storage));

            engine
                .store_permission_decision("persistent_tool", PermissionDecision::AllowAlways, None)
                .await
                .unwrap();

            let result = engine
                .evaluate_tool_call("persistent_tool", &serde_json::json!({}))
                .await
                .unwrap();
            assert!(matches!(result, PolicyEvaluation::Allowed));
        }

        // Create a new engine with the same storage path
        // Cached decision should be loaded from disk
        {
            let storage = FilePermissionStorage::new(storage_path);
            let engine = PermissionPolicyEngine::new(Box::new(storage));

            let result = engine
                .evaluate_tool_call("persistent_tool", &serde_json::json!({}))
                .await
                .unwrap();
            assert!(matches!(result, PolicyEvaluation::Allowed));

            // Verify it was loaded from disk
            let cached = engine.lookup_permission("persistent_tool").await.unwrap();
            assert!(cached.is_some());
        }
    }
}
