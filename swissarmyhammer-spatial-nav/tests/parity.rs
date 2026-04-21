//! JS shim ↔ Rust parity tests.
//!
//! This test reads `kanban-app/ui/src/test/spatial-parity-cases.json` —
//! the same fixture the vitest-browser suite consumes — and runs every
//! scenario through the production [`SpatialState`]. Each step asserts
//! the same event shape and post-step focused key that the JS shim's
//! parity test asserts.
//!
//! The contract: both implementations must agree byte-for-byte on
//! every fixture scenario. When they diverge (Rust adds an edge case,
//! or the JS shim's beam-test scoring drifts) exactly one of the two
//! test suites fails, forcing the author to reconcile the shim against
//! Rust (never the other way around — Rust is the production path).
//!
//! The JSON lives under `kanban-app/ui/src/test/` and is loaded via
//! `include_str!` so the Rust crate does not need a filesystem path at
//! runtime; the file is embedded at compile time.

use std::collections::HashMap;

use serde::Deserialize;
use swissarmyhammer_spatial_nav::{Direction, FocusChanged, Rect, SpatialState};

/// JSON rect format: shared with the JS shim fixture. Uses `w`/`h` for
/// width/height to match the `ShimRect` shape in TypeScript.
#[derive(Debug, Deserialize)]
struct ParityRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl From<&ParityRect> for Rect {
    fn from(r: &ParityRect) -> Rect {
        Rect {
            x: r.x,
            y: r.y,
            width: r.w,
            height: r.h,
        }
    }
}

/// JSON entry format — deserializes to the arguments of
/// `SpatialState::register`.
#[derive(Debug, Deserialize)]
struct ParityEntry {
    key: String,
    moniker: String,
    rect: ParityRect,
    layer_key: String,
    parent_scope: Option<String>,
    overrides: HashMap<String, Option<String>>,
}

/// Tagged-union op shape. Matches the TS `ParityOp` type exactly.
#[derive(Debug, Deserialize)]
#[serde(tag = "op")]
enum ParityOp {
    #[serde(rename = "push_layer")]
    PushLayer { key: String, name: String },
    #[serde(rename = "remove_layer")]
    RemoveLayer { key: String },
    #[serde(rename = "register")]
    Register { entry: ParityEntry },
    #[serde(rename = "unregister")]
    Unregister { key: String },
    #[serde(rename = "focus")]
    Focus { key: String },
    #[serde(rename = "clear_focus")]
    ClearFocus,
    #[serde(rename = "navigate")]
    Navigate { from_key: String, direction: String },
}

/// Expected event payload shape. `None` means the op must not emit.
#[derive(Debug, Deserialize)]
struct ParityExpectedEvent {
    prev_key: Option<String>,
    next_key: Option<String>,
}

/// Expectation for a single step: either an event and new focused key,
/// or no event and the focused key stays the same.
#[derive(Debug, Deserialize)]
struct ParityExpect {
    event: Option<ParityExpectedEvent>,
    focused: Option<String>,
}

/// One step in a parity case: an op and its expected outcome.
#[derive(Debug, Deserialize)]
struct ParityStep {
    op: ParityOp,
    expect: ParityExpect,
}

/// A single named scenario composed of sequential steps.
#[derive(Debug, Deserialize)]
struct ParityCase {
    name: String,
    steps: Vec<ParityStep>,
}

/// Apply one op to the state machine and return the emitted event.
fn apply_op(state: &SpatialState, op: &ParityOp) -> Option<FocusChanged> {
    match op {
        ParityOp::PushLayer { key, name } => {
            state.push_layer(key.clone(), name.clone());
            None
        }
        ParityOp::RemoveLayer { key } => state.remove_layer(key),
        ParityOp::Register { entry } => {
            state.register(
                entry.key.clone(),
                entry.moniker.clone(),
                Rect::from(&entry.rect),
                entry.layer_key.clone(),
                entry.parent_scope.clone(),
                entry.overrides.clone(),
            );
            None
        }
        ParityOp::Unregister { key } => state.unregister(key),
        ParityOp::Focus { key } => state.focus(key),
        ParityOp::ClearFocus => state.clear_focus(),
        ParityOp::Navigate {
            from_key,
            direction,
        } => {
            let dir: Direction = direction
                .parse()
                .unwrap_or_else(|e| panic!("invalid direction {direction}: {e}"));
            // Unwrap the Result; every parity case is expected to provide a
            // valid `from_key` + direction pair. The inner Option distinguishes
            // "no navigation target" (blocked / nothing to move to) from a
            // move, matching the shim's `navigate()` contract.
            state
                .navigate(from_key, dir)
                .unwrap_or_else(|e| panic!("navigate {from_key} {direction} errored: {e}"))
        }
    }
}

/// Assert that the observed event matches the expectation shape.
fn assert_event_matches(
    case_name: &str,
    step_idx: usize,
    expected: &Option<ParityExpectedEvent>,
    observed: &Option<FocusChanged>,
) {
    match (expected, observed) {
        (None, None) => {}
        (
            Some(ParityExpectedEvent { prev_key, next_key }),
            Some(FocusChanged {
                prev_key: obs_prev,
                next_key: obs_next,
            }),
        ) => {
            assert_eq!(
                prev_key, obs_prev,
                "[{case_name}] step {step_idx} prev_key mismatch",
            );
            assert_eq!(
                next_key, obs_next,
                "[{case_name}] step {step_idx} next_key mismatch",
            );
        }
        _ => panic!(
            "[{case_name}] step {step_idx} event mismatch: expected {expected:?}, got {observed:?}",
        ),
    }
}

/// Embedded JSON fixture — shared with the JS parity test. Changing this
/// path breaks both sides; that is the intended tripwire.
const PARITY_JSON: &str = include_str!("../../kanban-app/ui/src/test/spatial-parity-cases.json");

#[test]
fn js_shim_rust_state_parity() {
    let cases: Vec<ParityCase> = serde_json::from_str(PARITY_JSON)
        .expect("spatial-parity-cases.json parses as ParityCase[]");
    assert!(
        !cases.is_empty(),
        "parity fixture must contain at least one case",
    );
    for case in &cases {
        let state = SpatialState::new();
        for (step_idx, step) in case.steps.iter().enumerate() {
            let observed = apply_op(&state, &step.op);
            assert_event_matches(&case.name, step_idx, &step.expect.event, &observed);
            assert_eq!(
                step.expect.focused,
                state.focused_key(),
                "[{}] step {} focused_key after {:?}",
                case.name,
                step_idx,
                step.op,
            );
        }
    }
}
