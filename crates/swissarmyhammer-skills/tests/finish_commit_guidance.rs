//! Enforces that the `finish` builtin skill instructs creating a **local
//! commit after each task reaches `done`** — a per-task rollback point in a
//! multi-task `/finish` run.
//!
//! The decision (see kanban task `^d3ghp7b`) is that `finish` orchestrates this
//! by delegating to the `/commit` skill (which stages all changes), commits
//! **locally only**, and never pushes — pushing is the user's separate step so
//! batch runs don't spam CI per task. Because scoped-batch mode reuses the
//! single-task loop, one commit per finished task falls out automatically.
//!
//! These tests assert against the *rendered* `finish` skill body (partial
//! includes expanded the way the deploy pipeline expands them). Failing this
//! test means the commit-on-done guidance drifted out of `builtin/skills/finish/
//! SKILL.md`.

mod common;
use common::rendered_builtin_instructions;

/// Alternative phrasings that all express "finish does not push". The skill only
/// needs to use one of them; new wordings can be added here without touching the
/// assertion logic below.
const NO_PUSH_PHRASES: &[&str] = &[
    "never push",
    "not push",
    "no push",
    "don't push",
    "does not push",
];

/// The rendered `finish` skill must instruct delegating to `/commit` once a task
/// is confirmed in `done`, and must scope it to a local-only commit with no push.
#[test]
fn finish_skill_renders_commit_on_done_guidance() {
    let body = rendered_builtin_instructions("finish");

    assert!(
        body.contains("/commit"),
        "finish skill must delegate committing to the `/commit` skill"
    );
    assert!(
        body.contains("done"),
        "finish skill must tie the commit to a task reaching `done`"
    );

    // Local-only — explicitly NOT a push. The skill must say both that the
    // commit is local and that finish does not push.
    let lower = body.to_lowercase();
    assert!(
        lower.contains("local"),
        "finish skill must state the commit is a LOCAL commit"
    );
    assert!(
        NO_PUSH_PHRASES.iter().any(|p| lower.contains(p)),
        "finish skill must state it does NOT push (pushing is the user's separate step)"
    );
}
