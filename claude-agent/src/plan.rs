//! Agent plan generation and reporting for ACP compliance
//!
//! ACP requires agent plan reporting for transparency and progress tracking:
//! 1. Generate actionable plan entries based on user request
//! 2. Report initial plan via session/update notification
//! 3. Update plan entry status as work progresses
//! 4. Connect plan entries to actual tool executions
//! 5. Provide clear visibility into agent's approach
//!
//! Plans should be realistic, specific, and trackable.

use agent_client_protocol::{
    Plan as AcpPlan, PlanEntry as AcpPlanEntry, PlanEntryPriority as AcpPriority,
    PlanEntryStatus as AcpStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;

/// Plan entry status lifecycle according to ACP specification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlanEntryStatus {
    /// Entry is pending execution
    #[serde(rename = "pending")]
    Pending,
    /// Entry is currently being executed
    #[serde(rename = "in_progress")]
    InProgress,
    /// Entry has been completed successfully
    #[serde(rename = "completed")]
    Completed,
    /// Entry execution failed
    #[serde(rename = "failed")]
    Failed,
    /// Entry was cancelled before completion
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl PlanEntryStatus {
    /// Convert to ACP status format.
    ///
    /// # ACP Compliance Note
    /// ACP only supports pending, in_progress, and completed states.
    /// Internal Failed and Cancelled states are mapped to Completed for ACP compliance,
    /// allowing clients to see these entries in their final state without exposing
    /// implementation-specific failure modes.
    pub fn to_acp_status(&self) -> AcpStatus {
        match self {
            PlanEntryStatus::Pending => AcpStatus::Pending,
            PlanEntryStatus::InProgress => AcpStatus::InProgress,
            PlanEntryStatus::Completed => AcpStatus::Completed,
            // ACP only supports pending, in_progress, completed
            // Map failed and cancelled to completed for ACP compliance
            PlanEntryStatus::Failed | PlanEntryStatus::Cancelled => AcpStatus::Completed,
        }
    }
}

/// Priority levels for plan entries
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// High priority - critical for task completion
    #[serde(rename = "high")]
    High,
    /// Medium priority - important but not critical
    #[serde(rename = "medium")]
    Medium,
    /// Low priority - nice to have or cleanup tasks
    #[serde(rename = "low")]
    Low,
}

impl Priority {
    /// Convert to ACP priority format.
    ///
    /// Maps internal priority levels to ACP protocol priority values
    /// for client communication.
    pub fn to_acp_priority(&self) -> AcpPriority {
        match self {
            Priority::High => AcpPriority::High,
            Priority::Medium => AcpPriority::Medium,
            Priority::Low => AcpPriority::Low,
        }
    }
}

/// Individual plan entry representing a specific action or step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    /// Unique identifier for this plan entry
    pub id: String,
    /// Human-readable description of what this entry will accomplish
    pub content: String,
    /// Priority level for execution order and importance
    pub priority: Priority,
    /// Current execution status
    pub status: PlanEntryStatus,
    /// Optional additional context or notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Timestamp when this entry was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<std::time::SystemTime>,
    /// Timestamp when this entry was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<std::time::SystemTime>,
}

impl PlanEntry {
    /// Create a new plan entry with pending status
    pub fn new(content: String, priority: Priority) -> Self {
        let now = std::time::SystemTime::now();
        Self {
            id: Ulid::new().to_string(),
            content,
            priority,
            status: PlanEntryStatus::Pending,
            notes: None,
            created_at: Some(now),
            updated_at: Some(now),
        }
    }

    /// Update the status of this plan entry
    pub fn update_status(&mut self, new_status: PlanEntryStatus) {
        if self.status != new_status {
            self.status = new_status;
            self.updated_at = Some(std::time::SystemTime::now());
        }
    }

    /// Add or update notes for this plan entry
    pub fn set_notes(&mut self, notes: String) {
        self.notes = Some(notes);
        self.updated_at = Some(std::time::SystemTime::now());
    }

