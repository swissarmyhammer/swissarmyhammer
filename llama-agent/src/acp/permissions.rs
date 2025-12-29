//! Permission policy for ACP operations
//!
//! This module handles permission checking and policy enforcement.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionPolicy {
    /// Always ask user for permission
    AlwaysAsk,

    /// Auto-approve read operations, ask for writes
    AutoApproveReads,

    /// Use rule-based policy
    RuleBased(Vec<PermissionRule>),
}

/// A rule that defines permission behavior for tools matching a specific pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    pub pattern: ToolPattern,
    pub action: PermissionAction,
}

impl PermissionRule {
    /// Check if this rule matches the given tool name
    pub fn matches(&self, tool_name: &str) -> bool {
        self.pattern.matches(tool_name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolPattern {
    /// Match specific tool name
    Exact(String),

    /// Match tool name pattern (glob-style)
    Pattern(String),

    /// Match all tools
    All,
}

impl ToolPattern {
    /// Check if this pattern matches the given tool name
    pub fn matches(&self, tool_name: &str) -> bool {
        match self {
            ToolPattern::Exact(name) => name == tool_name,
            ToolPattern::Pattern(pattern) => matches_pattern(pattern, tool_name),
            ToolPattern::All => true,
        }
    }
}

/// Check if a tool pattern matches a tool name (supports basic wildcards)
fn matches_pattern(pattern: &str, tool_name: &str) -> bool {
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

/// Action to take when a permission rule matches a tool call
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionAction {
    Allow,
    Deny,
    Ask,
}

/// Storage for granted permissions
#[derive(Debug, Default, Clone)]
pub struct PermissionStorage {
    /// Map of tool_name -> granted permissions
    granted: HashMap<String, PermissionGrant>,
}

#[derive(Debug, Clone)]
struct PermissionGrant {
    _tool_name: String,
    _granted_at: std::time::SystemTime,
}

impl PermissionStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&mut self, tool_name: String) {
        self.granted.insert(
            tool_name.clone(),
            PermissionGrant {
                _tool_name: tool_name,
                _granted_at: std::time::SystemTime::now(),
            },
        );
    }

    pub fn is_granted(&self, tool_name: &str) -> bool {
        self.granted.contains_key(tool_name)
    }

    pub fn revoke(&mut self, tool_name: &str) {
        self.granted.remove(tool_name);
    }

    pub fn clear(&mut self) {
        self.granted.clear();
    }
}

/// Result of permission evaluation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionEvaluation {
    /// Permission granted automatically
    Allowed,

    /// Permission denied automatically
    Denied,

    /// User consent required
    RequireUserConsent,
}

/// Permission policy engine for evaluating tool call permissions
pub struct PermissionPolicyEngine {
    policy: PermissionPolicy,
}

impl PermissionPolicyEngine {
    /// Create a new permission policy engine with the given policy
    pub fn new(policy: PermissionPolicy) -> Self {
        Self { policy }
    }

    /// Evaluate a tool call against the permission policy and storage
    ///
    /// This method checks:
    /// 1. If permission was already granted in storage
    /// 2. The configured permission policy
    /// 3. Tool-specific rules if using RuleBased policy
    pub fn evaluate_tool_call(
        &self,
        tool_name: &str,
        storage: &PermissionStorage,
    ) -> PermissionEvaluation {
        // First check if permission was already granted
        if storage.is_granted(tool_name) {
            return PermissionEvaluation::Allowed;
        }

        // Evaluate based on policy
        match &self.policy {
            PermissionPolicy::AlwaysAsk => PermissionEvaluation::RequireUserConsent,

            PermissionPolicy::AutoApproveReads => {
                if is_read_operation(tool_name) {
                    PermissionEvaluation::Allowed
                } else {
                    PermissionEvaluation::RequireUserConsent
                }
            }

            PermissionPolicy::RuleBased(rules) => {
                // Find the first matching rule
                for rule in rules {
                    if rule.matches(tool_name) {
                        return match rule.action {
                            PermissionAction::Allow => PermissionEvaluation::Allowed,
                            PermissionAction::Deny => PermissionEvaluation::Denied,
                            PermissionAction::Ask => PermissionEvaluation::RequireUserConsent,
                        };
                    }
                }

                // No matching rule - default to asking user
                PermissionEvaluation::RequireUserConsent
            }
        }
    }

