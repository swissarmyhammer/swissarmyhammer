//! Scenario-driven spatial navigation tests.
//!
//! Each case in `spatial_cases.json` is a named scenario composed of
//! sequential ops (push_layer, register, focus, navigate, …). The test
//! applies every op to a fresh [`SpatialState`] and asserts the emitted
//! event shape and post-step focused key.
//!
//! ## Why a JSON-driven table?
//!
//! The cases cover higher-level interaction shapes — beam tests with
//! out-of-beam distractors, layer stacks, parent-scope container-first
//! search, boot-race focus restoration — that read more naturally as
//! declarative fixtures than as hand-rolled Rust helpers. The JSON lets
//! us add scenarios without touching boilerplate and keeps the
//! algorithm coverage in one place.
//!
//! ## Scope
//!
//! Pure algorithm coverage. Rust is the sole owner of spatial-nav
//! logic — the frontend is a dumb registrar that invokes into
//! [`SpatialState`] via Tauri commands. There is no JS-side mirror to
//! keep in sync; these tests simply exercise [`SpatialState`] against a
//! table of scenarios.

use std::collections::HashMap;

use serde::Deserialize;
use swissarmyhammer_spatial_nav::{Direction, FocusChanged, Rect, SpatialState};

/// JSON rect format. Uses `w`/`h` for width/height to keep the case
/// files compact.
#[derive(Debug, Deserialize)]
struct CaseRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl From<&CaseRect> for Rect {
    fn from(r: &CaseRect) -> Rect {
        Rect {
            x: r.x,
            y: r.y,
            width: r.w,
            height: r.h,
        }
    }
}

/// JSON entry format — deserializes to the arguments of
/// [`SpatialState::register`].
#[derive(Debug, Deserialize)]
struct CaseEntry {
    key: String,
    moniker: String,
    rect: CaseRect,
    layer_key: String,
    parent_scope: Option<String>,
    overrides: HashMap<String, Option<String>>,
}

/// Tagged-union op shape — one variant per [`SpatialState`] mutation.
#[derive(Debug, Deserialize)]
#[serde(tag = "op")]
enum CaseOp {
    #[serde(rename = "push_layer")]
    PushLayer { key: String, name: String },
    #[serde(rename = "remove_layer")]
    RemoveLayer { key: String },
    #[serde(rename = "register")]
    Register { entry: CaseEntry },
    #[serde(rename = "unregister")]
    Unregister { key: String },
    #[serde(rename = "focus")]
    Focus { key: String },
    #[serde(rename = "clear_focus")]
    ClearFocus,
    /// `from_key` is optional so cases can exercise the null-source
    /// safety net (`"from_key": null`) alongside normal and stale-source
    /// nav.
    #[serde(rename = "navigate")]
    Navigate {
        from_key: Option<String>,
        direction: String,
    },
    #[serde(rename = "focus_first_in_layer")]
    FocusFirstInLayer { layer_key: String },
}

/// Expected event payload shape. `None` means the op must not emit.
#[derive(Debug, Deserialize)]
struct CaseExpectedEvent {
    prev_key: Option<String>,
    next_key: Option<String>,
}

/// Expectation for a single step: either an event and new focused key,
/// or no event and the focused key stays the same.
#[derive(Debug, Deserialize)]
struct CaseExpect {
    event: Option<CaseExpectedEvent>,
    focused: Option<String>,
}

/// One step in a scenario: an op and its expected outcome.
#[derive(Debug, Deserialize)]
struct CaseStep {
    op: CaseOp,
    expect: CaseExpect,
}

/// A single named scenario composed of sequential steps.
#[derive(Debug, Deserialize)]
struct Case {
    name: String,
    steps: Vec<CaseStep>,
}

/// Apply one op to the state machine and return the emitted event.
fn apply_op(state: &SpatialState, op: &CaseOp) -> Option<FocusChanged> {
    match op {
        CaseOp::PushLayer { key, name } => {
            state.push_layer(key.clone(), name.clone());
            None
        }
        CaseOp::RemoveLayer { key } => state.remove_layer(key),
        CaseOp::Register { entry } => {
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
        CaseOp::Unregister { key } => state.unregister(key),
        CaseOp::Focus { key } => state.focus(key),
        CaseOp::ClearFocus => state.clear_focus(),
        CaseOp::Navigate {
            from_key,
            direction,
        } => {
            let dir: Direction = direction
                .parse()
                .unwrap_or_else(|e| panic!("invalid direction {direction}: {e}"));
            // Unwrap the Result; every case is expected to provide a
            // valid direction. The inner Option distinguishes "no
            // navigation target" (blocked / nothing to move to) from a
            // move. `from_key` may be `None` or an unregistered string
            // to exercise the null/stale-source fallback.
            state
                .navigate(from_key.as_deref(), dir)
                .unwrap_or_else(|e| panic!("navigate {from_key:?} {direction} errored: {e}"))
        }
        CaseOp::FocusFirstInLayer { layer_key } => state.focus_first_in_layer(layer_key),
    }
}

/// Assert that the observed event matches the expectation shape.
fn assert_event_matches(
    case_name: &str,
    step_idx: usize,
    expected: &Option<CaseExpectedEvent>,
    observed: &Option<FocusChanged>,
) {
    match (expected, observed) {
        (None, None) => {}
        (
            Some(CaseExpectedEvent { prev_key, next_key }),
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

/// Embedded JSON fixture — lives next to this test file inside the
/// crate so the Rust crate owns its own test data with no cross-crate
/// path dependency.
const CASES_JSON: &str = include_str!("spatial_cases.json");

#[test]
fn spatial_state_scenarios() {
    let cases: Vec<Case> =
        serde_json::from_str(CASES_JSON).expect("spatial_cases.json parses as Case[]");
    assert!(
        !cases.is_empty(),
        "scenario fixture must contain at least one case",
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