    /// Check if this plan entry is complete (completed, failed, or cancelled)
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            PlanEntryStatus::Completed | PlanEntryStatus::Failed | PlanEntryStatus::Cancelled
        )
    }

    /// Check if this plan entry is currently being executed
    pub fn is_in_progress(&self) -> bool {
        matches!(self.status, PlanEntryStatus::InProgress)
    }

    /// Convert to ACP plan entry format.
    ///
    /// Creates an ACP-compliant plan entry for client communication.
    /// The meta field is populated when notes are present and includes
    /// the entry ID and timestamps for client tracking.
    pub fn to_acp_entry(&self) -> AcpPlanEntry {
        let mut entry = AcpPlanEntry::new(
            self.content.clone(),
            self.priority.to_acp_priority(),
            self.status.to_acp_status(),
        );

        if let Some(notes) = &self.notes {
            let mut meta_map = serde_json::Map::new();
            meta_map.insert("id".to_string(), serde_json::json!(self.id));
            meta_map.insert("notes".to_string(), serde_json::json!(notes));
            meta_map.insert("created_at".to_string(), serde_json::json!(self.created_at));
            meta_map.insert("updated_at".to_string(), serde_json::json!(self.updated_at));
            entry = entry.meta(meta_map);
        }

        entry
    }
}

/// Container for all plan entries representing the complete execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlan {
    /// Unique identifier for this plan
    pub id: String,
    /// List of plan entries in execution order
    pub entries: Vec<PlanEntry>,
    /// Timestamp when this plan was created
    pub created_at: std::time::SystemTime,
    /// Optional metadata about the plan
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl AgentPlan {
    /// Create a new empty agent plan
    pub fn new() -> Self {
        Self {
            id: Ulid::new().to_string(),
            entries: Vec::new(),
            created_at: std::time::SystemTime::now(),
            metadata: None,
        }
    }

    /// Create a plan from a list of plan entries
    pub fn from_entries(entries: Vec<PlanEntry>) -> Self {
        Self {
            id: Ulid::new().to_string(),
            entries,
            created_at: std::time::SystemTime::now(),
            metadata: None,
        }
    }

    /// Add a plan entry to this plan
    pub fn add_entry(&mut self, entry: PlanEntry) {
        self.entries.push(entry);
    }

    /// Get a plan entry by ID
    pub fn get_entry(&self, id: &str) -> Option<&PlanEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Get a mutable reference to a plan entry by ID
    pub fn get_entry_mut(&mut self, id: &str) -> Option<&mut PlanEntry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    /// Update the status of a specific plan entry
    pub fn update_entry_status(&mut self, entry_id: &str, new_status: PlanEntryStatus) -> bool {
        if let Some(entry) = self.get_entry_mut(entry_id) {
            entry.update_status(new_status);
            true
        } else {
            false
        }
    }

    /// Get the next pending plan entry (highest priority first)
    pub fn next_pending_entry(&self) -> Option<&PlanEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.status == PlanEntryStatus::Pending)
            .min_by_key(|entry| &entry.priority)
    }

    /// Get count of entries by status
    pub fn count_by_status(&self, status: PlanEntryStatus) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.status == status)
            .count()
    }

    /// Check if all plan entries are complete
    pub fn is_complete(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|entry| entry.is_complete())
    }

    /// Get plan completion percentage (0.0 to 1.0)
    pub fn completion_percentage(&self) -> f64 {
        if self.entries.is_empty() {
            return 1.0;
        }

        let completed_count = self
            .entries
            .iter()
            .filter(|entry| entry.is_complete())
            .count();
        completed_count as f64 / self.entries.len() as f64
    }

    /// Convert plan to ACP-compliant format for session/update notifications
    pub fn to_acp_plan(&self) -> AcpPlan {
        let entries = self
            .entries
            .iter()
            .map(|entry| entry.to_acp_entry())
            .collect();

        let mut plan = AcpPlan::new(entries);

        if let Some(metadata) = self.metadata.as_ref().and_then(|v| v.as_object().cloned()) {
            plan = plan.meta(metadata);
        }

        plan
    }

    /// Deprecated: Use to_acp_plan() instead
    #[deprecated(note = "Use to_acp_plan() to get proper ACP Plan type")]
    pub fn to_acp_format(&self) -> serde_json::Value {
        serde_json::json!({
            "sessionUpdate": "plan",
            "entries": self.entries.iter().map(|entry| {
                serde_json::json!({
                    "content": entry.content,
                    "priority": entry.priority,
                    "status": entry.status
                })
            }).collect::<Vec<_>>()
        })
    }
}

