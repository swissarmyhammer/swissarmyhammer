//! Realistic-app fixture builders for the spatial-nav kernel integration
//! tests.
//!
//! These builders construct a [`SpatialRegistry`] whose shape mirrors what
//! the production React tree mounts at runtime in `kanban-app/ui` —
//! navigation through this registry exercises the same kernel paths the
//! user hits when keyboard-navigating the running app, but without the
//! Tauri runtime, jsdom, or a Playwright browser. The kernel + registry
//! shape is the system under test; consumers swap in a [`BeamNavStrategy`]
//! and assert on returned [`Moniker`] values.
//!
//! # Why this lives here
//!
//! The directional-nav card [`01KQ7STZN3G5N2WB3FF4PM4DKX`] explicitly
//! relocated the source-of-truth tests for card-level nav from the React
//! browser-mode harness into Rust integration tests. The earlier per-
//! direction browser tests (`board-view.cross-column-nav.spatial.test.tsx`)
//! built a JS shadow registry that mimicked the kernel — which let
//! algorithmic bugs through whenever the JS port disagreed with the Rust
//! implementation. Building the realistic state in Rust and calling the
//! actual kernel removes the mimicry layer.
//!
//! The unified-policy card [`01KQ7S6WHK9RCCG2R4FN474EFD`] also depends on
//! this shape, so the fixture is shared rather than duplicated per test
//! file. Each integration test pulls it in via the standard Rust
//! `tests/<module>/mod.rs` shared-module pattern (`mod fixtures;` at the
//! top of the integration `.rs`).
//!
//! # Layout philosophy
//!
//! Geometrically realistic rectangles drive Android-beam scoring; if the
//! fixture's rects are wrong the tests pass against synthetic state that
//! diverges from production. Three columns sit side-by-side under the
//! board; each column holds a name-leaf header followed by cards stacked
//! vertically. The dimensions match the kanban-app's typical desktop
//! layout (~440 px-wide columns, ~80 px-tall cards) so beam-search runs
//! against the same scale the user sees.
//!
//! # Stable monikers
//!
//! Every entity in the fixture has a deterministic moniker that the
//! tests can read back by name:
//!
//! - Window layer chrome: `ui:navbar`, `ui:navbar.<button>`,
//!   `field:board:b1.percent_complete` (a `<FocusZone>` sibling of the
//!   navbar leaves), `ui:perspective-bar`, `ui:perspective-bar.<button>`,
//!   `ui:board`.
//! - Columns: `column:TODO`, `column:DOING`, `column:DONE` for the zone;
//!   `column:TODO.name`, `column:DOING.name`, `column:DONE.name` for the
//!   header leaf.
//! - Cards: `task:T1A`, `task:T2A`, …, `task:T3C`. The first digit is the
//!   row position (1 = top, 3 = bottom); the letter is the column
//!   (A = TODO, B = DOING, C = DONE).
//! - Inspector layer: `panel:task:T1A` for the panel zone, `field:task:T1A.title`,
//!   `field:task:T1A.status` for the field-row zones inside.
//!
//! [`01KQ7STZN3G5N2WB3FF4PM4DKX`]: # "directional-nav supersession card"
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy card"
//! [`SpatialRegistry`]: swissarmyhammer_focus::SpatialRegistry
//! [`BeamNavStrategy`]: swissarmyhammer_focus::BeamNavStrategy
//! [`Moniker`]: swissarmyhammer_focus::Moniker

#![allow(dead_code)] // Some helpers are consumed by tests added in later cards.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FocusLayer, FocusScope, FocusZone, LayerKey, LayerName, Moniker, Pixels, Rect, SpatialKey,
    SpatialRegistry, WindowLabel,
};

// ---------------------------------------------------------------------------
// Layout constants — chosen to match production scale.
// ---------------------------------------------------------------------------

/// Total viewport width in pixels. 1400 px matches the desktop layout
/// `board-view.cross-column-nav.spatial.test.tsx` pins for the React
/// browser tests, so the kernel-level fixture exercises the same scale.
pub const VIEWPORT_WIDTH: f64 = 1400.0;
/// Total viewport height in pixels.
pub const VIEWPORT_HEIGHT: f64 = 900.0;

