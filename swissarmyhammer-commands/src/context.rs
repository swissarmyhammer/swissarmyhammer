use crate::error::{CommandError, Result};
use crate::ui_state::UIState;
use serde_json::Value;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// Context passed to every command — provides scope chain, target, args, and
/// service accessors.
///
/// The `CommandContext` is built fresh for each command invocation by the
/// dispatcher. It carries the resolved scope chain (list of `type:id` monikers),
/// optional target moniker, explicit args, and references to shared services.
///
/// Domain-specific services (e.g., KanbanContext) are stored in the `extensions`
/// map, keyed by type name, and retrieved via `extension::<T>()`.
pub struct CommandContext {
    pub command_id: String,
    pub scope_chain: Vec<String>,
    pub target: Option<String>,
    pub args: HashMap<String, Value>,
    /// The window label this command originated from (e.g. "main", "board-01abc...").
    pub window_label: Option<String>,
    /// Shared UI state (inspector stack, palette, keymap, etc.).
    pub ui_state: Option<Arc<UIState>>,
    /// Extension point for domain-specific services (e.g., KanbanContext).
    /// Keyed by TypeId for stability across crates and compiler versions.
    extensions: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl std::fmt::Debug for CommandContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandContext")
            .field("command_id", &self.command_id)
            .field("scope_chain", &self.scope_chain)
            .field("target", &self.target)
            .field("args", &self.args)
            .field("window_label", &self.window_label)
            .field("extensions_count", &self.extensions.len())
            .finish()
    }
}

impl CommandContext {
    /// Create a new CommandContext with the given scope chain, target, and args.
    pub fn new(
        command_id: impl Into<String>,
        scope_chain: Vec<String>,
        target: Option<String>,
        args: HashMap<String, Value>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            scope_chain,
            target,
            args,
            window_label: None,
            ui_state: None,
            extensions: HashMap::new(),
        }
    }

    /// Builder method to set the window label.
    pub fn with_window_label(mut self, label: impl Into<String>) -> Self {
        self.window_label = Some(label.into());
        self
    }

    /// Builder method to set the UI state.
    pub fn with_ui_state(mut self, ui_state: Arc<UIState>) -> Self {
        self.ui_state = Some(ui_state);
        self
    }

    /// Insert a typed extension service into the context.
    ///
    /// Extensions are keyed by `TypeId`, so each concrete type can have at
    /// most one instance. `TypeId` is stable across crates within a process.
    pub fn set_extension<T: Any + Send + Sync>(&mut self, value: Arc<T>) {
        self.extensions.insert(TypeId::of::<T>(), value);
    }

    /// Retrieve a typed extension service from the context.
    ///
    /// Returns `None` if no extension of type `T` has been set.
    pub fn extension<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.extensions
            .get(&TypeId::of::<T>())
            .and_then(|v| v.clone().downcast::<T>().ok())
    }

    /// Retrieve a required typed extension, returning an error if not set.
    ///
    /// Convenience wrapper around `extension::<T>()` that converts `None`
    /// into a `CommandError::ExecutionFailed` with the type name.
    pub fn require_extension<T: Any + Send + Sync>(&self) -> Result<Arc<T>> {
        self.extension::<T>().ok_or_else(|| {
            CommandError::ExecutionFailed(format!("{} not available", std::any::type_name::<T>()))
        })
    }

    /// Find the nearest moniker in the scope chain matching the given entity type.
    ///
    /// Scope chains are ordered innermost-first. Returns the first moniker whose
    /// type prefix matches `entity_type`, parsed into `(type, id)`.
    pub fn resolve_moniker(&self, entity_type: &str) -> Option<(&str, &str)> {
        for moniker in &self.scope_chain {
            if let Some((t, id)) = parse_moniker(moniker) {
                if t == entity_type {
                    return Some((t, id));
                }
            }
        }
        None
    }

    /// Check whether the scope chain contains a moniker of the given entity type.
    pub fn has_in_scope(&self, entity_type: &str) -> bool {
        self.resolve_moniker(entity_type).is_some()
    }

    /// Get an explicit argument by name.
    pub fn arg(&self, name: &str) -> Option<&Value> {
        self.args.get(name)
    }

    /// Get a required string argument, returning an error if missing.
    pub fn require_arg_str(&self, name: &str) -> Result<&str> {
        self.args
            .get(name)
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg(name.to_string()))
    }

    /// Resolve the entity ID for a given type from the scope chain.
    ///
    /// Convenience wrapper that returns just the ID portion.
    pub fn resolve_entity_id(&self, entity_type: &str) -> Option<&str> {
        self.resolve_moniker(entity_type).map(|(_, id)| id)
    }

    /// Parse the target moniker into `(type, id)`.
    pub fn target_moniker(&self) -> Option<(&str, &str)> {
        self.target.as_deref().and_then(parse_moniker)
    }
}

