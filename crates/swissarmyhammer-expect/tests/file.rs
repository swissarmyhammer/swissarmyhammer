//! Integration coverage for the `file` surface adapter against a real scratch
//! directory.
//!
//! The adapter under test is synchronous (it uses `std::fs` directly — no
//! external harness), so the test functions are plain `#[test]`s exercising the
//! production path: provision a scratch dir, drive (write files/dirs), observe
//! (capture files/dirs/content), and teardown (drop the scratch dir). The file
//! locator dialect — a path + content, plus a json-path **sub-locator** into a
//! structured file — is then built and evaluated against the captured state, the
//! way a hand-authored locator would be (`ideas/expect.md` §"Locators are a
//! per-surface dialect": `path + content (+ sub-locator if structured)`).

use std::time::Duration;

use swissarmyhammer_expect::{
    AssertOp, AssertionOutcome, BoundValue, Checkpoint, CompiledAssertion, ExpectError, Expected,
    FileAdapter, Locator, Observation, SurfaceAdapter, SurfaceState, Trajectory, VerdictTier,
};

/// The structured (JSON) file the sub-locator reads, and the single source of
/// truth its assertions are checked against.
const CONFIG_PATH: &str = "config/app.json";

/// The JSON content written to [`CONFIG_PATH`].
const CONFIG_BODY: &str = r#"{"total": 40, "items": ["a", "b"]}"#;

/// A plain-text file the path+content locator reads.
const NOTES_PATH: &str = "notes.txt";

/// The plain-text content written to [`NOTES_PATH`].
const NOTES_BODY: &str = "hello world";

/// The expected total the json sub-locator must observe inside [`CONFIG_PATH`].
const EXPECTED_TOTAL: f64 = 40.0;

/// Wrap an observed file state in a single-checkpoint observation so the file
/// locator dialect can be built and evaluated against it.
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

/// A Tier-1 equality assertion over `locator` expecting `expected` at the only
/// checkpoint.
fn equals_assertion(locator: Locator, expected: BoundValue) -> CompiledAssertion {
    CompiledAssertion {
        checkpoint: 0,
        locator,
        op: AssertOp::Equals,
        expected: Expected::Literal { value: expected },
        tier: VerdictTier::Deterministic,
        criterion_text: "a file projection holds".to_string(),
    }
}

#[test]
fn writes_to_a_scratch_dir_and_path_content_and_json_sublocators_observe_state() {
    let adapter = FileAdapter::new();
    let repo = tempfile::TempDir::new().unwrap();
    let mut sut = adapter.provision(None, repo.path()).expect("provision");

    // Drive: write a structured file (into a nested dir) and a plain-text file.
    adapter
        .drive(&mut sut, &format!("write {CONFIG_PATH} {CONFIG_BODY}"))
        .expect("drive write json");
    adapter
        .drive(&mut sut, &format!("write {NOTES_PATH} {NOTES_BODY}"))
        .expect("drive write text");

    // Observe: capture files / dirs / content.
    let state = adapter.observe(&sut).expect("observe");
    let SurfaceState::File(file) = &state else {
        panic!("expected a file surface state, got {state:?}");
    };
    assert_eq!(
        file.files.get(CONFIG_PATH).map(String::as_str),
        Some(CONFIG_BODY),
        "observed the structured file content"
    );
    assert_eq!(
        file.files.get(NOTES_PATH).map(String::as_str),
        Some(NOTES_BODY),
        "observed the plain-text file content"
    );
    assert!(
        file.dirs.iter().any(|dir| dir == "config"),
        "observed the directory created for the nested write: {:?}",
        file.dirs
    );

    let observation = observation_of(state.clone());
    let checkpoint_state = &observation.checkpoints[0].state;

    // A path + content locator observes the plain-text file verbatim.
    let content = Locator::FileContent {
        path: NOTES_PATH.to_string(),
    };
    assert_eq!(
        content.resolve(checkpoint_state),
        Some(BoundValue::Text(NOTES_BODY.to_string())),
        "the path+content locator observes the file content"
    );

    // A json-path sub-locator observes a value inside the structured file.
    let total = Locator::FileJsonPath {
        path: CONFIG_PATH.to_string(),
        pointer: "$.total".to_string(),
    };
    assert_eq!(
        total.resolve(checkpoint_state),
        Some(BoundValue::Number(EXPECTED_TOTAL)),
        "the json sub-locator observes a scalar inside the file"
    );

    // The sub-locator reaches into arrays too.
    let first_item = Locator::FileJsonPath {
        path: CONFIG_PATH.to_string(),
        pointer: "$.items[0]".to_string(),
    };
    assert_eq!(
        first_item.resolve(checkpoint_state),
        Some(BoundValue::Text("a".to_string())),
        "the json sub-locator indexes into arrays"
    );

    // A Tier-1 equality assertion over the json sub-locator holds.
    let assertion = equals_assertion(total, BoundValue::Number(EXPECTED_TOTAL));
    assert_eq!(assertion.evaluate(&observation), AssertionOutcome::Holds);

    adapter.teardown(sut).expect("teardown");
}