/// Height of the navbar strip at the top of the window.
pub const NAVBAR_HEIGHT: f64 = 40.0;
/// Height of the perspective bar directly below the navbar.
pub const PERSPECTIVE_BAR_HEIGHT: f64 = 40.0;
/// Top edge of the board area (after navbar + perspective bar).
pub const BOARD_TOP: f64 = NAVBAR_HEIGHT + PERSPECTIVE_BAR_HEIGHT;

/// Width of one column. Matches the lower bound of the production
/// `min-w-[24em]` (24em ≈ 384 px) bumped to a round 440 px so three
/// columns at 440 px each plus margin stay inside a 1400 px viewport.
pub const COLUMN_WIDTH: f64 = 440.0;
/// Height of the column-name leaf in a column header.
pub const COLUMN_HEADER_HEIGHT: f64 = 40.0;
/// Vertical extent of a single card.
pub const CARD_HEIGHT: f64 = 80.0;
/// Top edge of the first card in any column (just below the header).
pub const FIRST_CARD_TOP: f64 = BOARD_TOP + COLUMN_HEADER_HEIGHT;

// ---------------------------------------------------------------------------
// Layer + window constants — names that appear in monikers.
// ---------------------------------------------------------------------------

/// Window label for the realistic-app fixture's main window.
pub const MAIN_WINDOW: &str = "main";
/// Spatial key used for the window-root layer.
pub const WINDOW_LAYER_KEY: &str = "L_window";
/// Spatial key used for the inspector layer (child of the window layer).
pub const INSPECTOR_LAYER_KEY: &str = "L_inspector";

// ---------------------------------------------------------------------------
// Small constructors — keep the assembly in `RealisticApp` readable.
// ---------------------------------------------------------------------------

/// Build a [`Rect`] from raw `f64` coordinates. Mirrors the helper the
/// existing `tests/navigate.rs` uses so the fixture's rect math reads the
/// same way.
fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(width),
        height: Pixels::new(height),
    }
}

/// Build a [`FocusLayer`] with the given role and parent.
fn make_layer(key: &str, role: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        key: LayerKey::from_string(key),
        name: LayerName::from_string(role),
        parent: parent.map(LayerKey::from_string),
        window_label: WindowLabel::from_string(MAIN_WINDOW),
        last_focused: None,
    }
}

/// Build a [`FocusZone`] with empty overrides and no `last_focused`. The
/// directional-nav tests do not exercise the override or memory paths;
/// that coverage lives in [`tests/overrides.rs`] and [`tests/fallback.rs`].
fn make_zone(
    key: &str,
    moniker: &str,
    layer_key: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer_key),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        last_focused: None,
        overrides: HashMap::new(),
    }
}

/// Build a [`FocusScope`] leaf with empty overrides.
fn make_leaf(
    key: &str,
    moniker: &str,
    layer_key: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer_key),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Column / card identity helpers — keep production-shape monikers
// generated from one place.
// ---------------------------------------------------------------------------

/// Column letters in left-to-right layout order.
///
/// Index `i` corresponds to `COLUMN_NAMES[i]`. Tests reference cards by
/// `task:T<row><letter>` and columns by `column:<NAME>` — both halves
/// derive from this single ordered list, so changing the order here is
/// the only edit needed to swap, drop, or add a column.
pub const COLUMN_LETTERS: &[char] = &['A', 'B', 'C'];

/// Column display names in left-to-right layout order. Aligns with
/// [`COLUMN_LETTERS`] index-by-index — `A` is `TODO`, `B` is `DOING`,
/// `C` is `DONE`.
pub const COLUMN_NAMES: &[&str] = &["TODO", "DOING", "DONE"];

/// Number of cards stacked inside each column. Three is the minimum that
/// exercises both adjacent-card moves (rows 1↔2, 2↔3) and a non-adjacent
/// boundary (row 3 has nothing below).
pub const CARDS_PER_COLUMN: usize = 3;

