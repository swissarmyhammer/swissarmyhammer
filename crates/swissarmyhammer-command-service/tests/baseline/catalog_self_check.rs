//! Internal-consistency checks on the locked catalog itself.
//!
//! These do not touch the source YAMLs — they assert the catalog's own
//! invariants: the locked tallies (7 plugins / 12 source YAMLs / 62 commands),
//! command-id uniqueness, and that every `backend` is in the known server set.

use std::collections::BTreeSet;

use super::{load_catalog, KNOWN_BACKENDS};

#[test]
fn tally_plugins_is_7() {
    let catalog = load_catalog();
    assert_eq!(catalog.plugins.len(), 7, "expected exactly 7 plugins");
}

#[test]
fn tally_source_yamls_is_12() {
    let catalog = load_catalog();
    // Union of every plugin's declared source_yamls. Plugins in the same
    // directory must not share file names, so the union of (dir, file) pairs is
    // the true count of distinct source files.
    let files: BTreeSet<(String, String)> = catalog
        .plugins
        .iter()
        .flat_map(|p| {
            p.source_yamls
                .iter()
                .map(move |f| (p.directory.clone(), f.clone()))
        })
        .collect();
    assert_eq!(files.len(), 12, "expected exactly 12 distinct source YAMLs, got {files:?}");
}

#[test]
fn tally_commands_is_62() {
    let catalog = load_catalog();
    let total: usize = catalog.plugins.iter().map(|p| p.commands.len()).sum();
    assert_eq!(total, 62, "expected exactly 62 commands across all plugins");

    // Per-plugin tallies: 3 + 5 + 4 + 17 + 8 + 10 + 15 = 62.
    let by_name: Vec<(&str, usize)> = catalog
        .plugins
        .iter()
        .map(|p| (p.name.as_str(), p.commands.len()))
        .collect();
    let expected = [
        ("task-commands", 3),
        ("kanban-misc-commands", 5),
        ("file-commands", 4),
        ("perspective-commands", 17),
        ("entity-commands", 8),
        ("ui-commands", 10),
        ("app-shell-commands", 15),
    ];
    assert_eq!(by_name, expected, "per-plugin command tallies drifted");
}

#[test]
fn command_ids_are_unique() {
    let catalog = load_catalog();
    let mut seen = BTreeSet::new();
    for plugin in &catalog.plugins {
        for cmd in &plugin.commands {
            assert!(
                seen.insert(cmd.metadata.id.clone()),
                "duplicate command id: {}",
                cmd.metadata.id
            );
        }
    }
    assert_eq!(seen.len(), 62, "62 unique command ids expected");
}

#[test]
fn every_backend_is_known() {
    let catalog = load_catalog();
    let known: BTreeSet<&str> = KNOWN_BACKENDS.iter().copied().collect();
    for plugin in &catalog.plugins {
        for cmd in &plugin.commands {
            assert!(
                known.contains(cmd.backend.as_str()),
                "command {} has unknown backend {:?} (known: {:?})",
                cmd.metadata.id,
                cmd.backend,
                KNOWN_BACKENDS,
            );
        }
    }
}

#[test]
fn every_ensure_service_is_known() {
    // ensure_services name backend servers too (plus `commands`), so they must
    // also be drawn from the known set.
    let catalog = load_catalog();
    let known: BTreeSet<&str> = KNOWN_BACKENDS.iter().copied().collect();
    for plugin in &catalog.plugins {
        for svc in &plugin.ensure_services {
            assert!(
                known.contains(svc.as_str()),
                "plugin {} ensures unknown service {:?}",
                plugin.name,
                svc,
            );
        }
    }
}
