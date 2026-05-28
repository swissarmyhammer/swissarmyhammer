//! Drift test: the locked catalog vs. the live source YAMLs.
//!
//! Scans EXACTLY the pinned 12-file source set (see
//! [`super::PINNED_SOURCE_YAMLS`]) — never a glob. Three layers:
//!
//! 1. Positive: the union of command ids across the 12 source YAMLs equals the
//!    catalog's command-id set (all 62 present, none extra).
//! 2. Negative: the 13th YAML (`swissarmyhammer-focus/.../nav.yaml`, 9 `nav.*`
//!    commands) is NOT in the catalog.
//! 3. Fidelity: every per-command metadata field is preserved 1:1 between the
//!    source YAML and the catalog. `CommandMetadata: PartialEq` IS the locked
//!    field set — adding/removing a field there changes what fidelity means,
//!    and `#[serde(deny_unknown_fields)]` makes a new YAML field a hard parse
//!    error until the schema is updated deliberately.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    load_catalog, load_source_yaml, CommandMetadata, EXCLUDED_NAV_YAML, PINNED_SOURCE_YAMLS,
};

/// All commands across the pinned 12 source YAMLs, keyed by id. Asserts no id
/// collides across files.
fn source_commands_by_id() -> BTreeMap<String, CommandMetadata> {
    let mut by_id: BTreeMap<String, CommandMetadata> = BTreeMap::new();
    for (dir, file) in PINNED_SOURCE_YAMLS {
        for cmd in load_source_yaml(dir, file) {
            let prev = by_id.insert(cmd.id.clone(), cmd.clone());
            assert!(
                prev.is_none(),
                "duplicate command id {} across pinned source YAMLs",
                cmd.id
            );
        }
    }
    by_id
}

fn catalog_commands_by_id() -> BTreeMap<String, CommandMetadata> {
    let catalog = load_catalog();
    let mut by_id = BTreeMap::new();
    for plugin in &catalog.plugins {
        for cmd in &plugin.commands {
            by_id.insert(cmd.metadata.id.clone(), cmd.metadata.clone());
        }
    }
    by_id
}

#[test]
fn pinned_set_is_exactly_12_files() {
    assert_eq!(PINNED_SOURCE_YAMLS.len(), 12, "the pinned source set must be exactly 12 files");
}

#[test]
fn positive_source_ids_equal_catalog_ids() {
    let source: BTreeSet<String> = source_commands_by_id().into_keys().collect();
    let catalog: BTreeSet<String> = catalog_commands_by_id().into_keys().collect();

    let missing_from_catalog: Vec<_> = source.difference(&catalog).collect();
    let extra_in_catalog: Vec<_> = catalog.difference(&source).collect();

    assert!(
        missing_from_catalog.is_empty(),
        "source-YAML ids absent from catalog: {missing_from_catalog:?}"
    );
    assert!(
        extra_in_catalog.is_empty(),
        "catalog ids absent from source YAMLs: {extra_in_catalog:?}"
    );
    assert_eq!(source, catalog, "source-YAML id set != catalog id set");
    assert_eq!(source.len(), 62, "expected 62 ids in the union");
}

#[test]
fn negative_nav_commands_not_in_catalog() {
    let (dir, file) = EXCLUDED_NAV_YAML;
    let nav = load_source_yaml(dir, file);
    assert_eq!(nav.len(), 9, "nav.yaml is expected to carry 9 nav.* commands");

    let catalog: BTreeSet<String> = catalog_commands_by_id().into_keys().collect();
    for cmd in &nav {
        assert!(
            cmd.id.starts_with("nav."),
            "nav.yaml command {} is not a nav.* id",
            cmd.id
        );
        assert!(
            !catalog.contains(&cmd.id),
            "nav.* command {} must NOT be in the builtin-commands catalog",
            cmd.id
        );
    }
}

#[test]
fn fidelity_every_metadata_field_matches() {
    let source = source_commands_by_id();
    let catalog = catalog_commands_by_id();

    // Equal key sets (also covered by the positive test, re-asserted so a
    // metadata diff doesn't get masked by a missing key panic below).
    assert_eq!(
        source.keys().collect::<BTreeSet<_>>(),
        catalog.keys().collect::<BTreeSet<_>>(),
        "id sets diverged before the field-level fidelity comparison"
    );

    for (id, src_meta) in &source {
        let cat_meta = catalog
            .get(id)
            .unwrap_or_else(|| panic!("catalog missing command {id}"));
        assert_eq!(
            src_meta, cat_meta,
            "metadata fidelity drift for command {id}:\n  source  = {src_meta:#?}\n  catalog = {cat_meta:#?}"
        );
    }
}

/// `ui.setFocus` is sourced from ui.yaml but routed to the `focus` backend —
/// source-file membership and backend are independent dimensions. This locks
/// that the catalog records both correctly.
#[test]
fn ui_set_focus_source_and_backend_are_independent() {
    let catalog = load_catalog();
    let cmd = catalog
        .plugins
        .iter()
        .flat_map(|p| &p.commands)
        .find(|c| c.metadata.id == "ui.setFocus")
        .expect("ui.setFocus must be in the catalog");

    assert_eq!(cmd.source_yaml, "ui.yaml", "ui.setFocus source YAML must be ui.yaml");
    assert_eq!(cmd.backend, "focus", "ui.setFocus backend must be focus");

    // And it really is a member of ui.yaml in the live source set.
    let ui = load_source_yaml(
        "crates/swissarmyhammer-commands/builtin/commands",
        "ui.yaml",
    );
    assert!(
        ui.iter().any(|c| c.id == "ui.setFocus"),
        "ui.setFocus must be present in the live ui.yaml"
    );
}
