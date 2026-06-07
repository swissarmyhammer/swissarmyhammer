//! The `#[operation]` structs that make up the `focus` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the
//! same [`FOCUS_OPERATIONS`] slice via the `operation_tool!` macro, so the
//! two cannot drift.
//!
//! # 1:1 port of the spatial-nav Tauri commands
//!
//! Each op mirrors exactly one `spatial_*` Tauri command in
//! `apps/kanban-app/src/commands.rs`, wrapping the corresponding
//! [`crate::SpatialRegistry`] / [`crate::SpatialState`] method with no
//! behavior change:
//!
//! | op           | Tauri command          | backing method                         |
//! |--------------|------------------------|----------------------------------------|
//! | `set focus`  | `spatial_focus`        | [`crate::SpatialState::focus`]          |
//! | `clear focus`| `spatial_clear_focus`  | [`crate::SpatialState::clear_focus`]    |
//! | `navigate focus` | `spatial_navigate` | [`crate::SpatialState::navigate`]       |
//! | `lose focus` | `spatial_focus_lost`   | [`crate::SpatialState::focus_lost`]     |
//! | `push layer` | `spatial_push_layer`   | [`crate::SpatialRegistry::push_layer`]  |
//! | `pop layer`  | `spatial_pop_layer`    | [`crate::SpatialRegistry::remove_layer`]|
//! | `drill_in layer` | `spatial_drill_in` | [`crate::drill_in`]                     |
//! | `drill_out layer`| `spatial_drill_out`| [`crate::drill_out`]                    |
//!
//! `ui.setFocus` (in the ui-commands plugin) routes to the `set focus` op.
//!
//! # The `window` parameter
//!
//! The Tauri commands derived the owning [`crate::WindowLabel`] from the
//! ambient `tauri::Window` parameter. There is no ambient window on the
//! MCP wire, so `clear focus` and `push layer` take an explicit `window`
//! field. `set focus` / `navigate focus` derive the window from the
//! snapshot's layer (exactly as the kernel did), so they need no window
//! field.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use swissarmyhammer_operations::{operation, Operation};

use crate::snapshot::NavSnapshot;
use crate::types::{
    Direction, FullyQualifiedMoniker, LayerName, Pixels, Rect, SegmentMoniker, WindowLabel,
};

/// Move focus to a scope (the `ui.setFocus` routing target).
///
/// Ports `spatial_focus`: when `snapshot` is `None` the call drops the
/// commit silently (transient unmount race), matching the Tauri command.
/// Otherwise delegates to [`crate::SpatialState::focus`], which derives
/// the owning window from the snapshot's layer.
///
/// Returns `{ ok: true, event: <FocusChangedEvent|null> }` — `event` is
/// `null` when focus did not actually move (no snapshot, window unknown,
/// already focused, or the FQM is missing from `snapshot.scopes`).
#[operation(
    verb = "set",
    noun = "focus",
    description = "Move focus to a scope (routing target for ui.setFocus)"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Focus {
    /// Fully-qualified moniker of the scope to focus.
    pub fq: FullyQualifiedMoniker,
    /// Per-decision scope geometry. `None` drops the commit silently
    /// (transient unmount race), exactly as the Tauri command did.
    #[serde(default)]
    pub snapshot: Option<NavSnapshot>,
}

/// Clear focus for a window.
///
/// Ports `spatial_clear_focus`. Delegates to
/// [`crate::SpatialState::clear_focus`]. The window is an explicit field
/// because the MCP wire has no ambient `tauri::Window`.
///
/// Returns `{ ok: true, event: <FocusChangedEvent|null> }` — `event` is
/// `null` when the window had no prior focus (idempotent no-op).
#[operation(
    verb = "clear",
    noun = "focus",
    description = "Clear focus for a window"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClearFocus {
    /// Window whose focus slot is cleared.
    pub window: WindowLabel,
}

