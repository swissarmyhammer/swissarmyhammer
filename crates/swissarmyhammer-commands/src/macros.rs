//! Macros for composing builtin command sources at the app layer.
//!
//! Each contributor crate (e.g. `swissarmyhammer-commands`,
//! `swissarmyhammer-kanban`, `swissarmyhammer-focus`) ships its own
//! `builtin/commands/` directory and exposes a
//! `pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)>`
//! function that returns the embedded YAML sources for that crate.
//!
//! The **app** crate (kanban-app, kanban-cli, etc.) decides which
//! contributors to compose and in what order via [`compose_registry!`]
//! or [`compose_yaml_sources!`]. This keeps the aggregation decision at
//! the app layer where it belongs — the app knows which subsystems it
//! includes, and contributor crates remain simple data-providers with
//! no knowledge of one another.
//!
//! ## Order matters
//!
//! The order of crates in the macro IS the partial-merge precedence:
//! later sources override earlier ones by command id. A typical app
//! lists the most generic contributor first and the most domain-specific
//! contributor last, so domain commands can override generic ones.

/// Compose a [`CommandsRegistry`] from the `builtin_yaml_sources()`
/// functions exposed by each listed crate, in order.
///
/// Each crate must export
/// `pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)>`.
/// The order of crates is the partial-merge precedence: later sources
/// override earlier ones by command id.
///
/// Each entry is a `::`-separated path of one or more identifiers
/// (e.g. `swissarmyhammer_commands` or `crate::macros::tests::stub_a`).
///
/// # Example
///
/// ```ignore
/// use swissarmyhammer_commands::compose_registry;
///
/// let registry = compose_registry![
///     swissarmyhammer_commands,  // generic UI commands
///     swissarmyhammer_kanban,    // domain commands (overrides allowed)
/// ];
/// ```
///
/// [`CommandsRegistry`]: crate::CommandsRegistry
#[macro_export]
macro_rules! compose_registry {
    ($($($crate_path:ident)::+),+ $(,)?) => {{
        let sources: ::std::vec::Vec<(&'static str, &'static str)> =
            $crate::compose_yaml_sources![$($($crate_path)::+),+];
        $crate::CommandsRegistry::from_yaml_sources(&sources)
    }};
}

/// Compose a flat `Vec<(&'static str, &'static str)>` of YAML sources by
/// concatenating the `builtin_yaml_sources()` outputs of each listed
/// crate, in order.
///
/// Use this when you need to layer additional sources (e.g. user
/// overrides loaded from disk) onto the builtin stack before
/// constructing a registry.
///
/// Each entry is a `::`-separated path of one or more identifiers
/// (e.g. `swissarmyhammer_commands` or `crate::macros::tests::stub_a`).
///
/// # Example
///
/// ```ignore
/// use swissarmyhammer_commands::{compose_yaml_sources, CommandsRegistry};
///
/// let mut sources = compose_yaml_sources![
///     swissarmyhammer_commands,
///     swissarmyhammer_kanban,
/// ];
/// // Append user overrides — they apply last and override builtins.
/// let user_refs: Vec<(&str, &str)> = user_overrides
///     .iter()
///     .map(|(n, c)| (n.as_str(), c.as_str()))
///     .collect();
/// sources.extend(user_refs);
/// let registry = CommandsRegistry::from_yaml_sources(&sources);
/// ```
#[macro_export]
macro_rules! compose_yaml_sources {
    ($($($crate_path:ident)::+),+ $(,)?) => {{
        let mut sources: ::std::vec::Vec<(&'static str, &'static str)> =
            ::std::vec::Vec::new();
        $( sources.extend($($crate_path)::+::builtin_yaml_sources()); )+
        sources
    }};
}

#[cfg(test)]
mod tests {
    // Stub modules each exposing a `builtin_yaml_sources()` returning a
    // known fixture. The macros take a path expression, so a test-local
    // module is the simplest stand-in for a contributor crate.

    mod stub_a {
        pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
            vec![(
                "stub_a",
                "- id: stub_a.one\n  name: A One\n- id: stub_a.two\n  name: A Two\n",
            )]
        }
    }

    mod stub_b {
        pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
            vec![("stub_b", "- id: stub_b.one\n  name: B One\n")]
        }
    }

    /// Two contributor stubs combined by `compose_registry!` must produce
    /// a registry containing every id from both. This is the core
    /// invariant the macro is responsible for: concatenation, in order,
    /// with no silent loss.
    #[test]
    fn compose_registry_yields_concatenated_sources() {
        let registry = crate::compose_registry![self::stub_a, self::stub_b];

        assert_eq!(registry.all_commands().len(), 3);
        assert!(registry.get("stub_a.one").is_some());
        assert!(registry.get("stub_a.two").is_some());
        assert!(registry.get("stub_b.one").is_some());
    }

    /// `compose_yaml_sources!` returns the flat vec of sources without
    /// constructing a registry, so callers can append user overrides
    /// before calling `from_yaml_sources` themselves.
    #[test]
    fn compose_yaml_sources_returns_flat_concatenation() {
        let sources: Vec<(&'static str, &'static str)> =
            crate::compose_yaml_sources![self::stub_a, self::stub_b];

        // stub_a contributes 1 source, stub_b contributes 1 source.
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].0, "stub_a");
        assert_eq!(sources[1].0, "stub_b");
    }

    /// The macros must accept a trailing comma — a common ergonomic
    /// expectation for list-shaped macros that mirrors `vec![]`.
    #[test]
    fn compose_macros_accept_trailing_comma() {
        let registry = crate::compose_registry![self::stub_a, self::stub_b,];
        assert_eq!(registry.all_commands().len(), 3);

        let sources = crate::compose_yaml_sources![self::stub_a, self::stub_b,];
        assert_eq!(sources.len(), 2);
    }
}
