//! The `observe` step: run an expectation and assemble its authoritative
//! [`Observation`] timeline.
//!
//! This is the deterministic, no-agent half of the loop in `ideas/expect.md`
//! §"The Check Loop": **provision → arrange (Given) → act (When) → observe →
//! teardown**, with the three roles kept strictly separate. The **driver**
//! (here, the surface adapter actuating mechanically) causes each transition;
//! the **adapter** reads the *authoritative* state at every checkpoint; the
//! resulting [`Observation`] — not the driver's transcript — is the result.
//! Grading (the verdict ladder) is a separate step and lives elsewhere.
//!
//! A checkpoint is captured after *each* `When` step plus a trailing
//! [`FINAL_CHECKPOINT`], because real criteria are multi-step, relational, and
//! temporal: the timeline, not a single end snapshot, is what `evaluate` reasons
//! over. Each [`Checkpoint`] records the adapter's [`SurfaceState`] and the
//! wall-clock [`duration`](Checkpoint::duration) it took to reach it.
//!
//! [`write_received`] persists an observation to the gitignored
//! `.expect/received/<path>.received.json`, the "last run per spec" slot from
//! `ideas/expect.md` §"The dot-folder".

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::config::EXPECT_DIR;
use crate::error::ExpectError;
use crate::spec::{Expectation, Isolation, EXPECT_EXTENSION};
use crate::surface::SurfaceAdapter;
use crate::types::{Checkpoint, Observation, Trajectory};

/// The `after` label of the trailing checkpoint captured once every `When` step
/// has been driven — the end-of-run snapshot in the timeline.
pub const FINAL_CHECKPOINT: &str = "final";

/// The gitignored subdirectory of `.expect/` that holds the last observation per
/// spec (the `received` slot evaluated against the committed golden).
const RECEIVED_SUBDIR: &str = "received";

/// The extension appended to a spec's repo-relative identity to name its
/// received observation file.
const RECEIVED_EXTENSION: &str = ".received.json";

/// The committed subdirectory of `.expect/` that holds the approved, scrubbed
/// observation per spec (the `golden` baseline `evaluate` re-grades against).
const GOLDEN_SUBDIR: &str = "goldens";

/// The extension appended to a spec's repo-relative identity to name its golden
/// baseline file.
const GOLDEN_EXTENSION: &str = ".golden.json";

/// The trajectory prefix recording a `Given` arrangement the driver performed.
const ARRANGE_STEP_PREFIX: &str = "arrange: ";

/// The trajectory prefix recording a `When` transition the driver caused.
const ACT_STEP_PREFIX: &str = "act: ";

/// Inputs the [`observe`] run needs beyond the spec and adapter.
///
/// Carries the repo root the adapter provisions against (the same base the
/// spec's identity is relative to). A struct rather than a bare path so further
/// run-scoped knobs can land without churning every call site.
#[derive(Debug, Clone)]
pub struct ObserveConfig {
    /// The repo root: where the SUT is provisioned and built.
    pub repo_root: PathBuf,
}

