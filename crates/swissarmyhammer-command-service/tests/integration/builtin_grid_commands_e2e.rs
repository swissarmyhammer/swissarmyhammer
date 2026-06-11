//! End-to-end test for the committed `grid-commands` builtin plugin.
//!
//! This is the acceptance for Card C — the eleven `grid.*` commands moved OUT
//! of the client-side `CommandDef` list in
//! `apps/kanban-app/ui/src/components/grid-view.tsx` INTO the
//! `builtin/plugins/grid-commands/` bundle, so the catalogue (palette, keymap
//! metadata) is built FROM the CommandService and the React tree only
//! registers webview-bus HANDLERS for the ids (Card B's
//! `registerWebviewCommandHandler`).
//!
//! None of the eleven commands has a backend op: every one is "handled in
//! webview" — its effect manipulates live grid state (cell cursor, edit /
//! visual mode) or re-dispatches an existing backend-op command
//! (`grid.deleteRow` → `${entityType}.archive`, `grid.newBelow` /
//! `grid.newAbove` → `entity.add:{entityType}`) from inside the webview
//! handler. The host `execute` registered here is therefore an inert no-op,
//! mirroring `nav.jump` in the `nav-commands` bundle.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, all eleven `grid.*` commands
//!    are registered, and exactly those eleven.
//! 2. **Metadata fidelity** — each command's `name` / `keys` match the
//!    retired grid-view.tsx `CommandDef`s 1:1 (table test), every one is
//!    scoped to the grid zone (`scope: ["ui:grid"]`) so its keys never claim
//!    a global binding, and none carries a menu placement (the React defs
//!    had none).
//! 3. **No backend routing** — dispatching a grid id host-side succeeds as an
//!    inert `{ ok: true }` (no backend module is exposed, so a command that
//!    tried to reach one would raise).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_plugin::{CallerId, PluginHost, PLUGINS_SUBDIR};
use tempfile::TempDir;

use crate::support::{call_command, execute_result, try_call_command};

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

// ───────────────────────────────────────────────────────────────────────────
// Staging the committed builtin bundle
// ───────────────────────────────────────────────────────────────────────────

/// Resolve the workspace root (two levels above this crate's manifest dir).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate manifest dir")
        .to_path_buf()
}

/// Recursively copy a directory tree from `source` to `destination`.
fn copy_dir_recursive(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("staging directory should be created");
    for entry in std::fs::read_dir(source).expect("bundle dir should be readable") {
        let entry = entry.expect("a directory entry should be readable");
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("bundle file should copy");
        }
    }
}

