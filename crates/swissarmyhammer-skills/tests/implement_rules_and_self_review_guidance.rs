//! Enforces that the `implement` builtin skill tells the agent to read the
//! validator rules BEFORE it edits a file, and to review its own work until the
//! review is clean BEFORE it hands the task off.
//!
//! Both steps exist to stop the same failure: an implementer that never reads
//! the rules writes code the review engine then rejects, and every fix pass adds
//! more unread-rule code. Reading the rules first and self-reviewing before
//! handoff replaces those review iterations.
//!
//! The guidance lives inline in `builtin/skills/implement/SKILL.md`. These tests
//! assert against the *rendered* skill body — includes expanded the way the
//! deploy pipeline expands them — so a partial that swallowed the text would
//! still satisfy them.
//!
//! Failing these tests means the implement skill stopped telling agents to read
//! the rules first, stopped telling them to self-review, or reordered the steps
//! so either one lands after the work it is supposed to guard.

mod common;
use common::rendered_builtin_instructions;

/// The rules fetch the skill must prescribe: ONE `dump validators` call that
/// carries one example file for each distinct extension, so the returned file
/// holds every applicable rule body.
const RULES_CALL: &str =
    r#"{"op": "dump validators", "paths": ["<one example file per extension>"]}"#;

/// The self-review call the skill must prescribe before handoff.
const SELF_REVIEW_CALL: &str = r#"{"op": "review working"}"#;

/// The heading of the step that does the editing. The rules fetch must precede
/// it, and the self-review must follow it.
const IMPLEMENT_STEP: &str = "### Implement";

/// The adversarial verification the card places *after* the self-review. The
/// self-review must precede it, so the work is already review-clean by the time
/// it is verified.
const DOUBLE_CHECK_STEP: &str = "/double-check";

/// The heading of the step that hands the task off for the formal review. The
/// self-review must precede it.
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

/// The skill must prescribe `review working` on its own changes, repeated until
/// clean, and must place that step after the editing and before the handoff.
#[test]
fn implement_skill_prescribes_self_review_until_clean_before_handoff() {
    let body = rendered_builtin_instructions("implement");

    let implement_at = offset_of(&body, IMPLEMENT_STEP, "keep its `Implement` step");
    let review_at = offset_of(
        &body,
        SELF_REVIEW_CALL,
        "prescribe the `review working` self-review call",
    );
    let double_check_at = offset_of(&body, DOUBLE_CHECK_STEP, "keep its double-check step");
    let handoff_at = offset_of(&body, HANDOFF_STEP, "keep its handoff step");
    assert!(
        implement_at < review_at,
        "the self-review must run after the work, not before it"
    );
    assert!(
        review_at < double_check_at,
        "the self-review must run before `/double-check`, so the work it verifies is already review-clean"
    );
    assert!(
        double_check_at < handoff_at,
        "both the self-review and `/double-check` must run before the handoff"
    );

    // Each phrase the self-review step must carry, with the requirement it
    // encodes.
    let required_markers: &[(&str, &str)] = &[
        // Every finding is fixed — no ranking, no deferring, no labelling.
        ("Fix every finding", "require fixing every finding"),
        (
            "A finding is a requirement",
            "state that a finding is a requirement",
        ),
        ("Do not rank findings", "forbid ranking findings"),
        ("Do not defer findings", "forbid deferring findings"),
        ("Do not label findings", "forbid labelling findings"),
        // The loop terminates on clean, not on effort spent.
        (
            "until the review is clean",
            "require repeating the review until it is clean",
        ),
    ];
    for (marker, requirement) in required_markers {
        assert!(
            body.contains(marker),
            "the implement skill must {requirement} (marker {marker:?})"
        );
    }
}