impl Default for AgentPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert TodoWrite tool parameters to AgentPlan
///
/// This is the internal conversion function that creates an AgentPlan from TodoWrite parameters.
///
/// # TodoWrite Format
/// ```json
/// {
///   "todos": [
///     {
///       "content": "Task description",
///       "status": "pending" | "in_progress" | "completed",
///       "activeForm": "Present continuous form of task"
///     }
///   ]
/// }
/// ```
///
/// # Mapping Rules
/// - status "pending" → PlanEntryStatus::Pending, Priority::Medium
/// - status "in_progress" → PlanEntryStatus::InProgress, Priority::High
/// - status "completed" → PlanEntryStatus::Completed, Priority::Low
/// - For in-progress items: activeForm is used as content (present continuous shows current activity)
/// - For other statuses: activeForm is stored in entry notes
///
/// # Errors
/// Returns error if:
/// - Missing "todos" field
/// - Invalid todo item structure
/// - Invalid status value
pub fn todowrite_to_agent_plan(
    todowrite_params: &serde_json::Value,
) -> Result<AgentPlan, crate::error::AgentError> {
    // Extract todos array
    let todos_array = todowrite_params
        .get("todos")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            crate::error::AgentError::Internal("TodoWrite params missing 'todos' array".to_string())
        })?;

    let mut plan = AgentPlan::new();

    // Convert each todo item to a plan entry
    for todo_item in todos_array {
        let content = todo_item
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::error::AgentError::Internal(
                    "TodoWrite item missing 'content' field".to_string(),
                )
            })?
            .to_string();

        let status_str = todo_item
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::error::AgentError::Internal(
                    "TodoWrite item missing 'status' field".to_string(),
                )
            })?;

        // Map TodoWrite status to PlanEntryStatus and infer priority
        let (status, priority) = match status_str {
            "pending" => (PlanEntryStatus::Pending, Priority::Medium),
            "in_progress" => (PlanEntryStatus::InProgress, Priority::High),
            "completed" => (PlanEntryStatus::Completed, Priority::Low),
            _ => {
                return Err(crate::error::AgentError::Internal(format!(
                    "Invalid TodoWrite status: {}",
                    status_str
                )))
            }
        };

        // Create plan entry
        // For in-progress items, use activeForm as content if present (present continuous tense)
        // For other statuses, keep the base content and store activeForm in notes
        let display_content = if status == PlanEntryStatus::InProgress {
            todo_item
                .get("activeForm")
                .and_then(|v| v.as_str())
                .unwrap_or(&content)
                .to_string()
        } else {
            content.clone()
        };

        let mut entry = PlanEntry::new(display_content, priority);
        entry.update_status(status);

        // Store original content and activeForm in notes for reference
        if let Some(active_form) = todo_item.get("activeForm").and_then(|v| v.as_str()) {
            if status == PlanEntryStatus::InProgress {
                // For in-progress items, store the original content in notes
                entry.set_notes(format!("Original: {}", content));
            } else {
                // For other statuses, store the activeForm in notes
                entry.set_notes(format!("Active form: {}", active_form));
            }
        }

        plan.add_entry(entry);
    }

    // Add metadata about the source
    plan.metadata = Some(serde_json::json!({
        "source": "todowrite",
        "tool": "TodoWrite"
    }));

    Ok(plan)
}

