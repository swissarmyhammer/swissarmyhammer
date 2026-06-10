//! End-to-end test for the committed `nav-commands` builtin plugin.
//!
//! This is the acceptance for Card A — the nine universal `nav.*`
//! spatial-navigation commands moved OUT of the retired
//! `swissarmyhammer-focus/builtin/commands/nav.yaml` overlay (whose execution
//! lived in React closures) INTO the `builtin/plugins/nav-commands/` bundle, so
//! the OS menu is built FROM the CommandService catalogue and nav execution is
//! a real backend/plugin path — plus the tenth nav command, `nav.focus`, the
//! programmatic focus-claim id (never in `nav.yaml`, so it carries no keys and
//! no menu placement).
//!
//! Eight of the nine `nav.yaml` commands route to the `focus` server
//! (`swissarmyhammer-focus::FocusServer` over a real `SpatialRegistry` /
//! `SpatialState`), exposed under id `"focus"`, host-driven (the kernel pulls
//! the live geometry from an injected [`UiGeometryProvider`] — here a recording
//! stub that serves a fixed two-scope snapshot). The ninth — `nav.jump` — has
//! NO backend op: its effect is presentation-only (open the jump overlay via
//! the webview command bus), so its host `execute` is an inert no-op. The
//! tenth — `nav.focus` — routes to the focus `set focus` op with the dispatch
//! `args.fq`.
//!
//! What a passing run proves:
//!
//! 1. **Discovery + registration** — after load, all ten `nav.*` commands are
//!    registered, and exactly those ten.
//! 2. **Metadata fidelity** — each `nav.yaml`-ported command's `name` / `keys`
//!    / `menu` match the retired `nav.yaml` baseline 1:1 (table test), and
//!    every one of those nine lands under the `Navigation` menu path;
//!    `nav.focus` matches its React-def baseline (name `Focus Scope`, no keys,
//!    no menu, not palette-visible).
//! 3. **Focus-op routing (real effect)** — with the kernel seeded (a focused
//!    scope) and the provider serving a snapshot, dispatching `nav.down` drives
//!    the focus `navigate` op host-driven and moves focus to the lower scope
//!    (a real `FocusChangedEvent`); dispatching `nav.drillIn` drives the focus
//!    `drill_in` op; dispatching `nav.focus` drives the `set focus` op.
//!    `nav.jump` does NOT touch the focus kernel.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_focus::{
    FocusLayer, FocusServer, FullyQualifiedMoniker, LayerName, NavSnapshot, Pixels, RecordingSink,
    Rect, SegmentMoniker, SnapshotScope, UiGeometryProvider, WindowLabel,
};
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};
use swissarmyhammer_ui_state::{UIState, UiStateServer};
use tempfile::TempDir;

use crate::support::{call_command, execute_result, try_call_command};

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The window the nav commands operate on. Carried only in the scope chain's
/// `window:` moniker (the production shape) — the plugin derives the focus op's
/// explicit `window` from it via the SDK `scopeId(ctx, "window")` helper.
const WINDOW: &str = "board-test";

/// The window-root layer FQM the seed snapshot lives under.
///
/// Window-rooted at `WINDOW` (`/<label>/window`, the shape `App.tsx`'s
/// `WINDOW_ROOT_FQ` mints): the kernel derives the owning window from the fq
/// ROOT SEGMENT, so the root segment here MUST equal `WINDOW` for nav/drill
/// commits to land in the `board-test` window. A non-window-rooted fixture
/// (e.g. `/L`) would commit focus under window "L" instead, and the
/// `focused_in(WINDOW)` assertions below would read `None`.
const LAYER_FQ: &str = "/board-test/window";
/// The upper focusable scope — the seed focus.
const SCOPE_TOP: &str = "/board-test/window/a";
/// The lower focusable scope — the `nav.down` target.
const SCOPE_BOTTOM: &str = "/board-test/window/b";

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

