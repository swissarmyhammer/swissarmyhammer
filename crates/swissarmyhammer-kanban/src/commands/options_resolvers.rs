//! Kanban-side composition of every domain crate's built-in
//! [`OptionsResolver`]s into a single [`OptionsRegistry`].
//!
//! Every built-in resolver now lives in its domain crate and is
//! registered here via the per-domain helpers:
//!
//! - `perspective.fields` — via
//!   [`swissarmyhammer_perspectives::register_perspective_resolvers`]
//! - `view.kinds` — via
//!   [`swissarmyhammer_views::register_view_resolvers`]
//! - `sort.directions` — via
//!   [`swissarmyhammer_commands::register_command_resolvers`]
//!
//! Kanban currently defines zero of its own resolvers (boards have
//! no enum-shaped params today). The function below is the
//! composition point for every consumer that wants the full
//! default-set; downstream consumers can further
//! [`OptionsRegistry::register`] additional resolvers on top.
//!
//! [`default_options_registry`] returns a fresh [`OptionsRegistry`]
//! with every built-in pre-registered. Consumers (the kanban-app GUI,
//! the kanban-cli, headless tests) call this exactly once at startup
//! and thread the registry into every
//! [`crate::scope_commands::commands_for_scope`] invocation.
//!
//! [`OptionsRegistry`]: swissarmyhammer_commands::OptionsRegistry
//! [`OptionsResolver`]: swissarmyhammer_commands::OptionsResolver

use swissarmyhammer_commands::{register_command_resolvers, OptionsRegistry};
use swissarmyhammer_perspectives::register_perspective_resolvers;
use swissarmyhammer_views::register_view_resolvers;

/// Construct an [`OptionsRegistry`] with every built-in resolver
/// pre-registered.
///
/// Built-ins registered, in registration order:
///
/// 1. `perspective.fields` — from `swissarmyhammer-perspectives`
///    via [`register_perspective_resolvers`]
/// 2. `view.kinds` — from `swissarmyhammer-views` via
///    [`register_view_resolvers`]
/// 3. `sort.directions` — from `swissarmyhammer-commands` via
///    [`register_command_resolvers`]
///
/// Downstream consumers can further [`OptionsRegistry::register`]
/// additional resolvers on top of the returned registry.
///
/// [`OptionsRegistry`]: swissarmyhammer_commands::OptionsRegistry
pub fn default_options_registry() -> OptionsRegistry {
    let mut registry = OptionsRegistry::new();
    register_perspective_resolvers(&mut registry);
    register_view_resolvers(&mut registry);
    register_command_resolvers(&mut registry);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`default_options_registry`] must register every built-in
    /// resolver. This guard fires if a new built-in is added to any
    /// domain crate without being wired into this composition.
    #[test]
    fn default_options_registry_registers_every_builtin_key() {
        let registry = default_options_registry();
        for key in ["perspective.fields", "view.kinds", "sort.directions"] {
            assert!(
                registry.has(key),
                "default_options_registry must register `{key}`",
            );
        }
    }
}
