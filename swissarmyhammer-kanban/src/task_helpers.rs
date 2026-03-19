//! Task-specific logic for Entity-based tasks.
//!
//! These free functions work with `Entity` and raw field values, providing
//! computed fields (progress, readiness, dependency graph) and JSON serialization
//! that matches the API contract expected by the frontend.
//!
//! Tags and progress are populated by `ComputeEngine` during `EntityContext::read()`.
//! The functions here simply read those pre-computed fields.

use serde_json::{json, Value};
use std::collections::HashMap;
use swissarmyhammer_entity::Entity;

use crate::types::Ordinal;

/// Generate a default title for a new task.
///
/// Returns a static default. This centralizes the default so it can be
/// shared between the CLI, Tauri commands, and the Command trait layer.
pub fn default_task_title() -> &'static str {
    "New task"
}

/// Compute an ordinal for a task being dropped at `drop_index` within
/// a list of task entities ordered by their current ordinals.
///
/// Looks at the ordinals of the neighbors at `drop_index - 1` and
/// `drop_index` (the item that will shift right) to find a midpoint.
/// Falls back to `Ordinal::first()` if the list is empty, `Ordinal::after()`
/// if appending at the end, etc.
///
/// This mirrors the TypeScript `computeOrdinal()` from `board-view.tsx`,
/// moving the logic server-side so the frontend only needs to send
/// `drop_index`.
pub fn compute_ordinal_for_drop(tasks: &[Entity], drop_index: usize) -> Ordinal {
    if tasks.is_empty() {
        return Ordinal::first();
    }

    let get_ordinal = |entity: &Entity| -> Ordinal {
        entity
            .get_str("position_ordinal")
            .map(Ordinal::from_string)
            .unwrap_or_else(Ordinal::first)
    };

    // Appending at the end
    if drop_index >= tasks.len() {
        let last = &tasks[tasks.len() - 1];
        return Ordinal::after(&get_ordinal(last));
    }

    // Inserting at the beginning
    if drop_index == 0 {
        let first = &tasks[0];
        let first_ord = get_ordinal(first);
        return Ordinal::before(&first_ord);
    }

    // Between two neighbors
    let prev = get_ordinal(&tasks[drop_index - 1]);
    let next = get_ordinal(&tasks[drop_index]);
    Ordinal::between(&prev, &next)
}

/// Compute an ordinal for a task being inserted between two known neighbor ordinals.
///
/// - `before` — ordinal of the task that will be immediately above (lower ordinal).
/// - `after`  — ordinal of the task that will be immediately below (higher ordinal).
///
/// Any combination of `None` values is handled:
/// - Both `None`: returns `Ordinal::first()`
/// - Only `after` given (inserting at top): returns an ordinal before `after`
/// - Only `before` given (appending at bottom): returns `Ordinal::after(before)`
/// - Both given: returns `Ordinal::between(before, after)`
///
/// Compute an ordinal for a task given its neighbors.
///
/// Uses the `fractional_index` crate (via `Ordinal`) for correct
/// fractional key generation. Only ONE ordinal is ever computed —
/// no other entities are modified.
pub fn compute_ordinal_for_neighbors(before: Option<&Ordinal>, after: Option<&Ordinal>) -> Ordinal {
    match (before, after) {
        (None, None) => Ordinal::first(),
        (None, Some(after_ord)) => Ordinal::before(after_ord),
        (Some(before_ord), None) => Ordinal::after(before_ord),
        (Some(before_ord), Some(after_ord)) => Ordinal::between(before_ord, after_ord),
    }
}

/// Read tag names from the entity's pre-computed `tags` field.
///
/// Tags are derived by `ComputeEngine` (parse-body-tags) on read.
pub fn task_tags(entity: &Entity) -> Vec<String> {
    entity.get_string_list("tags")
}

