//! Source-of-truth integration tests for `DismissCmd::execute` against a
//! realistic [`CommandContext`] + [`UIState`] populated with palette /
//! inspector state.
//!
//! These tests pin the **layered close** contract that the React-side
//! Escape chain (`nav.drillOut` → `app.dismiss` →
//! `DismissCmd::execute`) ultimately reaches: when the React adapter
//! falls through to `app.dismiss` because drill-out returned `None` at
//! a layer-root scope, the dismiss command is what actually closes the
//! topmost modal layer. The chain is brittle if the dismiss command
//! itself misbehaves, so its full matrix of preconditions
//! (`palette_open`, `inspector_stack` length, scope-chain window label)
//! is exercised here in addition to the unit-level coverage that lives
//! alongside `register_commands` in `swissarmyhammer-kanban/src/commands/mod.rs`.
//!
//! # Why this file complements the existing inline tests
//!
//! The inline `tests` module in `commands/mod.rs` already covers the
//! happy paths of `dismiss_closes_palette_when_open`,
//! `dismiss_closes_inspector_when_palette_closed`, and
//! `dismiss_returns_null_when_nothing_to_dismiss`. The bug card
//! [`01KQ9TVZYXN65JHA479D1CS91T`] explicitly asks for a separate
//! integration file pinning the **multi-panel** and **palette-shadows-
//! inspector** matrix because those branches are the dismiss-chain seams
//! the user-reported bug ("Escape doesn't close the inspector") brushes
//! against. Putting them in a top-level `tests/` file keeps the matrix
//! visible at the crate boundary alongside the other dispatch
//! integration tests (`command_dispatch_integration.rs`,
//! `dispatch_move_placement.rs`).
//!
//! [`CommandContext`]: swissarmyhammer_commands::CommandContext
//! [`UIState`]: swissarmyhammer_commands::UIState
//! [`01KQ9TVZYXN65JHA479D1CS91T`]: # "Escape does not close the inspector"

use std::collections::HashMap;
use std::sync::Arc;

use swissarmyhammer_commands::{CommandContext, UIState};
use swissarmyhammer_kanban::commands::register_commands;

/// Build a minimal [`CommandContext`] for `app.dismiss` carrying the
/// supplied scope chain and a shared [`UIState`].
///
/// `app.dismiss` does not use a target or args, but it does read the
/// scope chain (to derive the window label via
/// [`CommandContext::window_label_from_scope`]) and the [`UIState`]
/// (to read `palette_open` and `inspector_stack`). Centralising the
/// build keeps each test focused on the *state* being asserted.
fn dismiss_ctx(scope: &[&str], ui: Arc<UIState>) -> CommandContext {
    let mut ctx = CommandContext::new(
        "app.dismiss",
        scope.iter().map(|s| s.to_string()).collect(),
        None,
        HashMap::new(),
    );
    ctx.ui_state = Some(ui);
    ctx
}

// ---------------------------------------------------------------------------
// Single-panel dismiss
// ---------------------------------------------------------------------------

/// One inspector panel open, palette closed → `app.dismiss` pops the
/// panel and `inspector_stack` becomes empty.
///
/// This is the canonical "Escape closes the inspector" path the user
/// expects. The React adapter's `nav.drillOut` returns `None` when
/// focus reaches the panel zone (a layer-root scope), then the closure
/// dispatches `app.dismiss` which lands here.
#[tokio::test]
async fn dismiss_with_inspector_open_pops_topmost_panel() {
    let cmds = register_commands();
    let cmd = cmds.get("app.dismiss").expect("app.dismiss registered");

    let ui = Arc::new(UIState::new());
    ui.inspect("main", "task:01XYZ");
    assert_eq!(ui.inspector_stack("main"), vec!["task:01XYZ"]);
    assert!(!ui.palette_open("main"));

    let ctx = dismiss_ctx(&[], Arc::clone(&ui));
    cmd.execute(&ctx)
        .await
        .expect("dismiss should succeed against an open inspector");

    assert!(
        ui.inspector_stack("main").is_empty(),
        "dismiss must pop the topmost (and only) panel — the inspector \
         layer unmounts on next React render when the stack is empty",
    );
}

// ---------------------------------------------------------------------------
// Two-panel dismiss — top-only pop
// ---------------------------------------------------------------------------