/// Build the moniker for a card at row `row_one_based` in column index
/// `column_index`. Uses the production `task:T<row><letter>` shape.
pub fn card_moniker(row_one_based: usize, column_index: usize) -> String {
    let letter = COLUMN_LETTERS[column_index];
    format!("task:T{row_one_based}{letter}")
}

/// Build the spatial key for a card at row `row_one_based` in column
/// index `column_index`. The key matches the moniker so test setup and
/// tear-down can use either as a stable handle.
pub fn card_key(row_one_based: usize, column_index: usize) -> String {
    let letter = COLUMN_LETTERS[column_index];
    format!("k_task_T{row_one_based}{letter}")
}

/// Build the moniker for a column zone at index `column_index`. Uses the
/// production `column:<NAME>` shape.
pub fn column_moniker(column_index: usize) -> String {
    format!("column:{}", COLUMN_NAMES[column_index])
}

/// Build the spatial key for a column zone.
pub fn column_key(column_index: usize) -> String {
    format!("k_column_{}", COLUMN_NAMES[column_index])
}

/// Build the moniker for a column-header name leaf. Uses the production
/// `column:<NAME>.name` shape.
pub fn column_name_moniker(column_index: usize) -> String {
    format!("column:{}.name", COLUMN_NAMES[column_index])
}

/// Build the spatial key for a column-header name leaf.
pub fn column_name_key(column_index: usize) -> String {
    format!("k_column_{}_name", COLUMN_NAMES[column_index])
}

// ---------------------------------------------------------------------------
// RealisticApp — the assembly.
// ---------------------------------------------------------------------------

/// A populated [`SpatialRegistry`] paired with the moniker constants the
/// tests assert against.
///
/// Construct one with [`RealisticApp::new`]; read its
/// [`registry`](Self::registry) for the kernel under test, or call the
/// `*_key` helpers below for the [`SpatialKey`] of any pre-registered
/// scope so test code stays moniker-driven.
pub struct RealisticApp {
    /// The populated [`SpatialRegistry`]. Move into a navigator strategy
    /// or borrow for test assertions.
    pub registry: SpatialRegistry,
}

impl Default for RealisticApp {
    fn default() -> Self {
        Self::new()
    }
}

impl RealisticApp {
    /// Build the registry with both layers, the window-layer chrome
    /// zones, the three columns × three cards, and the inspector layer's
    /// panel + field-row zones.
    ///
    /// The build is deterministic — every call produces the same
    /// keys/monikers/rects, so two test functions that build the
    /// fixture independently land on identical state.
    pub fn new() -> Self {
        let mut registry = SpatialRegistry::new();
        register_layers(&mut registry);
        register_window_chrome(&mut registry);
        register_board_and_columns(&mut registry);
        register_inspector(&mut registry);
        Self { registry }
    }

    /// Borrow the [`SpatialRegistry`] for navigator calls.
    pub fn registry(&self) -> &SpatialRegistry {
        &self.registry
    }

    /// [`SpatialKey`] for the card at row `row_one_based` (1-based) in
    /// column index `column_index` (0 = TODO, 1 = DOING, 2 = DONE).
    pub fn card_key(&self, row_one_based: usize, column_index: usize) -> SpatialKey {
        SpatialKey::from_string(card_key(row_one_based, column_index))
    }

    /// [`SpatialKey`] for the column-header name leaf at the given index.
    pub fn column_name_key(&self, column_index: usize) -> SpatialKey {
        SpatialKey::from_string(column_name_key(column_index))
    }

    /// [`SpatialKey`] for the column zone at the given index.
    pub fn column_key(&self, column_index: usize) -> SpatialKey {
        SpatialKey::from_string(column_key(column_index))
    }

    /// [`SpatialKey`] for the `ui:navbar.board-selector` leaf, the
    /// leftmost entry inside the navbar zone.
    pub fn navbar_board_selector_key(&self) -> SpatialKey {
        SpatialKey::from_string("k_ui_navbar_board_selector")
    }

