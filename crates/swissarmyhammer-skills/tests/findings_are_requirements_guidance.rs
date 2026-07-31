//! Enforces that the rule-obedience stance — a validator rule and a review
//! finding are requirements, not suggestions — has exactly ONE source of truth
//! under `builtin/`, and that every builtin skill that writes or judges code
//! renders it.
//!
//! The stance lives in `builtin/_partials/findings-are-requirements.md` and is
//! pulled into each skill via Liquid
//! `{% include "_partials/findings-are-requirements" %}`. The single-source check
//! is the important one here: before this partial existed the same stance was
//! written out longhand in both `finish` and `review`, and the two copies had
//! drifted to different exception lists.
//!
//! The coverage guard that renders all four agents and all three skills through
//! the production `TemplateLibrary` lives in
//! `crates/mirdan/tests/findings_are_requirements_coverage.rs` — mirdan is the one
//! crate that can see builtin agents, builtin skills, and the Liquid renderer at
//! once.

use std::path::Path;

mod common;
use common::{assert_guidance_single_source, rendered_builtin_instructions};

/// Builtin skills that write or judge code. Each must render the stance.
const COVERED_SKILLS: &[&str] = &["implement", "finish", "review"];

/// Sentences that exist ONLY in `builtin/_partials/findings-are-requirements.md`.
/// Finding one in a rendered skill proves the include expanded; finding one in
/// more than one `builtin/` source file means the stance was duplicated.
///
/// Two sentences, not one: the first is the anti-editorializing rule and the
/// second is the no-severity-tier rule. Both were previously duplicated in prose,
/// so both are pinned.
const CANONICAL_STANCE: &[&str] = &[
    "Do not decide you know better than the rule.",
    "There is no severity tier. Every finding is mandatory.",
];

/// Labels the stance forbids. The partial must name each one, so no agent can
/// invent a severity tier the rules do not have.
const BANNED_LABELS: &[&str] = &["nit", "minor", "cosmetic", "polish", "pedantry", "churn"];

#[test]
fn code_writing_skills_render_findings_are_requirements() {
    for name in COVERED_SKILLS {
        let body = rendered_builtin_instructions(name);
        assert!(
            !body.contains("{% include \"_partials/findings-are-requirements\" %}"),
            "builtin skill '{name}' must expand the findings-are-requirements include"
        );
        for stance in CANONICAL_STANCE {
            assert!(
                body.contains(stance),
                "builtin skill '{name}' must render the stance sentence: {stance}"
            );
        }
        for label in BANNED_LABELS {
            assert!(
                body.contains(&format!("\"{label}\"")),
                "builtin skill '{name}' must forbid labelling a finding '{label}'"
            );
        }
        assert!(
            body.contains("mark the task stuck"),
            "builtin skill '{name}' must route a true rule conflict to a stuck task"
        );
    }
}

#[test]
fn findings_are_requirements_has_single_source_of_truth() {
    for stance in CANONICAL_STANCE {
        assert_guidance_single_source(stance, Path::new("_partials/findings-are-requirements.md"));
    }
}