/// Two inspector panels open → `app.dismiss` pops the topmost one,
/// leaves the other on the stack. Pressing Escape twice closes both.
///
/// Asserts the **stack semantics** of `inspector_close`: it pops one
/// entry per call, never the whole stack. The "close all" gesture
/// (clicking the backdrop) routes through a separate command
/// (`ui.inspector.close_all`) and is not covered here.
#[tokio::test]
async fn dismiss_with_two_panels_open_pops_topmost_only() {
    let cmds = register_commands();
    let cmd = cmds.get("app.dismiss").unwrap();

    let ui = Arc::new(UIState::new());
    ui.inspect("main", "task:a");
    ui.inspect("main", "task:b");
    assert_eq!(ui.inspector_stack("main"), vec!["task:a", "task:b"]);

    let ctx = dismiss_ctx(&[], Arc::clone(&ui));
    cmd.execute(&ctx).await.expect("dismiss should succeed");

    assert_eq!(
        ui.inspector_stack("main"),
        vec!["task:a"],
        "dismiss pops only the topmost panel; the underlying panel stays \
         on the stack so a second Escape can close it",
    );

    // Second dispatch — closes the remaining panel.
    let ctx = dismiss_ctx(&[], Arc::clone(&ui));
    cmd.execute(&ctx)
        .await
        .expect("second dismiss should succeed");

    assert!(
        ui.inspector_stack("main").is_empty(),
        "second dismiss empties the stack; the inspector layer unmounts",
    );
}

// ---------------------------------------------------------------------------
// Palette + inspector both open — palette wins
// ---------------------------------------------------------------------------

/// Palette open AND inspector panel open → `app.dismiss` closes the
/// palette first; the inspector stays. A second Escape is needed to
/// close the inspector.
///
/// Pins the **layer ordering** in `DismissCmd::execute`: layer 1 is the
/// palette (a transient overlay above everything else), layer 2 is the
/// inspector stack. The user's mental model of "Escape closes the most
/// recently opened modal first" maps onto this ordering.
#[tokio::test]
async fn dismiss_with_palette_and_inspector_open_closes_palette_first() {
    let cmds = register_commands();
    let cmd = cmds.get("app.dismiss").unwrap();

    let ui = Arc::new(UIState::new());
    ui.inspect("main", "task:01XYZ");
    ui.set_palette_open("main", true);
    assert!(ui.palette_open("main"));
    assert_eq!(ui.inspector_stack("main").len(), 1);

    let ctx = dismiss_ctx(&[], Arc::clone(&ui));
    cmd.execute(&ctx).await.expect("dismiss should succeed");

    assert!(
        !ui.palette_open("main"),
        "palette closes first — it shadows the inspector",
    );
    assert_eq!(
        ui.inspector_stack("main"),
        vec!["task:01XYZ"],
        "inspector stack is untouched while the palette is closing",
    );

    // Second dispatch — now the inspector closes.
    let ctx = dismiss_ctx(&[], Arc::clone(&ui));
    cmd.execute(&ctx)
        .await
        .expect("second dismiss should succeed against the inspector");

    assert!(
        ui.inspector_stack("main").is_empty(),
        "with the palette closed, the next dismiss pops the inspector",
    );
}

// ---------------------------------------------------------------------------
// Nothing open — null result
// ---------------------------------------------------------------------------

/// Nothing open (palette closed, inspector stack empty) → `app.dismiss`
/// returns `Value::Null` and mutates nothing.
///
/// This is the regression guard for the "Escape no-ops at the layer
/// root with nothing open" case: `nav.drillOut` returns `None` from a
/// layer-root scope, the React adapter dispatches `app.dismiss`, and
/// the dismiss command must not produce a side effect or a state-
/// change payload that the frontend would mistake for an event to
/// react to.
#[tokio::test]
async fn dismiss_with_nothing_open_returns_null() {
    let cmds = register_commands();
    let cmd = cmds.get("app.dismiss").unwrap();

    let ui = Arc::new(UIState::new());
    assert!(!ui.palette_open("main"));
    assert!(ui.inspector_stack("main").is_empty());

    let ctx = dismiss_ctx(&[], Arc::clone(&ui));
    let result = cmd.execute(&ctx).await.expect("dismiss should succeed");

    assert!(
        result.is_null(),
        "dismiss with nothing open must return Value::Null so the React \
         adapter knows there is nothing to react to",
    );
    assert!(!ui.palette_open("main"));
    assert!(ui.inspector_stack("main").is_empty());
}

// ---------------------------------------------------------------------------
// Multi-window — dismiss targets the invoking window
// ---------------------------------------------------------------------------

/// Inspector open in two windows → `app.dismiss` invoked from a scope
/// chain carrying `window:secondary` closes only that window's panel.
///
/// Pins the per-window contract of `inspector_stack`: it is keyed by
/// window label, and `DismissCmd::execute` reads the label from the
/// scope chain via [`CommandContext::window_label_from_scope`]. A
/// regression that always read `"main"` would surface here.
#[tokio::test]
async fn dismiss_targets_invoking_window_only() {
    let cmds = register_commands();
    let cmd = cmds.get("app.dismiss").unwrap();

    let ui = Arc::new(UIState::new());
    ui.inspect("main", "task:m1");
    ui.inspect("secondary", "task:s1");

    let ctx = dismiss_ctx(&["window:secondary"], Arc::clone(&ui));
    cmd.execute(&ctx).await.expect("dismiss should succeed");

    assert!(
        ui.inspector_stack("secondary").is_empty(),
        "secondary window's panel must close",
    );
    assert_eq!(
        ui.inspector_stack("main"),
        vec!["task:m1"],
        "main window's panel is untouched — dismiss is per-window",
    );
}