/// Parse a "type:id" moniker string into (entity_type, id).
///
/// The id portion may itself contain colons (e.g. "task:01JAB:extra" parses
/// as ("task", "01JAB:extra")).
pub fn parse_moniker(s: &str) -> Option<(&str, &str)> {
    let (entity_type, id) = s.split_once(':')?;
    if entity_type.is_empty() || id.is_empty() {
        return None;
    }
    Some((entity_type, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx(scope: &[&str]) -> CommandContext {
        CommandContext::new(
            "test.cmd",
            scope.iter().map(|s| s.to_string()).collect(),
            None,
            HashMap::new(),
        )
    }

    #[test]
    fn resolve_moniker_finds_matching_type() {
        let ctx = test_ctx(&["tag:bug", "task:01ABC", "column:todo", "board:board"]);
        let (t, id) = ctx.resolve_moniker("task").unwrap();
        assert_eq!(t, "task");
        assert_eq!(id, "01ABC");
    }

    #[test]
    fn resolve_moniker_returns_innermost() {
        let ctx = test_ctx(&["column:doing", "column:todo"]);
        let (_, id) = ctx.resolve_moniker("column").unwrap();
        assert_eq!(id, "doing");
    }

    #[test]
    fn resolve_moniker_returns_none_when_missing() {
        let ctx = test_ctx(&["task:01ABC", "column:todo"]);
        assert!(ctx.resolve_moniker("tag").is_none());
    }

    #[test]
    fn has_in_scope_true_and_false() {
        let ctx = test_ctx(&["task:01ABC", "column:todo"]);
        assert!(ctx.has_in_scope("task"));
        assert!(ctx.has_in_scope("column"));
        assert!(!ctx.has_in_scope("tag"));
        assert!(!ctx.has_in_scope("board"));
    }

    #[test]
    fn arg_retrieves_value() {
        let mut args = HashMap::new();
        args.insert("title".into(), serde_json::json!("Hello"));
        args.insert("count".into(), serde_json::json!(42));
        let ctx = CommandContext::new("test", vec![], None, args);

        assert_eq!(ctx.arg("title").unwrap(), "Hello");
        assert_eq!(ctx.arg("count").unwrap(), 42);
        assert!(ctx.arg("missing").is_none());
    }

    #[test]
    fn require_arg_str_ok_and_err() {
        let mut args = HashMap::new();
        args.insert("name".into(), serde_json::json!("Alice"));
        args.insert("count".into(), serde_json::json!(42));
        let ctx = CommandContext::new("test", vec![], None, args);

        assert_eq!(ctx.require_arg_str("name").unwrap(), "Alice");
        assert!(ctx.require_arg_str("count").is_err()); // not a string
        assert!(ctx.require_arg_str("missing").is_err());
    }

    #[test]
    fn resolve_entity_id_convenience() {
        let ctx = test_ctx(&["task:01ABC", "column:todo"]);
        assert_eq!(ctx.resolve_entity_id("task"), Some("01ABC"));
        assert_eq!(ctx.resolve_entity_id("column"), Some("todo"));
        assert_eq!(ctx.resolve_entity_id("tag"), None);
    }

    #[test]
    fn target_moniker_parsing() {
        let ctx = CommandContext::new("test", vec![], Some("column:doing".into()), HashMap::new());
        let (t, id) = ctx.target_moniker().unwrap();
        assert_eq!(t, "column");
        assert_eq!(id, "doing");
    }

    #[test]
    fn target_moniker_none_when_absent() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(ctx.target_moniker().is_none());
    }

    #[test]
    fn parse_moniker_valid() {
        let (t, id) = parse_moniker("task:01ABC").unwrap();
        assert_eq!(t, "task");
        assert_eq!(id, "01ABC");
    }

    #[test]
    fn parse_moniker_colon_in_id() {
        let (t, id) = parse_moniker("task:01ABC:extra").unwrap();
        assert_eq!(t, "task");
        assert_eq!(id, "01ABC:extra");
    }

    #[test]
    fn parse_moniker_invalid() {
        assert!(parse_moniker("nocolon").is_none());
        assert!(parse_moniker(":id").is_none());
        assert!(parse_moniker("type:").is_none());
    }
}
