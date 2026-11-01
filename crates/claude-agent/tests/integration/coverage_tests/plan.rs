//! Coverage tests: Plan types.

use claude_agent::plan::*;

// ============================================================================
// Plan tests
// ============================================================================

#[test]
fn test_plan_entry_status_to_acp() {
    assert_eq!(
        PlanEntryStatus::Pending.to_acp_status(),
        agent_client_protocol::schema::PlanEntryStatus::Pending
    );
    assert_eq!(
        PlanEntryStatus::InProgress.to_acp_status(),
        agent_client_protocol::schema::PlanEntryStatus::InProgress
    );
    assert_eq!(
        PlanEntryStatus::Completed.to_acp_status(),
        agent_client_protocol::schema::PlanEntryStatus::Completed
    );
    // Failed and Cancelled map to Completed
    assert_eq!(
        PlanEntryStatus::Failed.to_acp_status(),
        agent_client_protocol::schema::PlanEntryStatus::Completed
    );
    assert_eq!(
        PlanEntryStatus::Cancelled.to_acp_status(),
        agent_client_protocol::schema::PlanEntryStatus::Completed
    );
}

#[test]
fn test_priority_to_acp() {
    assert_eq!(
        Priority::High.to_acp_priority(),
        agent_client_protocol::schema::PlanEntryPriority::High
    );
    assert_eq!(
        Priority::Medium.to_acp_priority(),
        agent_client_protocol::schema::PlanEntryPriority::Medium
    );
    assert_eq!(
        Priority::Low.to_acp_priority(),
        agent_client_protocol::schema::PlanEntryPriority::Low
    );
}

#[test]
fn test_priority_ordering() {
    assert!(Priority::High < Priority::Medium);
    assert!(Priority::Medium < Priority::Low);
    assert!(Priority::High < Priority::Low);
}

#[test]
fn test_plan_entry_new() {
    let entry = PlanEntry::new("Do something".to_string(), Priority::High);
    assert_eq!(entry.content, "Do something");
    assert_eq!(entry.priority, Priority::High);
    assert_eq!(entry.status, PlanEntryStatus::Pending);
    assert!(entry.notes.is_none());
    assert!(entry.created_at.is_some());
    assert!(entry.updated_at.is_some());
    assert!(!entry.id.is_empty());
}

#[test]
fn test_plan_entry_update_status() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Medium);
    assert_eq!(entry.status, PlanEntryStatus::Pending);
    assert!(!entry.is_in_progress());
    assert!(!entry.is_complete());

    entry.update_status(PlanEntryStatus::InProgress);
    assert!(entry.is_in_progress());
    assert!(!entry.is_complete());

    entry.update_status(PlanEntryStatus::Completed);
    assert!(!entry.is_in_progress());
    assert!(entry.is_complete());
}

#[test]
fn test_plan_entry_failed_is_complete() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Low);
    entry.update_status(PlanEntryStatus::Failed);
    assert!(entry.is_complete());
}

#[test]
fn test_plan_entry_cancelled_is_complete() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Low);
    entry.update_status(PlanEntryStatus::Cancelled);
    assert!(entry.is_complete());
}

#[test]
fn test_plan_entry_set_notes() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Medium);
    entry.set_notes("Important note".to_string());
    assert_eq!(entry.notes, Some("Important note".to_string()));
}

#[test]
fn test_plan_entry_same_status_no_update() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Medium);
    let initial_updated = entry.updated_at;
    // Updating with same status should not change updated_at
    entry.update_status(PlanEntryStatus::Pending);
    assert_eq!(entry.updated_at, initial_updated);
}

#[test]
fn test_plan_entry_to_acp_entry() {
    let entry = PlanEntry::new("Test task".to_string(), Priority::High);
    let acp = entry.to_acp_entry();
    assert_eq!(acp.content, "Test task");
}

#[test]
fn test_plan_entry_to_acp_entry_with_notes() {
    let mut entry = PlanEntry::new("Test task".to_string(), Priority::High);
    entry.set_notes("some notes".to_string());
    let acp = entry.to_acp_entry();
    assert!(acp.meta.is_some());
    let meta = acp.meta.unwrap();
    assert!(meta.contains_key("notes"));
    assert!(meta.contains_key("id"));
}

#[test]
fn test_agent_plan_new() {
    let plan = AgentPlan::new();
    assert!(plan.entries.is_empty());
    assert!(plan.metadata.is_none());
    assert!(!plan.id.is_empty());
}

#[test]
fn test_agent_plan_add_entry() {
    let mut plan = AgentPlan::new();
    plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::High));
    plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::Medium));
    assert_eq!(plan.entries.len(), 2);
}

#[test]
fn test_agent_plan_get_entry() {
    let mut plan = AgentPlan::new();
    let entry = PlanEntry::new("Test".to_string(), Priority::High);
    let entry_id = entry.id.clone();
    plan.add_entry(entry);

    let found = plan.get_entry(&entry_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().content, "Test");

    assert!(plan.get_entry("nonexistent").is_none());
}

#[test]
fn test_agent_plan_update_entry_status() {
    let mut plan = AgentPlan::new();
    let entry = PlanEntry::new("Test".to_string(), Priority::High);
    let entry_id = entry.id.clone();
    plan.add_entry(entry);

    let updated = plan.update_entry_status(&entry_id, PlanEntryStatus::Completed);
    assert!(updated);
    assert_eq!(
        plan.get_entry(&entry_id).unwrap().status,
        PlanEntryStatus::Completed
    );

    let not_found = plan.update_entry_status("nonexistent", PlanEntryStatus::Failed);
    assert!(!not_found);
}

#[test]
fn test_agent_plan_is_complete() {
    let mut plan = AgentPlan::new();
    assert!(!plan.is_complete()); // Empty plan is NOT complete (requires entries)

    let entry = PlanEntry::new("task".to_string(), Priority::High);
    let id = entry.id.clone();
    plan.add_entry(entry);
    assert!(!plan.is_complete());

    plan.update_entry_status(&id, PlanEntryStatus::Completed);
    assert!(plan.is_complete());
}

#[test]
fn test_agent_plan_to_acp_plan() {
    let mut plan = AgentPlan::new();
    plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::High));
    plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::Low));

    let acp_plan = plan.to_acp_plan();
    assert_eq!(acp_plan.entries.len(), 2);
}

#[test]
fn test_plan_entry_status_serialization() {
    let status = PlanEntryStatus::InProgress;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"in_progress\"");

    let deserialized: PlanEntryStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, PlanEntryStatus::InProgress);
}

#[test]
fn test_priority_serialization() {
    let p = Priority::High;
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(json, "\"high\"");

    let deserialized: Priority = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, Priority::High);
}
