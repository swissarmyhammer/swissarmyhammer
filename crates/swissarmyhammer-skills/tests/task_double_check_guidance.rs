//! Enforces that the `task` builtin skill instructs the agent to double-check
//! the freshly created card before reporting done — an adversarial self-review
//! that catches stale/hallucinated paths, missing template sections, vague
//! acceptance criteria, manual-verification tests, and oversized scope.
//!
//! The guidance lives in ONE shared partial —
//! `builtin/_partials/task-double-check.md` — pulled into the `task` skill via
//! Liquid `{% include "_partials/task-double-check" %}`. These tests assert
//! against the *rendered* skill body (includes expanded the way the deploy
//! pipeline expands them) and that the canonical guidance text has exactly one
//! source of truth under `builtin/`.
//!
//! Failing this test means the task skill stopped telling agents to verify
//! their own card — or the guidance got duplicated outside the partial.

use std::path::Path;

mod common;
use common::{assert_guidance_single_source, rendered_builtin_instructions};

/// A sentence that exists ONLY in `builtin/_partials/task-double-check.md`.
/// Finding it in the rendered skill proves the include expanded; finding it in
/// more than one `builtin/` source file means the guidance was duplicated.
const CANONICAL_GUIDANCE: &str =
    "Do NOT spawn the diff-oriented `double-check` agent to verify a task card";

/// The rendered `task` skill body must carry the full double-check guidance:
/// the include must be expanded, and every adversarial self-review check the
/// card mandates must be present in the rendered text.
#[test]
fn task_skill_renders_double_check_guidance() {
    let body = rendered_builtin_instructions("task");

    assert!(
        !body.contains("{% include \"_partials/task-double-check\" %}"),
        "builtin skill 'task' must expand the task-double-check include"
    );

    // Each adversarial self-review check the card mandates is a single required
    // marker: the rendered guidance must contain every `(marker, requirement)`
    // pair below. Adding or removing a check is a data change to this table,
    // mirroring the `sym_op` and `section` loops further down.
    let required_markers: &[(&str, &str)] = &[
        // Re-read the created card.
        ("get task", "instruct re-reading the card with `get task`"),
        // Verify named paths/symbols actually exist.
        (
            "code_context",
            "instruct verifying paths/symbols via the `code_context` tool",
        ),
        // Sizing limits (≤5 files, ≤5 subtasks).
        ("5 files", "restate the ≤5 files sizing limit"),
        ("5 subtasks", "restate the ≤5 subtasks sizing limit"),
        // Acceptance criteria observable, not vague.
        (
            "observable",
            "require acceptance criteria be observable, not vague",
        ),
        // Fix-and-re-verify loop.
        (
            "update task",
            "describe the fix-and-re-verify loop via `update task`",
        ),
        // Explicit warning against the diff-oriented double-check agent.
        (
            CANONICAL_GUIDANCE,
            "warn against spawning the diff-oriented `double-check` agent",
        ),
    ];
    for (marker, requirement) in required_markers {
        assert!(
            body.contains(marker),
            "double-check guidance must {requirement} (marker {marker:?})"
        );
    }

    // The two `code_context` symbol ops used to verify symbols exist.
    for sym_op in ["search symbol", "get symbol"] {
        assert!(
            body.contains(sym_op),
            "double-check guidance must name the `{sym_op}` op for verifying symbols exist"
        );
    }

    // All four required sections present.
    for section in [
        "## What",
        "## Acceptance Criteria",
        "## Tests",
        "## Workflow",
    ] {
        assert!(
            body.contains(section),
            "double-check guidance must require the `{section}` section be present"
        );
    }

    // Tests automated — at least one manual-verification phrase must be named so
    // the guidance can forbid it. This is an OR (either phrase suffices), so it
    // stays a distinct assertion rather than a row in the required-markers table.
    assert!(
        body.contains("manually verify") || body.contains("smoke test"),
        "double-check guidance must forbid manual-verification language in the Tests section"
    );
}

/// The canonical guidance sentence must exist in exactly one `builtin/` source
/// file — the shared partial. Any duplication means the guidance drifted back
/// to an inlined copy.
#[test]
fn task_double_check_guidance_has_single_source_of_truth() {
    assert_guidance_single_source(
        CANONICAL_GUIDANCE,
        Path::new("_partials/task-double-check.md"),
    );
}
