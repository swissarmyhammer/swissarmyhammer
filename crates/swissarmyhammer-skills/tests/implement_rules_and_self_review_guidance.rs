//! Makes sure the `implement` builtin skill tells the agent to read the
//! validator rules BEFORE it edits a file, and to not run its own review loop.
//!
//! The stance is this. The agent reads the rules one time, before the edit.
//! The agent keeps the rules in mind while it writes the code. The agent does
//! not run the `review working` self-review loop. The formal `/review` skill
//! and step do the review. The `/double-check` step is removed.
//!
//! The guidance lives inline in `builtin/skills/implement/SKILL.md`. These tests
//! assert against the *rendered* skill body — includes expanded the way the
//! deploy pipeline expands them — so a partial that swallowed the text would
//! still satisfy them.
//!
//! A failure of these tests means one of two regressions. The skill stopped
//! telling agents to read the rules first. Or the skill started to prescribe a
//! self-review loop again.

mod common;
use common::rendered_builtin_instructions;

/// The rules fetch the skill must prescribe: ONE `dump validators` call that
/// carries one example file for each distinct extension, so the returned file
/// holds every applicable rule body.
const RULES_CALL: &str =
    r#"{"op": "dump validators", "paths": ["<one example file per extension>"]}"#;

/// The self-review call the skill must NOT prescribe. The formal `/review`
/// step owns the review.
const SELF_REVIEW_CALL: &str = r#"{"op": "review working"}"#;

/// The heading of the step that does the editing. The rules fetch must precede
/// it.
const IMPLEMENT_STEP: &str = "### Implement";

/// The removed adversarial step. The skill must NOT prescribe it.
const DOUBLE_CHECK_STEP: &str = "/double-check";

/// The heading of the step that hands the task off for the formal review.
const HANDOFF_STEP: &str = "### Leave the task in `doing` for review";

/// Locate a marker in the rendered body, failing with the requirement it carries.
fn offset_of(body: &str, marker: &str, requirement: &str) -> usize {
    body.find(marker)
        .unwrap_or_else(|| panic!("the implement skill must {requirement} (marker {marker:?})"))
}

/// The skill must prescribe the one-call-per-extension rules fetch, and must
/// place it before the step that edits code — rules read after the edit are
/// review findings, not guidance.
#[test]
fn implement_skill_prescribes_the_rules_call_before_editing() {
    let body = rendered_builtin_instructions("implement");

    let rules_at = offset_of(
        &body,
        RULES_CALL,
        "prescribe the `dump validators` call with one example file per extension",
    );
    let implement_at = offset_of(&body, IMPLEMENT_STEP, "keep its `Implement` step");
    assert!(
        rules_at < implement_at,
        "the rules fetch must come before the `Implement` step, not after it"
    );

    // Each phrase the rules step must carry, with the requirement it encodes.
    let required_markers: &[(&str, &str)] = &[
        // One example file per extension — a call per file (or a loop of
        // `get validator` calls) is the failure mode this replaces.
        (
            "one example file for each extension",
            "state that the fetch takes one example file for each extension",
        ),
        // The fetch happens before the edit, not after.
        (
            "before you edit a file",
            "place the fetch before the file is edited",
        ),
        // The returned file is read whole, one time.
        (
            "Read that file whole, one time",
            "require reading the returned rules file whole, one time",
        ),
        // A re-fetch happens only for a new extension.
        (
            "Call again only when a later edit targets a file with a new extension",
            "limit re-fetches to files with a new extension",
        ),
        // The response is authoritative: the bodies arrive verbatim.
        (
            "word for word",
            "say the returned file carries the rule bodies word for word",
        ),
        // The rules bind while the code is written, not afterwards.
        (
            "as you write the code, not after",
            "require obeying each rule while the code is written",
        ),
        // The six obey-items the card names, each a rule class the review
        // engine enforces on the finished code.
        ("Document each public item", "name the public-docs rule"),
        ("Name each numeric constant", "name the magic-number rule"),
        ("Do not copy blocks", "name the duplication rule"),
        (
            "Keep functions small and flat",
            "name the function-size and nesting rule",
        ),
        ("Follow the project naming", "name the naming rule"),
        ("Delete dead code", "name the dead-code rule"),
    ];
    for (marker, requirement) in required_markers {
        assert!(
            body.contains(marker),
            "the implement skill must {requirement} (marker {marker:?})"
        );
    }

    assert!(
        !body.contains("get validator"),
        "the implement skill must not send agents through a per-rule `get validator` loop"
    );
}

/// The skill must NOT prescribe a self-review loop, and must NOT prescribe the
/// `/double-check` step. The validators are preloaded and bind while the agent
/// codes. The formal `/review` skill and step own the review. The handoff step
/// must stay.
#[test]
fn implement_skill_leaves_review_to_the_review_step() {
    let body = rendered_builtin_instructions("implement");

    assert!(
        !body.contains(SELF_REVIEW_CALL),
        "the implement skill must not prescribe the `review working` self-review call \
         (marker {SELF_REVIEW_CALL:?})"
    );
    assert!(
        !body.contains(DOUBLE_CHECK_STEP),
        "the implement skill must not prescribe the `/double-check` step \
         (marker {DOUBLE_CHECK_STEP:?})"
    );

    // Each phrase the new stance must carry, with the requirement it encodes.
    let required_markers: &[(&str, &str)] = &[
        // The rules are already loaded and bind while the code is written.
        (
            "you should have all the validators preloaded and keep them in mind as you code",
            "state that the validators are preloaded and bind while the agent codes",
        ),
        // The formal review step owns the review.
        (
            "Let the `/review` skill and step take care of reviewing",
            "leave the review to the `/review` skill and step",
        ),
    ];
    for (marker, requirement) in required_markers {
        assert!(
            body.contains(marker),
            "the implement skill must {requirement} (marker {marker:?})"
        );
    }

    offset_of(&body, HANDOFF_STEP, "keep its handoff step");
}

/// The skill must bind the agent to the card: the card is an order, every
/// requirement gets implemented, silent scope-narrowing is forbidden, and the
/// only exit from a requirement is a recorded blocker with a `stuck` outcome.
/// A missing requirement reported as `changed` is the fire-a-human failure
/// mode this section exists to prevent.
#[test]
fn implement_skill_binds_the_agent_to_the_card() {
    let body = rendered_builtin_instructions("implement");

    // Each phrase the obedience contract must carry, with the requirement it
    // encodes.
    let required_markers: &[(&str, &str)] = &[
        (
            "The card is a decision, not a proposal",
            "state that the card is a decision, not a proposal",
        ),
        (
            "Implement every requirement on the card",
            "require implementing every requirement on the card",
        ),
        (
            "Deciding not to do a listed item is forbidden",
            "forbid deciding not to do a listed item",
        ),
        (
            "Do not re-evaluate a decision the card records",
            "forbid re-evaluating decisions recorded on the card",
        ),
        (
            "record the blocker on the card and report `stuck`",
            "make a recorded blocker plus `stuck` the only exit",
        ),
        (
            "re-read the card and check every requirement against your diff",
            "require a card-versus-diff completeness check before reporting",
        ),
        (
            "never `changed` presented as complete",
            "forbid reporting `changed` when a requirement is missing",
        ),
    ];
    for (marker, requirement) in required_markers {
        assert!(
            body.contains(marker),
            "the implement skill must {requirement} (marker {marker:?})"
        );
    }
}