    /// Get the underlying policy
    pub fn policy(&self) -> &PermissionPolicy {
        &self.policy
    }
}

/// Check if a tool name represents a read operation
fn is_read_operation(tool_name: &str) -> bool {
    // Common patterns for read operations
    let read_indicators = ["read", "get", "list", "show", "view", "fetch", "load"];

    let tool_lower = tool_name.to_lowercase();
    read_indicators
        .iter()
        .any(|indicator| tool_lower.contains(indicator))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_pattern_exact() {
        assert!(matches_pattern("exact_match", "exact_match"));
        assert!(!matches_pattern("exact_match", "different"));
    }

    #[test]
    fn test_matches_pattern_wildcard_all() {
        assert!(matches_pattern("*", "any_tool"));
        assert!(matches_pattern("*", "another_tool"));
    }

    #[test]
    fn test_matches_pattern_prefix() {
        assert!(matches_pattern("fs_*", "fs_read"));
        assert!(matches_pattern("fs_*", "fs_write"));
        assert!(!matches_pattern("fs_*", "terminal_create"));
    }

    #[test]
    fn test_matches_pattern_suffix() {
        assert!(matches_pattern("*_read", "fs_read"));
        assert!(matches_pattern("*_read", "http_read"));
        assert!(!matches_pattern("*_read", "fs_write"));
    }

    #[test]
    fn test_tool_pattern_exact() {
        let pattern = ToolPattern::Exact("test_tool".to_string());
        assert!(pattern.matches("test_tool"));
        assert!(!pattern.matches("other_tool"));
    }

    #[test]
    fn test_tool_pattern_pattern() {
        let pattern = ToolPattern::Pattern("fs_*".to_string());
        assert!(pattern.matches("fs_read"));
        assert!(pattern.matches("fs_write"));
        assert!(!pattern.matches("terminal_create"));
    }

    #[test]
    fn test_tool_pattern_all() {
        let pattern = ToolPattern::All;
        assert!(pattern.matches("any_tool"));
        assert!(pattern.matches("another_tool"));
    }

    #[test]
    fn test_permission_storage_grant_and_check() {
        let mut storage = PermissionStorage::new();
        assert!(!storage.is_granted("test_tool"));

        storage.grant("test_tool".to_string());
        assert!(storage.is_granted("test_tool"));
        assert!(!storage.is_granted("other_tool"));
    }

    #[test]
    fn test_permission_storage_revoke() {
        let mut storage = PermissionStorage::new();
        storage.grant("test_tool".to_string());
        assert!(storage.is_granted("test_tool"));

        storage.revoke("test_tool");
        assert!(!storage.is_granted("test_tool"));
    }

    #[test]
    fn test_permission_storage_clear() {
        let mut storage = PermissionStorage::new();
        storage.grant("tool1".to_string());
        storage.grant("tool2".to_string());
        assert!(storage.is_granted("tool1"));
        assert!(storage.is_granted("tool2"));

        storage.clear();
        assert!(!storage.is_granted("tool1"));
        assert!(!storage.is_granted("tool2"));
    }

    #[test]
    fn test_is_read_operation() {
        assert!(is_read_operation("fs_read"));
        assert!(is_read_operation("get_file"));
        assert!(is_read_operation("list_files"));
        assert!(is_read_operation("show_content"));
        assert!(is_read_operation("view_data"));
        assert!(is_read_operation("fetch_url"));
        assert!(is_read_operation("load_config"));

        assert!(!is_read_operation("fs_write"));
        assert!(!is_read_operation("create_file"));
        assert!(!is_read_operation("delete_file"));
        assert!(!is_read_operation("terminal_execute"));
    }

    #[test]
    fn test_policy_engine_always_ask() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AlwaysAsk);
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("any_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_always_ask_with_read_operation() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AlwaysAsk);
        let storage = PermissionStorage::new();

        // Even read operations should require consent
        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("get_file", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("list_files", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_always_ask_with_write_operation() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AlwaysAsk);
        let storage = PermissionStorage::new();

        // Write operations should require consent
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("create_file", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("delete_file", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_always_ask_with_granted_permission() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AlwaysAsk);
        let mut storage = PermissionStorage::new();

        // Without grant, should require consent
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );

        // After grant, should allow (grant overrides policy)
        storage.grant("test_tool".to_string());
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_always_ask_with_multiple_tools() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AlwaysAsk);
        let storage = PermissionStorage::new();

        // All tools should require consent
        assert_eq!(
            engine.evaluate_tool_call("tool_1", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("tool_2", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("tool_3", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_always_ask_with_revoked_permission() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AlwaysAsk);
        let mut storage = PermissionStorage::new();

        // Grant then revoke
        storage.grant("test_tool".to_string());
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::Allowed
        );

        storage.revoke("test_tool");
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_always_ask_with_cleared_storage() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AlwaysAsk);
        let mut storage = PermissionStorage::new();

        // Grant multiple tools
        storage.grant("tool_1".to_string());
        storage.grant("tool_2".to_string());
        assert_eq!(
            engine.evaluate_tool_call("tool_1", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("tool_2", &storage),
            PermissionEvaluation::Allowed
        );

        // Clear storage
        storage.clear();
        assert_eq!(
            engine.evaluate_tool_call("tool_1", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("tool_2", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );

        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_all_read_patterns() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let storage = PermissionStorage::new();

        // Test all read indicators
        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("get_file", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("list_files", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("show_content", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("view_data", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("fetch_url", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("load_config", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_write_operations() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let storage = PermissionStorage::new();

        // Test various write operations - all should require consent
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("create_file", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("delete_file", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("update_data", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("modify_config", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("terminal_execute", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_case_insensitive() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let storage = PermissionStorage::new();

        // Test that read detection is case insensitive
        assert_eq!(
            engine.evaluate_tool_call("READ_FILE", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("Get_Data", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("LIST_ITEMS", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("SHOW_INFO", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_partial_match() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let storage = PermissionStorage::new();

        // Test that read indicators work anywhere in the tool name
        assert_eq!(
            engine.evaluate_tool_call("file_read_operation", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("data_getter", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("item_lister", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("content_viewer", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_with_granted_permission() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let mut storage = PermissionStorage::new();

        // Write operation without grant should require consent
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::RequireUserConsent
        );

        // After granting permission, should allow (grant overrides policy)
        storage.grant("fs_write".to_string());
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_granted_overrides_read_check() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let mut storage = PermissionStorage::new();

        // Read operation is allowed by policy
        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );

        // Even if we grant permission explicitly, still allowed
        storage.grant("fs_read".to_string());
        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_mixed_operations() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let storage = PermissionStorage::new();

        // Test a variety of operations
        let read_ops = vec![
            "db_read_record",
            "api_get_user",
            "cache_fetch_value",
            "log_viewer",
            "file_loader",
            "data_lister",
        ];

        for op in read_ops {
            assert_eq!(
                engine.evaluate_tool_call(op, &storage),
                PermissionEvaluation::Allowed,
                "Expected '{}' to be allowed as a read operation",
                op
            );
        }

        let write_ops = vec![
            "db_write_record",
            "api_post_user",
            "cache_set_value",
            "file_creator",
            "data_updater",
            "record_deleter",
        ];

        for op in write_ops {
            assert_eq!(
                engine.evaluate_tool_call(op, &storage),
                PermissionEvaluation::RequireUserConsent,
                "Expected '{}' to require consent as a write operation",
                op
            );
        }
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_ambiguous_names() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let storage = PermissionStorage::new();

        // Tool names that contain read indicators anywhere in the name are treated as reads
        // This includes "thread_creator" which contains "read" in "thread"
        assert_eq!(
            engine.evaluate_tool_call("thread_creator", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("thread_reader", &storage),
            PermissionEvaluation::Allowed
        );

        // "fetch" is a read indicator even if followed by other actions
        assert_eq!(
            engine.evaluate_tool_call("fetch_and_process", &storage),
            PermissionEvaluation::Allowed
        );

        // Tool names with no read indicators should require consent
        assert_eq!(
            engine.evaluate_tool_call("process_data", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("create_item", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_revoked_permission() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let mut storage = PermissionStorage::new();

        // Grant permission for a write operation
        storage.grant("fs_write".to_string());
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::Allowed
        );

        // Revoke the permission
        storage.revoke("fs_write");
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::RequireUserConsent
        );

        // Read operations should still be allowed by policy
        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_cleared_storage() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let mut storage = PermissionStorage::new();

        // Grant multiple permissions
        storage.grant("write_op1".to_string());
        storage.grant("write_op2".to_string());
        assert_eq!(
            engine.evaluate_tool_call("write_op1", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("write_op2", &storage),
            PermissionEvaluation::Allowed
        );

        // Clear storage
        storage.clear();
        assert_eq!(
            engine.evaluate_tool_call("write_op1", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("write_op2", &storage),
            PermissionEvaluation::RequireUserConsent
        );

        // Read operations should still work
        assert_eq!(
            engine.evaluate_tool_call("read_op", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_auto_approve_reads_no_read_indicator() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AutoApproveReads);
        let storage = PermissionStorage::new();

        // Tools with no read indicators should require consent
        assert_eq!(
            engine.evaluate_tool_call("execute_command", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("process_data", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("transform_input", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("calculate_result", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_rule_based_allow() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("fs_*".to_string()),
            action: PermissionAction::Allow,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_rule_based_deny() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("terminal_*".to_string()),
            action: PermissionAction::Deny,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("terminal_create", &storage),
            PermissionEvaluation::Denied
        );
    }

    #[test]
    fn test_policy_engine_rule_based_ask() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("http_*".to_string()),
            action: PermissionAction::Ask,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("http_get", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_rule_based_no_match_defaults_to_ask() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("fs_*".to_string()),
            action: PermissionAction::Allow,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("terminal_create", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_policy_engine_rule_based_first_match_wins() {
        let rules = vec![
            PermissionRule {
                pattern: ToolPattern::Pattern("fs_*".to_string()),
                action: PermissionAction::Allow,
            },
            PermissionRule {
                pattern: ToolPattern::Pattern("fs_write*".to_string()),
                action: PermissionAction::Deny,
            },
        ];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        // First rule matches and allows
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_granted_permission_overrides() {
        let engine = PermissionPolicyEngine::new(PermissionPolicy::AlwaysAsk);
        let mut storage = PermissionStorage::new();

        // Without grant, should require consent
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );

        // After grant, should allow
        storage.grant("test_tool".to_string());
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_granted_permission_overrides_deny_rule() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::All,
            action: PermissionAction::Deny,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let mut storage = PermissionStorage::new();

        // Without grant, should deny
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::Denied
        );

        // After grant, should allow (grant overrides policy)
        storage.grant("test_tool".to_string());
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_policy_engine_policy_getter() {
        let policy = PermissionPolicy::AlwaysAsk;
        let engine = PermissionPolicyEngine::new(policy);

        assert!(matches!(engine.policy(), PermissionPolicy::AlwaysAsk));
    }

    #[test]
    fn test_policy_engine_policy_getter_auto_approve_reads() {
        let policy = PermissionPolicy::AutoApproveReads;
        let engine = PermissionPolicyEngine::new(policy);

        assert!(matches!(
            engine.policy(),
            PermissionPolicy::AutoApproveReads
        ));
    }

    #[test]
    fn test_policy_engine_policy_getter_rule_based() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::All,
            action: PermissionAction::Allow,
        }];
        let policy = PermissionPolicy::RuleBased(rules);
        let engine = PermissionPolicyEngine::new(policy);

        if let PermissionPolicy::RuleBased(returned_rules) = engine.policy() {
            assert_eq!(returned_rules.len(), 1);
            assert!(matches!(returned_rules[0].pattern, ToolPattern::All));
            assert_eq!(returned_rules[0].action, PermissionAction::Allow);
        } else {
            panic!("Expected RuleBased policy");
        }
    }

    #[test]
    fn test_permission_policy_serialization_camelcase() {
        // Test PermissionPolicy enum variants serialize to camelCase
        let policy = PermissionPolicy::AlwaysAsk;
        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(json, "\"alwaysAsk\"");

        let policy = PermissionPolicy::AutoApproveReads;
        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(json, "\"autoApproveReads\"");

        // Test PermissionRule struct fields serialize to camelCase
        let rule = PermissionRule {
            pattern: ToolPattern::All,
            action: PermissionAction::Allow,
        };
        let json = serde_json::to_value(&rule).unwrap();
        assert!(json.get("pattern").is_some());
        assert!(json.get("action").is_some());

        // Test PermissionAction enum variants serialize to camelCase
        let action = PermissionAction::Allow;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"allow\"");
    }

    // Comprehensive RuleBased policy tests

    #[test]
    fn test_rule_based_with_exact_pattern_allow() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Exact("specific_tool".to_string()),
            action: PermissionAction::Allow,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("specific_tool", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("specific_tool_extended", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("other_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_rule_based_with_exact_pattern_deny() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Exact("forbidden_tool".to_string()),
            action: PermissionAction::Deny,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("forbidden_tool", &storage),
            PermissionEvaluation::Denied
        );
        assert_eq!(
            engine.evaluate_tool_call("forbidden_tool_v2", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_rule_based_with_all_pattern_allow() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::All,
            action: PermissionAction::Allow,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("any_tool", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("another_tool", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("third_tool", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_rule_based_with_all_pattern_deny() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::All,
            action: PermissionAction::Deny,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("any_tool", &storage),
            PermissionEvaluation::Denied
        );
        assert_eq!(
            engine.evaluate_tool_call("another_tool", &storage),
            PermissionEvaluation::Denied
        );
    }

    #[test]
    fn test_rule_based_with_suffix_wildcard() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("*_read".to_string()),
            action: PermissionAction::Allow,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("db_read", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_rule_based_multiple_rules_different_actions() {
        let rules = vec![
            PermissionRule {
                pattern: ToolPattern::Pattern("fs_*".to_string()),
                action: PermissionAction::Allow,
            },
            PermissionRule {
                pattern: ToolPattern::Pattern("terminal_*".to_string()),
                action: PermissionAction::Deny,
            },
            PermissionRule {
                pattern: ToolPattern::Pattern("http_*".to_string()),
                action: PermissionAction::Ask,
            },
        ];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("terminal_execute", &storage),
            PermissionEvaluation::Denied
        );
        assert_eq!(
            engine.evaluate_tool_call("http_get", &storage),
            PermissionEvaluation::RequireUserConsent
        );
        assert_eq!(
            engine.evaluate_tool_call("unknown_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_rule_based_empty_rules_list() {
        let rules = vec![];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        // With no rules, should default to asking
        assert_eq!(
            engine.evaluate_tool_call("any_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_rule_based_rule_order_matters() {
        // More specific rule should come first
        let rules = vec![
            PermissionRule {
                pattern: ToolPattern::Exact("fs_write".to_string()),
                action: PermissionAction::Deny,
            },
            PermissionRule {
                pattern: ToolPattern::Pattern("fs_*".to_string()),
                action: PermissionAction::Allow,
            },
        ];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        // fs_write should be denied by the first rule
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::Denied
        );
        // Other fs_ tools should be allowed by the second rule
        assert_eq!(
            engine.evaluate_tool_call("fs_read", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_rule_based_catch_all_at_end() {
        let rules = vec![
            PermissionRule {
                pattern: ToolPattern::Pattern("safe_*".to_string()),
                action: PermissionAction::Allow,
            },
            PermissionRule {
                pattern: ToolPattern::All,
                action: PermissionAction::Deny,
            },
        ];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("safe_operation", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("unsafe_operation", &storage),
            PermissionEvaluation::Denied
        );
        assert_eq!(
            engine.evaluate_tool_call("random_tool", &storage),
            PermissionEvaluation::Denied
        );
    }

    #[test]
    fn test_rule_based_with_granted_permission_override() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("dangerous_*".to_string()),
            action: PermissionAction::Deny,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let mut storage = PermissionStorage::new();

        // Should be denied by rule
        assert_eq!(
            engine.evaluate_tool_call("dangerous_operation", &storage),
            PermissionEvaluation::Denied
        );

        // Grant permission - should override the deny rule
        storage.grant("dangerous_operation".to_string());
        assert_eq!(
            engine.evaluate_tool_call("dangerous_operation", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_rule_based_granted_permission_for_unmatched_tool() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("fs_*".to_string()),
            action: PermissionAction::Allow,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let mut storage = PermissionStorage::new();

        // Tool doesn't match any rule, should ask
        assert_eq!(
            engine.evaluate_tool_call("terminal_execute", &storage),
            PermissionEvaluation::RequireUserConsent
        );

        // Grant permission
        storage.grant("terminal_execute".to_string());
        assert_eq!(
            engine.evaluate_tool_call("terminal_execute", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_rule_based_revoked_permission_falls_back_to_rules() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("fs_*".to_string()),
            action: PermissionAction::Deny,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let mut storage = PermissionStorage::new();

        // Grant permission first
        storage.grant("fs_write".to_string());
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::Allowed
        );

        // Revoke permission - should fall back to rule (deny)
        storage.revoke("fs_write");
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::Denied
        );
    }

    #[test]
    fn test_rule_based_complex_pattern_matching() {
        let rules = vec![
            PermissionRule {
                pattern: ToolPattern::Pattern("db_*".to_string()),
                action: PermissionAction::Allow,
            },
            PermissionRule {
                pattern: ToolPattern::Pattern("*_admin".to_string()),
                action: PermissionAction::Deny,
            },
            PermissionRule {
                pattern: ToolPattern::Exact("special_tool".to_string()),
                action: PermissionAction::Ask,
            },
        ];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        // Prefix match
        assert_eq!(
            engine.evaluate_tool_call("db_read", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("db_write", &storage),
            PermissionEvaluation::Allowed
        );

        // Suffix match
        assert_eq!(
            engine.evaluate_tool_call("user_admin", &storage),
            PermissionEvaluation::Denied
        );
        assert_eq!(
            engine.evaluate_tool_call("system_admin", &storage),
            PermissionEvaluation::Denied
        );

        // Exact match
        assert_eq!(
            engine.evaluate_tool_call("special_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );

        // No match
        assert_eq!(
            engine.evaluate_tool_call("random_tool", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_rule_based_wildcard_only() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("*".to_string()),
            action: PermissionAction::Allow,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        // Should match everything
        assert_eq!(
            engine.evaluate_tool_call("any_tool", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("another_tool", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_rule_based_multiple_exact_patterns() {
        let rules = vec![
            PermissionRule {
                pattern: ToolPattern::Exact("tool_a".to_string()),
                action: PermissionAction::Allow,
            },
            PermissionRule {
                pattern: ToolPattern::Exact("tool_b".to_string()),
                action: PermissionAction::Allow,
            },
            PermissionRule {
                pattern: ToolPattern::Exact("tool_c".to_string()),
                action: PermissionAction::Deny,
            },
        ];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("tool_a", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("tool_b", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("tool_c", &storage),
            PermissionEvaluation::Denied
        );
        assert_eq!(
            engine.evaluate_tool_call("tool_d", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_rule_based_cleared_storage_uses_rules() {
        let rules = vec![PermissionRule {
            pattern: ToolPattern::Pattern("fs_*".to_string()),
            action: PermissionAction::Allow,
        }];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let mut storage = PermissionStorage::new();

        // Grant permissions for multiple tools
        storage.grant("fs_write".to_string());
        storage.grant("terminal_execute".to_string());

        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("terminal_execute", &storage),
            PermissionEvaluation::Allowed
        );

        // Clear storage
        storage.clear();

        // fs_write should still be allowed by rule
        assert_eq!(
            engine.evaluate_tool_call("fs_write", &storage),
            PermissionEvaluation::Allowed
        );

        // terminal_execute no longer has granted permission and doesn't match any rule
        assert_eq!(
            engine.evaluate_tool_call("terminal_execute", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }

    #[test]
    fn test_rule_based_overlapping_patterns() {
        let rules = vec![
            PermissionRule {
                pattern: ToolPattern::Pattern("test_*".to_string()),
                action: PermissionAction::Allow,
            },
            PermissionRule {
                pattern: ToolPattern::Pattern("*_tool".to_string()),
                action: PermissionAction::Deny,
            },
        ];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        // First rule matches and wins
        assert_eq!(
            engine.evaluate_tool_call("test_tool", &storage),
            PermissionEvaluation::Allowed
        );

        // Only second rule matches
        assert_eq!(
            engine.evaluate_tool_call("other_tool", &storage),
            PermissionEvaluation::Denied
        );

        // Only first rule matches
        assert_eq!(
            engine.evaluate_tool_call("test_operation", &storage),
            PermissionEvaluation::Allowed
        );
    }

    #[test]
    fn test_rule_based_all_three_actions_in_sequence() {
        let rules = vec![
            PermissionRule {
                pattern: ToolPattern::Pattern("read_*".to_string()),
                action: PermissionAction::Allow,
            },
            PermissionRule {
                pattern: ToolPattern::Pattern("delete_*".to_string()),
                action: PermissionAction::Deny,
            },
            PermissionRule {
                pattern: ToolPattern::Pattern("write_*".to_string()),
                action: PermissionAction::Ask,
            },
        ];
        let engine = PermissionPolicyEngine::new(PermissionPolicy::RuleBased(rules));
        let storage = PermissionStorage::new();

        assert_eq!(
            engine.evaluate_tool_call("read_file", &storage),
            PermissionEvaluation::Allowed
        );
        assert_eq!(
            engine.evaluate_tool_call("delete_file", &storage),
            PermissionEvaluation::Denied
        );
        assert_eq!(
            engine.evaluate_tool_call("write_file", &storage),
            PermissionEvaluation::RequireUserConsent
        );
    }
}
