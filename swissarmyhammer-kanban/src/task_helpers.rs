//! Task-specific logic for Entity-based tasks.
//!
//! These free functions replace methods that were previously on the `Task` struct.
//! They work with `Entity` and raw field values, providing computed fields
//! (progress, readiness, dependency graph) and JSON serialization that matches
//! the API contract expected by the frontend.

use crate::tag_parser;
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;

/// Compute tag names from `#tag` patterns in the task body.
pub fn task_tags(entity: &Entity) -> Vec<String> {
    let body = entity.get_str("body").unwrap_or("");
    tag_parser::parse_tags(body)
}

/// Calculate progress as fraction of completed markdown checklist items.
///
/// Parses `- [ ]` (incomplete) and `- [x]`/`- [X]` (complete) from the body.
/// Returns 0.0 if no checklist items are found.
pub fn task_progress(entity: &Entity) -> f64 {
    let body = entity.get_str("body").unwrap_or("");
    let (total, completed) = parse_checklist_counts(body);
    if total == 0 {
        return 0.0;
    }
    completed as f64 / total as f64
}

/// Parse markdown checklist items from text, returning (total, completed) counts.
pub fn parse_checklist_counts(text: &str) -> (usize, usize) {
    let mut total = 0usize;
    let mut completed = 0usize;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("- [ ] ") || trimmed == "- [ ]" {
            total += 1;
        } else if trimmed.starts_with("- [x] ")
            || trimmed.starts_with("- [X] ")
            || trimmed == "- [x]"
            || trimmed == "- [X]"
        {
            total += 1;
            completed += 1;
        }
    }
    (total, completed)
}

/// Check if all dependencies are complete (in the given terminal column).
pub fn task_is_ready(entity: &Entity, all_tasks: &[Entity], terminal_column_id: &str) -> bool {
    let deps = entity.get_string_list("depends_on");
    deps.iter().all(|dep_id| {
        all_tasks
            .iter()
            .find(|t| t.id == *dep_id)
            .map(|t| t.get_str("position_column") == Some(terminal_column_id))
            .unwrap_or(true) // Missing dependency is treated as complete
    })
}

/// Get task IDs that this task is blocked by (incomplete dependencies).
pub fn task_blocked_by(
    entity: &Entity,
    all_tasks: &[Entity],
    terminal_column_id: &str,
) -> Vec<String> {
    let deps = entity.get_string_list("depends_on");
    deps.into_iter()
        .filter(|dep_id| {
            all_tasks
                .iter()
                .find(|t| t.id == *dep_id)
                .map(|t| t.get_str("position_column") != Some(terminal_column_id))
                .unwrap_or(false)
        })
        .collect()
}

/// Get task IDs that depend on this task.
pub fn task_blocks(entity: &Entity, all_tasks: &[Entity]) -> Vec<String> {
    all_tasks
        .iter()
        .filter(|t| t.get_string_list("depends_on").contains(&entity.id))
        .map(|t| t.id.clone())
        .collect()
}

/// Convert a task Entity to the JSON format expected by the API/frontend.
///
/// Transforms flat entity fields into the nested format:
/// - "body" → "description" (rename for backward compat)
/// - position_column/position_swimlane/position_ordinal → nested "position" object
/// - Adds computed fields: tags, progress
pub fn task_entity_to_json(entity: &Entity) -> Value {
    let tags = task_tags(entity);
    let progress = task_progress(entity);

    let position_column = entity.get_str("position_column").unwrap_or("");
    let position_swimlane = entity.get_str("position_swimlane");
    let position_ordinal = entity.get_str("position_ordinal").unwrap_or("a0");

    let position = if let Some(swimlane) = position_swimlane {
        json!({
            "column": position_column,
            "swimlane": swimlane,
            "ordinal": position_ordinal,
        })
    } else {
        json!({
            "column": position_column,
            "ordinal": position_ordinal,
        })
    };

    let mut result = json!({
        "id": entity.id,
        "title": entity.get_str("title").unwrap_or(""),
        "description": entity.get_str("body").unwrap_or(""),
        "position": position,
        "tags": tags,
        "assignees": entity.get_string_list("assignees"),
        "depends_on": entity.get_string_list("depends_on"),
        "progress": progress,
    });

    // Include attachments if present
    if let Some(attachments) = entity.get("attachments") {
        result["attachments"] = attachments.clone();
    } else {
        result["attachments"] = json!([]);
    }

    result
}