/// Move focus relative to a scope in a cardinal direction.
///
/// Ports `spatial_navigate`: when `snapshot` is `None` the call drops
/// silently. Otherwise delegates to [`crate::SpatialState::navigate`].
///
/// Returns `{ ok: true, event: <FocusChangedEvent|null> }`.
/// Move focus relative to a scope in a cardinal direction.
///
/// Two call shapes, distinguished by which geometry source is supplied:
///
/// - **Inline** (the React `SpatialFocusActions::navigate` path): the caller
///   sends both `focused_fq` and an inline `snapshot`. Ports `spatial_navigate`
///   verbatim — a `None` snapshot drops silently.
/// - **Host-driven pull** (Card F2 — the nav.* plugin commands): the caller
///   sends only `window` + `direction`. The kernel resolves the current focus
///   from its own `focus_by_window[window]` and PULLS the live geometry from
///   the injected [`crate::UiGeometryProvider`] — no `focused_fq`, no
///   `snapshot` on the wire. A window with no focused slot, or a provider that
///   yields no snapshot, drops silently.
///
/// Returns `{ ok: true, event: <FocusChangedEvent|null> }`.
#[operation(
    verb = "navigate",
    noun = "focus",
    description = "Move focus relative to a scope in a cardinal direction"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Navigate {
    /// Cardinal direction (or first/last) to move in.
    pub direction: Direction,
    /// Currently focused scope to navigate from (inline path). Omitted on the
    /// host-driven pull path, where the kernel resolves focus from
    /// `focus_by_window[window]`.
    #[serde(default)]
    pub focused_fq: Option<FullyQualifiedMoniker>,
    /// Per-decision scope geometry (inline path). `None` selects the
    /// host-driven pull path when `window` is present, else drops silently.
    #[serde(default)]
    pub snapshot: Option<NavSnapshot>,
    /// Owning window for the host-driven pull path. The kernel reads its
    /// focused FQM and pulls geometry for this window.
    #[serde(default)]
    pub window: Option<WindowLabel>,
}

/// React to the focused scope unmounting and compute a focus fallback.
///
/// Ports `spatial_focus_lost`. Delegates to
/// [`crate::SpatialState::focus_lost`]. The lost FQM is **not** present in
/// `snapshot.scopes` (already removed on the React side), so its
/// `parent_zone`, owning layer FQM, and bounding rect ride alongside.
///
/// Returns `{ ok: true, event: <FocusChangedEvent|null> }` — `event` is
/// `null` when the lost FQM was not the focused slot for any window.
#[operation(
    verb = "lose",
    noun = "focus",
    description = "React to the focused scope unmounting and compute a focus fallback"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusLost {
    /// The FQM that lost focus (already unregistered on the React side).
    pub focused_fq: FullyQualifiedMoniker,
    /// `parent_zone` of the lost FQM, or `None` if it was registered
    /// directly under the layer root.
    #[serde(default)]
    pub lost_parent_zone: Option<FullyQualifiedMoniker>,
    /// FQM of the layer the lost FQM lived in.
    pub lost_layer_fq: FullyQualifiedMoniker,
    /// Bounding rect of the lost FQM at the moment it was unregistered.
    pub lost_rect: Rect,
    /// Per-decision scope geometry, with the lost FQM already removed.
    pub snapshot: NavSnapshot,
}

/// Push a layer onto the registry under the given owning window.
///
/// Ports `spatial_push_layer`. Delegates to
/// [`crate::SpatialRegistry::push_layer`]. The window is an explicit field
/// because the MCP wire has no ambient `tauri::Window`.
///
/// Returns `{ ok: true }`.
#[operation(
    verb = "push",
    noun = "layer",
    description = "Push a layer onto the registry under the given owning window"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushLayer {
    /// Canonical FQM of the layer.
    pub fq: FullyQualifiedMoniker,
    /// Relative segment of the layer.
    pub segment: SegmentMoniker,
    /// Layer role (`"window"`, `"inspector"`, `"dialog"`, `"palette"`).
    pub name: LayerName,
    /// Stacking parent (`None` for a window root).
    #[serde(default)]
    pub parent: Option<FullyQualifiedMoniker>,
    /// Owning window of the layer.
    pub window: WindowLabel,
}

/// Pop a previously-pushed layer and return the focus-restoration target.
///
/// Ports `spatial_pop_layer`: reads the popped layer's `last_focused`
/// slot before removal and returns it so the caller can issue a follow-up
/// `set focus`. Delegates to [`crate::SpatialRegistry::remove_layer`].
///
/// Returns `{ ok: true, next_fq: <FullyQualifiedMoniker|null> }` — `null`
/// when the layer is unknown or has no recorded `last_focused`.
#[operation(
    verb = "pop",
    noun = "layer",
    description = "Pop a layer and return its focus-restoration target"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopLayer {
    /// Canonical FQM of the layer to pop.
    pub fq: FullyQualifiedMoniker,
}