/// Convert TodoWrite tool parameters to ACP Plan format
///
/// This function parses TodoWrite tool call parameters (as used by Claude LLM)
/// and converts them to ACP-compliant Plan format for client notifications.
///
/// # TodoWrite Format
/// ```json
/// {
///   "todos": [
///     {
///       "content": "Task description",
///       "status": "pending" | "in_progress" | "completed",
///       "activeForm": "Present continuous form of task"
///     }
///   ]
/// }
/// ```
///
/// # Mapping Rules
/// - status "pending" → PlanEntryStatus::Pending, Priority::Medium
/// - status "in_progress" → PlanEntryStatus::InProgress, Priority::High
/// - status "completed" → PlanEntryStatus::Completed, Priority::Low
/// - For in-progress items: activeForm is used as content (present continuous shows current activity)
/// - For other statuses: activeForm is stored in entry notes
///
/// # Errors
/// Returns error if:
/// - Missing "todos" field
/// - Invalid todo item structure
/// - Invalid status value
pub fn todowrite_to_acp_plan(
    todowrite_params: &serde_json::Value,
) -> Result<AcpPlan, crate::error::AgentError> {
    let agent_plan = todowrite_to_agent_plan(todowrite_params)?;
    Ok(agent_plan.to_acp_plan())
}

/// Plan manager for tracking plan state across sessions
pub struct PlanManager {
    /// Active plans by session ID
    active_plans: HashMap<String, AgentPlan>,
}

impl PlanManager {
    /// Create a new plan manager
    pub fn new() -> Self {
        Self {
            active_plans: HashMap::new(),
        }
    }

    /// Store a plan for a session
    ///
    /// If a plan already exists for this session, this will replace it entirely.
    /// Use `update_plan` to merge changes while preserving entry IDs.
    pub fn set_plan(&mut self, session_id: String, plan: AgentPlan) {
        self.active_plans.insert(session_id, plan);
    }

    /// Update an existing plan with new data, preserving entry IDs where possible
    ///
    /// This method attempts to match entries from the new plan to existing entries
    /// by content, preserving the original entry IDs and only updating the status
    /// and other fields. New entries are added, and entries not in the new plan
    /// are removed.
    ///
    /// Returns true if the plan was updated, false if no plan exists for this session.
    pub fn update_plan(&mut self, session_id: &str, new_plan: AgentPlan) -> bool {
        if let Some(existing_plan) = self.active_plans.get_mut(session_id) {
            // Create a map of content -> existing entry for matching
            let mut existing_by_content: HashMap<String, PlanEntry> = existing_plan
                .entries
                .drain(..)
                .map(|entry| (entry.content.clone(), entry))
                .collect();

            // Process new plan entries
            for new_entry in new_plan.entries {
                if let Some(mut existing_entry) = existing_by_content.remove(&new_entry.content) {
                    // Entry exists - update it while preserving ID and created_at
                    existing_entry.status = new_entry.status;
                    existing_entry.priority = new_entry.priority;
                    existing_entry.notes = new_entry.notes;
                    existing_entry.updated_at = Some(std::time::SystemTime::now());
                    existing_plan.entries.push(existing_entry);
                } else {
                    // New entry - add it as-is
                    existing_plan.entries.push(new_entry);
                }
            }

            // Update plan metadata
            existing_plan.metadata = new_plan.metadata;

            true
        } else {
            // No existing plan - just set it
            self.active_plans.insert(session_id.to_string(), new_plan);
            false
        }
    }

    /// Get the current plan for a session
    pub fn get_plan(&self, session_id: &str) -> Option<&AgentPlan> {
        self.active_plans.get(session_id)
    }

    /// Get a mutable reference to the current plan for a session
    pub fn get_plan_mut(&mut self, session_id: &str) -> Option<&mut AgentPlan> {
        self.active_plans.get_mut(session_id)
    }

    /// Update plan entry status for a session
    pub fn update_plan_entry_status(
        &mut self,
        session_id: &str,
        entry_id: &str,
        new_status: PlanEntryStatus,
    ) -> bool {
        if let Some(plan) = self.get_plan_mut(session_id) {
            plan.update_entry_status(entry_id, new_status)
        } else {
            false
        }
    }

    /// Remove plan for a session (cleanup)
    pub fn remove_plan(&mut self, session_id: &str) -> Option<AgentPlan> {
        self.active_plans.remove(session_id)
    }