impl ObserveConfig {
    /// Create a config rooted at `repo_root`.
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

/// Run `expectation` against its surface via `adapter` and assemble the
/// authoritative [`Observation`] timeline.
///
/// Executes the deterministic loop from `ideas/expect.md` §"The Check Loop":
///
/// 1. **Provision** the SUT from the spec's `setup` (or detected commands).
/// 2. **Arrange** each `## Given` step mechanically through the adapter to
///    establish preconditions; these are recorded in the trajectory but are
///    *not* checkpointed (they set up state, they are not the behavior).
/// 3. **Act + observe** each `## When` step: the adapter drives the transition,
///    then reads the authoritative [`SurfaceState`], yielding one [`Checkpoint`]
///    per step whose [`after`](Checkpoint::after) names the step.
/// 4. Capture a trailing [`FINAL_CHECKPOINT`] of the SUT's end state.
/// 5. **Teardown** the provisioned instance.
///
/// The driver causes the transitions and the adapter observes authoritative
/// state; the returned [`Observation`] — never the [`Trajectory`] of driver
/// actions — is the result the grader later evaluates.
///
/// # Errors
///
/// Returns [`ExpectError`] when the adapter cannot provision, drive, observe, or
/// tear down the SUT (e.g. a failed build, a run timeout, or unreadable state).
pub fn observe<A: SurfaceAdapter>(
    expectation: &Expectation,
    adapter: &A,
    config: &ObserveConfig,
) -> Result<Observation, ExpectError> {
    let mut sut = adapter.provision(expectation.frontmatter.setup.as_ref(), &config.repo_root)?;
    let observation = observe_round(expectation, adapter, &mut sut)?;
    adapter.teardown(sut)?;
    Ok(observation)
}

/// The default repeat count for a mechanically-driven (deterministic) surface
/// when neither `repeat` nor a wider `pass^k` is declared: a single run.
const MECHANICAL_DEFAULT_REPEAT: u32 = 1;

/// The default repeat count when an **agent** drives the run (the runtime
/// fallback): at least two, because a live agent action is the only source of
/// non-determinism, so one run cannot establish reliability (`ideas/expect.md`
/// §"Reliability and Non-Determinism").
const AGENT_DRIVEN_MIN_REPEAT: u32 = 2;

/// Run `expectation` repeatedly to judge `pass^k` reliability, returning one
/// [`Observation`] per run.
///
/// The number of runs is [`resolved_repeat`] of the spec's `reliability`/`repeat`
/// frontmatter and whether the run is mechanically driven (a deterministic
/// surface defaults to one run; an agent-driven run defaults to ≥2). Each run
/// **re-arranges the `Given`** so the repeats are independent — otherwise run 1's
/// effects bleed into run 2 and `pass^k` is theater (`ideas/expect.md`
/// §"Provisioning and Isolation"):
///
/// - [`Isolation::Shared`] (default) provisions the SUT **once** and re-arranges
///   `Given` on each run against that one shared instance, the fast path.
/// - [`Isolation::Fresh`] provisions a **dedicated, pristine instance per run**
///   (a full provision → arrange → act → observe → teardown each time), for an
///   expectation that genuinely needs a clean slate, at the cost of the rebuild.
///
/// The returned vector is always non-empty (the resolved repeat is ≥ 1). Grading
/// across the runs — the `pass^k` verdict and its per-run spread — is a separate
/// step ([`crate::evaluate::evaluate_repeated`]).
///
/// # Errors
///
/// Returns [`ExpectError`] when the adapter cannot provision, drive, observe, or
/// tear down the SUT on any run.
pub fn observe_repeated<A: SurfaceAdapter>(
    expectation: &Expectation,
    adapter: &A,
    config: &ObserveConfig,
) -> Result<Vec<Observation>, ExpectError> {
    let runs = resolved_repeat(
        expectation.frontmatter.reliability.required(),
        expectation.frontmatter.repeat,
        drives_mechanically(expectation, adapter),
    );

    match expectation.frontmatter.isolation {
        Isolation::Shared => observe_shared(expectation, adapter, config, runs),
        Isolation::Fresh => observe_fresh(expectation, adapter, config, runs),
    }
}

/// Resolve how many times to run `observe` before judging `pass^k`.
///
/// `required` is `k` from the declared `pass^k` policy (always ≥ 1) and is the
/// floor — `pass^k` cannot be judged with fewer than `k` runs. On top of it:
///
/// - An explicit `repeat` is honored (clamped up to the `required` floor).
/// - With no `repeat`, a mechanically-driven run uses `required` itself
///   ([`MECHANICAL_DEFAULT_REPEAT`] = 1 for the default `pass^1`), because a
///   deterministic surface reproduces its result and need not be re-run.
/// - With no `repeat`, an agent-driven run is bumped to at least
///   [`AGENT_DRIVEN_MIN_REPEAT`], the runtime-fallback non-determinism default.
pub fn resolved_repeat(required: u32, repeat: Option<u32>, drives_mechanically: bool) -> u32 {
    let baseline = match repeat {
        Some(explicit) => explicit,
        None if drives_mechanically => required.max(MECHANICAL_DEFAULT_REPEAT),
        None => required.max(AGENT_DRIVEN_MIN_REPEAT),
    };
    baseline.max(required)
}

/// Whether every `When` step of `expectation` resolves mechanically through
/// `adapter`, with no agent interpretation.
///
/// The signal behind the repeat default: a run is deterministic when the adapter
/// can bind every action itself (a cli step is always an argv), and agent-driven
/// the moment any step needs the [subagent fallback](SurfaceAdapter::resolves_mechanically).
/// An expectation with no `When` steps drives nothing, so it is mechanical.
pub fn drives_mechanically<A: SurfaceAdapter>(expectation: &Expectation, adapter: &A) -> bool {
    expectation
        .when
        .iter()
        .all(|step| adapter.resolves_mechanically(step))
}

/// Provision one shared SUT and observe `runs` times against it, re-arranging the
/// `Given` on each run — the [`Isolation::Shared`] fast path.
fn observe_shared<A: SurfaceAdapter>(
    expectation: &Expectation,
    adapter: &A,
    config: &ObserveConfig,
    runs: u32,
) -> Result<Vec<Observation>, ExpectError> {
    let mut sut = adapter.provision(expectation.frontmatter.setup.as_ref(), &config.repo_root)?;
    let mut observations = Vec::with_capacity(runs as usize);
    for _ in 0..runs {
        observations.push(observe_round(expectation, adapter, &mut sut)?);
    }
    adapter.teardown(sut)?;
    Ok(observations)
}

/// Provision a dedicated, pristine SUT for each of `runs` runs — the
/// [`Isolation::Fresh`] path, a full lifecycle per run.
fn observe_fresh<A: SurfaceAdapter>(
    expectation: &Expectation,
    adapter: &A,
    config: &ObserveConfig,
    runs: u32,
) -> Result<Vec<Observation>, ExpectError> {
    let mut observations = Vec::with_capacity(runs as usize);
    for _ in 0..runs {
        observations.push(observe(expectation, adapter, config)?);
    }
    Ok(observations)
}

/// Run one arrange → act → observe round against an already-provisioned `sut`,
/// assembling its [`Observation`] timeline.
///
/// The provision/teardown bookends live in the caller ([`observe`] for a single
/// run, [`observe_shared`] for the shared `pass^k` repeats), so this is the unit
/// that is re-run per `pass^k` iteration and always re-establishes the `Given`.
fn observe_round<A: SurfaceAdapter>(
    expectation: &Expectation,
    adapter: &A,
    sut: &mut A::ProvisionedSut,
) -> Result<Observation, ExpectError> {
    let mut steps = Vec::with_capacity(expectation.given.len() + expectation.when.len());

    // Arrange (Given): establish preconditions mechanically, without capturing a
    // checkpoint — the Given is setup state, not the behavior under test.
    for given in &expectation.given {
        adapter.drive(sut, given)?;
        steps.push(format!("{ARRANGE_STEP_PREFIX}{given}"));
    }

    // Act (When) + observe: one authoritative checkpoint per step, in order.
    let mut checkpoints = Vec::with_capacity(expectation.when.len() + 1);
    for when in &expectation.when {
        checkpoints.push(drive_and_capture(adapter, sut, when)?);
        steps.push(format!("{ACT_STEP_PREFIX}{when}"));
    }

    // The trailing checkpoint reads the SUT's end state — the timeline's close.
    checkpoints.push(capture(adapter, sut, FINAL_CHECKPOINT)?);

    Ok(Observation {
        path: expectation.path.clone(),
        checkpoints,
        trajectory: Trajectory { steps },
    })
}

/// Drive one `When` transition, then capture the authoritative checkpoint that
/// follows it, timing the whole drive-and-observe.
fn drive_and_capture<A: SurfaceAdapter>(
    adapter: &A,
    sut: &mut A::ProvisionedSut,
    when: &str,
) -> Result<Checkpoint, ExpectError> {
    let start = Instant::now();
    adapter.drive(sut, when)?;
    let state = adapter.observe(sut)?;
    Ok(Checkpoint {
        after: when.to_string(),
        state,
        duration: start.elapsed(),
    })
}

/// Capture a checkpoint labelled `after` from the SUT's current state, timing
/// the observation.
fn capture<A: SurfaceAdapter>(
    adapter: &A,
    sut: &A::ProvisionedSut,
    after: &str,
) -> Result<Checkpoint, ExpectError> {
    let start = Instant::now();
    let state = adapter.observe(sut)?;
    Ok(Checkpoint {
        after: after.to_string(),
        state,
        duration: start.elapsed(),
    })
}

/// Resolve the received-observation path for the spec identity `path` under
/// `repo_root`: `<repo_root>/.expect/received/<path>.received.json`.
///
/// The identity is a repo-relative path the loader derived, but a spec selected
/// by glob or crafted by hand could in principle carry an absolute or `..`
/// component; joining it verbatim would let a write escape `.expect/received/`.
/// Following the safe-join approach in [`crate::surface::cli`], the identity is
/// accepted only when it is relative and contains no parent-directory
/// component, which guarantees the result stays under the received directory.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when `path` is absolute or contains a
/// `..` component.
pub fn received_path(repo_root: &Path, path: &str) -> Result<PathBuf, ExpectError> {
    expect_artifact_path(repo_root, RECEIVED_SUBDIR, path, RECEIVED_EXTENSION)
}

/// Resolve the golden-baseline path for the spec identity `path` under
/// `repo_root`: `<repo_root>/.expect/goldens/<path>.golden.json`.
///
/// The committed counterpart to [`received_path`]: the golden tree mirrors each
/// spec's repo-relative identity (`ideas/expect.md` §"The dot-folder"). The same
/// safe-join applies — an absolute or `..`-bearing identity is rejected so a read
/// or write can never escape `.expect/goldens/`.
///
/// The golden store and its write side land with the drift ledger (a later task);
/// this resolver lets the `golden evaluate` op address the baseline file today.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when `path` is absolute or contains a
/// `..` component.
pub fn golden_path(repo_root: &Path, path: &str) -> Result<PathBuf, ExpectError> {
    expect_artifact_path(repo_root, GOLDEN_SUBDIR, path, GOLDEN_EXTENSION)
}

/// Resolve the `*.expect.md` spec file path for the identity `path` under
/// `repo_root`: `<repo_root>/<path>.expect.md`.
///
/// The mirror of [`received_path`]/[`golden_path`] for the spec leg of a
/// expectation's identity-mirrored fileset — the file `expectation delete`
/// removes alongside the received observation and golden. Unlike those two the
/// spec lives directly under the repo root (not under `.expect/`), but the same
/// [`validate_identity`] safe-join applies so an absolute or `..`-bearing
/// identity can never escape the repo root.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when `path` is absolute or contains a
/// `..` component.
pub fn spec_path(repo_root: &Path, path: &str) -> Result<PathBuf, ExpectError> {
    validate_identity(path)?;
    Ok(repo_root.join(format!("{path}{EXPECT_EXTENSION}")))
}

/// Safe-join a spec identity into an `.expect/<subdir>/<path><extension>` file
/// path under `repo_root`, the shared resolver behind [`received_path`] and
/// [`golden_path`].
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when `path` is absolute or contains a
/// `..` component (see [`validate_identity`]).
fn expect_artifact_path(
    repo_root: &Path,
    subdir: &str,
    path: &str,
    extension: &str,
) -> Result<PathBuf, ExpectError> {
    validate_identity(path)?;
    let dir = repo_root.join(EXPECT_DIR).join(subdir);
    Ok(dir.join(format!("{path}{extension}")))
}

/// The shared safe-join guard behind every identity-mirrored path resolver
/// ([`spec_path`], [`received_path`], [`golden_path`]).
///
/// The identity is a repo-relative path the loader derived, but a spec selected
/// by glob or crafted by hand could in principle carry an absolute or `..`
/// component; joining it verbatim would let a read or write escape the repo (or
/// the `.expect/` subdirectory). Following the safe-join approach in
/// [`crate::surface::cli`], the identity is accepted only when it is relative and
/// contains no parent-directory component.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when `path` is absolute or contains a
/// `..` component.
fn validate_identity(path: &str) -> Result<(), ExpectError> {
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(ExpectError::Expectation {
            path: path.to_string(),
            message: "identity must be a relative path without `..` components".to_string(),
        });
    }
    Ok(())
}