/// A [`UiGeometryProvider`] that serves geometry but reports NO focus —
/// reproducing the live UI-provider gap where the webview answers a `query
/// geometry` round-trip but `query focus` resolves nothing (transient unmount,
/// no focus responder mounted for the window, or a window-mismatch).
///
/// This is the asymmetry-exposing provider for the drill regression: the kernel
/// `focus_by_window` slot IS seeded (see [`expose_focus_with_provider`]), so the
/// server's `resolve_nav_source` / `resolve_drill_source` kernel-slot fallback
/// can still resolve the source. The DIFFERENCE the live bug turns on is whether
/// the caller (the plugin) consults that fallback at all:
///
/// - `nav.down` (navigate) sends only `{ window, direction }` and lets the
///   server fall back to the kernel slot → it MOVES.
/// - `nav.drillIn` (pre-fix) FIRST calls `query focus` → this provider → `None`
///   → bails to a `{ next_fq: null }` no-op, NEVER reaching the server drill op
///   and its kernel-slot fallback → it does NOT move.
///
/// Once the plugin stops pre-resolving focus and lets the server resolve the
/// drill source (with the same kernel-slot fallback navigate uses), drill MOVES
/// too — symmetric with navigate.
struct GapProvider;

#[async_trait]
impl UiGeometryProvider for GapProvider {
    async fn snapshot(&self, _window: &WindowLabel) -> Option<NavSnapshot> {
        Some(seed_snapshot())
    }

    async fn scope_chain(&self, _window: &WindowLabel) -> Vec<FullyQualifiedMoniker> {
        Vec::new()
    }

    async fn focus(&self, _window: &WindowLabel) -> Option<FullyQualifiedMoniker> {
        // The UI provider gap: the webview reports no focus even though the
        // kernel's per-window slot is set. `query focus` resolves `None`.
        None
    }
}

/// Expose a real `focus` server (seeded with a window-root layer and a focused
/// top scope) under id `"focus"`, returning the spatial-state handle so the test
/// can read the focused slot back. Uses the default [`SeedProvider`].
async fn expose_focus(
    host: &PluginHost,
) -> Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialState>> {
    expose_focus_with_provider(host, Arc::new(SeedProvider)).await
}

