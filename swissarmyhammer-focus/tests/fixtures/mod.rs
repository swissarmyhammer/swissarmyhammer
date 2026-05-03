//! Realistic-app fixture builders for the spatial-nav kernel integration
//! tests.
//!
//! These builders construct a [`SpatialRegistry`] whose shape mirrors what
//! the production React tree mounts at runtime in `kanban-app/ui` —
//! navigation through this registry exercises the same kernel paths the
//! user hits when keyboard-navigating the running app, but without the
//! Tauri runtime, jsdom, or a Playwright browser. The kernel + registry
//! shape is the system under test; consumers swap in a [`BeamNavStrategy`]
//! and assert on returned [`FullyQualifiedMoniker`] values.
//!
//! # Path-monikers identifier model
//!
//! The kernel uses **one** identifier shape per primitive: the
//! [`FullyQualifiedMoniker`]. The path through the focus hierarchy IS
//! the spatial key. The fixture builds FQMs by composing the parent
//! FQM and the consumer's [`SegmentMoniker`] at each level, exactly as
//! the React adapter does at runtime via `FullyQualifiedMonikerContext`.
//!
//! Each entity is exposed by:
//! - its `*_segment` (the [`SegmentMoniker`] that React declared, e.g.
//!   `task:T1A`, `column:TODO`, `field:task:T1A.title`).
//! - its `*_fq` (the canonical path FQM the kernel keys on).
//!
//! Tests that previously called `find_by_moniker(&Moniker::from_string("..."))`
//! migrate by replacing the lookup with `find_by_fq(&app.<entity>_fq())`.
//!
//! [`FullyQualifiedMoniker`]: swissarmyhammer_focus::FullyQualifiedMoniker
//! [`SegmentMoniker`]: swissarmyhammer_focus::SegmentMoniker
//! [`SpatialRegistry`]: swissarmyhammer_focus::SpatialRegistry
//! [`BeamNavStrategy`]: swissarmyhammer_focus::BeamNavStrategy

#![allow(dead_code)] // Some helpers are consumed by tests added in later cards.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FocusLayer, FocusScope, FocusZone, FullyQualifiedMoniker, LayerName, Pixels, Rect,
    SegmentMoniker, SpatialRegistry, WindowLabel,
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

/// Width of the LeftNav sidebar (a vertical icon column on the far
/// left of the viewport). Mirrors the `w-10` Tailwind class on the
/// production `<LeftNav>` (40 px).
pub const LEFT_NAV_WIDTH: f64 = 40.0;
/// Left edge of the columns of content sitting *to the right* of the
/// LeftNav sidebar — the perspective bar, the board, and everything
/// inside them. In production these are flex siblings of `<LeftNav>`
/// inside `ViewsContainer`'s row, so their left edge equals the
/// LeftNav's width.
pub const CONTENT_LEFT: f64 = LEFT_NAV_WIDTH;

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
// Layer + window constants.
// ---------------------------------------------------------------------------

/// Window label for the realistic-app fixture's main window.
pub const MAIN_WINDOW: &str = "main";

/// FQM for the window-root layer. Consumers compose all window-layer
/// scopes by appending segments to this FQM via
/// [`FullyQualifiedMoniker::compose`].
pub fn window_layer_fq() -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::root(&SegmentMoniker::from_string("window"))
}

/// FQM for the inspector layer (a child of the window-root layer).
pub fn inspector_layer_fq() -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::compose(
        &window_layer_fq(),
        &SegmentMoniker::from_string("inspector"),
    )
}

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

/// Build a [`FocusLayer`] with the given role, FQM, and parent FQM.
fn make_layer(
    fq: FullyQualifiedMoniker,
    segment: &str,
    role: &str,
    parent: Option<FullyQualifiedMoniker>,
) -> FocusLayer {
    FocusLayer {
        fq,
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string(role),
        parent,
        window_label: WindowLabel::from_string(MAIN_WINDOW),
        last_focused: None,
    }
}

