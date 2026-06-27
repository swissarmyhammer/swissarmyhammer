//! Integration coverage for the `gui` surface adapter driving a real native app
//! through the OS accessibility (AX) tree.
//!
//! **Doubly gated, and fixture-agnostic.** Driving a native app over AX needs (a)
//! a supported OS with the *test runner* holding Accessibility permission —
//! frequently unavailable in automated/headless CI — and (b) a launchable fixture
//! app whose controls the test cannot know in advance. So the test is configured
//! entirely by environment variables and skips cleanly (with a log) when any are
//! absent, rather than failing the suite:
//!
//! - [`FIXTURE_APP_ENV`] — the executable to launch (a macOS `.app` bundle's
//!   binary inside `Contents/MacOS/`, e.g. the in-repo `kanban-app`).
//! - [`DRIVE_STEP_ENV`] — the press step in the a11y drive dialect, e.g.
//!   `press AXButton[name="Go"]`. Required so the run exercises the **drive**
//!   path (`role[name=…]` → `find_element` → `AXPress`), not just a snapshot.
//! - [`ASSERT_ENV`] — optional `role[name=…]` criterion, e.g.
//!   `AXTextField[name="Result"] equals clicked`, compiled and evaluated against
//!   the observed tree to assert a concrete bridged AX node value.
//!
//! When fully configured (permission + app + drive step) this runs the same
//! provision → drive-by-`role[name=…]` → observe → (assert) → teardown loop as
//! the browser surface's integration test. The load-bearing, AX-free coverage —
//! the `RawAxNode` → `A11yNode` mapping, the shared `role[name=…]` matcher, and
//! structural-drift-on-rename — lives in the `gui` module unit tests and always
//! runs regardless of AX permission.

use std::path::Path;
use std::time::Duration;

use swissarmyhammer_expect::{
    compile, gui_automation_available, AssertionOutcome, Checkpoint, Criterion, GuiAdapter,
    Observation, SurfaceAdapter, SurfaceState, Trajectory,
};

/// Env var naming an executable to launch as the gui fixture (a native/Tauri
/// app's binary). Absent → the live-AX integration test skips cleanly.
const FIXTURE_APP_ENV: &str = "EXPECT_GUI_FIXTURE_APP";

/// Env var holding the press step in the a11y drive dialect (e.g.
/// `press AXButton[name="Go"]`). Absent → skip, since without it the drive path
/// is never exercised.
const DRIVE_STEP_ENV: &str = "EXPECT_GUI_DRIVE_STEP";

/// Env var holding an optional `role[name=…]` criterion to compile and evaluate
/// against the observed tree (e.g. `AXTextField[name="Result"] equals clicked`).
const ASSERT_ENV: &str = "EXPECT_GUI_ASSERT";

/// A generous readiness budget; a cold native-app launch can be slow.
const ACTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Wrap an observed surface state in a single-checkpoint observation so the a11y
/// locator dialect can be compiled and evaluated against it.
fn observation_of(state: SurfaceState) -> Observation {
    Observation {
        path: "fixture".to_string(),
        checkpoints: vec![Checkpoint {
            after: "final".to_string(),
            state,
            duration: Duration::from_millis(1),
        }],
        trajectory: Trajectory { steps: Vec::new() },
    }
}

/// An unchecked criterion from `text`.
fn criterion(text: &str) -> Criterion {
    Criterion {
        text: text.to_string(),
        checked: false,
    }
}

#[test]
fn presses_a_control_by_role_name_and_snapshots_the_ax_tree() {
    if !gui_automation_available() {
        eprintln!(
            "SKIP presses_a_control_by_role_name_and_snapshots_the_ax_tree: \
             gui automation unavailable (unsupported OS, or the test runner lacks \
             macOS Accessibility permission); the AX-free gui mapping/matcher/drift \
             unit tests still cover the logic"
        );
        return;
    }
    let Ok(app) = std::env::var(FIXTURE_APP_ENV) else {
        eprintln!(
            "SKIP presses_a_control_by_role_name_and_snapshots_the_ax_tree: \
             no {FIXTURE_APP_ENV} fixture app configured; set it to a launchable \
             native/Tauri executable to exercise the live AX path"
        );
        return;
    };
    let Ok(drive_step) = std::env::var(DRIVE_STEP_ENV) else {
        eprintln!(
            "SKIP presses_a_control_by_role_name_and_snapshots_the_ax_tree: \
             no {DRIVE_STEP_ENV} press step configured (e.g. `press AXButton[name=\"Go\"]`); \
             set it so the run exercises the drive path, not just a snapshot"
        );
        return;
    };

    let adapter = GuiAdapter::new(app).with_action_timeout(ACTION_TIMEOUT);

    // Provision launches the native app and waits for its AX tree to appear.
    let mut sut = adapter
        .provision(None, Path::new("."))
        .expect("provision launches the native app and its AX tree appears");

    // Drive: press the control by accessibility role + name through the AX API.
    adapter
        .drive(&mut sut, &drive_step)
        .expect("press the control by `role[name=…]`");

    // Observe: snapshot the bridged accessibility tree.
    let state = adapter.observe(&sut).expect("observe the AX tree");
    let SurfaceState::A11y { tree } = &state else {
        adapter.teardown(sut).expect("teardown");
        panic!("expected an a11y surface state, got {state:?}");
    };
    assert!(
        !tree.children.is_empty(),
        "the observed AX tree should not be empty"
    );

    // The same `role[name=…]` dialect as the browser surface binds and evaluates
    // over the live observation when an assertion is configured.
    if let Ok(assert_text) = std::env::var(ASSERT_ENV) {
        let observation = observation_of(state.clone());
        let assertion = compile(&criterion(&assert_text), &observation)
            .unwrap_or_else(|err| panic!("compile `{assert_text}`: {err}"));
        assert_eq!(
            assertion.evaluate(&observation),
            AssertionOutcome::Holds,
            "the configured `{assert_text}` assertion should hold against the observed AX tree"
        );
    }

    // Teardown closes the app.
    adapter
        .teardown(sut)
        .expect("teardown closes the native app");
}
