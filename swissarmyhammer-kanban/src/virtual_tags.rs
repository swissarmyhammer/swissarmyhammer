//! Virtual tag strategy abstraction and registry.
//!
//! Virtual tags are computed tags that appear on tasks based on board state
//! rather than being explicitly assigned. Each virtual tag has a strategy
//! that determines whether it applies to a given task, plus metadata
//! (color, description) and commands that appear in the context menu.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use swissarmyhammer_entity::EntityFilterContext;

/// Newtype for terminal column ID stored in [`EntityFilterContext`] extras.
///
/// Strategies extract this via `ctx.get::<TerminalColumnId>()` to determine
/// which column represents "done".
pub struct TerminalColumnId(pub String);

/// A command declared by a virtual tag strategy.
///
/// Carries the same shape as `CommandDef` so virtual-tag commands surface in
/// the context menu alongside registry commands when the user right-clicks
/// the virtual tag pill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualTagCommand {
    /// Unique command identifier (e.g. "resolve-blocked").
    pub id: String,
    /// Human-readable command name (e.g. "Resolve Blocked").
    pub name: String,
    /// Whether this command appears in the context menu.
    pub context_menu: bool,
    /// Optional key bindings for the command.
    pub keys: Option<HashMap<String, String>>,
}

/// Sealing module for [`VirtualTagStrategy`].
///
/// The `Sealed` supertrait lives in a public module so that sibling workspace
/// crates can implement it, but the trait is `#[doc(hidden)]` so downstream
/// consumers cannot discover or implement it.
pub mod sealed {
    /// Marker trait that seals [`VirtualTagStrategy`](super::VirtualTagStrategy).
    ///
    /// Implement this for any type that should be allowed to implement
    /// `VirtualTagStrategy`. This prevents arbitrary downstream types from
    /// implementing the trait, preserving semver freedom to add methods.
    #[doc(hidden)]
    pub trait Sealed {}
}

/// Trait for virtual tag evaluation strategies.
///
/// Each implementation defines a single virtual tag with its metadata
/// and matching logic. Strategies must be Send + Sync to support
/// concurrent evaluation across threads.
///
/// This trait is sealed and cannot be implemented outside this workspace.
pub trait VirtualTagStrategy: sealed::Sealed + Send + Sync {
    /// The tag slug (e.g. "READY", "BLOCKED").
    fn slug(&self) -> &str;

    /// Display color as a 6-character hex string (e.g. "22c55e").
    fn color(&self) -> &str;

    /// Human-readable description of what this virtual tag means.
    fn description(&self) -> &str;

    /// Commands available on this virtual tag's context menu.
    ///
    /// Returns owned `VirtualTagCommand` values. Because strategies are static
    /// singletons with compile-time-known data, each call allocates new
    /// `String`s. This is fine today since `commands()` is only called during
    /// metadata serialization (not on every `evaluate`). If it becomes a hot
    /// path, consider caching with `LazyLock<Vec<VirtualTagCommand>>` or
    /// switching fields to `Cow<'static, str>`.
    fn commands(&self) -> Vec<VirtualTagCommand>;

    /// Whether this virtual tag applies to the entity in the given context.
    ///
    /// The context provides `ctx.entity` (the entity under evaluation),
    /// `ctx.entities` (the full task list for cross-task analysis), and
    /// typed extras such as [`TerminalColumnId`].
    fn matches(&self, ctx: &EntityFilterContext) -> bool;
}

/// Serializable metadata for a virtual tag.
///
/// Used to send virtual tag definitions to the frontend API
/// without exposing the strategy implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualTagMeta {
    /// The tag slug.
    pub slug: String,
    /// Display color (6-char hex).
    pub color: String,
    /// Human-readable description.
    pub description: String,
    /// Commands available for this virtual tag.
    pub commands: Vec<VirtualTagCommand>,
}

/// Registry that maps tag slugs to virtual tag strategies.
///
/// Provides lookup, evaluation, and metadata serialization for all
/// registered virtual tags.
pub struct VirtualTagRegistry {
    strategies: HashMap<String, Box<dyn VirtualTagStrategy>>,
    /// Insertion order for deterministic iteration.
    order: Vec<String>,
}

