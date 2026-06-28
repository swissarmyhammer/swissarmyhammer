//! Drift guard: the service-side `INSPECTABLE_ENTITY_PREFIXES` (the
//! `{{entity.type}}` caption renderer's entity-context filter in
//! `src/caption.rs`) matches the `app-shell-commands` builtin plugin's
//! `INSPECTABLE_ENTITY_PREFIXES` declaration 1:1.
//!
//! The list exists in two places that cannot import each other:
//!
//!   - `builtin/plugins/app-shell-commands/commands/ui.ts` — the server-side
//!     filter `entity.inspect` / `app.inspect` use to resolve their target
//!     from a dispatch's scope chain (`resolveInspectTarget`).
//!   - `src/caption.rs` — the caption renderer's entity-context filter, so a
//!     palette row's caption ("Inspect Task") and what picking it inspects
//!     resolve from the SAME rule and can never disagree (kanban card
//!     01KTY6XTJQFCG9ENKTAMC6N3JV).
//!
//! Exactly like the webview-side
//! `ui-plugin-inspectable-prefixes-mirror.spatial.node.test.ts` guard, this
//! test reads the plugin SOURCE from disk, parses the prefix array out of it,
//! and asserts set equality. Without it the lists could silently drift — an
//! entity kind added to the plugin but not the renderer would make the
//! caption promise one entity while the inspect resolves another.

use std::path::{Path, PathBuf};

use swissarmyhammer_command_service::INSPECTABLE_ENTITY_PREFIXES;

/// Resolve the workspace root (two levels above this crate's manifest dir).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate manifest dir")
        .to_path_buf()
}

/// Parse the string literals out of `const <name> ... = [ ... ]` in a
/// TypeScript source — the same forgiving extraction the webview-side
/// `parseStringArrayConst` helper performs.
fn parse_string_array_const(source: &str, const_name: &str) -> Vec<String> {
    let decl = format!("const {const_name}");
    let Some(decl_at) = source.find(&decl) else {
        return Vec::new();
    };
    let after_decl = &source[decl_at..];
    let Some(open) = after_decl.find('[') else {
        return Vec::new();
    };
    let Some(close) = after_decl[open..].find(']') else {
        return Vec::new();
    };
    let body = &after_decl[open + 1..open + close];
    body.split(',')
        .filter_map(|item| {
            let item = item.trim();
            let unquoted = item
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| item.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .or_else(|| item.strip_prefix('`').and_then(|s| s.strip_suffix('`')))?;
            Some(unquoted.to_string())
        })
        .collect()
}

/// Sorted copy for order-insensitive set comparison.
fn sorted(list: &[String]) -> Vec<String> {
    let mut sorted = list.to_vec();
    sorted.sort();
    sorted
}

#[test]
fn caption_prefixes_match_the_plugin_inspectable_entity_prefixes() {
    let plugin_source_path =
        workspace_root().join("builtin/plugins/app-shell-commands/commands/ui.ts");
    let source = std::fs::read_to_string(&plugin_source_path).unwrap_or_else(|e| {
        panic!(
            "the committed app-shell-commands plugin source must be readable at {}: {e}",
            plugin_source_path.display()
        )
    });

    let plugin_prefixes = parse_string_array_const(&source, "INSPECTABLE_ENTITY_PREFIXES");

    // Anchor sanity: a refactor that renames/moves the array must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    assert!(
        !plugin_prefixes.is_empty(),
        "INSPECTABLE_ENTITY_PREFIXES must be parseable out of the plugin source"
    );
    assert!(
        plugin_prefixes.contains(&"task:".to_string()),
        "the parsed plugin prefix list must contain task:, got {plugin_prefixes:?}"
    );

    let service_prefixes: Vec<String> = INSPECTABLE_ENTITY_PREFIXES
        .iter()
        .map(|p| p.to_string())
        .collect();
    assert_eq!(
        sorted(&service_prefixes),
        sorted(&plugin_prefixes),
        "the caption renderer's INSPECTABLE_ENTITY_PREFIXES (src/caption.rs) \
         must mirror the plugin's chain-resolution list \
         (builtin/plugins/app-shell-commands/commands/ui.ts) 1:1 — the two \
         surfaces share ONE rule so caption and inspect target never disagree"
    );
}

#[test]
fn field_is_not_a_chain_resolution_prefix() {
    // `field:` monikers (`field:{type}:{id}.{name}`) are projections of their
    // containing entity, never chain-resolution targets — a focused field
    // resolves to the CONTAINING task (kanban card 01KTY6XTJQFCG9ENKTAMC6N3JV).
    // Fields stay inspectable via an explicit target (double-click
    // `<Inspectable>`), which bypasses this list.
    assert!(
        !INSPECTABLE_ENTITY_PREFIXES.contains(&"field:"),
        "field: must NOT be a chain-resolution prefix — a focused field's \
         entity context is its containing entity"
    );
}
