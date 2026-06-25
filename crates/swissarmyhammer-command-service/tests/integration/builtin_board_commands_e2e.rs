//! End-to-end test for the committed `board-commands` builtin plugin.
//!
//! This is the acceptance for Card F — the three `board.*` commands moved OUT
//! of the client-side `CommandDef` factories in
//! `apps/kanban-app/ui/src/components/board-view.tsx`
//! (`makeNewTaskCommand` / `makeNavCommand`) INTO the
//! `builtin/plugins/board-commands/` bundle, so the catalogue (palette, keymap
//! metadata) is built FROM the CommandService and the board React tree only
//! registers a webview-bus HANDLER for the one presentation-orchestration id
//! (Card B's `registerWebviewCommandHandler`).
//!
//! The three commands split two ways:
//!
//! - `board.firstColumn` / `board.lastColumn` have a REAL backend op: they
//!   route to the focus kernel's `navigate focus` op host-driven with
//!   direction `first` / `last` — exactly the `nav.first` / `nav.last`
//!   wire shape from the `nav-commands` bundle. They exist only to fill the
//!   keymap gap those global commands leave (vim `0` / `$`, cua `Mod+Home` /
//!   `Mod+End`), gated to the board zone.
//! - `board.newTask` has NO backend op: its effect is webview orchestration
//!   (resolve the focused column, re-dispatch `entity.add:task`, focus the
//!   created card), so its host `execute` is an inert no-op, mirroring
//!   `nav.jump` / the `grid.*` set.
//!
//! `group.toggleCollapse` is the fourth command in the bundle: the vim `z o`
//! collapse-toggle for the focused group section of the grouped board view.
//! Like `board.newTask` it has NO backend op — its host execute is an inert
//! no-op and the real effect (flip the focused group's collapsed state) is a
//! webview-bus handler `GroupSection` registers per group via
//! `useFocusedWebviewCommandHandlers`. It is board-scoped (`ui:board`) and its
//! single binding is the vim chord `z o`.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, all four bundle commands
//!    are registered, and exactly those four.
//! 2. **Metadata fidelity** — each command's `name` / `keys` match the retired
//!    board-view.tsx `CommandDef`s 1:1 (table test), every one is scoped to
//!    the board zone (`scope: ["ui:board"]`) so its keys never claim a global
//!    binding, and none carries a menu placement (the React defs had none).
//! 3. **Focus-op routing (real effect)** — with the kernel seeded (a focused
//!    middle scope in a three-scope column row), dispatching
//!    `board.firstColumn` moves the kernel focus to the first scope and
//!    `board.lastColumn` to the last scope — real `navigate focus` round
//!    trips, not inert no-ops.
//! 4. **board.newTask does NOT touch the focus kernel** — its host dispatch
//!    succeeds as an inert `{ ok: true }` with no kernel envelope.

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

use crate::support::{call_command, copy_dir_recursive, execute_result, try_call_command};

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The window the board commands operate on. Carried only in the scope
/// chain's `window:` moniker (the production shape) — the plugin derives the
/// focus op's explicit `window` from it via the SDK `scopeId(ctx, "window")`
/// helper.
const WINDOW: &str = "board-test";

/// The window-root layer FQM the seed snapshot lives under (window-rooted at
/// `WINDOW` so the kernel derives the owning window from the fq root segment).
const LAYER_FQ: &str = "/board-test/window";
/// The board zone scope — the seed focus. The kernel's `first` / `last`
/// directions focus the FOCUSED SCOPE'S CHILDREN (`navigate.rs::edge_command`
/// picks among entries whose `parent_zone` is the focused scope), so the
/// column scopes below nest under this zone.
const SCOPE_BOARD: &str = "/board-test/window/board:b1";
/// The left column scope — the `board.firstColumn` target.
const SCOPE_FIRST: &str = "/board-test/window/board:b1/column:a";
/// The middle column scope.
const SCOPE_MIDDLE: &str = "/board-test/window/board:b1/column:b";
/// The right column scope — the `board.lastColumn` target.
const SCOPE_LAST: &str = "/board-test/window/board:b1/column:c";