impl std::fmt::Debug for VirtualTagRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualTagRegistry")
            .field("strategies", &self.order)
            .finish()
    }
}

impl VirtualTagRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Registers a virtual tag strategy.
    ///
    /// If a strategy with the same slug already exists, it is replaced.
    pub fn register(&mut self, strategy: Box<dyn VirtualTagStrategy>) {
        let slug = strategy.slug().to_string();
        if !self.strategies.contains_key(&slug) {
            self.order.push(slug.clone());
        }
        self.strategies.insert(slug, strategy);
    }

    /// Returns a reference to the strategy for the given slug, if registered.
    pub fn get(&self, slug: &str) -> Option<&dyn VirtualTagStrategy> {
        self.strategies.get(slug).map(|s| s.as_ref())
    }

    /// Returns references to all registered strategies in insertion order.
    pub fn all(&self) -> Vec<&dyn VirtualTagStrategy> {
        self.order
            .iter()
            .filter_map(|slug| self.strategies.get(slug).map(|s| s.as_ref()))
            .collect()
    }

    /// Evaluates all strategies against the entity in the given context
    /// and returns the slugs of matching virtual tags.
    ///
    /// The caller is responsible for populating `ctx.entity`, `ctx.entities`,
    /// and any extras (e.g. [`TerminalColumnId`]) before calling this method.
    pub fn evaluate(&self, ctx: &EntityFilterContext) -> Vec<String> {
        self.order
            .iter()
            .filter(|slug| {
                self.strategies
                    .get(*slug)
                    .map(|s| s.matches(ctx))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Returns serializable metadata for all registered virtual tags.
    pub fn metadata(&self) -> Vec<VirtualTagMeta> {
        self.all()
            .into_iter()
            .map(|s| VirtualTagMeta {
                slug: s.slug().to_string(),
                color: s.color().to_string(),
                description: s.description().to_string(),
                commands: s.commands(),
            })
            .collect()
    }

    /// Returns true if the given slug belongs to a registered virtual tag.
    pub fn is_virtual_slug(&self, slug: &str) -> bool {
        self.strategies.contains_key(slug)
    }
}

impl Default for VirtualTagRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Strategy for the READY virtual tag.
///
/// A task is READY when it is not in the terminal column and all of its
/// dependencies (if any) are in the terminal column. Tasks with no
/// dependencies are always ready (unless already completed).
pub struct ReadyStrategy;

impl sealed::Sealed for ReadyStrategy {}

impl VirtualTagStrategy for ReadyStrategy {
    fn slug(&self) -> &str {
        "READY"
    }

    fn color(&self) -> &str {
        "0e8a16"
    }

    fn description(&self) -> &str {
        "Task has no unmet dependencies"
    }

    fn commands(&self) -> Vec<VirtualTagCommand> {
        vec![]
    }

    fn matches(&self, ctx: &EntityFilterContext) -> bool {
        let entity = match ctx.entity {
            Some(e) => e,
            None => return false,
        };
        let terminal_column_id = ctx
            .get::<TerminalColumnId>()
            .map(|t| t.0.as_str())
            .unwrap_or("done");

        // Completed tasks are not READY
        let col = entity.get_str("position_column").unwrap_or("");
        if col == terminal_column_id {
            return false;
        }

        // A task is READY when every dependency is in the terminal column.
        // Missing dependencies (not found in all_tasks) count as not ready.
        let deps = entity.get_string_list("depends_on");
        deps.iter().all(|dep_id| {
            ctx.entities
                .iter()
                .find(|t| t.id.as_str() == dep_id)
                .map(|t| t.get_str("position_column") == Some(terminal_column_id))
                .unwrap_or(false)
        })
    }
}

/// Strategy that tags tasks which other tasks depend on.
///
/// A task is BLOCKING when at least one other task lists it in `depends_on`
/// AND the task itself is not yet in the terminal (done) column.
pub struct BlockingStrategy;

impl sealed::Sealed for BlockingStrategy {}

impl VirtualTagStrategy for BlockingStrategy {
    fn slug(&self) -> &str {
        "BLOCKING"
    }

    fn color(&self) -> &str {
        "d73a4a"
    }

    fn description(&self) -> &str {
        "Other tasks depend on this one"
    }

    fn commands(&self) -> Vec<VirtualTagCommand> {
        vec![VirtualTagCommand {
            id: "vtag.blocking.show_dependents".into(),
            name: "Show Dependents".into(),
            context_menu: true,
            keys: None,
        }]
    }

    fn matches(&self, ctx: &EntityFilterContext) -> bool {
        let entity = match ctx.entity {
            Some(e) => e,
            None => return false,
        };
        let terminal_column_id = ctx
            .get::<TerminalColumnId>()
            .map(|t| t.0.as_str())
            .unwrap_or("done");

        // Completed tasks don't block anything.
        let col = entity.get_str("position_column").unwrap_or("");
        if col == terminal_column_id {
            return false;
        }

        // True if at least one other task depends on this entity.
        let my_id = entity.id.as_str();
        ctx.entities.iter().any(|t| {
            t.get_string_list("depends_on")
                .iter()
                .any(|dep| dep == my_id)
        })
    }
}

/// Strategy for the BLOCKED virtual tag.
///
/// A task is BLOCKED when it has at least one dependency that is NOT
/// in the terminal (done) column. Missing dependencies (not found in
/// `all_tasks`) are also considered blocking.
pub struct BlockedStrategy;

impl sealed::Sealed for BlockedStrategy {}

impl VirtualTagStrategy for BlockedStrategy {
    fn slug(&self) -> &str {
        "BLOCKED"
    }

    fn color(&self) -> &str {
        "e36209"
    }

    fn description(&self) -> &str {
        "Task has unmet dependencies"
    }

    fn commands(&self) -> Vec<VirtualTagCommand> {
        vec![VirtualTagCommand {
            id: "vtag.blocked.show_blockers".into(),
            name: "Show Blockers".into(),
            context_menu: true,
            keys: None,
        }]
    }

    fn matches(&self, ctx: &EntityFilterContext) -> bool {
        let entity = match ctx.entity {
            Some(e) => e,
            None => return false,
        };
        let terminal_column_id = ctx
            .get::<TerminalColumnId>()
            .map(|t| t.0.as_str())
            .unwrap_or("done");

        let deps = entity.get_string_list("depends_on");
        if deps.is_empty() {
            return false;
        }
        deps.iter().any(|dep_id| {
            ctx.entities
                .iter()
                .find(|t| t.id.as_str() == dep_id)
                .map(|t| t.get_str("position_column") != Some(terminal_column_id))
                .unwrap_or(true) // missing dep = blocked
        })
    }
}

/// Static singleton for the default virtual tag registry.
///
/// The registry is immutable once built and never changes at runtime,
/// so we build it once and share it via a static reference.
static DEFAULT_REGISTRY: LazyLock<VirtualTagRegistry> = LazyLock::new(|| {
    let mut registry = VirtualTagRegistry::new();
    registry.register(Box::new(ReadyStrategy));
    registry.register(Box::new(BlockedStrategy));
    registry.register(Box::new(BlockingStrategy));
    registry
});

/// Returns a reference to the default virtual tag registry with all built-in strategies.
///
/// The registry is created once on first access and reused for all subsequent calls.
pub fn default_virtual_tag_registry() -> &'static VirtualTagRegistry {
    &DEFAULT_REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::HashMap;
    use swissarmyhammer_entity::{Entity, EntityFilterContext};

    /// Mock strategy that matches entities whose "status" field equals "active".
    struct ActiveStrategy;

    impl sealed::Sealed for ActiveStrategy {}

    impl VirtualTagStrategy for ActiveStrategy {
        fn slug(&self) -> &str {
            "ACTIVE"
        }

        fn color(&self) -> &str {
            "22c55e"
        }

        fn description(&self) -> &str {
            "Task is actively being worked on"
        }

        fn commands(&self) -> Vec<VirtualTagCommand> {
            vec![VirtualTagCommand {
                id: "deactivate".to_string(),
                name: "Deactivate".to_string(),
                context_menu: true,
                keys: None,
            }]
        }

        fn matches(&self, ctx: &EntityFilterContext) -> bool {
            ctx.entity
                .and_then(|e| e.fields.get("status"))
                .and_then(|v| v.as_str())
                .map(|s| s == "active")
                .unwrap_or(false)
        }
    }

    /// Mock strategy that always matches.
    struct AlwaysStrategy;

    impl sealed::Sealed for AlwaysStrategy {}

    impl VirtualTagStrategy for AlwaysStrategy {
        fn slug(&self) -> &str {
            "ALWAYS"
        }

        fn color(&self) -> &str {
            "ef4444"
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
    struct NeverStrategy;

    impl sealed::Sealed for NeverStrategy {}

    impl VirtualTagStrategy for NeverStrategy {
        fn slug(&self) -> &str {
            "NEVER"
        }

        fn color(&self) -> &str {
            "6b7280"
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

    fn make_entity(fields: Vec<(&str, &str)>) -> Entity {
        let mut entity = Entity::new("task", "test-001");
        for (k, v) in fields {
            entity
                .fields
                .insert(k.to_string(), Value::String(v.to_string()));
        }
        entity
    }

    /// Build an `EntityFilterContext` for test assertions.
    fn make_ctx<'a>(
        entity: &'a Entity,
        all_tasks: &'a [Entity],
        terminal_column_id: &str,
    ) -> EntityFilterContext<'a> {
        let mut ctx = EntityFilterContext::for_entity(entity, all_tasks);
        ctx.insert(TerminalColumnId(terminal_column_id.to_string()));
        ctx
    }

    #[test]
    fn test_empty_registry() {
        let registry = VirtualTagRegistry::new();
        assert!(registry.all().is_empty());
        assert!(registry.metadata().is_empty());
        assert!(!registry.is_virtual_slug("anything"));
    }

    #[test]
    fn test_default_registry_has_ready() {
        let registry = default_virtual_tag_registry();
        assert!(registry.is_virtual_slug("READY"));
        assert!(registry.is_virtual_slug("BLOCKED"));
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(ActiveStrategy));

        let strategy = registry.get("ACTIVE");
        assert!(strategy.is_some());
        assert_eq!(strategy.unwrap().slug(), "ACTIVE");
        assert_eq!(strategy.unwrap().color(), "22c55e");
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let registry = VirtualTagRegistry::new();
        assert!(registry.get("NONEXISTENT").is_none());
    }

    #[test]
    fn test_all_returns_insertion_order() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(ActiveStrategy));
        registry.register(Box::new(AlwaysStrategy));
        registry.register(Box::new(NeverStrategy));

        let all = registry.all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].slug(), "ACTIVE");
        assert_eq!(all[1].slug(), "ALWAYS");
        assert_eq!(all[2].slug(), "NEVER");
    }

    #[test]
    fn test_evaluate_returns_matching_slugs() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(ActiveStrategy));
        registry.register(Box::new(AlwaysStrategy));
        registry.register(Box::new(NeverStrategy));

        let entity = make_entity(vec![("status", "active")]);
        let ctx = make_ctx(&entity, &[], "done");
        let result = registry.evaluate(&ctx);

        assert_eq!(result, vec!["ACTIVE", "ALWAYS"]);
    }

    #[test]
    fn test_evaluate_no_matches() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(NeverStrategy));

        let entity = make_entity(vec![]);
        let ctx = make_ctx(&entity, &[], "done");
        let result = registry.evaluate(&ctx);

        assert!(result.is_empty());
    }

    #[test]
    fn test_evaluate_with_inactive_entity() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(ActiveStrategy));

        let entity = make_entity(vec![("status", "inactive")]);
        let ctx = make_ctx(&entity, &[], "done");
        let result = registry.evaluate(&ctx);

        assert!(result.is_empty());
    }

    #[test]
    fn test_metadata_includes_commands() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(ActiveStrategy));

        let meta = registry.metadata();
        assert_eq!(meta.len(), 1);

        let tag_meta = &meta[0];
        assert_eq!(tag_meta.slug, "ACTIVE");
        assert_eq!(tag_meta.color, "22c55e");
        assert_eq!(tag_meta.description, "Task is actively being worked on");
        assert_eq!(tag_meta.commands.len(), 1);
        assert_eq!(tag_meta.commands[0].id, "deactivate");
        assert_eq!(tag_meta.commands[0].name, "Deactivate");
        assert!(tag_meta.commands[0].context_menu);
        assert!(tag_meta.commands[0].keys.is_none());
    }

    #[test]
    fn test_metadata_preserves_order() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(AlwaysStrategy));
        registry.register(Box::new(ActiveStrategy));

        let meta = registry.metadata();
        assert_eq!(meta[0].slug, "ALWAYS");
        assert_eq!(meta[1].slug, "ACTIVE");
    }

    #[test]
    fn test_is_virtual_slug() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(ActiveStrategy));

        assert!(registry.is_virtual_slug("ACTIVE"));
        assert!(!registry.is_virtual_slug("NONEXISTENT"));
        assert!(!registry.is_virtual_slug("active")); // case-sensitive
    }

    #[test]
    fn test_register_replaces_existing() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(ActiveStrategy));
        registry.register(Box::new(ActiveStrategy)); // re-register same slug

        // Should still have exactly one entry
        assert_eq!(registry.all().len(), 1);
        assert_eq!(registry.metadata().len(), 1);
    }

    #[test]
    fn test_metadata_serializes_to_json() {
        let mut registry = VirtualTagRegistry::new();
        registry.register(Box::new(ActiveStrategy));

        let meta = registry.metadata();
        let json = serde_json::to_value(&meta).expect("metadata should serialize to JSON");

        assert!(json.is_array());
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["slug"], "ACTIVE");
        assert_eq!(arr[0]["color"], "22c55e");
        assert_eq!(arr[0]["commands"][0]["id"], "deactivate");
    }

    #[test]
    fn test_command_with_keys() {
        let cmd = VirtualTagCommand {
            id: "test-cmd".to_string(),
            name: "Test Command".to_string(),
            context_menu: false,
            keys: Some(HashMap::from([
                ("mac".to_string(), "cmd+shift+t".to_string()),
                ("win".to_string(), "ctrl+shift+t".to_string()),
            ])),
        };

        let json = serde_json::to_value(&cmd).expect("command should serialize");
        assert_eq!(json["id"], "test-cmd");
        assert!(!json["context_menu"].as_bool().unwrap());
        assert!(json["keys"].is_object());
    }

    // --- ReadyStrategy tests ---

    #[test]
    fn ready_no_deps() {
        let strategy = ReadyStrategy;
        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("todo".into()));
        // No depends_on field at all

        let ctx = make_ctx(&task, &[], "done");
        assert!(strategy.matches(&ctx));
    }

    #[test]
    fn ready_all_deps_complete() {
        let strategy = ReadyStrategy;

        let mut dep = Entity::new("task", "dep1");
        dep.set("position_column", Value::String("done".into()));

        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("todo".into()));
        task.set(
            "depends_on",
            Value::Array(vec![Value::String("dep1".into())]),
        );

        let all = [dep];
        let ctx = make_ctx(&task, &all, "done");
        assert!(strategy.matches(&ctx));
    }

    #[test]
    fn ready_incomplete_dep() {
        let strategy = ReadyStrategy;

        let mut dep = Entity::new("task", "dep1");
        dep.set("position_column", Value::String("doing".into()));

        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("todo".into()));
        task.set(
            "depends_on",
            Value::Array(vec![Value::String("dep1".into())]),
        );

        let all = [dep];
        let ctx = make_ctx(&task, &all, "done");
        assert!(!strategy.matches(&ctx));
    }

    #[test]
    fn ready_completed_task() {
        let strategy = ReadyStrategy;
        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("done".into()));

        let ctx = make_ctx(&task, &[], "done");
        assert!(!strategy.matches(&ctx));
    }

    #[test]
    fn ready_missing_dep_is_not_ready() {
        // Dependency "ghost" doesn't exist in entities — should NOT be ready.
        // This must match task_is_ready semantics (unwrap_or(false)).
        let strategy = ReadyStrategy;
        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("todo".into()));
        task.set(
            "depends_on",
            Value::Array(vec![Value::String("ghost".into())]),
        );

        let ctx = make_ctx(&task, &[], "done");
        assert!(!strategy.matches(&ctx));
    }

    #[test]
    fn blocked_missing_dep_is_blocked() {
        // Dependency "ghost" doesn't exist in entities — should be BLOCKED.
        // This must match task_blocked_by semantics (unwrap_or(true)).
        let strategy = BlockedStrategy;
        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("todo".into()));
        task.set(
            "depends_on",
            Value::Array(vec![Value::String("ghost".into())]),
        );

        let ctx = make_ctx(&task, &[], "done");
        assert!(strategy.matches(&ctx));
    }

    // --- BlockingStrategy tests ---

    #[test]
    fn blocking_has_dependents() {
        let strategy = BlockingStrategy;

        let mut blocker = Entity::new("task", "blocker");
        blocker.set("position_column", Value::String("doing".into()));

        let mut dependent = Entity::new("task", "dep");
        dependent.set("position_column", Value::String("todo".into()));
        dependent.set(
            "depends_on",
            Value::Array(vec![Value::String("blocker".into())]),
        );

        let all = vec![blocker.clone(), dependent];
        let ctx = make_ctx(&blocker, &all, "done");
        assert!(strategy.matches(&ctx));
    }

    #[test]
    fn blocking_completed() {
        let strategy = BlockingStrategy;

        let mut blocker = Entity::new("task", "blocker");
        blocker.set("position_column", Value::String("done".into()));

        let mut dependent = Entity::new("task", "dep");
        dependent.set("position_column", Value::String("todo".into()));
        dependent.set(
            "depends_on",
            Value::Array(vec![Value::String("blocker".into())]),
        );

        // Even though another task depends on it, it's in the terminal column
        let all = vec![blocker.clone(), dependent];
        let ctx = make_ctx(&blocker, &all, "done");
        assert!(!strategy.matches(&ctx));
    }

    #[test]
    fn blocking_no_dependents() {
        let strategy = BlockingStrategy;

        let mut task = Entity::new("task", "lonely");
        task.set("position_column", Value::String("doing".into()));

        let mut other = Entity::new("task", "other");
        other.set("position_column", Value::String("todo".into()));
        // other has no depends_on at all

        let all = vec![task.clone(), other];
        let ctx = make_ctx(&task, &all, "done");
        assert!(!strategy.matches(&ctx));
    }

    #[test]
    fn blocking_commands() {
        let strategy = BlockingStrategy;
        let cmds = strategy.commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].id, "vtag.blocking.show_dependents");
        assert_eq!(cmds[0].name, "Show Dependents");
        assert!(cmds[0].context_menu);
        assert!(cmds[0].keys.is_none());
    }

    // --- BlockedStrategy tests ---

    #[test]
    fn blocked_incomplete_dep() {
        let strategy = BlockedStrategy;

        let mut dep = Entity::new("task", "dep1");
        dep.set("position_column", Value::String("todo".into()));

        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("todo".into()));
        task.set(
            "depends_on",
            Value::Array(vec![Value::String("dep1".into())]),
        );

        let all = [dep];
        let ctx = make_ctx(&task, &all, "done");
        assert!(strategy.matches(&ctx));
    }

    #[test]
    fn blocked_no_deps() {
        let strategy = BlockedStrategy;

        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("todo".into()));

        let ctx = make_ctx(&task, &[], "done");
        assert!(!strategy.matches(&ctx));
    }

    #[test]
    fn blocked_all_deps_complete() {
        let strategy = BlockedStrategy;

        let mut dep = Entity::new("task", "dep1");
        dep.set("position_column", Value::String("done".into()));

        let mut task = Entity::new("task", "t1");
        task.set("position_column", Value::String("todo".into()));
        task.set(
            "depends_on",
            Value::Array(vec![Value::String("dep1".into())]),
        );

        let all = [dep];
        let ctx = make_ctx(&task, &all, "done");
        assert!(!strategy.matches(&ctx));
    }

    #[test]
    fn blocked_commands() {
        let strategy = BlockedStrategy;
        let cmds = strategy.commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].id, "vtag.blocked.show_blockers");
        assert_eq!(cmds[0].name, "Show Blockers");
        assert!(cmds[0].context_menu);
        assert!(cmds[0].keys.is_none());
    }
}