#[test]
fn a_file_locator_reports_drift_when_the_path_no_longer_binds() {
    let adapter = FileAdapter::new();
    let repo = tempfile::TempDir::new().unwrap();
    let mut sut = adapter.provision(None, repo.path()).expect("provision");
    adapter
        .drive(&mut sut, &format!("write {NOTES_PATH} {NOTES_BODY}"))
        .expect("drive");
    let observation = observation_of(adapter.observe(&sut).expect("observe"));

    // A locator for a file that was never written no longer binds: drift.
    let assertion = equals_assertion(
        Locator::FileContent {
            path: "missing.txt".to_string(),
        },
        BoundValue::Text(NOTES_BODY.to_string()),
    );
    assert!(
        matches!(
            assertion.evaluate(&observation),
            AssertionOutcome::Drifted { .. }
        ),
        "a non-binding file locator reports drift"
    );

    adapter.teardown(sut).expect("teardown");
}

#[test]
fn write_rejects_path_traversal_out_of_the_scratch_dir() {
    let adapter = FileAdapter::new();
    let repo = tempfile::TempDir::new().unwrap();
    let mut sut = adapter.provision(None, repo.path()).expect("provision");

    // A `..` component must be rejected before any write happens.
    let traversal = adapter
        .drive(&mut sut, "write ../escape.txt pwned")
        .expect_err("traversal must be rejected");
    assert!(
        matches!(traversal, ExpectError::Surface(_)),
        "got {traversal:?}"
    );

    // An absolute path must be rejected too.
    let absolute = adapter
        .drive(&mut sut, "write /tmp/escape.txt pwned")
        .expect_err("absolute path must be rejected");
    assert!(
        matches!(absolute, ExpectError::Surface(_)),
        "got {absolute:?}"
    );

    adapter.teardown(sut).expect("teardown");
}

#[test]
fn setup_arranges_an_initial_file_fixture() {
    // The `setup:` fixture writes a file at provision; observe sees it without
    // any drive step.
    let adapter = FileAdapter::new();
    let repo = tempfile::TempDir::new().unwrap();
    let setup = swissarmyhammer_expect::Setup::Command(format!("write {NOTES_PATH} {NOTES_BODY}"));
    let sut = adapter
        .provision(Some(&setup), repo.path())
        .expect("provision with fixture");

    let state = adapter.observe(&sut).expect("observe");
    let SurfaceState::File(file) = &state else {
        panic!("expected a file surface state, got {state:?}");
    };
    assert_eq!(
        file.files.get(NOTES_PATH).map(String::as_str),
        Some(NOTES_BODY),
        "the setup fixture arranged the initial file"
    );

    adapter.teardown(sut).expect("teardown");
}
