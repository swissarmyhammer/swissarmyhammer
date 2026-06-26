//! Integration coverage for the `observe` engine step against the cli surface.
//!
//! Exercises the real production path end to end: parse a `*.expect.md` spec
//! with [`ExpectationLoader`], drive a fixture cli system under test through
//! [`observe`], and assert the assembled [`Observation`] is the multi-checkpoint
//! timeline from `ideas/expect.md` §"The Check Loop" — one checkpoint per `When`
//! step plus a final — and that [`write_received`] persists it to
//! `.expect/received/<path>.received.json`.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use swissarmyhammer_expect::{
    observe, write_received, CliAdapter, ExpectationLoader, Observation, ObserveConfig,
    SurfaceState,
};

/// A generous per-run budget; the fixture script returns immediately.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The repo-relative identity of the fixture spec.
const SPEC_IDENTITY: &str = "echo";

/// The two `When` steps the fixture spec drives, in order.
const WHEN_STEPS: &[&str] = &["first", "second"];

/// The `after` label of the trailing checkpoint, mirroring the engine constant.
const FINAL_LABEL: &str = "final";

/// Write `body` as an executable script at `dir/name`.
fn write_executable(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
}

/// Stand up a temp repo holding an echoing cli SUT plus a two-`When` spec, and
/// return the loaded sole expectation alongside the repo dir.
fn fixture() -> (tempfile::TempDir, swissarmyhammer_expect::Expectation) {
    let repo = tempfile::TempDir::new().unwrap();
    // The SUT echoes whatever argument the driver appends, to stdout, exit 0.
    write_executable(repo.path(), "app.sh", "#!/bin/sh\necho \"$@\"\n");

    let spec = format!(
        "---\n\
         description: the app echoes each command it is given\n\
         surface: cli\n\
         setup: ./app.sh\n\
         ---\n\
         \n\
         The app echoes the argument it is driven with.\n\
         \n\
         ## When\n\
         - {}\n\
         - {}\n\
         \n\
         ## Then\n\
         - [ ] it echoes the first command\n\
         - [ ] it echoes the second command\n",
        WHEN_STEPS[0], WHEN_STEPS[1],
    );
    std::fs::write(repo.path().join(format!("{SPEC_IDENTITY}.expect.md")), spec).unwrap();

    let loader = ExpectationLoader::new(repo.path());
    let mut specs = loader
        .resolve_scope(Some(SPEC_IDENTITY), None)
        .expect("resolve the fixture spec");
    assert_eq!(specs.len(), 1, "fixture must resolve to exactly one spec");
    (repo, specs.remove(0))
}

/// Assert a checkpoint's cli state echoes `expected` on stdout with a clean exit.
fn assert_cli_echo(state: &SurfaceState, expected: &str) {
    match state {
        SurfaceState::Cli(cli) => {
            assert_eq!(
                cli.stdout.trim(),
                expected,
                "stdout should echo the command"
            );
            assert_eq!(cli.stderr, "", "the fixture writes nothing to stderr");
            assert_eq!(cli.exit_code, Some(0), "the fixture exits cleanly");
        }
        other => panic!("expected a cli surface state, got {other:?}"),
    }
}

#[test]
fn observe_captures_one_checkpoint_per_when_step_plus_a_final() {
    let (repo, expectation) = fixture();
    let adapter = CliAdapter::new(TEST_TIMEOUT);
    let config = ObserveConfig::new(repo.path());

    let observation = observe(&expectation, &adapter, &config).expect("observe the cli SUT");

    assert_eq!(observation.path, SPEC_IDENTITY);
    // Two When steps yield two checkpoints; the final makes three.
    assert_eq!(
        observation.checkpoints.len(),
        WHEN_STEPS.len() + 1,
        "one checkpoint per When step plus a final"
    );

    // The timeline is ordered: each When step in turn, then the final.
    let labels: Vec<&str> = observation
        .checkpoints
        .iter()
        .map(|cp| cp.after.as_str())
        .collect();
    let mut expected_labels: Vec<&str> = WHEN_STEPS.to_vec();
    expected_labels.push(FINAL_LABEL);
    assert_eq!(
        labels, expected_labels,
        "checkpoints name their When step or final"
    );

    // Each checkpoint's cli state carries the authoritative stdout/stderr/exit.
    assert_cli_echo(&observation.checkpoints[0].state, WHEN_STEPS[0]);
    assert_cli_echo(&observation.checkpoints[1].state, WHEN_STEPS[1]);
    // The final re-reads the SUT's end state — the last driven run.
    assert_cli_echo(&observation.checkpoints[2].state, WHEN_STEPS[1]);

    // Durations are recorded for every checkpoint (bounded by the run budget).
    for checkpoint in &observation.checkpoints {
        assert!(
            checkpoint.duration <= TEST_TIMEOUT,
            "duration {:?} should be within the run budget",
            checkpoint.duration
        );
    }

    // The trajectory records what the driver did — never the verdict source.
    assert!(
        !observation.trajectory.steps.is_empty(),
        "the driver trajectory is recorded"
    );
}

#[test]
fn write_received_persists_the_observation_under_dot_expect() {
    let (repo, expectation) = fixture();
    let adapter = CliAdapter::new(TEST_TIMEOUT);
    let config = ObserveConfig::new(repo.path());

    let observation = observe(&expectation, &adapter, &config).expect("observe the cli SUT");
    let received = write_received(repo.path(), &observation).expect("write received");

    let expected = repo
        .path()
        .join(".expect")
        .join("received")
        .join(format!("{SPEC_IDENTITY}.received.json"));
    assert_eq!(
        received, expected,
        "received path follows the .expect layout"
    );
    assert!(received.is_file(), "the received file is written to disk");

    // It round-trips back to the same Observation. The wire form encodes each
    // duration as whole milliseconds (`duration_ms`), so the comparison is
    // against an observation already normalized through that same encoding,
    // not the in-memory value with its sub-millisecond precision.
    let json = std::fs::read_to_string(&received).unwrap();
    let reloaded: Observation = serde_json::from_str(&json).expect("parse received json");
    let normalized: Observation =
        serde_json::from_str(&serde_json::to_string(&observation).unwrap()).unwrap();
    assert_eq!(reloaded, normalized);
}