    /// [`SpatialKey`] for the `ui:navbar.inspect` leaf, the second
    /// entry inside the navbar zone (between board-selector and the
    /// percent-complete field zone).
    pub fn navbar_inspect_key(&self) -> SpatialKey {
        SpatialKey::from_string("k_ui_navbar_inspect")
    }

    /// [`SpatialKey`] for the `field:board:b1.percent_complete` zone,
    /// a sibling of the navbar leaves whose `parent_zone` is the
    /// navbar. Production renders this as `<Field>` which itself is a
    /// `<FocusZone>` — see `kanban-app/ui/src/components/fields/field.tsx`.
    pub fn navbar_percent_field_key(&self) -> SpatialKey {
        SpatialKey::from_string("k_field_board_b1_percent_complete")
    }

    /// [`SpatialKey`] for the `ui:navbar.search` leaf, the rightmost
    /// entry inside the navbar zone.
    pub fn navbar_search_key(&self) -> SpatialKey {
        SpatialKey::from_string("k_ui_navbar_search")
    }

    /// [`SpatialKey`] for the `perspective_tab:p1` leaf, the leftmost
    /// perspective tab inside the `ui:perspective-bar` zone.
    pub fn perspective_tab_p1_key(&self) -> SpatialKey {
        SpatialKey::from_string("k_perspective_tab_p1")
    }

    /// [`SpatialKey`] for the `perspective_tab:p2` leaf — the middle
    /// perspective tab. Production widens this leaf when the tab is
    /// active because it renders extra inline chrome
    /// ([`FilterFocusButton`] + [`GroupPopoverButton`]) inside the same
    /// `<FocusScope>` wrapper. The fixture mirrors that by giving p2 a
    /// wider rect than p1 / p3 so beam-search runs against the same
    /// rect-growth pattern the user produces by clicking p2.
    pub fn perspective_tab_p2_key(&self) -> SpatialKey {
        SpatialKey::from_string("k_perspective_tab_p2")
    }

    /// [`SpatialKey`] for the `perspective_tab:p3` leaf, the rightmost
    /// perspective tab inside the `ui:perspective-bar` zone.
    pub fn perspective_tab_p3_key(&self) -> SpatialKey {
        SpatialKey::from_string("k_perspective_tab_p3")
    }
}

// ---------------------------------------------------------------------------
// Internal builders — one per logical region.
// ---------------------------------------------------------------------------

/// Register the two layers: a window root and an inspector child layer.
///
/// The inspector layer is included even though the directional-nav card
/// asserts only on window-layer trajectories — having a second layer in
/// the fixture surfaces any kernel rule that accidentally crosses the
/// layer boundary (the absolute boundary contract from
/// `tests/navigate.rs::nav_never_crosses_layer_boundary_within_one_window`).
fn register_layers(reg: &mut SpatialRegistry) {
    reg.push_layer(make_layer(WINDOW_LAYER_KEY, "window", None));
    reg.push_layer(make_layer(
        INSPECTOR_LAYER_KEY,
        "inspector",
        Some(WINDOW_LAYER_KEY),
    ));
}

