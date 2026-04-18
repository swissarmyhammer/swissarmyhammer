//! Task-specific logic for Entity-based tasks.
//!
//! These free functions work with `Entity` and raw field values, providing
//! computed fields (progress, readiness, dependency graph) and JSON serialization
//! that matches the API contract expected by the frontend.
//!
//! Tags and progress are populated by `ComputeEngine` during `EntityContext::read()`.
//! The functions here simply read those pre-computed fields.

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use swissarmyhammer_entity::{Entity, EntityFilterContext};

use crate::types::Ordinal;
use crate::virtual_tags::{TerminalColumnId, VirtualTagRegistry};

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
///
/// A missing dependency (ID not found in `all_tasks`) is treated as incomplete,
/// making the task not ready. This matches the semantics in [`ReadyStrategy`].
pub fn task_is_ready(entity: &Entity, all_tasks: &[Entity], terminal_column_id: &str) -> bool {
    let deps = entity.get_string_list("depends_on");
    deps.iter().all(|dep_id| {
        all_tasks
            .iter()
            .find(|t| t.id == *dep_id)
            .map(|t| t.get_str("position_column") == Some(terminal_column_id))
            .unwrap_or(false) // Missing dependency = not ready (safer default)
    })
}

/// Get task IDs that this task is blocked by (incomplete dependencies).
///
/// A missing dependency (ID not found in `all_tasks`) is included in the
/// result, treating it as blocking. This matches the semantics in
/// [`BlockedStrategy`].
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
                .unwrap_or(true) // Missing dependency = blocked (safer default)
        })
        .collect()
}

/// Get task IDs that depend on this task.
///
/// Thin wrapper around [`find_dependent_task_ids`] that accepts an [`Entity`]
/// for the common case where the caller already holds one. For cases where
/// only a bare task id is available (e.g. fan-out after a delete when the
/// entity is no longer in `all_tasks`), call [`find_dependent_task_ids`]
/// directly.
pub fn task_blocks(entity: &Entity, all_tasks: &[Entity]) -> Vec<String> {
    find_dependent_task_ids(entity.id.as_ref(), all_tasks)
}