/// The production-shape scope chain a real dispatch carries: a
/// `window:<label>` moniker plus the `engine` root.
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

/// Stage the committed `builtin/plugins/board-commands` bundle into a temp
/// builtin layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/board-commands/`.
fn stage_board_commands(layer_root: &Path) {
    let source = workspace_root()
        .join("builtin/plugins")
        .join("board-commands");
    assert!(
        source.is_dir(),
        "the committed board-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join("board-commands");
    copy_dir_recursive(&source, &destination);
}

// ───────────────────────────────────────────────────────────────────────────
// The `focus` backend: a real FocusServer with a fixed three-column snapshot
// ───────────────────────────────────────────────────────────────────────────

/// A rect at the given top-left, 100×200 — a column-shaped box so the three
/// seed scopes form a horizontal row the `first` / `last` directions order.
fn rect_at(x: f64, y: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(100.0),
        height: Pixels::new(200.0),
    }
}

/// A focusable snapshot scope with the given parent zone and no overrides.
fn scope(fq: &str, rect: Rect, parent_zone: Option<&str>) -> SnapshotScope {
    SnapshotScope {
        fq: FullyQualifiedMoniker::from_string(fq),
        rect,
        parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
        nav_override: Default::default(),
        focusable: true,
    }
}

/// The seed snapshot: a board zone containing three side-by-side focusable
/// column scopes, left → middle → right. With the board zone focused,
/// `first` lands on the leftmost child column and `last` on the rightmost —
/// the kernel's edge-command picks among the focused scope's children.
fn seed_snapshot() -> NavSnapshot {
    NavSnapshot {
        layer_fq: FullyQualifiedMoniker::from_string(LAYER_FQ),
        scopes: vec![
            scope(
                SCOPE_BOARD,
                Rect {
                    x: Pixels::new(0.0),
                    y: Pixels::new(0.0),
                    width: Pixels::new(450.0),
                    height: Pixels::new(250.0),
                },
                None,
            ),
            scope(SCOPE_FIRST, rect_at(0.0, 10.0), Some(SCOPE_BOARD)),
            scope(SCOPE_MIDDLE, rect_at(150.0, 10.0), Some(SCOPE_BOARD)),
            scope(SCOPE_LAST, rect_at(300.0, 10.0), Some(SCOPE_BOARD)),
        ],
    }
}

/// A [`UiGeometryProvider`] that serves the fixed [`seed_snapshot`] for the
/// test window — the host-driven `navigate focus` op pulls this on every
/// call. The kernel resolves the navigate source from its own
/// `focus_by_window` slot, so the focus pull just echoes the seed.
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
        Some(FullyQualifiedMoniker::from_string(SCOPE_BOARD))
    }
}

/// The registry + state handle pair [`expose_focus`] hands back so the test
/// can re-seed the focused slot between assertions.
type FocusHandles = (
    Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialRegistry>>,
    Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialState>>,
);