/// Compute the FQM to focus when drilling *into* a scope.
///
/// Ports `spatial_drill_in`: pure query returning [`crate::drill_in`]'s
/// result. Two call shapes, like [`Navigate`]:
///
/// - **Inline**: caller supplies `focused_fq` + `snapshot`. A `None`
///   snapshot returns `focused_fq` (transient unmount window).
/// - **Host-driven pull**: caller supplies `window`; the kernel resolves
///   `focused_fq` from `focus_by_window[window]` and pulls the snapshot from
///   the [`crate::UiGeometryProvider`]. With no resolvable focus or no pulled
///   snapshot, returns the resolved focus (or the wire `focused_fq`).
///
/// Returns `{ ok: true, next_fq: <FullyQualifiedMoniker|null> }` — `null`
/// only when neither a wire `focused_fq` nor a kernel focus is available.
#[operation(
    verb = "drill_in",
    noun = "layer",
    description = "Compute the FQM to focus when drilling into a scope"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillIn {
    /// Scope being drilled into.
    pub fq: FullyQualifiedMoniker,
    /// Currently focused scope (the no-op return value). Omitted on the
    /// host-driven pull path.
    #[serde(default)]
    pub focused_fq: Option<FullyQualifiedMoniker>,
    /// Per-decision scope geometry. `None` selects the host-driven pull path
    /// when `window` is present, else returns `focused_fq`.
    #[serde(default)]
    pub snapshot: Option<NavSnapshot>,
    /// Owning window for the host-driven pull path.
    #[serde(default)]
    pub window: Option<WindowLabel>,
}

/// Compute the FQM to focus when drilling *out of* a scope.
///
/// Ports `spatial_drill_out`: pure query returning [`crate::drill_out`]'s
/// result. Mirrors [`DrillIn`]'s two call shapes — inline (`focused_fq` +
/// `snapshot`) and host-driven pull (`window`, focus resolved from the
/// kernel, geometry pulled from the [`crate::UiGeometryProvider`]).
///
/// Returns `{ ok: true, next_fq: <FullyQualifiedMoniker|null> }`.
#[operation(
    verb = "drill_out",
    noun = "layer",
    description = "Compute the FQM to focus when drilling out of a scope"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillOut {
    /// Scope being drilled out of.
    pub fq: FullyQualifiedMoniker,
    /// Currently focused scope (the no-op return value). Omitted on the
    /// host-driven pull path.
    #[serde(default)]
    pub focused_fq: Option<FullyQualifiedMoniker>,
    /// Per-decision scope geometry. `None` selects the host-driven pull path
    /// when `window` is present, else returns `focused_fq`.
    #[serde(default)]
    pub snapshot: Option<NavSnapshot>,
    /// Owning window for the host-driven pull path.
    #[serde(default)]
    pub window: Option<WindowLabel>,
}

/// Generate prefix-free Jump-To sneak codes from the kernel's
/// ergonomic 23-letter alphabet.
///
/// Ports the `generate_jump_codes` Tauri command. Pure compute — wraps
/// [`crate::generate_sneak_codes`] verbatim. Surfaced on the `focus`
/// MCP tool so the React `useJumpTargets` hook can reach it through the
/// generic MCP transport instead of a dedicated Tauri command.
///
/// Returns `{ ok: true, codes: [String] }` on success; the kernel's
/// `SneakError` is surfaced as an `invalid_params` MCP error.
#[operation(
    verb = "generate",
    noun = "sneak_codes",
    description = "Generate prefix-free Jump-To codes from the ergonomic alphabet"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateSneakCodes {
    /// Number of distinct codes to generate. Must be `<= 529` (23²).
    pub count: usize,
}

/// Pull the live [`NavSnapshot`] for the focused layer in a window.
///
/// The on-demand geometry query (Card F2): the kernel asks the injected
/// [`crate::UiGeometryProvider`] for a freshly-built snapshot
/// (`getBoundingClientRect` at call time in the webview) and returns it
/// verbatim. Lets plugins reach the live geometry through the generic MCP
/// transport (`this.focus.query_geometry`) instead of building it client-side.
///
/// Returns `{ ok: true, snapshot: <NavSnapshot|null> }` — `null` when the
/// provider has no snapshot (window closed, no responder, transient unmount).
#[operation(
    verb = "query",
    noun = "geometry",
    description = "Pull the live navigation snapshot for the focused layer in a window"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryGeometry {
    /// Window whose live geometry to pull.
    pub window: WindowLabel,
}

/// Pull the current focus/command scope chain for a window.
///
/// Returns `{ ok: true, scope_chain: [FullyQualifiedMoniker] }`, outermost
/// first — empty when the window has no active scope chain.
#[operation(
    verb = "query",
    noun = "scope_chain",
    description = "Pull the current focus scope chain for a window"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryScopeChain {
    /// Window whose scope chain to pull.
    pub window: WindowLabel,
}