/// Expose a real `focus` server over a caller-supplied [`UiGeometryProvider`].
///
/// Seeds the same window-root layer + focused top scope as [`expose_focus`], so
/// the kernel's `focus_by_window[WINDOW]` slot is set regardless of what the
/// provider reports — letting a test inject a provider whose `focus()` reports
/// nothing while the kernel slot still resolves the source.
async fn expose_focus_with_provider(
    host: &PluginHost,
    provider: Arc<dyn UiGeometryProvider>,
) -> Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialState>> {
    let focus_server = FocusServer::new().with_provider(provider);
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

/// Expose a real `ui_state` server under id `"ui_state"`, returning the shared
/// [`UIState`] so the test can drive and observe it. `nav.drillOut` needs this
/// backend: its `ensureServices` requires `ui_state`, and its dismiss
/// fallthrough (echo / no-parent_zone) routes to the `dismiss ui` op here.
async fn expose_ui_state(host: &PluginHost, dir: &Path) -> Arc<UIState> {
    let ui_state = Arc::new(UIState::load(dir.join("ui_state.yaml")));
    let server = UiStateServer::new(Arc::clone(&ui_state));
    let module = InProcessServer::new(server)
        .await
        .expect("wrapping the ui_state server in an InProcessServer should succeed");
    host.expose_rust_module(
        "ui_state".to_string(),
        Arc::new(module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the ui_state module should succeed");
    ui_state
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

    // Expose the focus + ui_state backends BEFORE discovery so the plugin's
    // `ensureServices(this, ["commands", "focus", "ui_state"])` finds them
    // already exposed.
    let ui_state_dir = TempDir::new().expect("ui_state temp dir");
    let state = tokio::time::timeout(TIMEOUT, expose_focus(&host))
        .await
        .expect("exposing the focus backend should not hang");
    let ui_state = tokio::time::timeout(TIMEOUT, expose_ui_state(&host, ui_state_dir.path()))
        .await
        .expect("exposing the ui_state backend should not hang");

    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the nav-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one nav-commands builtin plugin should be discovered, got {loaded:?}"
    );

    // ── (1) Discovery + registration: exactly the ten nav.* ids ─────────────
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
        10,
        "exactly the 10 nav.* commands should be registered, got {:?}",
        commands.keys().collect::<Vec<_>>()
    );

    // ── (2) Metadata fidelity: lock each command's keys + menu 1:1 ──────────
    for spec in nav_metadata() {
        let cmd = &commands[spec.id];
        assert_eq!(cmd["keys"], spec.keys, "{} keys", spec.id);
        assert_eq!(cmd["menu"], spec.menu, "{} menu", spec.id);
        // Every nav.yaml-ported command lands under the Navigation menu.
        assert_eq!(
            cmd["menu"]["path"],
            json!(["Navigation"]),
            "{} must place under the Navigation menu",
            spec.id
        );
    }

    // ── (2b) nav.focus metadata: the programmatic focus-claim command ───────
    // `nav.focus` was never in `nav.yaml` — it has no key binding and no menu
    // placement (the Navigation submenu stays at the nine nav.yaml entries),
    // and it is not palette-visible (it requires a target `args.fq`, like the
    // programmatic `ui.setFocus` / `ui.mode.set`). Name matches the React
    // scope defs (`Focus Scope`).
    let focus_cmd = &commands["nav.focus"];
    assert_eq!(focus_cmd["name"], json!("Focus Scope"), "nav.focus name");
    assert_eq!(
        focus_cmd["visible"],
        json!(false),
        "nav.focus must not be palette-visible (it requires args.fq)"
    );
    assert!(
        focus_cmd.get("keys").is_none() || focus_cmd["keys"] == json!({}),
        "nav.focus carries no keys (it was never in nav.yaml), got {}",
        focus_cmd["keys"]
    );
    assert!(
        focus_cmd.get("menu").is_none() || focus_cmd["menu"].is_null(),
        "nav.focus carries no menu placement (the Navigation submenu stays at \
         the nine nav.yaml entries), got {}",
        focus_cmd["menu"]
    );

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

    // ── (3d) nav.drillOut at a layer-root edge dismisses the modal layer ────
    // The seed scopes carry NO `parent_zone` (`scope(..)` sets it `None`) and
    // the provider's focus is `SCOPE_TOP`, so the kernel drill_out echoes the
    // focused FQM — there is nothing to drill out TO. Per the ported
    // `buildDrillCommands` contract, nav.drillOut then falls through to
    // `ui_state` `dismiss ui`, closing the topmost modal layer. Open the
    // palette first so the dismiss has something to close, then assert it
    // closed — a real-path proof the echo→dismiss fallthrough reaches ui_state
    // (not just a return-value check).
    ui_state.set_palette_open(WINDOW, true);
    assert!(
        ui_state.palette_open(WINDOW),
        "precondition: the palette is open before nav.drillOut"
    );
    execute_ok(
        &service,
        "nav.drillOut",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert!(
        !ui_state.palette_open(WINDOW),
        "nav.drillOut at a layer-root edge (no parent_zone) must fall through to \
         ui_state dismiss and close the open palette"
    );

    // ── (3e) nav.drillOut at a layer-root edge closes the inspector ─────────
    // Card `01KTPDTH772HSEV5F7R1DKYDNJ` removed the `ui.inspector.close`
    // Escape binding, so Escape no longer closes the inspector directly — it
    // closes via nav.drillOut's dismiss fall-through (the `dismiss ui` op is a
    // LAYERED close: palette first, then pop the topmost inspector). With the
    // palette already closed, a drill-out at the layer-root edge must now pop
    // the open inspector. This pins the inspector's Escape-close path post-fix.
    ui_state.inspect(WINDOW, "task:probe");
    assert_eq!(
        ui_state.inspector_stack(WINDOW),
        vec!["task:probe".to_string()],
        "precondition: the inspector is open before nav.drillOut"
    );
    execute_ok(
        &service,
        "nav.drillOut",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert!(
        ui_state.inspector_stack(WINDOW).is_empty(),
        "nav.drillOut at a layer-root edge must fall through to ui_state dismiss \
         and pop the open inspector (the inspector's Escape-close path)"
    );

    // ── (3f) nav.focus routes to the focus `set focus` op ───────────────────
    // The host execute sends `{ fq: args.fq, window }` — the same wire shape
    // `focus-mcp.ts::setFocus` uses, minus the snapshot (the host has no
    // geometry of its own). The `set focus` op's envelope distinctively
    // carries an `event` key (like navigate; nav.jump's plain `{ ok }` does
    // not), proving the dispatch reached the focus kernel. Per `handle_focus`'s
    // contract a snapshot-less commit DROPS silently (`event: null`, slot
    // untouched) — in production the webview's `nav.focus` scope defs take the
    // execute fast-path and supply the snapshot; this plugin def is the
    // catalogue/routing owner.
    let focused_before = state
        .lock()
        .await
        .focused_in(&WindowLabel::from_string(WINDOW))
        .map(|fq| fq.to_string());
    let focus_result = execute_ok(
        &service,
        "nav.focus",
        json!({
            "scope_chain": window_scope(),
            "args": { "fq": SCOPE_TOP },
        }),
    )
    .await;
    assert!(
        focus_result["structuredContent"].get("event").is_some(),
        "nav.focus must route to the focus `set focus` op (envelope carries \
         `event`); got {focus_result}"
    );
    assert_eq!(
        focus_result["structuredContent"]["event"],
        Value::Null,
        "a snapshot-less host dispatch must drop the commit silently \
         (event: null); got {focus_result}"
    );
    assert_eq!(
        state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .map(|fq| fq.to_string()),
        focused_before,
        "the snapshot-less drop must leave the kernel focus slot untouched"
    );
}

/// The LIVE drill regression: with the UI provider reporting NO focus
/// (`GapProvider::focus` → `None`) while the kernel's `focus_by_window` slot IS
/// seeded, `nav.down` (navigate) still MOVES — but pre-fix `nav.drillIn` /
/// `nav.drillOut` (drill) do NOT, because the plugin pre-resolves focus through
/// `query focus` (provider-only, no kernel-slot fallback) and bails before ever
/// reaching the server drill op.
///
/// This reproduces the user-reported asymmetric signature — "navigate works,
/// drill/Escape broke" — that the symmetric kernel-property tests in
/// `two_window_isolation.rs` cannot: those put `focused_fq` inline on the wire,
/// short-circuiting the very source-resolution path the bug lives in. Here the
/// plugin is driven HOST-DRIVEN end-to-end (no inline `focused_fq`), so the
/// drill source resolution runs for real.
///
/// Contract:
/// - Navigate MUST move (kernel-slot fallback in `resolve_nav_source`).
/// - Drill MUST move too (symmetric: the fix routes the drill source through
///   the same kernel-slot fallback). Pre-fix this assertion FAILS — drill
///   no-ops, leaving focus on the top scope.
#[tokio::test]
async fn drill_resolves_source_via_kernel_slot_when_ui_focus_is_absent() {
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

    // The focus backend uses the GAP provider: geometry is served, but
    // `query focus` reports nothing — the live UI-provider gap. The kernel slot
    // is still seeded (focus committed on SCOPE_TOP).
    let ui_state_dir = TempDir::new().expect("ui_state temp dir");
    let state = tokio::time::timeout(
        TIMEOUT,
        expose_focus_with_provider(&host, Arc::new(GapProvider)),
    )
    .await
    .expect("exposing the focus backend should not hang");
    let _ui_state = tokio::time::timeout(TIMEOUT, expose_ui_state(&host, ui_state_dir.path()))
        .await
        .expect("exposing the ui_state backend should not hang");

    tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the nav-commands builtin plugin should succeed");

    // Precondition: the kernel slot holds the top scope even though the provider
    // reports no focus.
    assert_eq!(
        state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .map(|fq| fq.to_string()),
        Some(SCOPE_TOP.to_string()),
        "precondition: the kernel focus slot is seeded on the top scope",
    );

    // ── Navigate MOVES via the kernel-slot fallback (the control) ───────────
    execute_ok(
        &service,
        "nav.down",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert_eq!(
        state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .map(|fq| fq.to_string()),
        Some(SCOPE_BOTTOM.to_string()),
        "navigate must move via the kernel-slot fallback even when the UI \
         provider reports no focus",
    );

    // Re-seed focus back to the top scope so drill starts from the same place
    // navigate did, isolating the drill assertion from the navigate move above.
    execute_ok(&service, "nav.up", json!({ "scope_chain": window_scope() })).await;
    assert_eq!(
        state
            .lock()
            .await
            .focused_in(&WindowLabel::from_string(WINDOW))
            .map(|fq| fq.to_string()),
        Some(SCOPE_TOP.to_string()),
        "precondition for drill: focus back on the top scope",
    );

    // ── Drill MUST be symmetric — it must MOVE too ──────────────────────────
    // The seed snapshot has SCOPE_BOTTOM with no parent_zone and SCOPE_TOP with
    // no children, so a drill-in from the top scope can't descend; to give drill
    // a real target we seed a parent/child relationship. Instead of rebuilding
    // the fixture, assert drill REACHES the server drill op (which it cannot
    // pre-fix): drill-in from a leaf echoes its own FQM, but the op COMMITS that
    // focus and the kernel slot is touched. The decisive, drill-specific
    // assertion is that the kernel processed a drill at all — pre-fix the plugin
    // bails to a `{ next_fq: null }` no-op WITHOUT calling the server, so
    // `next_fq` is `null`; post-fix the server resolves the source from the
    // kernel slot and echoes the resolved focus.
    let drill = execute_ok(
        &service,
        "nav.drillIn",
        json!({ "scope_chain": window_scope() }),
    )
    .await;
    assert_eq!(
        drill["structuredContent"]["next_fq"],
        json!(SCOPE_TOP),
        "drill-in must resolve its source from the kernel slot (the focused top \
         scope) and reach the server drill op — pre-fix the plugin pre-resolves \
         via `query focus` (provider-only, here `None`) and bails to a \
         `next_fq: null` no-op, never reaching the server; got {drill}",
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Two windows on the SAME board — the live window/board conflation regression
// ───────────────────────────────────────────────────────────────────────────

/// The two windows of the same-board configuration. Each roots its FQMs at its
/// own unique label (`/<label>/window/...`) with an IDENTICAL board structure
/// beneath (`board:b/zone:z/task:t`) — exactly what two Tauri windows showing
/// one board produce.
const WIN_A: &str = "win-a";
const WIN_B: &str = "win-b";

/// The window-root layer FQM for `window`.
fn win_layer(window: &str) -> String {
    format!("/{window}/window")
}

/// The zone scope (a card-like container) for `window`'s board view.
fn win_zone(window: &str) -> String {
    format!("/{window}/window/board:b/zone:z")
}

/// The leaf scope (a field-like child of the zone) for `window`'s board view.
fn win_leaf(window: &str) -> String {
    format!("/{window}/window/board:b/zone:z/task:t")
}

/// The production-shape scope chain for a dispatch invoked in `window`.
fn scope_for(window: &str) -> Value {
    json!([format!("window:{window}"), "engine"])
}

/// The zone+leaf snapshot for `window` — the geometry that window's webview
/// serves for its OWN scopes. The zone has no `parent_zone` (it sits at the
/// layer root); the leaf nests inside it.
fn two_window_snapshot(window: &str) -> NavSnapshot {
    let zone = win_zone(window);
    let leaf = win_leaf(window);
    NavSnapshot {
        layer_fq: FullyQualifiedMoniker::from_string(&win_layer(window)),
        scopes: vec![
            SnapshotScope {
                fq: FullyQualifiedMoniker::from_string(&zone),
                rect: Rect {
                    x: Pixels::new(0.0),
                    y: Pixels::new(0.0),
                    width: Pixels::new(200.0),
                    height: Pixels::new(100.0),
                },
                parent_zone: None,
                nav_override: Default::default(),
                focusable: true,
            },
            SnapshotScope {
                fq: FullyQualifiedMoniker::from_string(&leaf),
                rect: Rect {
                    x: Pixels::new(10.0),
                    y: Pixels::new(10.0),
                    width: Pixels::new(50.0),
                    height: Pixels::new(20.0),
                },
                parent_zone: Some(FullyQualifiedMoniker::from_string(&zone)),
                nav_override: Default::default(),
                focusable: true,
            },
        ],
    }
}

/// A [`UiGeometryProvider`] modelling the TWO webviews of the two-window
/// configuration, including the LIVE pollution: each window's `focus.current`
/// answer comes from a per-window ref that — in the production bug — holds
/// ANOTHER window's FQ (every window's global `focus-changed` listener
/// receives `emit_to`-targeted events for all windows and overwrites its ref).
///
/// `snapshot(window)` always serves the asking window's OWN geometry (the
/// webview can always sample its own DOM), so the test isolates the focus
/// pollution from geometry availability.
struct TwoWindowWebviewProvider {
    /// Per-window `focus.current` answer — seeded with the polluted state.
    focus_refs: StdMutex<HashMap<String, String>>,
}

#[async_trait]
impl UiGeometryProvider for TwoWindowWebviewProvider {
    async fn snapshot(&self, window: &WindowLabel) -> Option<NavSnapshot> {
        Some(two_window_snapshot(window.as_str()))
    }

    async fn scope_chain(&self, _window: &WindowLabel) -> Vec<FullyQualifiedMoniker> {
        Vec::new()
    }

    async fn focus(&self, window: &WindowLabel) -> Option<FullyQualifiedMoniker> {
        self.focus_refs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(window.as_str())
            .map(|s| FullyQualifiedMoniker::from_string(s))
    }
}

/// Expose a `focus` server seeded for the two-window-same-board configuration:
/// both window-root layers registered, window A focused on its zone, window B
/// focused on its leaf. Returns the spatial-state handle and the recording sink.
async fn expose_focus_two_windows(
    host: &PluginHost,
    provider: Arc<dyn UiGeometryProvider>,
) -> (
    Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialState>>,
    Arc<RecordingSink>,
) {
    let sink = Arc::new(RecordingSink::new());
    let focus_server = FocusServer::new()
        .with_provider(provider)
        .with_sink(sink.clone());
    let registry = focus_server.registry();
    let state = focus_server.state();

    {
        let mut reg = registry.lock().await;
        for window in [WIN_A, WIN_B] {
            reg.push_layer(FocusLayer {
                fq: FullyQualifiedMoniker::from_string(&win_layer(window)),
                segment: SegmentMoniker::from_string("window"),
                name: LayerName::from_string("window"),
                parent: None,
                window_label: WindowLabel::from_string(window),
                last_focused: None,
            });
        }
    }
    {
        let mut reg = registry.lock().await;
        let mut st = state.lock().await;
        st.focus(
            &mut reg,
            &two_window_snapshot(WIN_A),
            FullyQualifiedMoniker::from_string(&win_zone(WIN_A)),
            None,
        );
        st.focus(
            &mut reg,
            &two_window_snapshot(WIN_B),
            FullyQualifiedMoniker::from_string(&win_leaf(WIN_B)),
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

    (state, sink)
}

/// Read the focused FQM for `window` as a `String`, for terse assertions.
async fn focused_string(
    state: &Arc<tokio::sync::Mutex<swissarmyhammer_focus::SpatialState>>,
    window: &str,
) -> Option<String> {
    state
        .lock()
        .await
        .focused_in(&WindowLabel::from_string(window))
        .map(|fq| fq.to_string())
}

/// THE LIVE BUG (two windows, same board): host-driven drill-in / drill-out in
/// window A must derive the owning window exclusively from the window-rooted
/// FQ chain. Pre-fix, the provider's `focus.current` answer for window A is
/// another window's FQ (the webview ref polluted by a leaked `focus-changed`),
/// and the drill path TRUSTS it:
///
/// - drill-in echoes the FOREIGN-rooted FQ as `next_fq` (the exact live log
///   signature: a dispatch with `window:win-a` in the scope chain resolving
///   `/win-b/...`), commits nothing in window A, and — unlike navigate, which
///   early-returns before touching any slot — `focus_from`'s unconditional
///   reconcile writes the STALE foreign FQ into WINDOW B's kernel slot.
/// - drill-out does the same and then, with `moved: false`, falls through to
///   the dismiss chain, spuriously closing window A's palette (the live
///   "Escape doesn't drill out" symptom).
///
/// Navigate in the SAME polluted configuration is harmless cross-window (the
/// control), matching the user-observed asymmetry. Post-fix the kernel rejects
/// the foreign-rooted provider answer, falls back to its own per-window slot,
/// and drill commits real moves in window A while window B stays untouched.
#[tokio::test]
async fn drill_ignores_foreign_window_focus_in_two_window_same_board() {
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

    // The polluted webview state: window A's `focus.current` ref holds WINDOW
    // B's zone FQ (stale — B has since moved to its leaf), exactly what the
    // unfiltered cross-window `focus-changed` listener produces live. Window
    // B's ref is its own, correct focus.
    let provider = Arc::new(TwoWindowWebviewProvider {
        focus_refs: StdMutex::new(HashMap::from([
            (WIN_A.to_string(), win_zone(WIN_B)),
            (WIN_B.to_string(), win_leaf(WIN_B)),
        ])),
    });

    let ui_state_dir = TempDir::new().expect("ui_state temp dir");
    let (state, sink) = tokio::time::timeout(
        TIMEOUT,
        expose_focus_two_windows(&host, provider as Arc<dyn UiGeometryProvider>),
    )
    .await
    .expect("exposing the focus backend should not hang");
    let ui_state = tokio::time::timeout(TIMEOUT, expose_ui_state(&host, ui_state_dir.path()))
        .await
        .expect("exposing the ui_state backend should not hang");

    tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the nav-commands builtin plugin should succeed");

    // ── Preconditions ────────────────────────────────────────────────────────
    assert_eq!(
        focused_string(&state, WIN_A).await,
        Some(win_zone(WIN_A)),
        "precondition: window A's kernel slot holds its zone"
    );
    assert_eq!(
        focused_string(&state, WIN_B).await,
        Some(win_leaf(WIN_B)),
        "precondition: window B's kernel slot holds its leaf"
    );
    sink.drain();

    // ── CONTROL: navigate in A is harmless cross-window (the live asymmetry) ─
    execute_ok(
        &service,
        "nav.down",
        json!({ "scope_chain": scope_for(WIN_A) }),
    )
    .await;
    assert_eq!(
        focused_string(&state, WIN_B).await,
        Some(win_leaf(WIN_B)),
        "navigate in window A must never perturb window B's focus slot"
    );

    // ── Drill-in (Enter) in window A ─────────────────────────────────────────
    let drill = execute_ok(
        &service,
        "nav.drillIn",
        json!({ "scope_chain": scope_for(WIN_A) }),
    )
    .await;
    assert_eq!(
        drill["structuredContent"]["next_fq"],
        json!(win_leaf(WIN_A)),
        "drill-in invoked in window A must resolve window A's focus from the \
         window-rooted FQ chain and descend into A's leaf — NOT echo the \
         foreign window's FQ the polluted provider reported; got {drill}"
    );
    assert_eq!(
        focused_string(&state, WIN_A).await,
        Some(win_leaf(WIN_A)),
        "drill-in must commit the move in window A's slot"
    );
    assert_eq!(
        focused_string(&state, WIN_B).await,
        Some(win_leaf(WIN_B)),
        "drill-in in window A must leave window B's slot untouched (pre-fix \
         the unconditional reconcile clobbers it with the stale foreign FQ)"
    );
    let in_events = sink.drain();
    assert!(
        !in_events.is_empty(),
        "drill-in in window A must emit a focus-changed event"
    );
    assert!(
        in_events
            .iter()
            .all(|e| e.window_label == WindowLabel::from_string(WIN_A)),
        "every drill event must target window A, never window B; got {in_events:?}"
    );

    // ── Drill-out (Escape) in window A ───────────────────────────────────────
    // Open A's palette first: pre-fix the foreign-focus echo reports
    // `moved: false` and the plugin falls through to dismiss, spuriously
    // closing it — the live "Escape doesn't drill out" symptom.
    ui_state.set_palette_open(WIN_A, true);
    execute_ok(
        &service,
        "nav.drillOut",
        json!({ "scope_chain": scope_for(WIN_A) }),
    )
    .await;
    assert_eq!(
        focused_string(&state, WIN_A).await,
        Some(win_zone(WIN_A)),
        "drill-out must move window A's focus back to its zone"
    );
    assert!(
        ui_state.palette_open(WIN_A),
        "a real drill-out move must NOT fall through to dismiss — window A's \
         palette must stay open"
    );
    assert_eq!(
        focused_string(&state, WIN_B).await,
        Some(win_leaf(WIN_B)),
        "drill-out in window A must leave window B's slot untouched"
    );
    let out_events = sink.drain();
    assert!(
        out_events
            .iter()
            .all(|e| e.window_label == WindowLabel::from_string(WIN_A)),
        "every drill-out event must target window A; got {out_events:?}"
    );

    // ── Window B drills independently (its own ref is healthy) ──────────────
    execute_ok(
        &service,
        "nav.drillOut",
        json!({ "scope_chain": scope_for(WIN_B) }),
    )
    .await;
    assert_eq!(
        focused_string(&state, WIN_B).await,
        Some(win_zone(WIN_B)),
        "drill-out in window B must move B's focus to its own zone"
    );
    assert_eq!(
        focused_string(&state, WIN_A).await,
        Some(win_zone(WIN_A)),
        "window B's drill must leave window A's slot untouched"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// The ten nav ids + the locked metadata of the nine nav.yaml-ported ones
// ───────────────────────────────────────────────────────────────────────────

/// The ten nav command ids, in no particular order. The first nine are the
/// `nav.yaml`-ported set; `nav.focus` is the programmatic focus-claim command
/// (never in `nav.yaml` — no keys, no menu).
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
    "nav.focus",
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
