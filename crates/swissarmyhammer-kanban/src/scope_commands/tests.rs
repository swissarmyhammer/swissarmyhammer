//! Tests for [scope command resolution](super).
//!
//! The tests are in one module for each subject. The review engine puts a
//! whole file into one agent prompt. It does not review a file that is larger
//! than the per-file prompt cap. Thus a test tree of this size must be more
//! than one file.
//!
//! - [`availability`] — the gates that keep a command out of the list.
//! - [`cross_cutting`] — the pass that puts target-driven commands on every
//!   entity moniker.
//! - [`dynamic`] — the rows that come from `DynamicSources`: view switch,
//!   board switch, perspective goto and window focus.
//! - [`entity_add`] — the `entity.add` row, from hand-built views and from the
//!   real registry.
//! - [`entity_schema`] — the entity schema as a command source, and the field
//!   monikers that the walk skips.
//! - [`ordering`] — the order and the uniqueness of the merged list.
//! - [`perspective`] — the filter, group and sort commands of a perspective.
//! - [`scope`] — the command set that each moniker in the scope chain gives.
//! - [`templates`] — the name templates, such as `{{entity.type}}`.
//!
//! This module holds what those nine share: the imports, the harness type and
//! the `setup` fixture.

mod availability;
mod cross_cutting;
mod dynamic;
mod entity_add;
mod entity_schema;
mod ordering;
mod perspective;
mod scope;
mod templates;

use super::*;
use crate::defaults::{builtin_entity_definitions, builtin_field_definitions};
use crate::test_support::composed_builtin_yaml_sources;

/// Test harness tuple: registry, command impls, fields context, and UI state.
type TestHarness = (
    CommandsRegistry,
    HashMap<String, Arc<dyn Command>>,
    FieldsContext,
    Arc<UIState>,
);

/// Build a test harness with registry, command impls, and fields context.
fn setup() -> TestHarness {
    let registry = CommandsRegistry::from_yaml_sources(&composed_builtin_yaml_sources());
    let command_impls = crate::commands::register_commands();
    let defs = builtin_field_definitions();
    let entities = builtin_entity_definitions();
    let fields =
        FieldsContext::from_yaml_sources(std::path::PathBuf::from("/tmp/test"), &defs, &entities)
            .unwrap();
    let ui_state = Arc::new(UIState::new());
    (registry, command_impls, fields, ui_state)
}