/// Find the task IDs whose `depends_on` list currently contains `task_id`.
///
/// This is the reverse-dependency lookup: given a task ID, return every task
/// that names it in its `depends_on` list. Used by the post-mutation
/// enrichment fan-out pass to find tasks whose computed BLOCKED/READY state
/// may need re-derivation after `task_id` changed column or had its
/// dependencies edited.
///
/// Unlike [`task_blocks`], the input here is a bare `&str` — the target
/// entity may not be present in `all_tasks` (e.g. when the trigger was just
/// deleted), so the function must not require a full `Entity` to look up.
///
/// Returns owned `String`s rather than borrowed references because callers
/// typically store the result across scopes (cache updates, fan-out target
/// sets) where borrowing the slice would fight the borrow checker.
pub fn find_dependent_task_ids(task_id: &str, all_tasks: &[Entity]) -> Vec<String> {
    all_tasks
        .iter()
        .filter(|t| t.get_string_list("depends_on").iter().any(|d| d == task_id))
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
pub fn enrich_task_entity(
    entity: &mut Entity,
    all_tasks: &[Entity],
    terminal_column_id: &str,
    registry: &VirtualTagRegistry,
) {
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

    // virtual tags: evaluate strategies against this entity
    let mut vtag_ctx = EntityFilterContext::for_entity(entity, all_tasks);
    vtag_ctx.insert(TerminalColumnId(terminal_column_id.to_string()));
    let virtual_slugs = registry.evaluate(&vtag_ctx);
    entity.set("virtual_tags", json!(virtual_slugs));

    // filter_tags: union of body-parsed tags + virtual tags (for filtering)
    let body_tags = entity.get_string_list("tags");
    let mut union: Vec<String> = body_tags;
    let existing: HashSet<String> = union.iter().cloned().collect();
    for slug in &virtual_slugs {
        if !existing.contains(slug) {
            union.push(slug.clone());
        }
    }
    entity.set("filter_tags", json!(union));
}

/// Pre-built indexes for O(1) per-task dependency lookups during batch enrichment.
struct DependencyIndexes {
    /// dep_id -> list of task_ids that depend on it (i.e. "blocks")
    blocks: HashMap<String, Vec<String>>,
    /// task_id -> list of dep_ids it depends on
    depends_on: HashMap<String, Vec<String>>,
    /// task_id -> position_column value
    positions: HashMap<String, String>,
    /// Lightweight entity stubs for virtual tag evaluation.
    ///
    /// Strategies need an immutable `&[Entity]` while we mutate each entity in
    /// the enrichment loop, but they only read `id`, `position_column`, and
    /// `depends_on` from the slice. Stubs contain only those fields, avoiding
    /// full clones of large descriptions.
    stubs: Vec<Entity>,
}

/// Build dependency, position, and stub indexes from a task entity slice.
///
/// Returns indexes that enable O(1) per-task lookups for blocks, depends_on,
/// position columns, and virtual tag evaluation during batch enrichment.
fn build_dependency_indexes(entities: &[Entity]) -> DependencyIndexes {
    let mut blocks: HashMap<String, Vec<String>> = HashMap::new();
    let mut depends_on: HashMap<String, Vec<String>> = HashMap::new();

    for entity in entities.iter() {
        let deps = entity.get_string_list("depends_on");
        for dep_id in &deps {
            blocks
                .entry(dep_id.clone())
                .or_default()
                .push(entity.id.to_string());
        }
        depends_on.insert(entity.id.to_string(), deps);
    }

    let positions: HashMap<String, String> = entities
        .iter()
        .map(|e| {
            (
                e.id.to_string(),
                e.get_str("position_column").unwrap_or("").to_string(),
            )
        })
        .collect();

    let stubs: Vec<Entity> = entities
        .iter()
        .map(|e| {
            let mut stub = Entity::new(e.entity_type.clone(), e.id.clone());
            if let Some(col) = e.get("position_column") {
                stub.set("position_column", col.clone());
            }
            if let Some(deps) = e.get("depends_on") {
                stub.set("depends_on", deps.clone());
            }
            stub
        })
        .collect();

    DependencyIndexes {
        blocks,
        depends_on,
        positions,
        stubs,
    }
}

/// Enrich a single task entity using pre-built indexes.
///
/// Sets progress_fraction, ready, blocked_by, blocks, virtual_tags, and
/// filter_tags fields. Uses the indexes for O(1) dependency lookups instead
/// of scanning the full task list.
fn enrich_task_from_indexes(
    entity: &mut Entity,
    indexes: &DependencyIndexes,
    terminal_column_id: &str,
    registry: &VirtualTagRegistry,
) {
    entity.set("progress_fraction", json!(task_progress(entity)));

    // Ready: all deps in terminal column
    let deps = indexes
        .depends_on
        .get(&entity.id.to_string())
        .cloned()
        .unwrap_or_default();
    let blocked_by: Vec<String> = deps
        .iter()
        .filter(|dep_id| {
            indexes
                .positions
                .get(*dep_id)
                .map(|col| col != terminal_column_id)
                .unwrap_or(true) // Missing dependency = blocked (safer default)
        })
        .cloned()
        .collect();
    entity.set("ready", json!(blocked_by.is_empty()));
    entity.set("blocked_by", json!(blocked_by));

    // Blocks: tasks that depend on this one
    let blocks = indexes
        .blocks
        .get(&entity.id.to_string())
        .cloned()
        .unwrap_or_default();
    entity.set("blocks", json!(blocks));

    // Virtual tags: evaluate strategies against lightweight stubs
    let mut vtag_ctx = EntityFilterContext::for_entity(entity, &indexes.stubs);
    vtag_ctx.insert(TerminalColumnId(terminal_column_id.to_string()));
    let virtual_slugs = registry.evaluate(&vtag_ctx);
    entity.set("virtual_tags", json!(virtual_slugs));

    // filter_tags: union of body-parsed tags + virtual tags
    let body_tags = entity.get_string_list("tags");
    let mut union: Vec<String> = body_tags;
    let existing: HashSet<String> = union.iter().cloned().collect();
    for slug in &virtual_slugs {
        if !existing.contains(slug) {
            union.push(slug.clone());
        }
    }
    entity.set("filter_tags", json!(union));
}

/// Enrich all task entities in a single O(N) pass using pre-built indexes.
///
/// This is the batch alternative to calling `enrich_task_entity` in a loop,
/// which would be O(N^2) because each call scans all tasks for dependency
/// lookups. This function pre-builds `blocks` and `depends_on` indexes so
/// the per-task enrichment is O(1).
pub fn enrich_all_task_entities(
    entities: &mut [Entity],
    terminal_column_id: &str,
    registry: &VirtualTagRegistry,
) {
    let indexes = build_dependency_indexes(entities);
    for entity in entities.iter_mut() {
        enrich_task_from_indexes(entity, &indexes, terminal_column_id, registry);
    }
}

/// Names of the task date fields that are surfaced in JSON output.
///
/// User-set dates (`due`, `scheduled`) come from stored entity fields; system
/// dates (`created`, `updated`, `started`, `completed`) come from the
/// computed-field derivation pipeline during `EntityContext::read()`. Each
/// slot is emitted as either the stored/derived value or `null` when unset,
/// so API consumers can rely on a stable field shape.
const TASK_DATE_FIELDS: &[&str] = &[
    "due",
    "scheduled",
    "created",
    "updated",
    "started",
    "completed",
];

/// Copy each date field from `entity` into `result`, defaulting to null.
///
/// Ensures the output JSON always contains every known date key so consumers
/// can distinguish "unset" (explicit null) from "missing because of an older
/// schema" — the latter never happens for freshly-read entities.
fn include_date_fields(entity: &Entity, result: &mut Value) {
    for name in TASK_DATE_FIELDS {
        result[*name] = entity.get(name).cloned().unwrap_or(Value::Null);
    }
}

/// Convert a task Entity to the JSON format expected by the API/frontend.
///
/// Transforms flat entity fields into the nested format:
/// - "body" → "description" (rename for backward compat)
/// - position_column/position_ordinal → nested "position" object
/// - Adds computed fields: tags, progress
/// - Emits user-set dates (`due`, `scheduled`) and system dates (`created`,
///   `updated`, `started`, `completed`). Each is the stored/derived value or
///   `null` when unset.
pub fn task_entity_to_json(entity: &Entity) -> Value {
    let tags = task_tags(entity);
    let progress = task_progress(entity);

    let position_column = entity.get_str("position_column").unwrap_or("");
    let position_ordinal = entity
        .get_str("position_ordinal")
        .unwrap_or(Ordinal::DEFAULT_STR);

    let position = json!({
        "column": position_column,
        "ordinal": position_ordinal,
    });

    let mut result = json!({
        "id": entity.id,
        "title": entity.get_str("title").unwrap_or(""),
        "description": entity.get_str("body").unwrap_or(""),
        "position": position,
        "tags": tags,
        "assignees": entity.get_string_list("assignees"),
        "depends_on": entity.get_string_list("depends_on"),
        "project": entity.get_str("project").unwrap_or(""),
        "progress": progress,
    });

    // Include attachments if present
    if let Some(attachments) = entity.get("attachments") {
        result["attachments"] = attachments.clone();
    } else {
        result["attachments"] = json!([]);
    }

    // Include project reference (null when unset)
    if let Some(project) = entity.get("project") {
        result["project"] = project.clone();
    } else {
        result["project"] = Value::Null;
    }

    include_date_fields(entity, &mut result);

    result
}

/// Convert a pre-enriched task Entity to JSON with computed fields.
///
/// Reads `ready`, `blocked_by`, `blocks`, `virtual_tags`, and `filter_tags`
/// from fields already set by `enrich_task_entity` / `enrich_all_task_entities`.
/// Callers must enrich the entity before calling this function.
pub fn task_entity_to_rich_json(entity: &Entity) -> Value {
    let mut result = task_entity_to_json(entity);

    result["ready"] = entity.get("ready").cloned().unwrap_or(json!(true));
    result["blocked_by"] = json!(entity.get_string_list("blocked_by"));
    result["blocks"] = json!(entity.get_string_list("blocks"));
    result["virtual_tags"] = json!(entity.get_string_list("virtual_tags"));
    result["filter_tags"] = json!(entity.get_string_list("filter_tags"));

    result
}

/// Lookup table for resolving entity ids from display-name slugs.
///
/// The filter DSL lets users write `$project-slug`, `@user-slug`, or
/// `^task-slug`. Those literal strings may be an entity's stored id OR the
/// canonical slug of its display name — the frontend autocomplete offers the
/// name-slug, while older hand-written filters still use the id. To match
/// both, the adapter needs to resolve a filter value against a name-slug
/// index.
///
/// This struct pre-computes that index once per filter pass. The `HashMap`s
/// are keyed by the canonical slug of the entity's display-name field
/// (`name` for projects/actors, `title` for tasks) and yield the entity id.
/// Lookup is O(1); construction is O(N) over the respective entity lists.
///
/// The adapter holds a reference to this registry so we amortize the slug
/// computation across every task being filtered instead of rebuilding per
/// task.
#[derive(Debug, Default)]
pub struct EntitySlugRegistry {
    /// Slug of each project's `name` → project id.
    project_slug_to_id: HashMap<String, String>,
    /// Slug of each actor's `name` → actor id.
    actor_slug_to_id: HashMap<String, String>,
    /// Slug of each task's `title` → task id.
    task_slug_to_id: HashMap<String, String>,
}

impl EntitySlugRegistry {
    /// Build a slug registry from the full actor/project/task entity lists.
    ///
    /// Each entity contributes a `(slug(display_name), id)` entry; empty
    /// slugs (entities whose name contains no ASCII alphanumerics) are
    /// skipped. If two entities share a slug the later one wins — this
    /// is a display-name ambiguity the user already has to resolve in
    /// the UI, and picking either is better than refusing to match.
    pub fn build(projects: &[Entity], actors: &[Entity], tasks: &[Entity]) -> Self {
        let project_slug_to_id = build_slug_index(projects, "name");
        let actor_slug_to_id = build_slug_index(actors, "name");
        let task_slug_to_id = build_slug_index(tasks, "title");
        Self {
            project_slug_to_id,
            actor_slug_to_id,
            task_slug_to_id,
        }
    }

    /// Empty registry — equivalent to having no projects/actors/tasks loaded.
    /// Mainly useful for unit tests and the degenerate case where the caller
    /// doesn't have access to the registries.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Look up the project id whose `name` slugifies to `value`.
    ///
    /// Returns `None` when no project's name-slug matches. Matching is done
    /// against the pre-computed index; callers must pass the raw filter
    /// value — slugification of `value` itself is NOT applied, because
    /// frontend autocomplete already feeds canonical slugs in. Case is
    /// normalized to ASCII lowercase before lookup to match slug semantics.
    pub fn project_id_for_slug(&self, value: &str) -> Option<&str> {
        self.project_slug_to_id
            .get(&value.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Look up the actor id whose `name` slugifies to `value`.
    pub fn actor_id_for_slug(&self, value: &str) -> Option<&str> {
        self.actor_slug_to_id
            .get(&value.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Look up the task id whose `title` slugifies to `value`.
    pub fn task_id_for_slug(&self, value: &str) -> Option<&str> {
        self.task_slug_to_id
            .get(&value.to_ascii_lowercase())
            .map(String::as_str)
    }
}

/// Build a `slug(entity[field]) → entity.id` map from a slice of entities.
///
/// Skips entities whose display field is missing or slugifies to the empty
/// string — those can't participate in slug-based matching and would
/// otherwise pollute the index with ambiguous empty-string keys.
fn build_slug_index(entities: &[Entity], display_field: &str) -> HashMap<String, String> {
    let mut map = HashMap::with_capacity(entities.len());
    for entity in entities {
        if let Some(name) = entity.get_str(display_field) {
            let s = swissarmyhammer_common::slug(name);
            if !s.is_empty() {
                map.insert(s, entity.id.to_string());
            }
        }
    }
    map
}

/// Adapter that maps filter DSL atoms to enriched task entity fields.
///
/// Uses `filter_tags` (union of body tags + virtual tags) for `#tag` lookups,
/// `assignees` for `@user` lookups, `depends_on` + `id` for `^ref` lookups,
/// and the single-value `project` field for `$project` lookups.
/// Entities must be enriched (via `enrich_task_entity` or `enrich_all_task_entities`)
/// before evaluation — unenriched entities won't have `filter_tags`.
///
/// `$project`, `@actor`, and `^task` predicates match on the stored id OR
/// on the slug of the referenced entity's display name. The latter requires
/// a `registry` populated from the actor/project/task lists; passing the
/// empty registry falls back to id-only matching (backwards compatible).
pub struct TaskFilterAdapter<'a> {
    /// The enriched task entity to evaluate against.
    pub entity: &'a Entity,
    /// Optional slug-to-id registry for resolving `$project` / `@user` /
    /// `^task` values against display-name slugs. When omitted, only the
    /// stored id comparison is performed.
    pub registry: Option<&'a EntitySlugRegistry>,
}

impl<'a> TaskFilterAdapter<'a> {
    /// Construct an adapter with id-only matching semantics.
    ///
    /// Equivalent to `TaskFilterAdapter { entity, registry: None }`, but
    /// reads more clearly at call sites that don't have a registry handy.
    pub fn new(entity: &'a Entity) -> Self {
        Self {
            entity,
            registry: None,
        }
    }

    /// Construct an adapter that resolves id-or-slug against `registry`.
    pub fn with_registry(entity: &'a Entity, registry: &'a EntitySlugRegistry) -> Self {
        Self {
            entity,
            registry: Some(registry),
        }
    }
}

impl<'a> swissarmyhammer_filter_expr::FilterContext for TaskFilterAdapter<'a> {
    fn has_tag(&self, tag: &str) -> bool {
        self.entity
            .get_string_list("filter_tags")
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tag))
    }

    fn has_assignee(&self, user: &str) -> bool {
        // Direct id match on the task's assignees list.
        let assignees = self.entity.get_string_list("assignees");
        if assignees.iter().any(|a| a.eq_ignore_ascii_case(user)) {
            return true;
        }
        // Fall back to slug-of-name: resolve `user` through the actor
        // registry and check if any assignee id matches the resolved id.
        if let Some(registry) = self.registry {
            if let Some(resolved_id) = registry.actor_id_for_slug(user) {
                if assignees
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(resolved_id))
                {
                    return true;
                }
            }
        }
        false
    }

    fn has_ref(&self, id: &str) -> bool {
        // Direct id match on the task's own id or depends_on list.
        if self.entity.id.as_ref() == id {
            return true;
        }
        let depends_on = self.entity.get_string_list("depends_on");
        if depends_on.iter().any(|r| r == id) {
            return true;
        }
        // Fall back to slug-of-title: resolve `id` to a task id via the
        // registry and compare against both the entity's own id and its
        // depends_on list.
        if let Some(registry) = self.registry {
            if let Some(resolved_id) = registry.task_id_for_slug(id) {
                if self.entity.id.as_ref() == resolved_id {
                    return true;
                }
                if depends_on.iter().any(|r| r == resolved_id) {
                    return true;
                }
            }
        }
        false
    }

    fn has_project(&self, project: &str) -> bool {
        let Some(stored) = self.entity.get_str("project") else {
            return false;
        };
        // Direct id match.
        if stored.eq_ignore_ascii_case(project) {
            return true;
        }
        // Fall back to slug-of-name: resolve `project` to a project id
        // via the registry and compare against the stored value.
        if let Some(registry) = self.registry {
            if let Some(resolved_id) = registry.project_id_for_slug(project) {
                if stored.eq_ignore_ascii_case(resolved_id) {
                    return true;
                }
            }
        }
        false
    }
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
    fn test_task_is_ready_missing_dep_is_not_ready() {
        // Dependency "ghost" doesn't exist in all_tasks — should NOT be ready.
        // This must match ReadyStrategy::matches semantics (unwrap_or(false)).
        let mut e = make_task("t1", "Test", "", "todo");
        e.set("depends_on", json!(["ghost"]));
        assert!(!task_is_ready(&e, &[], "done"));
    }

    #[test]
    fn test_task_blocked_by_missing_dep_is_blocking() {
        // Dependency "ghost" doesn't exist in all_tasks — should appear as blocker.
        // This must match BlockedStrategy::matches semantics (unwrap_or(true)).
        let mut e = make_task("t1", "Test", "", "todo");
        e.set("depends_on", json!(["ghost"]));
        let blocked = task_blocked_by(&e, &[], "done");
        assert_eq!(blocked, vec!["ghost"]);
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
    fn test_find_dependent_task_ids_returns_reverse_dependents() {
        // Two tasks depend on t1; one does not.
        let t1 = make_task("t1", "Target", "", "todo");
        let mut t2 = make_task("t2", "Depends on t1", "", "todo");
        t2.set("depends_on", json!(["t1"]));
        let mut t3 = make_task("t3", "Also depends on t1", "", "todo");
        t3.set("depends_on", json!(["t1", "other"]));
        let t4 = make_task("t4", "Unrelated", "", "todo");

        let mut deps = find_dependent_task_ids("t1", &[t1, t2, t3, t4]);
        deps.sort();
        assert_eq!(deps, vec!["t2".to_string(), "t3".to_string()]);
    }

    #[test]
    fn test_find_dependent_task_ids_returns_empty_when_no_reverse_deps() {
        let t1 = make_task("t1", "Target", "", "todo");
        let t2 = make_task("t2", "Unrelated", "", "todo");
        let deps = find_dependent_task_ids("t1", &[t1, t2]);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_find_dependent_task_ids_works_when_target_entity_is_absent() {
        // Target task ID is not in all_tasks — should still find reverse
        // dependents (callers use this when the trigger may have been
        // deleted concurrently).
        let mut dependent = make_task("b", "Dependent", "", "todo");
        dependent.set("depends_on", json!(["ghost"]));
        let deps = find_dependent_task_ids("ghost", &[dependent]);
        assert_eq!(deps, vec!["b".to_string()]);
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
        let ordinal = Ordinal::after(&Ordinal::first());
        let ordinal_str = ordinal.as_str().to_string();
        e.set("position_ordinal", json!(ordinal_str));
        e.set("assignees", json!(["alice"]));

        let result = task_entity_to_json(&e);
        assert_eq!(result["id"], "t1");
        assert_eq!(result["title"], "Test Task");
        assert_eq!(result["description"], "Some #bug description");
        assert_eq!(result["position"]["column"], "todo");
        assert_eq!(result["position"]["ordinal"], ordinal_str);
        assert!(result["tags"].as_array().unwrap().contains(&json!("bug")));
        assert_eq!(result["assignees"], json!(["alice"]));
    }

    #[test]
    fn test_task_entity_to_rich_json() {
        let dep = make_task("dep1", "Dep", "", "todo");
        let mut e = make_task("t1", "Test", "", "todo");
        e.set("depends_on", json!(["dep1"]));

        let all = vec![dep, e.clone()];
        let registry = VirtualTagRegistry::new();
        enrich_task_entity(&mut e, &all, "done", &registry);

        let result = task_entity_to_rich_json(&e);
        assert_eq!(result["ready"], false);
        assert_eq!(result["blocked_by"], json!(["dep1"]));
        assert_eq!(result["virtual_tags"], json!([]));
        assert_eq!(result["filter_tags"], json!([]));
    }

    #[test]
    fn test_enrich_task_entity_injects_computed_fields() {
        let dep = make_task("dep1", "Dep", "", "todo");
        let mut e = make_task_computed("t1", "Test", "- [ ] a\n- [x] b", "todo", vec![], 2, 1);
        e.set("depends_on", json!(["dep1"]));

        let all = vec![dep, e.clone()];
        let registry = VirtualTagRegistry::new();
        enrich_task_entity(&mut e, &all, "done", &registry);

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
        let registry = VirtualTagRegistry::new();
        enrich_task_entity(&mut e, &all, "done", &registry);

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
        let registry = VirtualTagRegistry::new();
        enrich_all_task_entities(&mut entities, "done", &registry);

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
        let registry = VirtualTagRegistry::new();
        enrich_all_task_entities(&mut entities, "done", &registry);

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

    // =========================================================================
    // Virtual tag enrichment tests
    // =========================================================================

    use crate::virtual_tags::{VirtualTagCommand, VirtualTagStrategy};
    use swissarmyhammer_entity::EntityFilterContext;

    /// Mock strategy that always matches — used to test virtual tag injection.
    struct AlwaysVirtualTag;

    impl crate::virtual_tags::sealed::Sealed for AlwaysVirtualTag {}

    impl VirtualTagStrategy for AlwaysVirtualTag {
        fn slug(&self) -> &str {
            "ALWAYS"
        }
        fn color(&self) -> &str {
            "22c55e"
        }
        fn description(&self) -> &str {
            "Always applies"
        }
        fn commands(&self) -> Vec<VirtualTagCommand> {
            vec![]
        }
        fn matches(&self, _ctx: &EntityFilterContext) -> bool {
            true
        }
    }

    /// Mock strategy that never matches.
    struct NeverVirtualTag;

    impl crate::virtual_tags::sealed::Sealed for NeverVirtualTag {}

    impl VirtualTagStrategy for NeverVirtualTag {
        fn slug(&self) -> &str {
            "NEVER"
        }
        fn color(&self) -> &str {
            "ef4444"
        }
        fn description(&self) -> &str {
            "Never applies"
        }
        fn commands(&self) -> Vec<VirtualTagCommand> {
            vec![]
        }
        fn matches(&self, _ctx: &EntityFilterContext) -> bool {
            false
        }
    }

    #[test]
    fn test_enrich_sets_virtual_tags_field() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(AlwaysVirtualTag));
        registry.register(Box::new(NeverVirtualTag));

        let mut e = make_task("t1", "Test", "", "todo");
        let all = vec![e.clone()];
        enrich_task_entity(&mut e, &all, "done", &registry);

        let virtual_tags = e.get_string_list("virtual_tags");
        assert_eq!(virtual_tags, vec!["ALWAYS"]);
    }

    #[test]
    fn test_enrich_sets_filter_tags_as_union() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(AlwaysVirtualTag));

        let mut e = make_task_computed("t1", "Test", "#bug some text", "todo", vec!["bug"], 0, 0);
        let all = vec![e.clone()];
        enrich_task_entity(&mut e, &all, "done", &registry);

        let filter_tags = e.get_string_list("filter_tags");
        assert!(filter_tags.contains(&"bug".to_string()));
        assert!(filter_tags.contains(&"ALWAYS".to_string()));
        assert_eq!(filter_tags.len(), 2);
    }

    #[test]
    fn test_enrich_does_not_modify_tags_field() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(AlwaysVirtualTag));

        let mut e = make_task_computed("t1", "Test", "#bug text", "todo", vec!["bug"], 0, 0);
        let all = vec![e.clone()];
        enrich_task_entity(&mut e, &all, "done", &registry);

        // tags field should still only contain body-parsed tags
        let tags = e.get_string_list("tags");
        assert_eq!(tags, vec!["bug"]);
        assert!(!tags.contains(&"ALWAYS".to_string()));
    }

    #[test]
    fn test_enrich_all_sets_virtual_tags_and_filter_tags() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(AlwaysVirtualTag));

        let e1 = make_task_computed("t1", "Test1", "#bug text", "todo", vec!["bug"], 0, 0);
        let e2 = make_task("t2", "Test2", "", "todo");

        let mut entities = vec![e1, e2];
        enrich_all_task_entities(&mut entities, "done", &registry);

        // t1: has body tag "bug" + virtual tag "ALWAYS"
        let t1 = &entities[0];
        assert_eq!(t1.get_string_list("virtual_tags"), vec!["ALWAYS"]);
        let filter = t1.get_string_list("filter_tags");
        assert!(filter.contains(&"bug".to_string()));
        assert!(filter.contains(&"ALWAYS".to_string()));
        assert_eq!(t1.get_string_list("tags"), vec!["bug"]);

        // t2: no body tags, just virtual tag "ALWAYS"
        let t2 = &entities[1];
        assert_eq!(t2.get_string_list("virtual_tags"), vec!["ALWAYS"]);
        assert_eq!(t2.get_string_list("filter_tags"), vec!["ALWAYS"]);
    }

    #[test]
    fn test_enrich_filter_tags_deduplicates() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(AlwaysVirtualTag));

        // Body already has tag "ALWAYS" — should not appear twice in filter_tags
        let mut e = make_task_computed("t1", "Test", "#ALWAYS text", "todo", vec!["ALWAYS"], 0, 0);
        let all = vec![e.clone()];
        enrich_task_entity(&mut e, &all, "done", &registry);

        let filter_tags = e.get_string_list("filter_tags");
        assert_eq!(filter_tags, vec!["ALWAYS"]);
    }

    #[test]
    fn test_enrich_empty_registry_no_virtual_tags() {
        let registry = VirtualTagRegistry::new();

        let mut e = make_task_computed("t1", "Test", "#bug text", "todo", vec!["bug"], 0, 0);
        let all = vec![e.clone()];
        enrich_task_entity(&mut e, &all, "done", &registry);

        assert!(e.get_string_list("virtual_tags").is_empty());
        assert_eq!(e.get_string_list("filter_tags"), vec!["bug"]);
    }

    #[test]
    fn test_task_filter_adapter_has_project_matches_free_form_id() {
        // Regression guard: TaskFilterAdapter::has_project must match a
        // task whose stored `project` field is a free-form text id (e.g.
        // `AUTH-Migration`). The frontend produces filter text from the
        // project's raw id via the `mention_slug_field` schema signal, so
        // the backend compare must also work against that raw id. This
        // test pins the case-insensitive semantics the frontend relies on
        // so an accidental refactor of the compare cannot silently break
        // the unified-on-id mention behavior.
        use swissarmyhammer_filter_expr::FilterContext;

        let mut e = make_task("t1", "Test", "", "todo");
        e.set("project", json!("AUTH-Migration"));
        let adapter = TaskFilterAdapter::new(&e);

        // Exact match.
        assert!(adapter.has_project("AUTH-Migration"));
        // Case-insensitive match (matches has_project() impl).
        assert!(adapter.has_project("auth-migration"));
        assert!(adapter.has_project("AUTH-MIGRATION"));
        // Non-match.
        assert!(!adapter.has_project("frontend"));
    }

    // ─────────────────────────────────────────────────────────────────
    // EntitySlugRegistry + TaskFilterAdapter id-or-slug matching tests
    //
    // These tests pin the contract laid out in
    // `.kanban/tasks/01KPDWC4F4QPVTJZNN1NQKJAPJ.md`: `$project`, `@user`,
    // and `^task` predicates must match on BOTH the stored entity id AND
    // the slug of the referenced entity's display-name field. The
    // concrete reproducer (project "Task card & field polish" with id
    // `task-card-fields` matching filter `$task-card-field-polish`) is
    // covered by the reproducer test at the bottom of this block.
    // ─────────────────────────────────────────────────────────────────

    fn make_project(id: &str, name: &str) -> Entity {
        let mut e = Entity::new("project", id);
        e.set("name", json!(name));
        e
    }

    fn make_actor(id: &str, name: &str) -> Entity {
        let mut e = Entity::new("actor", id);
        e.set("name", json!(name));
        e
    }

    #[test]
    fn slug_registry_build_maps_name_slug_to_id() {
        let projects = vec![
            make_project("task-card-fields", "Task card & field polish"),
            make_project("auth-migration", "Auth Migration"),
        ];
        let actors = vec![make_actor("claude-code", "Claude Code")];
        let tasks = vec![{
            let mut t = make_task("01TASK", "Fix login bug", "", "todo");
            t.set("title", json!("Fix login bug"));
            t
        }];

        let reg = EntitySlugRegistry::build(&projects, &actors, &tasks);

        assert_eq!(
            reg.project_id_for_slug("task-card-field-polish"),
            Some("task-card-fields")
        );
        assert_eq!(
            reg.project_id_for_slug("auth-migration"),
            Some("auth-migration")
        );
        assert_eq!(reg.actor_id_for_slug("claude-code"), Some("claude-code"));
        assert_eq!(reg.task_id_for_slug("fix-login-bug"), Some("01TASK"));
        assert!(reg.project_id_for_slug("unknown").is_none());
    }

    #[test]
    fn slug_registry_skips_empty_slugs() {
        // A project with a name that slugifies to empty must not clobber
        // other empty-named entries. The registry simply skips them.
        let projects = vec![
            make_project("ok", "Normal"),
            make_project("punctuation", "!!!"),
        ];
        let reg = EntitySlugRegistry::build(&projects, &[], &[]);
        assert_eq!(reg.project_id_for_slug("normal"), Some("ok"));
        assert!(reg.project_id_for_slug("").is_none());
    }

    #[test]
    fn slug_registry_lookup_is_case_insensitive() {
        let projects = vec![make_project("backend", "Backend")];
        let reg = EntitySlugRegistry::build(&projects, &[], &[]);
        // `slug()` already lowercases — callers passing an upper-cased
        // value must still resolve.
        assert_eq!(reg.project_id_for_slug("BACKEND"), Some("backend"));
        assert_eq!(reg.project_id_for_slug("Backend"), Some("backend"));
    }

    #[test]
    fn task_filter_adapter_has_project_matches_slug_of_name() {
        // The task description's concrete reproducer: project with id
        // `task-card-fields` but a name slug of `task-card-field-polish`
        // must match a filter of `$task-card-field-polish`.
        use swissarmyhammer_filter_expr::FilterContext;

        let projects = vec![make_project("task-card-fields", "Task card & field polish")];
        let reg = EntitySlugRegistry::build(&projects, &[], &[]);

        let mut task = make_task("t1", "Test", "", "todo");
        task.set("project", json!("task-card-fields"));
        let adapter = TaskFilterAdapter::with_registry(&task, &reg);

        // Matches by id (current behavior).
        assert!(adapter.has_project("task-card-fields"));
        // Matches by slug of name (new behavior).
        assert!(adapter.has_project("task-card-field-polish"));
        // Does not match unrelated values.
        assert!(!adapter.has_project("other-project"));
    }

    #[test]
    fn task_filter_adapter_has_project_without_registry_falls_back_to_id_only() {
        // Backwards compatibility: callers that don't provide a registry
        // get the old id-only semantics.
        use swissarmyhammer_filter_expr::FilterContext;

        let mut task = make_task("t1", "Test", "", "todo");
        task.set("project", json!("task-card-fields"));
        let adapter = TaskFilterAdapter::new(&task);

        assert!(adapter.has_project("task-card-fields"));
        // Slug-of-name does NOT match because we have no registry.
        assert!(!adapter.has_project("task-card-field-polish"));
    }

    #[test]
    fn task_filter_adapter_has_assignee_matches_slug_of_name() {
        use swissarmyhammer_filter_expr::FilterContext;

        let actors = vec![make_actor("alice", "Alice Smith")];
        let reg = EntitySlugRegistry::build(&[], &actors, &[]);

        let mut task = make_task("t1", "Test", "", "todo");
        task.set("assignees", json!(["alice"]));
        let adapter = TaskFilterAdapter::with_registry(&task, &reg);

        // Matches by id (current behavior).
        assert!(adapter.has_assignee("alice"));
        // Matches by slug of name (new behavior).
        assert!(adapter.has_assignee("alice-smith"));
        // Does not match unrelated values.
        assert!(!adapter.has_assignee("bob"));
    }

    #[test]
    fn task_filter_adapter_has_ref_matches_slug_of_title() {
        use swissarmyhammer_filter_expr::FilterContext;

        // Build a task registry with a well-known title.
        let other = {
            let mut t = make_task("01OTHER", "Fix login bug", "", "todo");
            t.set("title", json!("Fix login bug"));
            t
        };
        let tasks = vec![other.clone()];
        let reg = EntitySlugRegistry::build(&[], &[], &tasks);

        // Task that depends on `01OTHER`.
        let mut task = make_task("01SELF", "Other", "", "todo");
        task.set("depends_on", json!(["01OTHER"]));
        let adapter = TaskFilterAdapter::with_registry(&task, &reg);

        // Matches by id (current behavior).
        assert!(adapter.has_ref("01OTHER"));
        // Matches by slug of title (new behavior).
        assert!(adapter.has_ref("fix-login-bug"));
        // Does not match unrelated values.
        assert!(!adapter.has_ref("unrelated"));
    }

    #[test]
    fn task_filter_adapter_has_ref_matches_own_title_slug() {
        // The adapter should also match when the filter's slug points at
        // the current task's own title (not just a dependency).
        use swissarmyhammer_filter_expr::FilterContext;

        let task = {
            let mut t = make_task("01SELF", "Refactor parser", "", "todo");
            t.set("title", json!("Refactor parser"));
            t
        };
        let tasks = vec![task.clone()];
        let reg = EntitySlugRegistry::build(&[], &[], &tasks);
        let adapter = TaskFilterAdapter::with_registry(&task, &reg);

        // Matches by own id.
        assert!(adapter.has_ref("01SELF"));
        // Matches by slug of own title.
        assert!(adapter.has_ref("refactor-parser"));
    }

    #[test]
    fn task_filter_adapter_ref_id_takes_precedence_over_slug() {
        // A direct id match must work even when the registry doesn't know
        // about a task of that id — the registry is an additive lookup,
        // not a gate.
        use swissarmyhammer_filter_expr::FilterContext;

        let reg = EntitySlugRegistry::empty();
        let mut task = make_task("01SELF", "whatever", "", "todo");
        task.set("depends_on", json!(["01OTHER"]));
        let adapter = TaskFilterAdapter::with_registry(&task, &reg);

        assert!(adapter.has_ref("01OTHER"));
        assert!(adapter.has_ref("01SELF"));
    }
}