/// Register the navbar and perspective-bar zones on the window layer.
///
/// Each chrome zone gets a small handful of leaf children so tests that
/// navigate from a non-card focused entry (the unified-policy card) have
/// realistic neighbours to land on. The directional-nav card never
/// focuses these directly, but their presence pressures the kernel's
/// rule-2 cross-zone fallback to consider them as candidates.
///
/// Inside `ui:navbar` the production layout from `nav-bar.tsx` is, left
/// to right: board-selector leaf → inspect leaf → percent-complete
/// **field zone** → search leaf. The percent-complete field is itself a
/// `<FocusZone>` (its `parent_zone` is `ui:navbar`), making it a sibling
/// of the navbar leaves at the kernel level even though visually it sits
/// in the same horizontal strip. Tests for navbar arrow-nav use this
/// mixed-kind sibling layout to verify the in-zone beam search treats
/// each entry as a peer regardless of leaf-vs-zone kind.
fn register_window_chrome(reg: &mut SpatialRegistry) {
    // ui:navbar — full-width strip across the top.
    reg.register_zone(make_zone(
        "k_ui_navbar",
        "ui:navbar",
        WINDOW_LAYER_KEY,
        None,
        rect(0.0, 0.0, VIEWPORT_WIDTH, NAVBAR_HEIGHT),
    ));
    // Navbar entries laid out left-to-right inside the navbar zone.
    // Three leaves and one field zone, mirroring the production layout
    // from `kanban-app/ui/src/components/nav-bar.tsx`.
    reg.register_scope(make_leaf(
        "k_ui_navbar_board_selector",
        "ui:navbar.board-selector",
        WINDOW_LAYER_KEY,
        Some("k_ui_navbar"),
        rect(8.0, 8.0, 200.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        "k_ui_navbar_inspect",
        "ui:navbar.inspect",
        WINDOW_LAYER_KEY,
        Some("k_ui_navbar"),
        rect(216.0, 8.0, 80.0, 24.0),
    ));
    // Percent-complete field zone — a `<FocusZone>` peer of the navbar
    // leaves. Its `parent_zone` is `ui:navbar` so beam search inside the
    // navbar treats it as a sibling.
    reg.register_zone(make_zone(
        "k_field_board_b1_percent_complete",
        "field:board:b1.percent_complete",
        WINDOW_LAYER_KEY,
        Some("k_ui_navbar"),
        rect(304.0, 8.0, 200.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        "k_ui_navbar_search",
        "ui:navbar.search",
        WINDOW_LAYER_KEY,
        Some("k_ui_navbar"),
        rect(VIEWPORT_WIDTH - 200.0, 8.0, 192.0, 24.0),
    ));

    // ui:perspective-bar — full-width strip directly below the navbar.
    reg.register_zone(make_zone(
        "k_ui_perspective_bar",
        "ui:perspective-bar",
        WINDOW_LAYER_KEY,
        None,
        rect(0.0, NAVBAR_HEIGHT, VIEWPORT_WIDTH, PERSPECTIVE_BAR_HEIGHT),
    ));
    // Three perspective tab leaves laid out left-to-right inside the
    // perspective bar. Monikers match the production shape
    // (`perspective_tab:{id}` from `kanban-app/ui/src/components/perspective-tab-bar.tsx`).
    //
    // The middle tab (p2) is wider than the flanking tabs so the
    // fixture mirrors the production active-tab rect growth: the active
    // perspective renders inline `<FilterFocusButton>` and
    // `<GroupPopoverButton>` siblings inside the same `<FocusScope>`
    // wrapper, growing the leaf's bounding rect. Beam search must still
    // pick the next tab to the right by left-edge ordering regardless
    // of the focused tab's width — that contract is pinned by the
    // `perspective_right_from_middle_active_tab_lands_on_rightmost_tab`
    // case in `tests/perspective_bar_arrow_nav.rs`.
    //
    // Layout: 8 px left padding, then p1 (96 px) + 8 px gap +
    // p2 (160 px, wider for active chrome) + 8 px gap + p3 (96 px).
    reg.register_scope(make_leaf(
        "k_perspective_tab_p1",
        "perspective_tab:p1",
        WINDOW_LAYER_KEY,
        Some("k_ui_perspective_bar"),
        rect(8.0, NAVBAR_HEIGHT + 8.0, 96.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        "k_perspective_tab_p2",
        "perspective_tab:p2",
        WINDOW_LAYER_KEY,
        Some("k_ui_perspective_bar"),
        rect(112.0, NAVBAR_HEIGHT + 8.0, 160.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        "k_perspective_tab_p3",
        "perspective_tab:p3",
        WINDOW_LAYER_KEY,
        Some("k_ui_perspective_bar"),
        rect(280.0, NAVBAR_HEIGHT + 8.0, 96.0, 24.0),
    ));
}

/// Register the board zone and the three columns inside it.
///
/// The board zone is the rectangle below the chrome bars; each column is
/// a child zone laid out horizontally; each column owns a column-name
/// leaf at its top and three card leaves stacked vertically beneath it.
/// This is the shape `column-view.tsx` and `board-view.tsx` produce at
/// runtime — see the `BoardSpatialZone` doc comment in `board-view.tsx`
/// for the production wiring.
fn register_board_and_columns(reg: &mut SpatialRegistry) {
    reg.register_zone(make_zone(
        "k_ui_board",
        "ui:board",
        WINDOW_LAYER_KEY,
        None,
        rect(0.0, BOARD_TOP, VIEWPORT_WIDTH, VIEWPORT_HEIGHT - BOARD_TOP),
    ));

    for (i, _name) in COLUMN_NAMES.iter().enumerate() {
        let col_x = (i as f64) * COLUMN_WIDTH;
        let col_key = column_key(i);
        let col_moniker = column_moniker(i);

        // Column zone — child of the board.
        reg.register_zone(make_zone(
            &col_key,
            &col_moniker,
            WINDOW_LAYER_KEY,
            Some("k_ui_board"),
            rect(col_x, BOARD_TOP, COLUMN_WIDTH, VIEWPORT_HEIGHT - BOARD_TOP),
        ));

        // Column-name leaf — top of the column.
        reg.register_scope(make_leaf(
            &column_name_key(i),
            &column_name_moniker(i),
            WINDOW_LAYER_KEY,
            Some(&col_key),
            rect(col_x + 8.0, BOARD_TOP + 4.0, COLUMN_WIDTH - 16.0, 32.0),
        ));

        // Card leaves — stacked vertically beneath the header.
        for row in 1..=CARDS_PER_COLUMN {
            let row_index = row - 1;
            let card_top = FIRST_CARD_TOP + (row_index as f64) * CARD_HEIGHT;
            reg.register_scope(make_leaf(
                &card_key(row, i),
                &card_moniker(row, i),
                WINDOW_LAYER_KEY,
                Some(&col_key),
                rect(
                    col_x + 8.0,
                    card_top,
                    COLUMN_WIDTH - 16.0,
                    CARD_HEIGHT - 8.0,
                ),
            ));
        }
    }
}

/// Register a panel zone on the inspector layer with three field-row
/// zones stacked inside it.
///
/// The directional-nav card does not focus the inspector — but the
/// fixture is shared with the unified-policy card which does, and even
/// the directional-nav card benefits from a cross-layer second layer
/// because it ensures none of its assertions accidentally pass by
/// reaching across the layer boundary. The panel is parented to the
/// inspector layer (not the window layer); the field rows are parented
/// to the panel.
///
/// Three rows (title, status, assignees) so the unified-policy card's
/// trajectory D (`field:title → field:status → field:assignees → None`)
/// has the full chain to walk.
fn register_inspector(reg: &mut SpatialRegistry) {
    // Inspector docks on the right side of the viewport — width 400 px.
    let inspector_x = VIEWPORT_WIDTH - 400.0;
    reg.register_zone(make_zone(
        "k_panel_t1a",
        "panel:task:T1A",
        INSPECTOR_LAYER_KEY,
        None,
        rect(inspector_x, BOARD_TOP, 400.0, VIEWPORT_HEIGHT - BOARD_TOP),
    ));

    // Field rows stack vertically inside the panel. Each row is 48 px
    // tall with no margin between rows so beam-search distance math
    // mirrors the production inspector pane layout.
    let field_specs: &[(&str, &str)] = &[
        ("k_field_t1a_title", "field:task:T1A.title"),
        ("k_field_t1a_status", "field:task:T1A.status"),
        ("k_field_t1a_assignees", "field:task:T1A.assignees"),
    ];
    for (i, (key, moniker)) in field_specs.iter().enumerate() {
        let row_top = BOARD_TOP + 8.0 + (i as f64) * 56.0;
        reg.register_zone(make_zone(
            key,
            moniker,
            INSPECTOR_LAYER_KEY,
            Some("k_panel_t1a"),
            rect(inspector_x + 8.0, row_top, 384.0, 48.0),
        ));
    }
}
