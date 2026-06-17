//! Enforces that the work-the-card builtin skills (`implement`, `finish`,
//! `kanban`) instruct the agent to keep a conversation log on the task via
//! the kanban comment ops (`add comment` / `list comments`).
//!
//! The guidance lives in ONE shared partial —
//! `builtin/_partials/record-progress.md` — pulled into each skill via Liquid
//! `{% include "_partials/record-progress" %}`. These tests assert against the
//! *rendered* skill bodies (includes expanded the way the deploy pipeline
//! expands them) and that the canonical guidance text has exactly one source
//! of truth under `builtin/`.
//!
//! Failing this test means a skill body drifted and no longer tells agents to
//! leave a comment trail — or the guidance got duplicated outside the partial.

use std::path::Path;

mod common;
use common::{assert_guidance_single_source, rendered_builtin_instructions};

/// A sentence that exists ONLY in `builtin/_partials/record-progress.md`.
/// Finding it in a rendered skill proves the include expanded; finding it in
/// more than one `builtin/` source file means the guidance was duplicated.
const CANONICAL_GUIDANCE: &str = "burn the same tokens repeating them";

/// Every work-the-card skill's rendered body must carry the full record-progress
/// guidance: the `add comment` op, reading prior context with `list comments`,
/// and explicit instructions to record failures, discoveries, and blockers —
/// not just milestones.
#[test]
fn work_the_card_skills_render_record_progress_guidance() {
    for name in ["implement", "finish", "kanban"] {
        let body = rendered_builtin_instructions(name);
        assert!(
            !body.contains("{% include \"_partials/record-progress\" %}"),
            "builtin skill '{name}' must expand the record-progress include"
        );
        assert!(
            body.contains("add comment"),
            "builtin skill '{name}' must instruct recording progress via the `add comment` op"
        );
        assert!(
            body.contains("list comments"),
            "builtin skill '{name}' must instruct reading prior context via `list comments`"
        );
        for (marker, what) in [
            ("Milestones", "milestones"),
            ("did not work", "failed approaches / dead ends"),
            ("discoveries", "interesting discoveries"),
            ("Blockers", "blockers"),
        ] {
            assert!(
                body.contains(marker),
                "builtin skill '{name}' must instruct recording {what} (marker '{marker}')"
            );
        }
        assert!(
            body.contains(CANONICAL_GUIDANCE),
            "builtin skill '{name}' must render the canonical record-progress guidance"
        );
    }
}

/// The canonical guidance sentence must exist in exactly one `builtin/` source
/// file — the shared partial. Any duplication means the skills drifted back to
/// inlined copies.
#[test]
fn record_progress_guidance_has_single_source_of_truth() {
    assert_guidance_single_source(
        CANONICAL_GUIDANCE,
        Path::new("_partials/record-progress.md"),
    );
}