/// Build a [`FocusZone`] with empty overrides and no `last_focused`.
fn make_zone(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer_fq: FullyQualifiedMoniker,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq,
        parent_zone,
        last_focused: None,
        overrides: HashMap::new(),
    }
}

/// Build a [`FocusScope`] leaf with empty overrides.
fn make_leaf(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer_fq: FullyQualifiedMoniker,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq,
        parent_zone,
        overrides: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Column / card identity helpers — keep production-shape segments
// generated from one place.
// ---------------------------------------------------------------------------

/// Column letters in left-to-right layout order.
pub const COLUMN_LETTERS: &[char] = &['A', 'B', 'C'];

/// Column display names in left-to-right layout order. Aligns with
/// [`COLUMN_LETTERS`] index-by-index — `A` is `TODO`, `B` is `DOING`,
/// `C` is `DONE`.
pub const COLUMN_NAMES: &[&str] = &["TODO", "DOING", "DONE"];

/// Number of cards stacked inside each column.
pub const CARDS_PER_COLUMN: usize = 3;

/// Build the [`SegmentMoniker`] for a card at row `row_one_based` (1-based)
/// in column index `column_index`. Uses the production
/// `task:T<row><letter>` shape.
pub fn card_segment(row_one_based: usize, column_index: usize) -> String {
    let letter = COLUMN_LETTERS[column_index];
    format!("task:T{row_one_based}{letter}")
}

/// Build the [`SegmentMoniker`] for a column zone at index `column_index`.
/// Uses the production `column:<NAME>` shape.
pub fn column_segment(column_index: usize) -> String {
    format!("column:{}", COLUMN_NAMES[column_index])
}

/// Build the [`SegmentMoniker`] for the column-header name field zone
/// at the given index.
///
/// In production this surface is the inner `<Field>` zone rendered by
/// `<ColumnNameField>` — the synthetic outer `<FocusScope
/// moniker="column:<id>.name">` was collapsed in card
/// `01KQAWVDS931PADB0559F2TVCS`. The kernel-side moniker now follows
/// the standard field-zone shape `field:<entityType>:<id>.<name>`.
pub fn column_name_segment(column_index: usize) -> String {
    format!("field:column:{}.name", COLUMN_NAMES[column_index])
}

// ---------------------------------------------------------------------------
// RealisticApp — the assembly.
// ---------------------------------------------------------------------------

/// A populated [`SpatialRegistry`] paired with the FQM helpers tests
/// assert against.
///
/// Construct one with [`RealisticApp::new`]; read its
/// [`registry`](Self::registry) for the kernel under test, or call the
/// `*_fq` helpers below for the [`FullyQualifiedMoniker`] of any
/// pre-registered scope so test code stays path-driven.
pub struct RealisticApp {
    /// The populated [`SpatialRegistry`].
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
    /// FQMs/segments/rects, so two test functions that build the
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

    /// FQM for the navbar zone — `/window/ui:navbar`.
    pub fn navbar_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &window_layer_fq(),
            &SegmentMoniker::from_string("ui:navbar"),
        )
    }

    /// FQM for the LeftNav sidebar zone — `/window/ui:left-nav`.
    ///
    /// Mirrors the production `<LeftNav>` (`kanban-app/ui/src/components/left-nav.tsx`)
    /// which mounts a `<FocusZone moniker="ui:left-nav">` inside
    /// `ViewsContainer` as a flex sibling of `PerspectivesContainer`. In
    /// the kernel registry the LeftNav lives at the layer root with
    /// `parent_zone = None`, peer to `ui:navbar`, `ui:perspective-bar`,
    /// and `ui:board`.
    pub fn left_nav_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &window_layer_fq(),
            &SegmentMoniker::from_string("ui:left-nav"),
        )
    }

    /// FQM for `view:grid`, the leftmost / topmost view-button leaf
    /// inside the LeftNav sidebar.
    ///
    /// Mirrors the production `ScopedViewButton` shape — each view in
    /// `kanban-app/ui/src/components/left-nav.tsx` wraps a single button
    /// in `<FocusScope moniker={asSegment(\`view:${view.id}\`)}>`, so
    /// the leaf segment shape is `view:{id}`.
    pub fn view_button_grid_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.left_nav_fq(),
            &SegmentMoniker::from_string("view:grid"),
        )
    }

    /// FQM for `view:list`, the second view-button leaf stacked beneath
    /// `view:grid` in the LeftNav sidebar.
    pub fn view_button_list_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.left_nav_fq(),
            &SegmentMoniker::from_string("view:list"),
        )
    }

    /// FQM for the perspective-bar zone — `/window/ui:perspective-bar`.
    pub fn perspective_bar_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &window_layer_fq(),
            &SegmentMoniker::from_string("ui:perspective-bar"),
        )
    }

    /// FQM for the board zone — `/window/ui:board`.
    pub fn board_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(&window_layer_fq(), &SegmentMoniker::from_string("ui:board"))
    }

    /// FQM for the column zone at `column_index` —
    /// `/window/ui:board/column:<NAME>`.
    pub fn column_fq(&self, column_index: usize) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.board_fq(),
            &SegmentMoniker::from_string(column_segment(column_index)),
        )
    }

    /// FQM for the column-name field zone at `column_index` —
    /// `/window/ui:board/column:<NAME>/field:column:<NAME>.name`.
    ///
    /// The column-name surface is registered as a `<FocusZone>` (kind
    /// `Zone`) parented at the enclosing column zone, mirroring the
    /// production wiring where `<Field>` is the sole spatial-nav
    /// primitive for a field surface.
    pub fn column_name_fq(&self, column_index: usize) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.column_fq(column_index),
            &SegmentMoniker::from_string(column_name_segment(column_index)),
        )
    }

    /// FQM for the card at row `row_one_based` (1-based) in column
    /// `column_index` —
    /// `/window/ui:board/column:<NAME>/task:T<row><letter>`.
    pub fn card_fq(&self, row_one_based: usize, column_index: usize) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.column_fq(column_index),
            &SegmentMoniker::from_string(card_segment(row_one_based, column_index)),
        )
    }

    /// FQM for `ui:navbar.board-selector`, the leftmost entry inside the
    /// navbar zone.
    pub fn navbar_board_selector_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.navbar_fq(),
            &SegmentMoniker::from_string("ui:navbar.board-selector"),
        )
    }

    /// FQM for `ui:navbar.inspect`.
    pub fn navbar_inspect_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.navbar_fq(),
            &SegmentMoniker::from_string("ui:navbar.inspect"),
        )
    }

    /// FQM for the `field:board:b1.percent_complete` zone — a sibling of
    /// the navbar leaves.
    pub fn navbar_percent_field_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.navbar_fq(),
            &SegmentMoniker::from_string("field:board:b1.percent_complete"),
        )
    }

    /// FQM for `ui:navbar.search`.
    pub fn navbar_search_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.navbar_fq(),
            &SegmentMoniker::from_string("ui:navbar.search"),
        )
    }

    /// FQM for `perspective_tab:p1`, the leftmost perspective tab.
    pub fn perspective_tab_p1_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.perspective_bar_fq(),
            &SegmentMoniker::from_string("perspective_tab:p1"),
        )
    }

    /// FQM for `perspective_tab:p2`, the middle (and visually wider when
    /// active) perspective tab.
    pub fn perspective_tab_p2_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.perspective_bar_fq(),
            &SegmentMoniker::from_string("perspective_tab:p2"),
        )
    }

    /// FQM for `perspective_tab:p3`, the rightmost perspective tab.
    pub fn perspective_tab_p3_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.perspective_bar_fq(),
            &SegmentMoniker::from_string("perspective_tab:p3"),
        )
    }

    /// FQM for the inspector panel zone — `/window/inspector/panel:task:T1A`.
    pub fn inspector_panel_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &inspector_layer_fq(),
            &SegmentMoniker::from_string("panel:task:T1A"),
        )
    }

    /// FQM for the inspector's title field zone.
    pub fn inspector_field_title_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.inspector_panel_fq(),
            &SegmentMoniker::from_string("field:task:T1A.title"),
        )
    }

    /// FQM for the inspector's status field zone.
    pub fn inspector_field_status_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.inspector_panel_fq(),
            &SegmentMoniker::from_string("field:task:T1A.status"),
        )
    }

    /// FQM for the inspector's assignees field zone.
    pub fn inspector_field_assignees_fq(&self) -> FullyQualifiedMoniker {
        FullyQualifiedMoniker::compose(
            &self.inspector_panel_fq(),
            &SegmentMoniker::from_string("field:task:T1A.assignees"),
        )
    }
}

