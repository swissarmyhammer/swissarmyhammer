//! Loader for the locked plugin catalog (`plugins.yaml`).
//!
//! Parses the catalog into a typed [`Catalog`] of [`PluginSpec`] /
//! [`CommandSpec`] values, and provides the pinned 12-file source-YAML set the
//! drift test scans. See `catalog_self_check.rs` and `yaml_vs_catalog.rs` for
//! the assertions layered on top of this.
//!
//! Fidelity strategy: a [`CommandSpec`] carries the catalog-only routing fields
//! (`source_yaml`, `backend`) PLUS the full per-command metadata in
//! [`CommandSpec::metadata`], which is the EXACT same shape a source-YAML
//! command parses into ([`CommandMetadata`]). That lets the drift test compare
//! catalog metadata 1:1 against the source YAML without any field-by-field
//! transcription.

#![allow(dead_code)]

#[path = "catalog_self_check.rs"]
mod catalog_self_check;
#[path = "yaml_vs_catalog.rs"]
mod yaml_vs_catalog;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The set of MCP servers a command may route to. Mirrors the card's
/// "Backend servers referenced" list; `catalog_self_check.rs` asserts every
/// `backend` in the catalog is one of these.
pub const KNOWN_BACKENDS: &[&str] = &[
    "commands", "store", "entity", "kanban", "views", "ui_state", "window", "app", "focus",
];

/// The pinned source-YAML set the drift test scans — EXACTLY 12 files across
/// 2 crates. Do NOT replace this with a glob: a blind
/// `**/builtin/commands/*.yaml` would wrongly pick up the 13th YAML
/// (`swissarmyhammer-focus/.../nav.yaml`) plus `kanban/.../ai.yaml`.
///
/// Each entry is `(crate_relative_dir, file_name)`.
pub const PINNED_SOURCE_YAMLS: &[(&str, &str)] = &[
    // swissarmyhammer-kanban (7)
    ("crates/swissarmyhammer-kanban/builtin/commands", "task.yaml"),
    ("crates/swissarmyhammer-kanban/builtin/commands", "tag.yaml"),
    ("crates/swissarmyhammer-kanban/builtin/commands", "view.yaml"),
    ("crates/swissarmyhammer-kanban/builtin/commands", "column.yaml"),
    ("crates/swissarmyhammer-kanban/builtin/commands", "attachment.yaml"),
    ("crates/swissarmyhammer-kanban/builtin/commands", "file.yaml"),
    ("crates/swissarmyhammer-kanban/builtin/commands", "perspective.yaml"),
    // swissarmyhammer-commands (5)
    ("crates/swissarmyhammer-commands/builtin/commands", "entity.yaml"),
    ("crates/swissarmyhammer-commands/builtin/commands", "ui.yaml"),
    ("crates/swissarmyhammer-commands/builtin/commands", "app.yaml"),
    ("crates/swissarmyhammer-commands/builtin/commands", "settings.yaml"),
    ("crates/swissarmyhammer-commands/builtin/commands", "drag.yaml"),
];

/// The EXCLUDED 13th YAML — belongs to the spatial-nav project, NOT
/// builtin-commands. The drift test asserts its `nav.*` ids are absent from
/// the catalog (a negative assertion catching accidental glob regressions).
pub const EXCLUDED_NAV_YAML: (&str, &str) =
    ("crates/swissarmyhammer-focus/builtin/commands", "nav.yaml");

/// Resolve a crate-relative path against the workspace root.
///
/// `CARGO_MANIFEST_DIR` for this crate is
/// `<root>/crates/swissarmyhammer-command-service`, so the workspace root is
/// two levels up.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate manifest dir")
        .to_path_buf()
}

// ---------------------------------------------------------------------------
// Catalog (plugins.yaml) types
// ---------------------------------------------------------------------------

/// Top-level `plugins.yaml` document.
#[derive(Debug, Clone, Deserialize)]
pub struct Catalog {
    pub plugins: Vec<PluginSpec>,
}

/// One builtin command plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginSpec {
    pub name: String,
    pub directory: String,
    pub ensure_services: Vec<String>,
    pub source_yamls: Vec<String>,
    pub commands: Vec<CommandSpec>,
}

/// One catalog command: routing fields (`source_yaml`, `backend`) plus the full
/// per-command metadata, flattened so the catalog YAML stays a single block per
/// command.
#[derive(Debug, Clone, Deserialize)]
pub struct CommandSpec {
    /// File (within the plugin's `directory`) this command is sourced from.
    pub source_yaml: String,
    /// MCP server this command routes to (independent of `source_yaml`).
    pub backend: String,
    /// All remaining fields — the metadata that must match the source YAML 1:1.
    #[serde(flatten)]
    pub metadata: CommandMetadata,
}

// ---------------------------------------------------------------------------
// Command metadata — shared shape between catalog commands and source-YAML
// commands. This is the full union of fields present across the 12 YAMLs.
// ---------------------------------------------------------------------------

/// Full per-command metadata. Used both for catalog commands (via
/// `#[serde(flatten)]` on [`CommandSpec`]) and for the raw source-YAML commands
/// parsed by [`load_source_yaml`], so the two can be compared field-for-field.
///
/// `PartialEq` is the fidelity equality the drift test relies on. Optional
/// fields are `None` when the YAML omits them, so an absent field on one side
/// only matches an absent field on the other.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandMetadata {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undoable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu_group: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu_order: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_kinds: Option<Vec<String>>,
    /// Keybindings per keymap. `Some({})` (an explicit empty map) is distinct
    /// from `None` (the field omitted) — several perspective commands carry
    /// `keys: {}` deliberately, and the fidelity test preserves that.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_button: Option<TabButton>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<MenuPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<Param>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabButton {
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuPlacement {
    pub path: Vec<String>,
    pub group: i64,
    pub order: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radio_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub name: String,
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_command: Option<String>,
}

// ---------------------------------------------------------------------------
// Loaders
// ---------------------------------------------------------------------------

/// Parse the locked `plugins.yaml` catalog that sits next to this module.
pub fn load_catalog() -> Catalog {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/baseline/plugins.yaml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read catalog {}: {e}", path.display()));
    serde_yaml_ng::from_str(&text)
        .unwrap_or_else(|e| panic!("parse catalog {}: {e}", path.display()))
}

/// Parse a source-YAML command file (a bare top-level sequence of commands)
/// into the same [`CommandMetadata`] shape catalog commands use.
pub fn load_source_yaml(dir: &str, file: &str) -> Vec<CommandMetadata> {
    let path = workspace_root().join(dir).join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read source yaml {}: {e}", path.display()));
    serde_yaml_ng::from_str(&text)
        .unwrap_or_else(|e| panic!("parse source yaml {}: {e}", path.display()))
}
