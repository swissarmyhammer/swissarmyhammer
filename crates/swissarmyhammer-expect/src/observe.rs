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
use crate::spec::Expectation;
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
    let mut steps = Vec::with_capacity(expectation.given.len() + expectation.when.len());

    // Arrange (Given): establish preconditions mechanically, without capturing a
    // checkpoint — the Given is setup state, not the behavior under test.
    for given in &expectation.given {
        adapter.drive(&mut sut, given)?;
        steps.push(format!("{ARRANGE_STEP_PREFIX}{given}"));
    }

    // Act (When) + observe: one authoritative checkpoint per step, in order.
    let mut checkpoints = Vec::with_capacity(expectation.when.len() + 1);
    for when in &expectation.when {
        checkpoints.push(drive_and_capture(adapter, &mut sut, when)?);
        steps.push(format!("{ACT_STEP_PREFIX}{when}"));
    }

    // The trailing checkpoint reads the SUT's end state — the timeline's close.
    checkpoints.push(capture(adapter, &sut, FINAL_CHECKPOINT)?);

    adapter.teardown(sut)?;

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
    let received_dir = repo_root.join(EXPECT_DIR).join(RECEIVED_SUBDIR);
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(ExpectError::Expectation {
            path: path.to_string(),
            message: "observation identity must be a relative path without `..` components"
                .to_string(),
        });
    }
    Ok(received_dir.join(format!("{path}{RECEIVED_EXTENSION}")))
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