    /// Clean up plans for expired sessions
    pub fn cleanup_expired_plans(&mut self, active_session_ids: &[String]) {
        let active_set: std::collections::HashSet<_> = active_session_ids.iter().collect();
        self.active_plans
            .retain(|session_id, _| active_set.contains(session_id));
    }
}

impl Default for PlanManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_entry_creation() {
        let entry = PlanEntry::new("Test task".to_string(), Priority::High);
        assert_eq!(entry.content, "Test task");
        assert_eq!(entry.priority, Priority::High);
        assert_eq!(entry.status, PlanEntryStatus::Pending);
        assert!(!entry.id.is_empty());
    }

    #[test]
    fn test_plan_entry_status_update() {
        let mut entry = PlanEntry::new("Test task".to_string(), Priority::Medium);
        assert_eq!(entry.status, PlanEntryStatus::Pending);

        entry.update_status(PlanEntryStatus::InProgress);
        assert_eq!(entry.status, PlanEntryStatus::InProgress);
        assert!(entry.is_in_progress());

        entry.update_status(PlanEntryStatus::Completed);
        assert_eq!(entry.status, PlanEntryStatus::Completed);
        assert!(entry.is_complete());
    }

    #[test]
    fn test_agent_plan_creation() {
        let mut plan = AgentPlan::new();
        assert!(plan.entries.is_empty());
        assert!(!plan.id.is_empty());

        let entry = PlanEntry::new("Test step".to_string(), Priority::High);
        plan.add_entry(entry);
        assert_eq!(plan.entries.len(), 1);
    }

    #[test]
    fn test_plan_completion_tracking() {
        let mut plan = AgentPlan::new();
        plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::High));
        plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::Medium));

        assert!(!plan.is_complete());
        assert_eq!(plan.completion_percentage(), 0.0);

        // Complete first entry
        let entry_id = plan.entries[0].id.clone();
        plan.update_entry_status(&entry_id, PlanEntryStatus::Completed);
        assert_eq!(plan.completion_percentage(), 0.5);

        // Complete second entry
        let entry_id = plan.entries[1].id.clone();
        plan.update_entry_status(&entry_id, PlanEntryStatus::Completed);
        assert!(plan.is_complete());
        assert_eq!(plan.completion_percentage(), 1.0);
    }

    #[test]
    fn test_plan_acp_format() {
        let mut plan = AgentPlan::new();
        plan.add_entry(PlanEntry::new("Test step".to_string(), Priority::High));

        let acp_plan = plan.to_acp_plan();
        assert_eq!(acp_plan.entries.len(), 1);
        assert_eq!(acp_plan.entries[0].content, "Test step");
    }

    #[test]
    fn test_plan_manager() {
        let mut manager = PlanManager::new();
        let plan = AgentPlan::new();
        let session_id = "test_session".to_string();

        manager.set_plan(session_id.clone(), plan);
        assert!(manager.get_plan(&session_id).is_some());

        manager.remove_plan(&session_id);
        assert!(manager.get_plan(&session_id).is_none());
    }

    #[test]
    fn test_priority_to_acp_conversion() {
        // Test by serializing to JSON and checking the values
        let high = Priority::High.to_acp_priority();
        let high_json = serde_json::to_value(&high).unwrap();
        assert_eq!(high_json, "high");

        let medium = Priority::Medium.to_acp_priority();
        let medium_json = serde_json::to_value(&medium).unwrap();
        assert_eq!(medium_json, "medium");

        let low = Priority::Low.to_acp_priority();
        let low_json = serde_json::to_value(&low).unwrap();
        assert_eq!(low_json, "low");
    }

    #[test]
    fn test_status_to_acp_conversion() {
        // Test by serializing to JSON and checking the values
        let pending = PlanEntryStatus::Pending.to_acp_status();
        let pending_json = serde_json::to_value(&pending).unwrap();
        assert_eq!(pending_json, "pending");

        let in_progress = PlanEntryStatus::InProgress.to_acp_status();
        let in_progress_json = serde_json::to_value(&in_progress).unwrap();
        assert_eq!(in_progress_json, "in_progress");

        let completed = PlanEntryStatus::Completed.to_acp_status();
        let completed_json = serde_json::to_value(&completed).unwrap();
        assert_eq!(completed_json, "completed");

        // Failed and Cancelled map to Completed in ACP
        let failed = PlanEntryStatus::Failed.to_acp_status();
        let failed_json = serde_json::to_value(&failed).unwrap();
        assert_eq!(failed_json, "completed");

        let cancelled = PlanEntryStatus::Cancelled.to_acp_status();
        let cancelled_json = serde_json::to_value(&cancelled).unwrap();
        assert_eq!(cancelled_json, "completed");
    }

    #[test]
    fn test_plan_entry_to_acp_conversion() {
        let entry = PlanEntry::new("Test task".to_string(), Priority::High);
        let acp_entry = entry.to_acp_entry();

        assert_eq!(acp_entry.content, "Test task");
        let priority_json = serde_json::to_value(&acp_entry.priority).unwrap();
        assert_eq!(priority_json, "high");
        let status_json = serde_json::to_value(&acp_entry.status).unwrap();
        assert_eq!(status_json, "pending");
    }

    #[test]
    fn test_plan_entry_to_acp_with_notes() {
        let mut entry = PlanEntry::new("Task with notes".to_string(), Priority::Medium);
        entry.set_notes("Important context".to_string());
        let acp_entry = entry.to_acp_entry();

        assert_eq!(acp_entry.content, "Task with notes");
        assert!(acp_entry.meta.is_some());
        let meta = acp_entry.meta.unwrap();
        assert_eq!(meta["notes"], "Important context");
        assert_eq!(meta["id"], entry.id);
    }

    #[test]
    fn test_agent_plan_to_acp_conversion() {
        let mut plan = AgentPlan::new();
        plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::High));
        plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::Medium));
        plan.add_entry(PlanEntry::new("Step 3".to_string(), Priority::Low));

        let acp_plan = plan.to_acp_plan();

        assert_eq!(acp_plan.entries.len(), 3);
        assert_eq!(acp_plan.entries[0].content, "Step 1");
        let priority_0_json = serde_json::to_value(&acp_plan.entries[0].priority).unwrap();
        assert_eq!(priority_0_json, "high");
        assert_eq!(acp_plan.entries[1].content, "Step 2");
        let priority_1_json = serde_json::to_value(&acp_plan.entries[1].priority).unwrap();
        assert_eq!(priority_1_json, "medium");
        assert_eq!(acp_plan.entries[2].content, "Step 3");
        let priority_2_json = serde_json::to_value(&acp_plan.entries[2].priority).unwrap();
        assert_eq!(priority_2_json, "low");
    }

    #[test]
    fn test_plan_to_acp_with_metadata() {
        let mut plan = AgentPlan::new();
        plan.metadata = Some(serde_json::json!({
            "generator": "test",
            "version": "1.0"
        }));
        plan.add_entry(PlanEntry::new("Test".to_string(), Priority::High));

        let acp_plan = plan.to_acp_plan();

        assert_eq!(acp_plan.entries.len(), 1);
        assert!(acp_plan.meta.is_some());
        let meta = acp_plan.meta.unwrap();
        assert_eq!(meta["generator"], "test");
        assert_eq!(meta["version"], "1.0");
    }

    #[test]
    fn test_plan_next_pending_entry() {
        let mut plan = AgentPlan::new();
        plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::Low));
        plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::High));
        plan.add_entry(PlanEntry::new("Step 3".to_string(), Priority::Medium));

        let next = plan.next_pending_entry();
        assert!(next.is_some());
        assert_eq!(next.unwrap().content, "Step 2"); // High priority comes first
    }

    #[test]
    fn test_plan_count_by_status() {
        let mut plan = AgentPlan::new();
        plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::High));
        plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::High));
        plan.add_entry(PlanEntry::new("Step 3".to_string(), Priority::High));

        assert_eq!(plan.count_by_status(PlanEntryStatus::Pending), 3);
        assert_eq!(plan.count_by_status(PlanEntryStatus::Completed), 0);

        let entry_id = plan.entries[0].id.clone();
        plan.update_entry_status(&entry_id, PlanEntryStatus::Completed);

        assert_eq!(plan.count_by_status(PlanEntryStatus::Pending), 2);
        assert_eq!(plan.count_by_status(PlanEntryStatus::Completed), 1);
    }

    #[test]
    fn test_todowrite_to_acp_plan() {
        let todowrite_json = serde_json::json!({
            "todos": [
                {
                    "content": "Discover changed files in git",
                    "status": "in_progress",
                    "activeForm": "Discovering changed files in git"
                },
                {
                    "content": "Check code quality issues",
                    "status": "pending",
                    "activeForm": "Checking code quality issues"
                },
                {
                    "content": "Fix any quality issues",
                    "status": "pending",
                    "activeForm": "Fixing quality issues"
                }
            ]
        });

        let result = todowrite_to_acp_plan(&todowrite_json);
        assert!(result.is_ok());

        let acp_plan = result.unwrap();
        assert_eq!(acp_plan.entries.len(), 3);

        // Check first entry (in_progress should map to InProgress)
        // For in-progress items, activeForm should be used as content
        assert_eq!(
            acp_plan.entries[0].content,
            "Discovering changed files in git"
        );
        let status_json = serde_json::to_value(&acp_plan.entries[0].status).unwrap();
        assert_eq!(status_json, "in_progress");
        let priority_json = serde_json::to_value(&acp_plan.entries[0].priority).unwrap();
        assert_eq!(priority_json, "high"); // in_progress gets high priority

        // Check second entry (pending should map to Pending)
        assert_eq!(acp_plan.entries[1].content, "Check code quality issues");
        let status_json = serde_json::to_value(&acp_plan.entries[1].status).unwrap();
        assert_eq!(status_json, "pending");
        let priority_json = serde_json::to_value(&acp_plan.entries[1].priority).unwrap();
        assert_eq!(priority_json, "medium"); // pending gets medium priority
    }

    #[test]
    fn test_todowrite_to_acp_plan_with_completed() {
        let todowrite_json = serde_json::json!({
            "todos": [
                {
                    "content": "Task 1",
                    "status": "completed",
                    "activeForm": "Completing Task 1"
                }
            ]
        });

        let result = todowrite_to_acp_plan(&todowrite_json);
        assert!(result.is_ok());

        let acp_plan = result.unwrap();
        assert_eq!(acp_plan.entries.len(), 1);
        let status_json = serde_json::to_value(&acp_plan.entries[0].status).unwrap();
        assert_eq!(status_json, "completed");
    }

    #[test]
    fn test_todowrite_to_acp_plan_invalid_json() {
        let invalid_json = serde_json::json!({
            "not_todos": []
        });

        let result = todowrite_to_acp_plan(&invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_plan_manager_update_preserves_ids() {
        let mut manager = PlanManager::new();
        let session_id = "test_session";

        // Create initial plan with 3 entries
        let mut initial_plan = AgentPlan::new();
        initial_plan.add_entry(PlanEntry::new("Task 1".to_string(), Priority::High));
        initial_plan.add_entry(PlanEntry::new("Task 2".to_string(), Priority::Medium));
        initial_plan.add_entry(PlanEntry::new("Task 3".to_string(), Priority::Low));

        // Store initial plan
        manager.set_plan(session_id.to_string(), initial_plan);

        // Get the stored plan and capture entry IDs
        let stored_plan = manager.get_plan(session_id).unwrap();
        let task1_id = stored_plan.entries[0].id.clone();
        let task2_id = stored_plan.entries[1].id.clone();
        let task3_id = stored_plan.entries[2].id.clone();

        assert_eq!(stored_plan.entries[0].status, PlanEntryStatus::Pending);
        assert_eq!(stored_plan.entries[1].status, PlanEntryStatus::Pending);
        assert_eq!(stored_plan.entries[2].status, PlanEntryStatus::Pending);

        // Create updated plan with status changes
        let mut updated_plan = AgentPlan::new();
        let mut entry1 = PlanEntry::new("Task 1".to_string(), Priority::High);
        entry1.update_status(PlanEntryStatus::Completed);
        updated_plan.add_entry(entry1);

        let mut entry2 = PlanEntry::new("Task 2".to_string(), Priority::High);
        entry2.update_status(PlanEntryStatus::InProgress);
        updated_plan.add_entry(entry2);

        updated_plan.add_entry(PlanEntry::new("Task 3".to_string(), Priority::Low));

        // Update the plan
        let was_updated = manager.update_plan(session_id, updated_plan);
        assert!(was_updated);

        // Verify IDs are preserved but status is updated
        let final_plan = manager.get_plan(session_id).unwrap();
        assert_eq!(final_plan.entries.len(), 3);

        // Find entries by content and verify IDs and status
        let task1 = final_plan
            .entries
            .iter()
            .find(|e| e.content == "Task 1")
            .unwrap();
        assert_eq!(task1.id, task1_id); // ID preserved
        assert_eq!(task1.status, PlanEntryStatus::Completed); // Status updated

        let task2 = final_plan
            .entries
            .iter()
            .find(|e| e.content == "Task 2")
            .unwrap();
        assert_eq!(task2.id, task2_id); // ID preserved
        assert_eq!(task2.status, PlanEntryStatus::InProgress); // Status updated
        assert_eq!(task2.priority, Priority::High); // Priority updated

        let task3 = final_plan
            .entries
            .iter()
            .find(|e| e.content == "Task 3")
            .unwrap();
        assert_eq!(task3.id, task3_id); // ID preserved
        assert_eq!(task3.status, PlanEntryStatus::Pending); // Status unchanged
    }

    #[test]
    fn test_plan_manager_update_adds_new_entries() {
        let mut manager = PlanManager::new();
        let session_id = "test_session";

        // Create initial plan with 2 entries
        let mut initial_plan = AgentPlan::new();
        initial_plan.add_entry(PlanEntry::new("Task 1".to_string(), Priority::High));
        initial_plan.add_entry(PlanEntry::new("Task 2".to_string(), Priority::Medium));
        manager.set_plan(session_id.to_string(), initial_plan);

        // Create updated plan with 3 entries (1 new)
        let mut updated_plan = AgentPlan::new();
        updated_plan.add_entry(PlanEntry::new("Task 1".to_string(), Priority::High));
        updated_plan.add_entry(PlanEntry::new("Task 2".to_string(), Priority::Medium));
        updated_plan.add_entry(PlanEntry::new("Task 3".to_string(), Priority::Low));

        manager.update_plan(session_id, updated_plan);

        let final_plan = manager.get_plan(session_id).unwrap();
        assert_eq!(final_plan.entries.len(), 3);
        assert!(final_plan.entries.iter().any(|e| e.content == "Task 3"));
    }

    #[test]
    fn test_plan_manager_update_removes_old_entries() {
        let mut manager = PlanManager::new();
        let session_id = "test_session";

        // Create initial plan with 3 entries
        let mut initial_plan = AgentPlan::new();
        initial_plan.add_entry(PlanEntry::new("Task 1".to_string(), Priority::High));
        initial_plan.add_entry(PlanEntry::new("Task 2".to_string(), Priority::Medium));
        initial_plan.add_entry(PlanEntry::new("Task 3".to_string(), Priority::Low));
        manager.set_plan(session_id.to_string(), initial_plan);

        // Create updated plan with only 2 entries
        let mut updated_plan = AgentPlan::new();
        updated_plan.add_entry(PlanEntry::new("Task 1".to_string(), Priority::High));
        updated_plan.add_entry(PlanEntry::new("Task 2".to_string(), Priority::Medium));

        manager.update_plan(session_id, updated_plan);

        let final_plan = manager.get_plan(session_id).unwrap();
        assert_eq!(final_plan.entries.len(), 2);
        assert!(!final_plan.entries.iter().any(|e| e.content == "Task 3"));
    }
}