/// Read progress as fraction of completed checklist items.
///
/// Progress is derived by `ComputeEngine` (parse-body-progress) on read.
/// Returns 0.0 if no progress data or no checklist items.
pub fn task_progress(entity: &Entity) -> f64 {
    let Some(progress) = entity.get("progress") else {
        return 0.0;
    };
    let total = progress.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
    if total == 0 {
        return 0.0;
    }
    let completed = progress
        .get("completed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
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
        .filter(|t| {
            t.get_string_list("depends_on")
                .contains(&entity.id.to_string())
        })
        .map(|t| t.id.to_string())
        .collect()
}

/// Inject computed fields into a task entity's fields map.
///
/// Enriches the raw entity with computed dependency-graph and progress data:
/// - `progress_fraction`: scalar 0.0–1.0 derived from checklist progress
/// - `ready`: true when all dependencies are in the terminal column
/// - `blocked_by`: list of incomplete dependency task IDs
/// - `blocks`: list of task IDs that depend on this task
///
/// Tags and raw progress are already populated by `ComputeEngine` during read;
/// this function adds the higher-level computed fields that require the full
/// task list for DAG analysis.
pub fn enrich_task_entity(entity: &mut Entity, all_tasks: &[Entity], terminal_column_id: &str) {
    // progress as a scalar fraction (the progress field from ComputeEngine is {total, completed, percent})
    let progress = task_progress(entity);
    entity.set("progress_fraction", json!(progress));

    // ready flag
    let ready = task_is_ready(entity, all_tasks, terminal_column_id);
    entity.set("ready", json!(ready));

    // blocked_by list
    let blocked_by = task_blocked_by(entity, all_tasks, terminal_column_id);
    entity.set("blocked_by", json!(blocked_by));

    // blocks list
    let blocks = task_blocks(entity, all_tasks);
    entity.set("blocks", json!(blocks));
}

/// Enrich all task entities in a single O(N) pass using pre-built indexes.
///
/// This is the batch alternative to calling `enrich_task_entity` in a loop,
/// which would be O(N^2) because each call scans all tasks for dependency
/// lookups. This function pre-builds `blocks` and `depends_on` indexes so
/// the per-task enrichment is O(1).
pub fn enrich_all_task_entities(entities: &mut [Entity], terminal_column_id: &str) {
    // Build dependency index: dep_id -> list of task_ids that depend on it (i.e. "blocks")
    let mut blocks_index: HashMap<String, Vec<String>> = HashMap::new();
    let mut depends_on_index: HashMap<String, Vec<String>> = HashMap::new();

    for entity in entities.iter() {
        let deps = entity.get_string_list("depends_on");
        for dep_id in &deps {
            blocks_index
                .entry(dep_id.clone())
                .or_default()
                .push(entity.id.to_string());
        }
        depends_on_index.insert(entity.id.to_string(), deps);
    }

    // Build position map for ready/blocked computation
    let positions: HashMap<String, String> = entities
        .iter()
        .map(|e| {
            (
                e.id.to_string(),
                e.get_str("position_column").unwrap_or("").to_string(),
            )
        })
        .collect();

    for entity in entities.iter_mut() {
        let progress = task_progress(entity);
        entity.set("progress_fraction", json!(progress));

        // Ready: all deps in terminal column
        let deps = depends_on_index
            .get(&entity.id.to_string())
            .cloned()
            .unwrap_or_default();
        let blocked_by: Vec<String> = deps
            .iter()
            .filter(|dep_id| {
                positions
                    .get(*dep_id)
                    .map(|col| col != terminal_column_id)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        let ready = blocked_by.is_empty();
        entity.set("ready", json!(ready));
        entity.set("blocked_by", json!(blocked_by));

        // Blocks: tasks that depend on this one
        let blocks = blocks_index
            .get(&entity.id.to_string())
            .cloned()
            .unwrap_or_default();
        entity.set("blocks", json!(blocks));
    }
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
    let position_ordinal = entity
        .get_str("position_ordinal")
        .unwrap_or(Ordinal::DEFAULT_STR);

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
        e.set("position_ordinal", json!(Ordinal::first().as_str()));
        e
    }

    /// Make a task with pre-computed fields (as ComputeEngine would populate).
    fn make_task_computed(
        id: &str,
        title: &str,
        body: &str,
        column: &str,
        tags: Vec<&str>,
        total: u32,
        completed: u32,
    ) -> Entity {
        let mut e = make_task(id, title, body, column);
        e.set("tags", json!(tags));
        let percent = if total > 0 {
            (completed as f64 / total as f64 * 100.0).round() as u32
        } else {
            0
        };
        e.set(
            "progress",
            json!({"total": total, "completed": completed, "percent": percent}),
        );
        e
    }

    #[test]
    fn test_task_tags_from_computed_field() {
        let e = make_task_computed(
            "t1",
            "Test",
            "Fix the #bug in #login",
            "todo",
            vec!["bug", "login"],
            0,
            0,
        );
        let tags = task_tags(&e);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"bug".to_string()));
        assert!(tags.contains(&"login".to_string()));
    }

    #[test]
    fn test_task_tags_empty() {
        let e = make_task_computed("t1", "Test", "", "todo", vec![], 0, 0);
        assert!(task_tags(&e).is_empty());
    }

    #[test]
    fn test_task_tags_no_field() {
        // No tags field set at all (bare entity without compute)
        let e = make_task("t1", "Test", "", "todo");
        assert!(task_tags(&e).is_empty());
    }

    #[test]
    fn test_task_progress() {
        let e = make_task_computed("t1", "Test", "- [ ] one\n- [x] two", "todo", vec![], 2, 1);
        assert_eq!(task_progress(&e), 0.5);
    }

    #[test]
    fn test_task_progress_no_checklist() {
        let e = make_task_computed("t1", "Test", "No checklist here", "todo", vec![], 0, 0);
        assert_eq!(task_progress(&e), 0.0);
    }

    #[test]
    fn test_task_progress_no_field() {
        // No progress field at all
        let e = make_task("t1", "Test", "", "todo");
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
        let mut e = make_task_computed(
            "t1",
            "Test Task",
            "Some #bug description",
            "todo",
            vec!["bug"],
            0,
            0,
        );
        e.set("position_swimlane", json!("feature"));
        let ordinal = Ordinal::after(&Ordinal::first());
        let ordinal_str = ordinal.as_str().to_string();
        e.set("position_ordinal", json!(ordinal_str));
        e.set("assignees", json!(["alice"]));

        let result = task_entity_to_json(&e);
        assert_eq!(result["id"], "t1");
        assert_eq!(result["title"], "Test Task");
        assert_eq!(result["description"], "Some #bug description");
        assert_eq!(result["position"]["column"], "todo");
        assert_eq!(result["position"]["swimlane"], "feature");
        assert_eq!(result["position"]["ordinal"], ordinal_str);
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

    #[test]
    fn test_enrich_task_entity_injects_computed_fields() {
        let dep = make_task("dep1", "Dep", "", "todo");
        let mut e = make_task_computed("t1", "Test", "- [ ] a\n- [x] b", "todo", vec![], 2, 1);
        e.set("depends_on", json!(["dep1"]));

        let all = vec![dep, e.clone()];
        enrich_task_entity(&mut e, &all, "done");

        assert_eq!(e.get("progress_fraction").unwrap(), &json!(0.5));
        assert_eq!(e.get("ready").unwrap(), &json!(false));
        assert_eq!(e.get("blocked_by").unwrap(), &json!(["dep1"]));
        assert_eq!(e.get("blocks").unwrap(), &json!([]));
    }

    #[test]
    fn test_enrich_task_entity_ready_when_deps_done() {
        let dep = make_task("dep1", "Dep", "", "done");
        let mut e = make_task("t1", "Test", "", "todo");
        e.set("depends_on", json!(["dep1"]));

        let all = vec![dep, e.clone()];
        enrich_task_entity(&mut e, &all, "done");

        assert_eq!(e.get("ready").unwrap(), &json!(true));
        assert!(e.get_string_list("blocked_by").is_empty());
    }

    #[test]
    fn test_enrich_all_task_entities_batch() {
        let dep = make_task("dep1", "Dep", "", "todo");
        let mut blocker =
            make_task_computed("t1", "Test", "- [ ] a\n- [x] b", "todo", vec![], 2, 1);
        blocker.set("depends_on", json!(["dep1"]));

        let mut entities = vec![dep, blocker];
        enrich_all_task_entities(&mut entities, "done");

        // dep1 should block t1
        let dep_enriched = &entities[0];
        assert_eq!(dep_enriched.get("blocks").unwrap(), &json!(["t1"]));
        assert_eq!(dep_enriched.get("ready").unwrap(), &json!(true));

        // t1 should be blocked by dep1
        let t1_enriched = &entities[1];
        assert_eq!(t1_enriched.get("progress_fraction").unwrap(), &json!(0.5));
        assert_eq!(t1_enriched.get("ready").unwrap(), &json!(false));
        assert_eq!(t1_enriched.get("blocked_by").unwrap(), &json!(["dep1"]));
        assert_eq!(t1_enriched.get("blocks").unwrap(), &json!([]));
    }

    #[test]
    fn test_enrich_all_task_entities_ready_when_deps_done() {
        let dep = make_task("dep1", "Dep", "", "done");
        let mut task = make_task("t1", "Test", "", "todo");
        task.set("depends_on", json!(["dep1"]));

        let mut entities = vec![dep, task];
        enrich_all_task_entities(&mut entities, "done");

        let t1_enriched = &entities[1];
        assert_eq!(t1_enriched.get("ready").unwrap(), &json!(true));
        assert!(t1_enriched.get_string_list("blocked_by").is_empty());
    }

    // =========================================================================
    // default_task_title tests
    // =========================================================================

    #[test]
    fn test_default_task_title() {
        let title = default_task_title();
        assert_eq!(title, "New task");
        // Should be a static str — called multiple times returns same value
        assert_eq!(default_task_title(), default_task_title());
    }

    // =========================================================================
    // compute_ordinal_for_drop tests
    // =========================================================================

    fn make_ordinal_task(id: &str, ordinal: &Ordinal) -> Entity {
        let mut e = Entity::new("task", id);
        e.set("position_ordinal", json!(ordinal.as_str()));
        e.set("position_column", json!("todo"));
        e
    }

    #[test]
    fn test_compute_ordinal_empty_list() {
        let ordinal = compute_ordinal_for_drop(&[], 0);
        // Should return Ordinal::first() — compare against a freshly generated first
        assert_eq!(ordinal.as_str(), Ordinal::first().as_str());
    }

    #[test]
    fn test_compute_ordinal_append_at_end() {
        let ord0 = Ordinal::first();
        let ord1 = Ordinal::after(&ord0);
        let tasks = vec![
            make_ordinal_task("t1", &ord0),
            make_ordinal_task("t2", &ord1),
        ];
        let ordinal = compute_ordinal_for_drop(&tasks, 2);
        assert!(
            ordinal > ord1,
            "appended ordinal '{}' should be after last '{}'",
            ordinal.as_str(),
            ord1.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_insert_at_beginning() {
        // Start from a couple steps above the minimum so there's room to prepend
        let base = Ordinal::after(&Ordinal::after(&Ordinal::after(&Ordinal::first())));
        let ord0 = base;
        let ord1 = Ordinal::after(&ord0);
        let tasks = vec![
            make_ordinal_task("t1", &ord0),
            make_ordinal_task("t2", &ord1),
        ];
        let ordinal = compute_ordinal_for_drop(&tasks, 0);
        assert!(
            ordinal < ord0,
            "prepended ordinal '{}' should be before first '{}'",
            ordinal.as_str(),
            ord0.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_insert_between() {
        let ord0 = Ordinal::first();
        let ord1 = Ordinal::after(&ord0);
        let ord2 = Ordinal::after(&ord1);
        let tasks = vec![
            make_ordinal_task("t1", &ord0),
            make_ordinal_task("t2", &ord2),
        ];
        let ordinal = compute_ordinal_for_drop(&tasks, 1);
        assert!(
            ordinal > ord0,
            "between ordinal '{}' should be after first '{}'",
            ordinal.as_str(),
            ord0.as_str()
        );
        assert!(
            ordinal < ord2,
            "between ordinal '{}' should be before second '{}'",
            ordinal.as_str(),
            ord2.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_single_element_append() {
        let ord0 = Ordinal::first();
        let tasks = vec![make_ordinal_task("t1", &ord0)];
        let ordinal = compute_ordinal_for_drop(&tasks, 1);
        assert!(
            ordinal > ord0,
            "'{}' should be > '{}'",
            ordinal.as_str(),
            ord0.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_single_element_prepend() {
        let ord0 = Ordinal::after(&Ordinal::after(&Ordinal::first()));
        let tasks = vec![make_ordinal_task("t1", &ord0)];
        let ordinal = compute_ordinal_for_drop(&tasks, 0);
        assert!(
            ordinal < ord0,
            "'{}' should be < '{}'",
            ordinal.as_str(),
            ord0.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_prepend_before_default_first() {
        // When the first task has Ordinal::first() (the default/minimum),
        // prepending at index 0 must still produce a strictly smaller ordinal.
        // This is the scenario that triggers the synthetic lower bound path.
        let ord0 = Ordinal::first();
        let tasks = vec![make_ordinal_task("t1", &ord0)];
        let ordinal = compute_ordinal_for_drop(&tasks, 0);
        assert!(
            ordinal < ord0,
            "prepend before default first: '{}' should be < '{}' but isn't",
            ordinal.as_str(),
            ord0.as_str()
        );
        assert_ne!(
            ordinal.as_str(),
            ord0.as_str(),
            "prepend must produce a distinct ordinal, not a duplicate"
        );
    }

    #[test]
    fn test_compute_ordinal_prepend_before_legacy_a0_ordinal() {
        // A task with legacy ordinal "a0" (invalid FractionalIndex) should
        // still allow prepending. The ordinal must be strictly less than "a0"
        // in string comparison (which is how the column is sorted).
        let mut task = Entity::new("task", "t1");
        task.set("position_ordinal", serde_json::json!("a0"));
        task.set("position_column", serde_json::json!("todo"));
        let tasks = vec![task];
        let ordinal = compute_ordinal_for_drop(&tasks, 0);
        // The ordinal must sort before "a0" in the same comparison used to
        // sort the column (string comparison on raw ordinal values).
        assert!(
            ordinal.as_str() < "a0",
            "prepend before legacy 'a0': ordinal '{}' should string-compare < 'a0'",
            ordinal.as_str(),
        );
    }

    #[test]
    fn test_sort_fallback_matches_ordinal_parse_fallback() {
        // The raw-string sort fallback (Ordinal::DEFAULT_STR) and the parsed
        // fallback (Ordinal::from_string on invalid input) must produce the
        // same string, so sort order and ordinal computation agree.
        let parsed_fallback = Ordinal::from_string("invalid-legacy-value");
        assert_eq!(
            Ordinal::DEFAULT_STR,
            parsed_fallback.as_str(),
            "DEFAULT_STR '{}' must equal from_string fallback '{}'",
            Ordinal::DEFAULT_STR,
            parsed_fallback.as_str()
        );

        // Verify that a valid ordinal sorts consistently whether compared
        // as raw strings or as parsed Ordinals.
        let valid_str = "8180"; // Ordinal::after(first)
        let valid_ord = Ordinal::from_string(valid_str);
        let default_ord = Ordinal::from_string(Ordinal::DEFAULT_STR);

        let raw_order = Ordinal::DEFAULT_STR.cmp(valid_str);
        let parsed_order = default_ord.cmp(&valid_ord);
        assert_eq!(
            raw_order,
            parsed_order,
            "raw sort ({:?}) must match parsed sort ({:?}) for '{}' vs '{}'",
            raw_order,
            parsed_order,
            Ordinal::DEFAULT_STR,
            valid_str
        );
    }

    #[test]
    fn test_compute_ordinal_drop_index_beyond_end() {
        let ord0 = Ordinal::first();
        let tasks = vec![make_ordinal_task("t1", &ord0)];
        // drop_index way beyond list length — should still append
        let ordinal = compute_ordinal_for_drop(&tasks, 100);
        assert!(
            ordinal > ord0,
            "'{}' should be > '{}'",
            ordinal.as_str(),
            ord0.as_str()
        );
    }

    // =========================================================================
    // compute_ordinal_for_neighbors tests
    // =========================================================================

    #[test]
    fn test_compute_ordinal_for_neighbors_both_none() {
        let ordinal = compute_ordinal_for_neighbors(None, None);
        // Should return Ordinal::first()
        assert_eq!(ordinal.as_str(), Ordinal::first().as_str());
    }

    #[test]
    fn test_compute_ordinal_for_neighbors_only_after() {
        // Use a valid ordinal a couple steps after first
        let after = Ordinal::after(&Ordinal::after(&Ordinal::first()));
        let ordinal = compute_ordinal_for_neighbors(None, Some(&after));
        assert!(
            ordinal < after,
            "should be before after ordinal '{}', got '{}'",
            after.as_str(),
            ordinal.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_for_neighbors_only_before() {
        let before = Ordinal::first();
        let ordinal = compute_ordinal_for_neighbors(Some(&before), None);
        assert!(
            ordinal > before,
            "should be after before ordinal '{}', got '{}'",
            before.as_str(),
            ordinal.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_for_neighbors_between() {
        let before = Ordinal::first();
        let after = Ordinal::after(&Ordinal::after(&before));
        let ordinal = compute_ordinal_for_neighbors(Some(&before), Some(&after));
        assert!(
            ordinal > before,
            "should be after before '{}', got '{}'",
            before.as_str(),
            ordinal.as_str()
        );
        assert!(
            ordinal < after,
            "should be before after '{}', got '{}'",
            after.as_str(),
            ordinal.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_for_neighbors_tight_range() {
        // Even with adjacent ordinals, should produce a valid in-between value
        let before = Ordinal::first();
        let after = Ordinal::after(&before);
        let ordinal = compute_ordinal_for_neighbors(Some(&before), Some(&after));
        assert!(
            ordinal > before,
            "between: '{}' should be > '{}'",
            ordinal.as_str(),
            before.as_str()
        );
        assert!(
            ordinal < after,
            "between: '{}' should be < '{}'",
            ordinal.as_str(),
            after.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_prepend_before_first() {
        // Inserting before the first task
        let after = Ordinal::first();
        let ordinal = compute_ordinal_for_neighbors(None, Some(&after));
        assert!(
            ordinal < after,
            "prepend ordinal '{}' should be < '{}'",
            ordinal.as_str(),
            after.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_prepend_before_third() {
        // Inserting before a task that is a few steps in
        let after = Ordinal::after(&Ordinal::after(&Ordinal::after(&Ordinal::first())));
        let ordinal = compute_ordinal_for_neighbors(None, Some(&after));
        assert!(
            ordinal < after,
            "prepend ordinal '{}' should be < '{}'",
            ordinal.as_str(),
            after.as_str()
        );
    }

    #[test]
    fn test_compute_ordinal_sequence_maintains_order() {
        // Simulate: 3 tasks. Move last to first position.
        let ord0 = Ordinal::first();
        let ord1 = Ordinal::after(&ord0);
        let ord2 = Ordinal::after(&ord1);

        // Insert before ord0
        let new_first = compute_ordinal_for_neighbors(None, Some(&ord0));
        assert!(
            new_first < ord0,
            "'{}' should be < '{}'",
            new_first.as_str(),
            ord0.as_str()
        );

        // Insert between ord0 and ord1
        let mid = compute_ordinal_for_neighbors(Some(&ord0), Some(&ord1));
        assert!(
            mid > ord0,
            "'{}' should be > '{}'",
            mid.as_str(),
            ord0.as_str()
        );
        assert!(
            mid < ord1,
            "'{}' should be < '{}'",
            mid.as_str(),
            ord1.as_str()
        );

        // Append after ord2
        let last = compute_ordinal_for_neighbors(Some(&ord2), None);
        assert!(
            last > ord2,
            "'{}' should be > '{}'",
            last.as_str(),
            ord2.as_str()
        );
    }
}