// ---------------------------------------------------------------------------
// Internal builders — one per logical region.
// ---------------------------------------------------------------------------

/// Register the two layers: a window root and an inspector child layer.
fn register_layers(reg: &mut SpatialRegistry) {
    reg.push_layer(make_layer(window_layer_fq(), "window", "window", None));
    reg.push_layer(make_layer(
        inspector_layer_fq(),
        "inspector",
        "inspector",
        Some(window_layer_fq()),
    ));
}

/// Register the navbar, LeftNav sidebar, and perspective-bar zones on
/// the window layer.
///
/// In production (`kanban-app/ui/src/App.tsx` →
/// `ViewsContainer`), the chrome under the navbar lays out as a flex
/// row with `<LeftNav>` to the left of `<PerspectivesContainer>`. The
/// perspective bar and the board both live to the right of LeftNav, so
/// their left edge starts at [`CONTENT_LEFT`] (= [`LEFT_NAV_WIDTH`])
/// rather than at `0`. The fixture mirrors that geometry so cardinal
/// `Left` from the leftmost perspective tab has a real LeftNav zone to
/// its left in the kernel's beam search.
fn register_window_chrome(reg: &mut SpatialRegistry) {
    let win = window_layer_fq();
    let navbar_fq = FullyQualifiedMoniker::compose(&win, &SegmentMoniker::from_string("ui:navbar"));

    // ui:navbar — full-width strip across the top.
    reg.register_zone(make_zone(
        navbar_fq.clone(),
        "ui:navbar",
        win.clone(),
        None,
        rect(0.0, 0.0, VIEWPORT_WIDTH, NAVBAR_HEIGHT),
    ));
    // Navbar entries laid out left-to-right inside the navbar zone.
    reg.register_scope(make_leaf(
        FullyQualifiedMoniker::compose(
            &navbar_fq,
            &SegmentMoniker::from_string("ui:navbar.board-selector"),
        ),
        "ui:navbar.board-selector",
        win.clone(),
        Some(navbar_fq.clone()),
        rect(8.0, 8.0, 200.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        FullyQualifiedMoniker::compose(
            &navbar_fq,
            &SegmentMoniker::from_string("ui:navbar.inspect"),
        ),
        "ui:navbar.inspect",
        win.clone(),
        Some(navbar_fq.clone()),
        rect(216.0, 8.0, 80.0, 24.0),
    ));
    // Percent-complete field zone — a `<FocusZone>` peer of the navbar
    // leaves.
    reg.register_zone(make_zone(
        FullyQualifiedMoniker::compose(
            &navbar_fq,
            &SegmentMoniker::from_string("field:board:b1.percent_complete"),
        ),
        "field:board:b1.percent_complete",
        win.clone(),
        Some(navbar_fq.clone()),
        rect(304.0, 8.0, 200.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        FullyQualifiedMoniker::compose(
            &navbar_fq,
            &SegmentMoniker::from_string("ui:navbar.search"),
        ),
        "ui:navbar.search",
        win.clone(),
        Some(navbar_fq),
        rect(VIEWPORT_WIDTH - 200.0, 8.0, 192.0, 24.0),
    ));

    // ui:left-nav — narrow vertical sidebar on the far left, spanning
    // from below the navbar to the bottom of the viewport. Mirrors
    // production: a flex sibling of `<PerspectivesContainer>` inside
    // `ViewsContainer`'s row, with the production `w-10` Tailwind class
    // (40 px) captured by [`LEFT_NAV_WIDTH`].
    let left_nav_fq =
        FullyQualifiedMoniker::compose(&win, &SegmentMoniker::from_string("ui:left-nav"));
    reg.register_zone(make_zone(
        left_nav_fq.clone(),
        "ui:left-nav",
        win.clone(),
        None,
        rect(
            0.0,
            NAVBAR_HEIGHT,
            LEFT_NAV_WIDTH,
            VIEWPORT_HEIGHT - NAVBAR_HEIGHT,
        ),
    ));
    // Two view-button leaves stacked vertically inside the LeftNav.
    // Mirrors `ScopedViewButton` in production — each declares
    // `<FocusScope moniker={asSegment("view:" + view.id)}>`.
    reg.register_scope(make_leaf(
        FullyQualifiedMoniker::compose(
            &left_nav_fq,
            &SegmentMoniker::from_string("view:grid"),
        ),
        "view:grid",
        win.clone(),
        Some(left_nav_fq.clone()),
        rect(4.0, NAVBAR_HEIGHT + 8.0, LEFT_NAV_WIDTH - 8.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        FullyQualifiedMoniker::compose(
            &left_nav_fq,
            &SegmentMoniker::from_string("view:list"),
        ),
        "view:list",
        win.clone(),
        Some(left_nav_fq),
        rect(4.0, NAVBAR_HEIGHT + 36.0, LEFT_NAV_WIDTH - 8.0, 24.0),
    ));

    // ui:perspective-bar — strip directly below the navbar, sitting to
    // the right of `ui:left-nav`. Width = viewport minus the LeftNav.
    let pbar_fq =
        FullyQualifiedMoniker::compose(&win, &SegmentMoniker::from_string("ui:perspective-bar"));
    reg.register_zone(make_zone(
        pbar_fq.clone(),
        "ui:perspective-bar",
        win.clone(),
        None,
        rect(
            CONTENT_LEFT,
            NAVBAR_HEIGHT,
            VIEWPORT_WIDTH - CONTENT_LEFT,
            PERSPECTIVE_BAR_HEIGHT,
        ),
    ));
    // Three perspective tab leaves laid out left-to-right inside the
    // perspective bar. Layout: 8 px left padding inside the bar (so 8 px
    // from `CONTENT_LEFT` in absolute coords), then p1 (96 px) + 8 px
    // gap + p2 (160 px, wider for active chrome) + 8 px gap + p3 (96
    // px).
    reg.register_scope(make_leaf(
        FullyQualifiedMoniker::compose(
            &pbar_fq,
            &SegmentMoniker::from_string("perspective_tab:p1"),
        ),
        "perspective_tab:p1",
        win.clone(),
        Some(pbar_fq.clone()),
        rect(CONTENT_LEFT + 8.0, NAVBAR_HEIGHT + 8.0, 96.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        FullyQualifiedMoniker::compose(
            &pbar_fq,
            &SegmentMoniker::from_string("perspective_tab:p2"),
        ),
        "perspective_tab:p2",
        win.clone(),
        Some(pbar_fq.clone()),
        rect(CONTENT_LEFT + 112.0, NAVBAR_HEIGHT + 8.0, 160.0, 24.0),
    ));
    reg.register_scope(make_leaf(
        FullyQualifiedMoniker::compose(
            &pbar_fq,
            &SegmentMoniker::from_string("perspective_tab:p3"),
        ),
        "perspective_tab:p3",
        win,
        Some(pbar_fq),
        rect(CONTENT_LEFT + 280.0, NAVBAR_HEIGHT + 8.0, 96.0, 24.0),
    ));
}

/// Register the board zone and the three columns inside it.
///
/// Like the perspective bar, the board sits to the right of
/// `ui:left-nav` in production — its left edge starts at
/// [`CONTENT_LEFT`] rather than `0`, and the columns/cards are
/// positioned relative to that edge.
fn register_board_and_columns(reg: &mut SpatialRegistry) {
    let win = window_layer_fq();
    let board_fq = FullyQualifiedMoniker::compose(&win, &SegmentMoniker::from_string("ui:board"));
    reg.register_zone(make_zone(
        board_fq.clone(),
        "ui:board",
        win.clone(),
        None,
        rect(
            CONTENT_LEFT,
            BOARD_TOP,
            VIEWPORT_WIDTH - CONTENT_LEFT,
            VIEWPORT_HEIGHT - BOARD_TOP,
        ),
    ));

    for (i, _name) in COLUMN_NAMES.iter().enumerate() {
        let col_x = CONTENT_LEFT + (i as f64) * COLUMN_WIDTH;
        let col_seg = column_segment(i);
        let col_fq = FullyQualifiedMoniker::compose(
            &board_fq,
            &SegmentMoniker::from_string(col_seg.clone()),
        );

        // Column zone — child of the board.
        reg.register_zone(make_zone(
            col_fq.clone(),
            &col_seg,
            win.clone(),
            Some(board_fq.clone()),
            rect(col_x, BOARD_TOP, COLUMN_WIDTH, VIEWPORT_HEIGHT - BOARD_TOP),
        ));

        // Column-name field zone — top of the column. Registered as a
        // `<FocusZone>` (kind `Zone`) with moniker
        // `field:column:<NAME>.name`, mirroring the production wiring
        // where `<Field>` is the sole spatial-nav primitive for a field
        // surface (the synthetic outer `<FocusScope>` was collapsed in
        // card 01KQAWVDS931PADB0559F2TVCS).
        let col_name_seg = column_name_segment(i);
        reg.register_zone(make_zone(
            FullyQualifiedMoniker::compose(
                &col_fq,
                &SegmentMoniker::from_string(col_name_seg.clone()),
            ),
            &col_name_seg,
            win.clone(),
            Some(col_fq.clone()),
            rect(col_x + 8.0, BOARD_TOP + 4.0, COLUMN_WIDTH - 16.0, 32.0),
        ));

        // Card leaves — stacked vertically beneath the header.
        for row in 1..=CARDS_PER_COLUMN {
            let row_index = row - 1;
            let card_top = FIRST_CARD_TOP + (row_index as f64) * CARD_HEIGHT;
            let card_seg = card_segment(row, i);
            reg.register_scope(make_leaf(
                FullyQualifiedMoniker::compose(
                    &col_fq,
                    &SegmentMoniker::from_string(card_seg.clone()),
                ),
                &card_seg,
                win.clone(),
                Some(col_fq.clone()),
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
fn register_inspector(reg: &mut SpatialRegistry) {
    let inspector = inspector_layer_fq();
    let inspector_x = VIEWPORT_WIDTH - 400.0;
    let panel_fq =
        FullyQualifiedMoniker::compose(&inspector, &SegmentMoniker::from_string("panel:task:T1A"));
    reg.register_zone(make_zone(
        panel_fq.clone(),
        "panel:task:T1A",
        inspector.clone(),
        None,
        rect(inspector_x, BOARD_TOP, 400.0, VIEWPORT_HEIGHT - BOARD_TOP),
    ));

    // Field rows stack vertically inside the panel.
    let field_specs: &[&str] = &[
        "field:task:T1A.title",
        "field:task:T1A.status",
        "field:task:T1A.assignees",
    ];
    for (i, segment) in field_specs.iter().enumerate() {
        let row_top = BOARD_TOP + 8.0 + (i as f64) * 56.0;
        reg.register_zone(make_zone(
            FullyQualifiedMoniker::compose(&panel_fq, &SegmentMoniker::from_string(*segment)),
            segment,
            inspector.clone(),
            Some(panel_fq.clone()),
            rect(inspector_x + 8.0, row_top, 384.0, 48.0),
        ));
    }
}
