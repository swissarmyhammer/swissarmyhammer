//! Enforces that the `expect` builtin skill exists and tells an agent to
//! proactively capture acceptance-criteria-shaped intent from a conversation as
//! a behavioral expectation (`ideas/expect.md` §"expect expectation create",
//! chat source).
//!
//! The skill is the agent-layer behavior — recognizing intent mid-conversation
//! is the agent's job, not the tool's — so these tests assert the rendered skill
//! body actually carries the load-bearing instructions: invoke `expect
//! expectation create --from-chat`, push for negative/edge criteria and domain
//! invariants, and leave the draft unapproved for a human to approve.
//!
//! Failing this test means the authoring skill drifted away from the design and
//! would stop capturing expectations the way the spec requires.

mod common;
use common::rendered_builtin_instructions;

use swissarmyhammer_skills::{validate_description, SkillResolver, SkillSource};

/// The `expect` skill must load as a builtin with the required frontmatter:
/// the canonical name, a non-empty guide-compliant description, and a non-empty
/// body. This mirrors the loader's `validate_frontmatter` contract for one
/// specific skill so a missing or malformed `expect` skill fails loudly.
#[test]
fn expect_skill_loads_with_required_frontmatter() {
    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();

    let skill = builtins
        .get("expect")
        .expect("builtin skill 'expect' should exist");

    assert_eq!(skill.name.as_str(), "expect");
    assert_eq!(skill.source, SkillSource::Builtin);
    assert!(
        !skill.description.is_empty(),
        "expect skill must have a non-empty description"
    );
    validate_description(&skill.description)
        .expect("expect skill description must comply with the Anthropic guide limits");
    assert!(
        !skill.instructions.is_empty(),
        "expect skill must have a non-empty body"
    );
}

/// The rendered `expect` skill body must carry every load-bearing instruction
/// the design requires of the chat-capture authoring behavior. Each row is a
/// `(marker, requirement)` pair the body must contain — adding or removing a
/// required behavior is a data change to this table.
#[test]
fn expect_skill_renders_chat_capture_guidance() {
    let body = rendered_builtin_instructions("expect");

    let required_markers: &[(&str, &str)] = &[
        // The op the skill drives, with the chat source, on accept.
        (
            "expect expectation create",
            "name the `expect expectation create` op it invokes on accept",
        ),
        (
            "--from-chat",
            "drive the chat source via the `--from-chat` flag",
        ),
        // Push for the negative / right-reason criteria (agents are weak here).
        (
            "does NOT",
            "explicitly prompt for negative/edge criteria (\"and it does NOT do X\")",
        ),
        // Prefer invariants over frozen literals.
        ("invariant", "push for invariants over frozen literals"),
        // Hand off unapproved for the human to approve.
        (
            "unapproved",
            "leave the drafted spec unapproved for a human",
        ),
        // Distinct from "unapproved" above: the concrete approve op the human
        // runs to baseline the golden.
        (
            "observation approve",
            "point the human at the `observation approve` op to baseline the golden",
        ),
    ];

    for (marker, requirement) in required_markers {
        assert!(
            body.contains(marker),
            "expect skill must {requirement} (marker {marker:?})"
        );
    }
}
