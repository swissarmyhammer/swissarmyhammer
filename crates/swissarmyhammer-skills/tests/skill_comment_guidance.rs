//! Enforces that the work-the-card builtin skills (`implement`, `finish`,
//! `kanban`) instruct the agent to record a conversation log on the task via
//! the kanban `add comment` op, referencing the ops by their canonical names
//! (`add comment` / `list comments`).
//!
//! Failing this test means a skill body drifted and no longer tells agents to
//! leave a comment trail while working a card.

use swissarmyhammer_skills::SkillResolver;

/// Fetch a builtin skill body by name, failing the test if it is missing.
fn builtin_instructions(name: &str) -> String {
    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    builtins
        .get(name)
        .unwrap_or_else(|| panic!("builtin skill '{name}' should exist"))
        .instructions
        .clone()
}

/// Every work-the-card skill must reference the canonical `add comment` op so
/// agents record progress as a conversation log on the task.
#[test]
fn work_the_card_skills_instruct_add_comment() {
    for name in ["implement", "finish", "kanban"] {
        let body = builtin_instructions(name);
        assert!(
            body.contains("add comment"),
            "builtin skill '{name}' must instruct recording progress via the `add comment` op"
        );
    }
}

/// The general pick-up-a-card skill must also suggest reviewing prior context
/// with the canonical `list comments` op before starting work.
#[test]
fn kanban_skill_suggests_list_comments_for_prior_context() {
    let body = builtin_instructions("kanban");
    assert!(
        body.contains("list comments"),
        "builtin skill 'kanban' must suggest `list comments` to review prior context"
    );
}