/// Persist `observation` to its received slot under `repo_root`, creating the
/// parent directories as needed, and return the path written.
///
/// The received file is the gitignored "last run per spec" — overwritten on each
/// observe — that `evaluate` later compares against the committed golden.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when the observation identity is unsafe
/// (see [`received_path`]), [`ExpectError::Json`] when it cannot be serialized,
/// or [`ExpectError::Io`] when the file cannot be written.
pub fn write_received(repo_root: &Path, observation: &Observation) -> Result<PathBuf, ExpectError> {
    let path = received_path(repo_root, &observation.path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(observation)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::Setup;
    use crate::types::SurfaceState;
    use std::cell::{Cell, RefCell};

    /// A surface adapter that counts its lifecycle calls and records every driven
    /// step, with a controllable [`SurfaceAdapter::resolves_mechanically`] gate so
    /// a test can assert the repeat count, the per-run re-arrange, and the
    /// provision granularity of [`observe_repeated`].
    #[derive(Default)]
    struct CountingAdapter {
        provisions: Cell<usize>,
        teardowns: Cell<usize>,
        driven: RefCell<Vec<String>>,
        mechanical: bool,
    }

    impl CountingAdapter {
        /// Build an adapter whose `When` steps resolve mechanically (deterministic
        /// surface) or not (forcing the agent-fallback repeat default).
        fn new(mechanical: bool) -> Self {
            CountingAdapter {
                mechanical,
                ..Default::default()
            }
        }

        /// How many recorded driven steps equal `step` — used to assert the
        /// `Given` was re-arranged once per run.
        fn driven_count(&self, step: &str) -> usize {
            self.driven.borrow().iter().filter(|s| *s == step).count()
        }
    }

    impl SurfaceAdapter for CountingAdapter {
        type ProvisionedSut = ();

        fn provision(&self, _setup: Option<&Setup>, _repo_root: &Path) -> Result<(), ExpectError> {
            self.provisions.set(self.provisions.get() + 1);
            Ok(())
        }

        fn drive(&self, _sut: &mut (), when_step: &str) -> Result<(), ExpectError> {
            self.driven.borrow_mut().push(when_step.to_string());
            Ok(())
        }

        fn observe(&self, _sut: &()) -> Result<SurfaceState, ExpectError> {
            Ok(SurfaceState::Json {
                body: serde_json::json!({}),
            })
        }

        fn teardown(&self, _sut: ()) -> Result<(), ExpectError> {
            self.teardowns.set(self.teardowns.get() + 1);
            Ok(())
        }

        fn resolves_mechanically(&self, _when_step: &str) -> bool {
            self.mechanical
        }
    }

    /// Parse a minimal cli spec with the given `reliability`/`isolation`
    /// frontmatter and `Given`/`When` bullets — the real parser, so the
    /// frontmatter defaults and section extraction are genuine.
    fn spec(reliability: &str, isolation: &str, given: &[&str], when: &[&str]) -> Expectation {
        let mut body = format!(
            "---\ndescription: a pass^k observe spec\nsurface: cli\nreliability: {reliability}\nisolation: {isolation}\n---\n\nIntent.\n"
        );
        if !given.is_empty() {
            body.push_str("\n## Given\n");
            for bullet in given {
                body.push_str(&format!("- {bullet}\n"));
            }
        }
        if !when.is_empty() {
            body.push_str("\n## When\n");
            for bullet in when {
                body.push_str(&format!("- {bullet}\n"));
            }
        }
        body.push_str("\n## Then\n- [ ] the exit code is 0\n");
        Expectation::parse(
            &body,
            Path::new("/repo/sample.expect.md"),
            Path::new("/repo"),
        )
        .expect("parse spec")
    }

    #[test]
    fn reliability_resolved_repeat_defaults_by_surface_and_policy() {
        // The default `pass^1`: a deterministic surface runs once; an agent-driven
        // run defaults to the ≥2 non-determinism floor.
        assert_eq!(resolved_repeat(1, None, true), MECHANICAL_DEFAULT_REPEAT);
        assert_eq!(resolved_repeat(1, None, false), AGENT_DRIVEN_MIN_REPEAT);

        // A declared `pass^3` is the run count for both surfaces (already at/above
        // the agent floor).
        assert_eq!(resolved_repeat(3, None, true), 3);
        assert_eq!(resolved_repeat(3, None, false), 3);

        // An explicit `repeat` is honored, but never below the `pass^k` floor.
        assert_eq!(resolved_repeat(1, Some(5), true), 5);
        assert_eq!(resolved_repeat(3, Some(1), true), 3);
    }

    #[test]
    fn observe_repeated_runs_pass_k_times_against_one_shared_instance() {
        let spec = spec("pass^3", "shared", &["seed the cart"], &["run checkout"]);
        let adapter = CountingAdapter::new(true);

        let observations = observe_repeated(&spec, &adapter, &ObserveConfig::new("/repo"))
            .expect("observe pass^3");

        assert_eq!(observations.len(), 3, "pass^3 runs observe three times");
        assert_eq!(
            adapter.provisions.get(),
            1,
            "shared isolation provisions once"
        );
        assert_eq!(
            adapter.teardowns.get(),
            1,
            "shared isolation tears down once"
        );
        assert_eq!(
            adapter.driven_count("seed the cart"),
            3,
            "the Given is re-established on every run, not just the first"
        );
        for observation in &observations {
            assert!(
                observation
                    .trajectory
                    .steps
                    .iter()
                    .any(|step| step.contains("seed the cart")),
                "each run re-records its arrange step"
            );
        }
    }

    #[test]
    fn observe_repeated_with_fresh_isolation_provisions_a_dedicated_instance_per_run() {
        let fresh = spec("pass^3", "fresh", &["seed the cart"], &["run checkout"]);
        let fresh_adapter = CountingAdapter::new(true);
        let fresh_runs = observe_repeated(&fresh, &fresh_adapter, &ObserveConfig::new("/repo"))
            .expect("observe fresh pass^3");

        assert_eq!(fresh_runs.len(), 3);
        assert_eq!(
            fresh_adapter.provisions.get(),
            3,
            "fresh isolation provisions a dedicated, pristine instance per run"
        );
        assert_eq!(fresh_adapter.teardowns.get(), 3);

        // The contrast: shared isolation provisions exactly once for the same
        // pass^3, so fresh's provision is distinct from the shared instance.
        let shared = spec("pass^3", "shared", &["seed the cart"], &["run checkout"]);
        let shared_adapter = CountingAdapter::new(true);
        observe_repeated(&shared, &shared_adapter, &ObserveConfig::new("/repo"))
            .expect("observe shared pass^3");
        assert_eq!(shared_adapter.provisions.get(), 1);
        assert!(fresh_adapter.provisions.get() > shared_adapter.provisions.get());
    }

    #[test]
    fn observe_repeated_defaults_to_a_single_run_for_a_deterministic_surface() {
        let spec = spec("pass^1", "shared", &[], &["run checkout"]);
        let adapter = CountingAdapter::new(true);

        let observations = observe_repeated(&spec, &adapter, &ObserveConfig::new("/repo"))
            .expect("observe deterministic");

        assert_eq!(
            observations.len(),
            1,
            "a deterministic cli spec defaults to a single run"
        );
        assert_eq!(adapter.provisions.get(), 1);
    }

    #[test]
    fn observe_repeated_defaults_to_at_least_two_runs_when_an_agent_drives() {
        let spec = spec(
            "pass^1",
            "shared",
            &[],
            &["explore and accomplish the behavior"],
        );
        let adapter = CountingAdapter::new(false);

        let observations =
            observe_repeated(&spec, &adapter, &ObserveConfig::new("/repo")).expect("observe agent");

        assert_eq!(
            observations.len(),
            AGENT_DRIVEN_MIN_REPEAT as usize,
            "an agent-driven spec defaults to at least two runs"
        );
    }

    #[test]
    fn received_path_follows_the_dot_expect_layout() {
        let repo = Path::new("/repo");
        let resolved = received_path(repo, "src/checkout/coupon").expect("safe identity");
        assert_eq!(
            resolved,
            repo.join(".expect/received/src/checkout/coupon.received.json")
        );
    }

    #[test]
    fn golden_path_follows_the_dot_expect_layout() {
        let repo = Path::new("/repo");
        let resolved = golden_path(repo, "src/checkout/coupon").expect("safe identity");
        assert_eq!(
            resolved,
            repo.join(".expect/goldens/src/checkout/coupon.golden.json")
        );
    }

    #[test]
    fn spec_path_resolves_directly_under_the_repo_root() {
        let repo = Path::new("/repo");
        let resolved = spec_path(repo, "src/checkout/coupon").expect("safe identity");
        assert_eq!(resolved, repo.join("src/checkout/coupon.expect.md"));
    }

    #[test]
    fn spec_path_rejects_parent_dir_traversal() {
        let repo = Path::new("/repo");
        let err = spec_path(repo, "../../etc/passwd").expect_err("traversal must be rejected");
        assert!(
            matches!(err, ExpectError::Expectation { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn golden_path_rejects_parent_dir_traversal() {
        let repo = Path::new("/repo");
        let err = golden_path(repo, "../../etc/passwd").expect_err("traversal must be rejected");
        assert!(
            matches!(err, ExpectError::Expectation { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn received_path_rejects_parent_dir_traversal() {
        let repo = Path::new("/repo");
        let err = received_path(repo, "../../etc/passwd").expect_err("traversal must be rejected");
        assert!(
            matches!(err, ExpectError::Expectation { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn received_path_rejects_absolute_identity() {
        let repo = Path::new("/repo");
        let err = received_path(repo, "/etc/passwd").expect_err("absolute must be rejected");
        assert!(
            matches!(err, ExpectError::Expectation { .. }),
            "got {err:?}"
        );
    }
}
