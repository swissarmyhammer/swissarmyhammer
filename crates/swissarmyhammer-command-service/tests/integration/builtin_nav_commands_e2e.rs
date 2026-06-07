//! End-to-end test for the committed `nav-commands` builtin plugin.
//!
//! This is the acceptance for Card A — the nine universal `nav.*`
//! spatial-navigation commands moved OUT of the retired
//! `swissarmyhammer-focus/builtin/commands/nav.yaml` overlay (whose execution
//! lived in React closures) INTO the `builtin/plugins/nav-commands/` bundle, so
//! the OS menu is built FROM the CommandService catalogue and nav execution is
//! a real backend/plugin path.
//!
//! Eight of the nine commands route to the `focus` server
//! (`swissarmyhammer-focus::FocusServer` over a real `SpatialRegistry` /
//! `SpatialState`), exposed under id `"focus"`, host-driven (the kernel pulls
//! the live geometry from an injected [`UiGeometryProvider`] — here a recording
//! stub that serves a fixed two-scope snapshot). The ninth — `nav.jump` — has
//! NO backend op: its effect is presentation-only (open the jump overlay via
//! the webview command bus), so its host `execute` is an inert no-op.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, all nine `nav.*` commands are
//!    registered, and exactly those nine.
//! 2. **Metadata fidelity** — each command's `name` / `keys` / `menu` match the
//!    retired `nav.yaml` baseline 1:1 (table test), and every nav command lands
//!    under the `Navigation` menu path.
//! 3. **Focus-op routing (real effect)** — with the kernel seeded (a focused
//!    scope) and the provider serving a snapshot, dispatching `nav.down` drives
//!    the focus `navigate` op host-driven and moves focus to the lower scope
//!    (a real `FocusChangedEvent`); dispatching `nav.drillIn` drives the focus
//!    `drill_in` op. `nav.jump` does NOT touch the focus kernel.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_focus::{
    FocusLayer, FocusServer, FullyQualifiedMoniker, LayerName, NavSnapshot, Pixels, Rect,
    SegmentMoniker, SnapshotScope, UiGeometryProvider, WindowLabel,
};
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use tempfile::TempDir;

use crate::support::{call_command, execute_result, try_call_command};

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The window the nav commands operate on. Carried only in the scope chain's
/// `window:` moniker (the production shape) — the plugin derives the focus op's
/// explicit `window` from it via the SDK `scopeId(ctx, "window")` helper.
const WINDOW: &str = "board-test";

/// The window-root layer FQM the seed snapshot lives under.
const LAYER_FQ: &str = "/L";
/// The upper focusable scope — the seed focus.
const SCOPE_TOP: &str = "/L/a";
/// The lower focusable scope — the `nav.down` target.
const SCOPE_BOTTOM: &str = "/L/b";

/// The production-shape scope chain a real dispatch carries: a `window:<label>`
/// moniker plus the `engine` root.
fn window_scope() -> Value {
    json!([format!("window:{WINDOW}"), "engine"])
}

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

/// Stage the committed `builtin/plugins/nav-commands` bundle into a temp builtin
/// layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/nav-commands/`.
fn stage_nav_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("nav-commands");
    assert!(
        source.is_dir(),
        "the committed nav-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("nav-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// The `focus` backend: a real FocusServer with a recording geometry provider
// ───────────────────────────────────────────────────────────────────────────

/// A rect at the given top-left, 100×20 — enough for the beam search to order
/// the two seed scopes vertically.
fn rect_at(x: f64, y: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(100.0),
        height: Pixels::new(20.0),
    }
}

/// A focusable snapshot scope with no parent zone and no overrides.
fn scope(fq: &str, rect: Rect) -> SnapshotScope {
    SnapshotScope {
        fq: FullyQualifiedMoniker::from_string(fq),
        rect,
        parent_zone: None,
        nav_override: Default::default(),
        focusable: true,
    }
}

/// The seed snapshot: two stacked focusable scopes under the window-root layer.
/// `SCOPE_TOP` sits above `SCOPE_BOTTOM`, so a `nav.down` from the top lands on
/// the bottom.
fn seed_snapshot() -> NavSnapshot {
    NavSnapshot {
        layer_fq: FullyQualifiedMoniker::from_string(LAYER_FQ),
        scopes: vec![
            scope(SCOPE_TOP, rect_at(0.0, 0.0)),
            scope(SCOPE_BOTTOM, rect_at(0.0, 100.0)),
        ],
    }
}

/// A [`UiGeometryProvider`] that serves the fixed [`seed_snapshot`] for the test
/// window — the host-driven nav ops pull this on every call. Scope chain /
/// focus pulls are not exercised by the navigate / drill plugin path (the
/// kernel resolves focus from its own `focus_by_window`), so they return empty.
struct SeedProvider;