/// Expose a real `focus` server (seeded with a window-root layer and the
/// board zone scope focused) under id `"focus"`, returning the registry +
/// spatial-state handles so the test can read and re-seed the focused slot.
async fn expose_focus(host: &PluginHost) -> FocusHandles {
    let focus_server = FocusServer::new().with_provider(Arc::new(SeedProvider));
    let registry = focus_server.registry();
    let state = focus_server.state();

    // Seed the window-root layer owned by WINDOW so the kernel can derive the
    // owning window and beam-search inside it.
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
    // Seed the focused slot on the board zone scope, establishing
    // `focus_by_window[WINDOW]` for the host-driven navigate to resolve.
    seed_focus(&registry, &state, SCOPE_BOARD).await;

    let module = InProcessServer::new(focus_server)
        .await
        .expect("wrapping the focus server in an InProcessServer should succeed");
    host.expose_rust_module(
        "focus".to_string(),
        Arc::new(module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the focus module should succeed");

    (registry, state)
}

/// Commit kernel focus on `fq` directly (using the seed snapshot for
/// geometry), so each column-extreme assertion starts from the board zone.
async fn seed_focus(
    registry: &Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialRegistry>>,
    state: &Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialState>>,
    fq: &str,
) {
    let snapshot = seed_snapshot();
    let mut reg = registry.lock().await;
    let mut st = state.lock().await;
    st.focus(
        &mut reg,
        &snapshot,
        FullyQualifiedMoniker::from_string(fq),
        None,
    );
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

/// Execute a command by id with the given `ctx` payload and assert it
/// succeeded. Returns the inner backend result (`structuredContent.result`).
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

/// Read the focused FQM for the test window as a `String`.
async fn focused_string(
    state: &Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialState>>,
) -> Option<String> {
    state
        .lock()
        .await
        .focused_in(&WindowLabel::from_string(WINDOW))
        .map(|fq| fq.to_string())
}

// ───────────────────────────────────────────────────────────────────────────
// The three board ids + the locked metadata table
// ───────────────────────────────────────────────────────────────────────────

/// The four board command ids, in no particular order.
///
/// `group.toggleCollapse` is the fourth — the vim `z o` collapse-toggle for
/// the focused group section in the grouped board view. Like `board.newTask`
/// it is webview-bus handled (no backend op, inert host execute); the board
/// React tree (`GroupSection`) registers the live handler that flips the
/// focused group's collapse state.
const BOARD_IDS: &[&str] = &[
    "board.newTask",
    "board.firstColumn",
    "board.lastColumn",
    "group.toggleCollapse",
];

/// One row of the metadata-fidelity table: a board id with its expected
/// `name` and `keys` JSON (locked against the retired board-view.tsx
/// `CommandDef`s).
struct BoardMeta {
    id: &'static str,
    name: &'static str,
    keys: Value,
}

/// The metadata-fidelity table — names + keys copied 1:1 from the retired
/// client-side defs in `board-view.tsx` (`makeNewTaskCommand` /
/// `makeNavCommand`).
fn board_metadata() -> Vec<BoardMeta> {
    vec![
        BoardMeta {
            id: "board.newTask",
            name: "New Task",
            keys: json!({ "vim": "o", "cua": "Mod+Enter" }),
        },
        BoardMeta {
            id: "board.firstColumn",
            name: "First Column",
            keys: json!({ "vim": "0", "cua": "Mod+Home" }),
        },
        BoardMeta {
            id: "board.lastColumn",
            name: "Last Column",
            keys: json!({ "vim": "$", "cua": "Mod+End" }),
        },
        BoardMeta {
            id: "group.toggleCollapse",
            name: "Toggle Group Collapse",
            // vim `z o` is a CHORD (Card J schema): canonical keystrokes
            // separated by a single space. The webview keymap resolves it
            // step-by-step with a pending buffer; the binding lives in the
            // catalogue like every other key (no successor to the retired
            // SEQUENCE_TABLES).
            keys: json!({ "vim": "z o" }),
        },
    ]
}

// ───────────────────────────────────────────────────────────────────────────
// The test
// ───────────────────────────────────────────────────────────────────────────

/// The committed `board-commands` builtin plugin, discovered from a builtin
/// layer, registers all three `board.*` commands with 1:1 metadata, every one
/// board-scoped, routes the column-extreme pair to the real focus kernel, and
/// dispatches `board.newTask` host-side as an inert webview-handled no-op.
#[tokio::test]
async fn board_commands_plugin_registers_and_routes_column_extremes_to_focus() {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_board_commands(builtin_root.path());

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
    let (registry, state) = tokio::time::timeout(TIMEOUT, expose_focus(&host))
        .await
        .expect("exposing the focus backend should not hang");

    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the board-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one board-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (1) Discovery + registration: exactly the three board.* ids ─────────
    let listed = call_command(
        &service,
        CallerId::HostInternal,
        json!({ "op": "list command" }),
    )
    .await;
    let commands = commands_by_id(&listed);
    for id in BOARD_IDS {
        assert!(
            commands.contains_key(*id),
            "list command must include the board command {id:?}; got {:?}",
            commands.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        commands.len(),
        4,
        "exactly the 4 board-commands ids should be registered, got {:?}",
        commands.keys().collect::<Vec<_>>()
    );

    // ── (2) Metadata fidelity: name / keys / scope / no menu, 1:1 ──────────
    for spec in board_metadata() {
        let cmd = &commands[spec.id];
        assert_eq!(cmd["name"], json!(spec.name), "{} name", spec.id);
        assert_eq!(cmd["keys"], spec.keys, "{} keys", spec.id);
        // Every board command is gated to the board zone: its keys apply only
        // when `ui:board` is in the focused scope chain, and
        // `extractKeymapBindings` must never lift them into the global table.
        assert_eq!(
            cmd["scope"],
            json!(["ui:board"]),
            "{} must be scoped to the board zone",
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

    // ── (3a) board.firstColumn drives `navigate focus` first ───────────────
    // Precondition: the kernel's focused slot is the board zone (seeded); its
    // children are the three columns, so `first` / `last` pick among them.
    assert_eq!(
        focused_string(&state).await,
        Some(SCOPE_BOARD.to_string()),
        "precondition: the seeded focus is the board zone scope"
    );
    let first = execute_ok(
        &service,
        "board.firstColumn",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    // The focus `navigate` op's distinctive envelope carries an `event` key —
    // an inert webview-handled command would not.
    assert!(
        first["structuredContent"].get("event").is_some(),
        "board.firstColumn must route to the focus navigate op (envelope \
         carries `event`); got {first}"
    );
    assert_eq!(
        focused_string(&state).await,
        Some(SCOPE_FIRST.to_string()),
        "board.firstColumn must move the kernel focus to the first column scope"
    );

    // ── (3b) board.lastColumn drives `navigate focus` last ─────────────────
    // Re-seed focus back on the board zone: `first` / `last` pick among the
    // FOCUSED scope's children, and the previous assertion left focus on a
    // leaf column (no children → echo).
    seed_focus(&registry, &state, SCOPE_BOARD).await;
    let last = execute_ok(
        &service,
        "board.lastColumn",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert!(
        last["structuredContent"].get("event").is_some(),
        "board.lastColumn must route to the focus navigate op (envelope \
         carries `event`); got {last}"
    );
    assert_eq!(
        focused_string(&state).await,
        Some(SCOPE_LAST.to_string()),
        "board.lastColumn must move the kernel focus to the last column scope"
    );

    // ── (4) board.newTask host dispatch is an inert webview-handled no-op ──
    // The webview command bus owns the real effect (column-resolve +
    // entity.add:task re-dispatch + focus); the host execute exists only to
    // satisfy the registration contract. Its `{ ok: true }` envelope carries
    // no kernel shape (`event`), and the kernel focus slot stays untouched.
    let focused_before = focused_string(&state).await;
    let new_task = execute_ok(
        &service,
        "board.newTask",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert_eq!(
        new_task["ok"],
        json!(true),
        "the inert host execute returns {{ ok: true }}; got {new_task}"
    );
    assert!(
        new_task["structuredContent"].get("event").is_none(),
        "board.newTask must not route to the focus kernel (no `event`); got {new_task}"
    );
    assert_eq!(
        focused_string(&state).await,
        focused_before,
        "board.newTask must leave the kernel focus slot untouched"
    );

    // ── (5) group.toggleCollapse host dispatch is an inert webview no-op ────
    // The collapse-toggle effect lives entirely in the webview: `GroupSection`
    // registers the focus-gated handler that flips the focused group's
    // collapsed state. The host execute exists only to satisfy the
    // registration contract, so a direct host-side dispatch (no webview
    // mounted, as here) returns an inert `{ ok: true }` with no kernel `event`
    // envelope and leaves the focus slot untouched — exactly `board.newTask`.
    let focused_before_toggle = focused_string(&state).await;
    let toggle = execute_ok(
        &service,
        "group.toggleCollapse",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert_eq!(
        toggle["ok"],
        json!(true),
        "the inert host execute returns {{ ok: true }}; got {toggle}"
    );
    assert!(
        toggle["structuredContent"].get("event").is_none(),
        "group.toggleCollapse must not route to the focus kernel (no `event`); got {toggle}"
    );
    assert_eq!(
        focused_string(&state).await,
        focused_before_toggle,
        "group.toggleCollapse must leave the kernel focus slot untouched"
    );
}