/// Pull the FQM currently focused in a window.
///
/// Returns `{ ok: true, focus: <FullyQualifiedMoniker|null> }` — `null` when
/// the window has no focus.
#[operation(
    verb = "query",
    noun = "focus",
    description = "Pull the FQM currently focused in a window"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryFocus {
    /// Window whose focused FQM to pull.
    pub window: WindowLabel,
}

/// All focus operations — the canonical list used for schema generation.
///
/// Both the wire-schema generator (`generate_mcp_schema`) and the
/// discovery `_meta` generator (`generate_operations_meta`) are driven
/// from this single slice via the `operation_tool!` macro, so there is
/// one source of truth for what the `focus` tool exposes.
static FOCUS_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    // The `Operation` trait methods (`verb`/`noun`/`description`/
    // `parameters`) never read field values, so a zero-valued prototype of
    // each op is a sufficient `&dyn Operation`. We build prototypes
    // explicitly because the spatial newtypes / `Direction` / `Rect` do not
    // derive `Default`.
    vec![
        Box::leak(Box::new(proto_focus())) as &dyn Operation,
        Box::leak(Box::new(proto_clear_focus())) as &dyn Operation,
        Box::leak(Box::new(proto_navigate())) as &dyn Operation,
        Box::leak(Box::new(proto_focus_lost())) as &dyn Operation,
        Box::leak(Box::new(proto_push_layer())) as &dyn Operation,
        Box::leak(Box::new(proto_pop_layer())) as &dyn Operation,
        Box::leak(Box::new(proto_drill_in())) as &dyn Operation,
        Box::leak(Box::new(proto_drill_out())) as &dyn Operation,
        Box::leak(Box::new(proto_generate_sneak_codes())) as &dyn Operation,
        Box::leak(Box::new(proto_query_geometry())) as &dyn Operation,
        Box::leak(Box::new(proto_query_scope_chain())) as &dyn Operation,
        Box::leak(Box::new(proto_query_focus())) as &dyn Operation,
    ]
});

/// Zero-valued FQM used only to mint operation prototypes for schema gen.
fn empty_fq() -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string("")
}

/// Zero rect used only to mint the `FocusLost` prototype.
fn zero_rect() -> Rect {
    Rect {
        x: Pixels::new(0.0),
        y: Pixels::new(0.0),
        width: Pixels::new(0.0),
        height: Pixels::new(0.0),
    }
}

fn proto_focus() -> Focus {
    Focus {
        fq: empty_fq(),
        snapshot: None,
    }
}

fn proto_clear_focus() -> ClearFocus {
    ClearFocus {
        window: WindowLabel::from_string(""),
    }
}

fn proto_navigate() -> Navigate {
    Navigate {
        direction: Direction::Up,
        focused_fq: None,
        snapshot: None,
        window: None,
    }
}

fn proto_focus_lost() -> FocusLost {
    FocusLost {
        focused_fq: empty_fq(),
        lost_parent_zone: None,
        lost_layer_fq: empty_fq(),
        lost_rect: zero_rect(),
        snapshot: NavSnapshot {
            layer_fq: empty_fq(),
            scopes: Vec::new(),
        },
    }
}

fn proto_push_layer() -> PushLayer {
    PushLayer {
        fq: empty_fq(),
        segment: SegmentMoniker::from_string(""),
        name: LayerName::from_string(""),
        parent: None,
        window: WindowLabel::from_string(""),
    }
}

fn proto_pop_layer() -> PopLayer {
    PopLayer { fq: empty_fq() }
}

fn proto_drill_in() -> DrillIn {
    DrillIn {
        fq: empty_fq(),
        focused_fq: None,
        snapshot: None,
        window: None,
    }
}

fn proto_drill_out() -> DrillOut {
    DrillOut {
        fq: empty_fq(),
        focused_fq: None,
        snapshot: None,
        window: None,
    }
}

fn proto_generate_sneak_codes() -> GenerateSneakCodes {
    GenerateSneakCodes { count: 0 }
}

fn proto_query_geometry() -> QueryGeometry {
    QueryGeometry {
        window: WindowLabel::from_string(""),
    }
}

fn proto_query_scope_chain() -> QueryScopeChain {
    QueryScopeChain {
        window: WindowLabel::from_string(""),
    }
}

fn proto_query_focus() -> QueryFocus {
    QueryFocus {
        window: WindowLabel::from_string(""),
    }
}

/// Get the canonical slice of all focus operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &FOCUS_OPERATIONS
}