#[async_trait]
impl UiGeometryProvider for SeedProvider {
    async fn snapshot(&self, _window: &WindowLabel) -> Option<NavSnapshot> {
        Some(seed_snapshot())
    }

    async fn scope_chain(&self, _window: &WindowLabel) -> Vec<FullyQualifiedMoniker> {
        Vec::new()
    }

    async fn focus(&self, _window: &WindowLabel) -> Option<FullyQualifiedMoniker> {
        // The kernel resolves the navigate/drill source from `focus_by_window`;
        // the plugin's `query focus` op for nav.drillIn reads this provider, so
        // return the seeded focused scope.
        Some(FullyQualifiedMoniker::from_string(SCOPE_TOP))
    }
}

/// Expose a real `focus` server (seeded with a window-root layer and a focused
/// top scope) under id `"focus"`, returning the spatial-state handle so the test
/// can read the focused slot back.
async fn expose_focus(
    host: &PluginHost,
) -> Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialState>> {
    let focus_server = FocusServer::new().with_provider(Arc::new(SeedProvider));
    let registry = focus_server.registry();
    let state = focus_server.state();

    // Seed the window-root layer `/L` owned by WINDOW so the kernel can derive
    // the owning window and beam-search inside it.
    {
        let mut reg = registry.lock().await;
        reg.push_layer(FocusLayer {
            fq: FullyQualifiedMoniker::from_string(LAYER_FQ),
            segment: SegmentMoniker::from_string("window"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string(WINDOW),
            last_focused: None,
        });
    }
    // Seed the focused slot: focus the top scope via the kernel (using the seed
    // snapshot so the kernel records geometry), establishing
    // `focus_by_window[WINDOW] = /L/a` for the host-driven navigate to resolve.
    {
        let snapshot = seed_snapshot();
        let mut reg = registry.lock().await;
        let mut st = state.lock().await;
        st.focus(
            &mut reg,
            &snapshot,
            FullyQualifiedMoniker::from_string(SCOPE_TOP),
            None,
        );
    }

    let module = InProcessServer::new(focus_server)
        .await
        .expect("wrapping the focus server in an InProcessServer should succeed");
    host.expose_rust_module(
        "focus".to_string(),
        Arc::new(module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the focus module should succeed");

    state
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

/// Execute a command by id with the given `ctx` payload and assert it succeeded.
/// Returns the inner backend result (`structuredContent.result`).
async fn execute_ok(
    service: &swissarmyhammer_command_service::CommandService,
    id: &str,
    ctx: Value,
) -> Value {
    let resp = try_call_command(
        service,
        CallerId::HostInternal,
        json!({ "op": "execute command", "id": id, "ctx": ctx }),
    )
    .await
    .unwrap_or_else(|e| panic!("executing {id} raised: {e:?}"));
    assert_eq!(
        resp["structuredContent"]["ok"],
        json!(true),
        "executing {id} should succeed, got {resp}"
    );
    execute_result(&resp)
}

// ───────────────────────────────────────────────────────────────────────────
// The test
// ───────────────────────────────────────────────────────────────────────────

/// The committed `nav-commands` builtin plugin, discovered from a builtin layer,
/// registers all nine `nav.*` commands with 1:1 metadata and drives the real
/// focus kernel for the directional / drill commands.
#[tokio::test]
async fn nav_commands_plugin_registers_and_routes_to_focus() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_nav_commands(builtin_root.path());

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

    // Expose the focus backend BEFORE discovery so the plugin's
    // `ensureServices(this, ["commands", "focus"])` finds it already exposed.
    let state = tokio::time::timeout(TIMEOUT, expose_focus(&host))
        .await
        .expect("exposing the focus backend should not hang");

    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the nav-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one nav-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (1) Discovery + registration: exactly the nine nav.* ids ────────────
    let listed = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "list command" }),
    )
    .await;
    let commands = commands_by_id(&listed);
    for id in NAV_IDS {
        assert!(
            commands.contains_key(*id),
            "list command must include the nav command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        commands.len(),
        9,
        "exactly the 9 nav.* commands should be registered, got {:?}",
        commands.keys().collect::<Vec<_>>()
    );

    // ── (2) Metadata fidelity: lock each command's keys + menu 1:1 ──────────
    for spec in nav_metadata() {
        let cmd = &commands[spec.id];
        assert_eq!(cmd["keys"], spec.keys, "{} keys", spec.id);
        assert_eq!(cmd["menu"], spec.menu, "{} menu", spec.id);
        // Every nav command lands under the Navigation top-level menu.
        assert_eq!(
            cmd["menu"]["path"],
            json!(["Navigation"]),
            "{} must place under the Navigation menu",
            spec.id
        );
    }

    // ── (3a) nav.down drives the focus navigate op and moves focus ──────────
    // Precondition: the kernel's focused slot is the top scope (seeded).
    assert_eq!(
        state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .map(|fq| fq.to_string()),
        Some(SCOPE_TOP.to_string()),
        "precondition: the seeded focus is the top scope"
    );
    let down = execute_ok(
        &service,
        "nav.down",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    // `execute_result` returns the backend op's CallToolResult; the focus
    // `navigate` op's distinctive envelope (under `structuredContent`) carries
    // an `event` key — a generic command would not. The host-driven path pulled
    // the snapshot and moved focus down to the bottom scope.
    assert!(
        down["structuredContent"].get("event").is_some(),
        "nav.down must route to the focus navigate op (envelope carries `event`); got {down}"
    );
    assert_eq!(
        state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .map(|fq| fq.to_string()),
        Some(SCOPE_BOTTOM.to_string()),
        "nav.down must move the kernel focus from the top scope to the bottom scope"
    );

    // ── (3b) nav.drillIn drives the focus drill_in op ───────────────────────
    // The drill op's distinctive envelope carries a `next_fq` key. The plugin
    // pulls the focused FQM (provider `focus`) then calls drill_in host-driven.
    let drill = execute_ok(
        &service,
        "nav.drillIn",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert!(
        drill["structuredContent"].get("next_fq").is_some(),
        "nav.drillIn must route to the focus drill_in op (envelope carries `next_fq`); got {drill}"
    );

    // ── (3c) nav.jump does NOT touch the focus kernel ───────────────────────
    // Its host execute is an inert no-op (the webview bus owns the real effect);
    // its `{ ok: true }` envelope carries neither the navigate (`event`) nor the
    // drill (`next_fq`) shape.
    let jump = execute_ok(
        &service,
        "nav.jump",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    let jump_sc = &jump["structuredContent"];
    assert!(
        jump_sc.get("event").is_none() && jump_sc.get("next_fq").is_none(),
        "nav.jump must not route to the focus kernel (no `event` / `next_fq`); got {jump}"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// The nine nav ids + their locked metadata (mirrors the retired nav.yaml)
// ───────────────────────────────────────────────────────────────────────────

/// The nine nav command ids, in no particular order.
const NAV_IDS: &[&str] = &[
    "nav.up",
    "nav.down",
    "nav.left",
    "nav.right",
    "nav.first",
    "nav.last",
    "nav.drillIn",
    "nav.drillOut",
    "nav.jump",
];

/// One row of the metadata-fidelity table: a nav id with its expected `keys`
/// and `menu` JSON (locked against the retired `nav.yaml`).
struct NavMeta {
    id: &'static str,
    keys: Value,
    menu: Value,
}

/// The metadata-fidelity table — keys + menu placement copied 1:1 from the
/// retired `swissarmyhammer-focus/builtin/commands/nav.yaml`.
fn nav_metadata() -> Vec<NavMeta> {
    vec![
        NavMeta {
            id: "nav.up",
            keys: json!({ "vim": "k", "cua": "ArrowUp", "emacs": "Ctrl+p" }),
            menu: json!({ "path": ["Navigation"], "group": 0, "order": 0 }),
        },
        NavMeta {
            id: "nav.down",
            keys: json!({ "vim": "j", "cua": "ArrowDown", "emacs": "Ctrl+n" }),
            menu: json!({ "path": ["Navigation"], "group": 0, "order": 1 }),
        },
        NavMeta {
            id: "nav.left",
            keys: json!({ "vim": "h", "cua": "ArrowLeft", "emacs": "Ctrl+b" }),
            menu: json!({ "path": ["Navigation"], "group": 0, "order": 2 }),
        },
        NavMeta {
            id: "nav.right",
            keys: json!({ "vim": "l", "cua": "ArrowRight", "emacs": "Ctrl+f" }),
            menu: json!({ "path": ["Navigation"], "group": 0, "order": 3 }),
        },
        NavMeta {
            id: "nav.first",
            keys: json!({ "cua": "Home", "emacs": "Alt+<" }),
            menu: json!({ "path": ["Navigation"], "group": 1, "order": 0 }),
        },
        NavMeta {
            id: "nav.last",
            keys: json!({ "vim": "Shift+G", "cua": "End", "emacs": "Alt+>" }),
            menu: json!({ "path": ["Navigation"], "group": 1, "order": 1 }),
        },
        NavMeta {
            id: "nav.drillIn",
            keys: json!({ "vim": "Enter", "cua": "Enter", "emacs": "Enter" }),
            menu: json!({ "path": ["Navigation"], "group": 2, "order": 0 }),
        },
        NavMeta {
            id: "nav.drillOut",
            keys: json!({ "vim": "Escape", "cua": "Escape", "emacs": "Escape" }),
            menu: json!({ "path": ["Navigation"], "group": 2, "order": 1 }),
        },
        NavMeta {
            id: "nav.jump",
            keys: json!({ "vim": "s", "cua": "Mod+G", "emacs": "Mod+G" }),
            menu: json!({ "path": ["Navigation"], "group": 3, "order": 0 }),
        },
    ]
}