/// Stage the committed `builtin/plugins/grid-commands` bundle into a temp
/// builtin layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/grid-commands/`.
fn stage_grid_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("grid-commands");
    assert!(
        source.is_dir(),
        "the committed grid-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("grid-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// Result-shape helpers
// ───────────────────────────────────────────────────────────────────────────

/// Pull the `commands` array out of a `list command` response, keyed by id.
fn commands_by_id(list_result: &Value) -> BTreeMap<String, Value> {
    list_result
        .get("structuredContent")
        .and_then(|sc| sc.get("commands"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|cmd| {
            let id = cmd.get("id").and_then(Value::as_str)?.to_string();
            Some((id, cmd))
        })
        .collect()
}

// ───────────────────────────────────────────────────────────────────────────
// The eleven grid ids + the locked metadata table
// ───────────────────────────────────────────────────────────────────────────

/// The eleven grid command ids, in no particular order.
const GRID_IDS: &[&str] = &[
    "grid.moveToRowStart",
    "grid.moveToRowEnd",
    "grid.firstCell",
    "grid.lastCell",
    "grid.edit",
    "grid.editEnter",
    "grid.exitEdit",
    "grid.toggleVisual",
    "grid.deleteRow",
    "grid.newBelow",
    "grid.newAbove",
];

/// One row of the metadata-fidelity table: a grid id with its expected `name`
/// and `keys` JSON (locked against the retired grid-view.tsx `CommandDef`s —
/// `keys: null` means the command had no keybinding).
struct GridMeta {
    id: &'static str,
    name: &'static str,
    keys: Value,
}

/// The metadata-fidelity table — names + keys copied 1:1 from the retired
/// client-side defs in `grid-view.tsx` (`buildGridExtremeCommands` /
/// `buildGridModeCommands` / `buildGridRowCommands`).
fn grid_metadata() -> Vec<GridMeta> {
    vec![
        GridMeta {
            id: "grid.moveToRowStart",
            name: "Row Start",
            keys: json!({ "vim": "0", "cua": "Home" }),
        },
        GridMeta {
            id: "grid.moveToRowEnd",
            name: "Row End",
            keys: json!({ "vim": "$", "cua": "End" }),
        },
        GridMeta {
            id: "grid.firstCell",
            name: "First Cell",
            keys: json!({ "cua": "Mod+Home" }),
        },
        GridMeta {
            id: "grid.lastCell",
            name: "Last Cell",
            keys: json!({ "cua": "Mod+End" }),
        },
        GridMeta {
            id: "grid.edit",
            name: "Edit Cell",
            keys: json!({ "vim": "i", "cua": "Enter" }),
        },
        GridMeta {
            id: "grid.editEnter",
            name: "Edit Cell (Enter)",
            keys: json!({ "vim": "Enter" }),
        },
        GridMeta {
            id: "grid.exitEdit",
            name: "Exit Edit",
            keys: Value::Null,
        },
        GridMeta {
            id: "grid.toggleVisual",
            name: "Toggle Visual Mode",
            keys: json!({ "vim": "v" }),
        },
        GridMeta {
            id: "grid.deleteRow",
            name: "Delete Row",
            keys: Value::Null,
        },
        GridMeta {
            id: "grid.newBelow",
            name: "New Row Below",
            keys: json!({ "vim": "o", "cua": "Mod+Enter" }),
        },
        GridMeta {
            id: "grid.newAbove",
            name: "New Row Above",
            keys: json!({ "vim": "O", "cua": "Mod+Shift+Enter" }),
        },
    ]
}

// ───────────────────────────────────────────────────────────────────────────
// The test
// ───────────────────────────────────────────────────────────────────────────

/// The committed `grid-commands` builtin plugin, discovered from a builtin
/// layer, registers all eleven `grid.*` commands with 1:1 metadata, every one
/// grid-scoped, and dispatches host-side as an inert webview-handled no-op.
#[tokio::test]
async fn grid_commands_plugin_registers_eleven_webview_handled_commands() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_grid_commands(builtin_root.path());

    let host = PluginHost::new(
        Some(builtin_root.path().to_path_buf()),
        user_root.path().to_path_buf(),
        None,
        user_root.path().to_path_buf(),
        false,
        user_root.path().to_path_buf(),
    );

    let service = install_commands_module(&host)
        .await
        .expect("install_commands_module must succeed");

    // NO other backend module is exposed: the grid commands route to no
    // backend — their `ensureServices` reaches only for `commands`, and their
    // executes are inert. A registration that reached for another module
    // would fail discovery here.
    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the grid-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one grid-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (1) Discovery + registration: exactly the eleven grid.* ids ─────────
    let listed = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "list command" }),
    )
    .await;
    let commands = commands_by_id(&listed);
    for id in GRID_IDS {
        assert!(
            commands.contains_key(*id),
            "list command must include the grid command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        commands.len(),
        11,
        "exactly the 11 grid.* commands should be registered, got {:?}",
        commands.keys().collect::<Vec<_>>()
    );

    // ── (2) Metadata fidelity: name / keys / scope / no menu, 1:1 ──────────
    for spec in grid_metadata() {
        let cmd = &commands[spec.id];
        assert_eq!(cmd["name"], json!(spec.name), "{} name", spec.id);
        if spec.keys.is_null() {
            assert!(
                cmd.get("keys").is_none() || cmd["keys"] == json!({}) || cmd["keys"].is_null(),
                "{} carries no keys in the retired React def, got {}",
                spec.id,
                cmd["keys"]
            );
        } else {
            assert_eq!(cmd["keys"], spec.keys, "{} keys", spec.id);
        }
        // Every grid command is gated to the grid zone: its keys apply only
        // when `ui:grid` is in the focused scope chain, and
        // `extractKeymapBindings` must never lift them into the global table.
        assert_eq!(
            cmd["scope"],
            json!(["ui:grid"]),
            "{} must be scoped to the grid zone",
            spec.id
        );
        // The React defs carried no menu placement — the plugin must not
        // invent one (the OS menu stays unchanged).
        assert!(
            cmd.get("menu").is_none() || cmd["menu"].is_null(),
            "{} carries no menu placement, got {}",
            spec.id,
            cmd["menu"]
        );
    }

    // ── (3) Host dispatch is an inert webview-handled no-op ────────────────
    // The webview command bus owns the real effect; the host execute exists
    // only to satisfy the registration contract. With NO backend module
    // exposed, a successful `{ ok: true }` proves the execute reaches nothing.
    let resp = try_call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "execute command", "id": "grid.edit", "ctx": {} }),
    )
    .await
    .expect("executing grid.edit host-side should not raise");
    assert_eq!(
        resp["structuredContent"]["ok"],
        json!(true),
        "executing grid.edit host-side should succeed as an inert no-op, got {resp}"
    );
    let result = execute_result(&resp);
    assert_eq!(
        result["ok"],
        json!(true),
        "the inert host execute returns {{ ok: true }}; got {result}"
    );
}