/// Convert a task Entity to JSON with full computed fields (ready, blocked_by, blocks).
///
/// This is the "rich" version used by get/list that requires all tasks for DAG analysis.
pub fn task_entity_to_rich_json(
    entity: &Entity,
    all_tasks: &[Entity],
    terminal_column_id: &str,
) -> Value {
    let mut result = task_entity_to_json(entity);

    let ready = task_is_ready(entity, all_tasks, terminal_column_id);
    let blocked_by = task_blocked_by(entity, all_tasks, terminal_column_id);
    let blocks = task_blocks(entity, all_tasks);

    result["ready"] = json!(ready);
    result["blocked_by"] = json!(blocked_by);
    result["blocks"] = json!(blocks);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_task(id: &str, title: &str, body: &str, column: &str) -> Entity {
        let mut e = Entity::new("task", id);
        e.set("title", json!(title));
        e.set("body", json!(body));
        e.set("position_column", json!(column));
        e.set("position_ordinal", json!("a0"));
        e
    }

    #[test]
    fn test_task_tags_from_body() {
        let e = make_task("t1", "Test", "Fix the #bug in #login", "todo");
        let tags = task_tags(&e);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"bug".to_string()));
        assert!(tags.contains(&"login".to_string()));
    }

    #[test]
    fn test_task_tags_empty_body() {
        let e = make_task("t1", "Test", "", "todo");
        assert!(task_tags(&e).is_empty());
    }

    #[test]
    fn test_task_progress() {
        let e = make_task("t1", "Test", "- [ ] one\n- [x] two", "todo");
        assert_eq!(task_progress(&e), 0.5);
    }

    #[test]
    fn test_task_progress_no_checklist() {
        let e = make_task("t1", "Test", "No checklist here", "todo");
        assert_eq!(task_progress(&e), 0.0);
    }

    #[test]
    fn test_parse_checklist_counts() {
        assert_eq!(parse_checklist_counts(""), (0, 0));
        assert_eq!(
            parse_checklist_counts("- [ ] one\n- [x] two\n- [X] three\n- [ ] four"),
            (4, 2)
        );
        assert_eq!(
            parse_checklist_counts("  - [ ] indented\n  - [x] done"),
            (2, 1)
        );
        assert_eq!(
            parse_checklist_counts("plain text\n- regular bullet\n- [ ] real item"),
            (1, 0)
        );
    }

    #[test]
    fn test_task_is_ready_no_deps() {
        let e = make_task("t1", "Test", "", "todo");
        assert!(task_is_ready(&e, &[], "done"));
    }

    #[test]
    fn test_task_is_ready_deps_complete() {
        let dep = make_task("dep1", "Dep", "", "done");
        let mut e = make_task("t1", "Test", "", "todo");
        e.set("depends_on", json!(["dep1"]));
        assert!(task_is_ready(&e, &[dep, e.clone()], "done"));
    }

    #[test]
    fn test_task_is_ready_deps_incomplete() {
        let dep = make_task("dep1", "Dep", "", "todo");
        let mut e = make_task("t1", "Test", "", "todo");
        e.set("depends_on", json!(["dep1"]));
        assert!(!task_is_ready(&e, &[dep, e.clone()], "done"));
    }

    #[test]
    fn test_task_blocked_by() {
        let dep = make_task("dep1", "Dep", "", "todo");
        let mut e = make_task("t1", "Test", "", "todo");
        e.set("depends_on", json!(["dep1"]));
        let blocked = task_blocked_by(&e, &[dep, e.clone()], "done");
        assert_eq!(blocked, vec!["dep1"]);
    }

    #[test]
    fn test_task_blocks() {
        let blocker = make_task("t1", "Blocker", "", "todo");
        let mut dependent = make_task("t2", "Dependent", "", "todo");
        dependent.set("depends_on", json!(["t1"]));
        let blocks = task_blocks(&blocker, &[blocker.clone(), dependent]);
        assert_eq!(blocks, vec!["t2"]);
    }

    #[test]
    fn test_task_entity_to_json() {
        let mut e = make_task("t1", "Test Task", "Some #bug description", "todo");
        e.set("position_swimlane", json!("feature"));
        e.set("position_ordinal", json!("a1"));
        e.set("assignees", json!(["alice"]));

        let result = task_entity_to_json(&e);
        assert_eq!(result["id"], "t1");
        assert_eq!(result["title"], "Test Task");
        assert_eq!(result["description"], "Some #bug description");
        assert_eq!(result["position"]["column"], "todo");
        assert_eq!(result["position"]["swimlane"], "feature");
        assert_eq!(result["position"]["ordinal"], "a1");
        assert!(result["tags"].as_array().unwrap().contains(&json!("bug")));
        assert_eq!(result["assignees"], json!(["alice"]));
    }

    #[test]
    fn test_task_entity_to_rich_json() {
        let dep = make_task("dep1", "Dep", "", "todo");
        let mut e = make_task("t1", "Test", "", "todo");
        e.set("depends_on", json!(["dep1"]));

        let result = task_entity_to_rich_json(&e, &[dep, e.clone()], "done");
        assert_eq!(result["ready"], false);
        assert_eq!(result["blocked_by"], json!(["dep1"]));
    }
}
