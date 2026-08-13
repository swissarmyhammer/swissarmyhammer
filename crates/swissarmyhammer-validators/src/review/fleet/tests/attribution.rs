//! Pinning a cited rule name onto the roster.
//!
//! An agent names the rule it fired in its own spelling. These tests hold
//! `resolve_rule` to a name the validator's loaded roster really carries, and
//! hold `tag_findings` to reporting no rule at all rather than an invented
//! one.

use super::*;

/// Rules named `names`, as a shard's prompt-rule list.
fn rules_named(names: &[&str]) -> Vec<Rule> {
    names
        .iter()
        .map(|name| Rule {
            name: (*name).to_string(),
            ..Rule::default()
        })
        .collect()
}

/// A rule roster of `names`, as the loaded validator carries it.
fn roster_named(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn resolve_rule_takes_a_name_the_roster_carries_verbatim() {
    let shown = rules_named(&["magic-numbers", "dead-code"]);
    let roster = roster_named(&["magic-numbers", "dead-code"]);
    assert_eq!(
        resolve_rule(Some("dead-code"), &shown, &roster).as_deref(),
        Some("dead-code")
    );
}

#[test]
fn resolve_rule_normalizes_spelling_onto_the_roster_name() {
    // A model writes the rule's title, not its file name. The roster spelling
    // is the one that reaches the report either way.
    let shown = rules_named(&["magic-numbers", "dead-code"]);
    let roster = roster_named(&["magic-numbers", "dead-code"]);
    assert_eq!(
        resolve_rule(Some("Magic Numbers"), &shown, &roster).as_deref(),
        Some("magic-numbers")
    );
    assert_eq!(
        resolve_rule(Some("dead_code"), &shown, &roster).as_deref(),
        Some("dead-code")
    );
}

#[test]
fn resolve_rule_reads_a_wrapped_name_when_exactly_one_roster_rule_fits() {
    // "no-magic-numbers" wraps the roster's "magic-numbers" and no other
    // roster rule is a candidate, so the attribution is unambiguous.
    let shown = rules_named(&["magic-numbers", "dead-code"]);
    let roster = roster_named(&["magic-numbers", "dead-code", "magic-numbers-python"]);
    assert_eq!(
        resolve_rule(Some("no-magic-numbers"), &shown, &roster).as_deref(),
        Some("magic-numbers")
    );
}

#[test]
fn resolve_rule_refuses_a_name_that_fits_several_roster_rules() {
    // "dead" fits two roster rules; guessing between them would attribute the
    // finding to a rule that may not have fired.
    let shown = rules_named(&["dead-code", "dead-code-rust"]);
    let roster = roster_named(&["dead-code", "dead-code-rust"]);
    assert_eq!(resolve_rule(Some("dead"), &shown, &roster), None);
}

#[test]
fn resolve_rule_attributes_a_single_rule_shard_with_no_cited_name() {
    // The shard put exactly one rule in front of the agent, so that rule is
    // the only one that could have fired — this is certainty, not a guess.
    let shown = rules_named(&["reuse"]);
    let roster = roster_named(&["reuse"]);
    assert_eq!(
        resolve_rule(None, &shown, &roster).as_deref(),
        Some("reuse")
    );
}

#[test]
fn resolve_rule_reaches_a_roster_rule_the_shard_did_not_show() {
    // A rule a healthy tool rule superseded leaves the shard but stays in the
    // roster. Naming it still points the reader at a real rule document.
    let shown = rules_named(&["data-driven"]);
    let roster = roster_named(&["data-driven", "dead-code"]);
    assert_eq!(
        resolve_rule(Some("dead-code"), &shown, &roster).as_deref(),
        Some("dead-code")
    );
}

#[test]
fn resolve_rule_reports_no_attribution_for_a_multi_rule_shard_with_no_name() {
    let shown = rules_named(&["magic-numbers", "dead-code"]);
    let roster = roster_named(&["magic-numbers", "dead-code"]);
    assert_eq!(resolve_rule(None, &shown, &roster), None);
}

#[test]
fn tag_findings_replaces_an_invented_rule_name_with_the_roster_name() {
    // The agent is not the authority on either half of the attribution: the
    // engine overwrites the validator AND resolves the rule, so a report can
    // never name a set or a rule the roster does not carry.
    let findings = vec![Finding {
        file: "src/a.rs".to_string(),
        line: TEST_FINDING_LINE,
        validator: "agent-tagged".to_string(),
        rule: Some("r".to_string()),
        claim: "c".to_string(),
        evidence: "e".to_string(),
        suggestion: None,
    }];
    let shown = rules_named(&["magic-numbers", "dead-code"]);
    let roster = roster_named(&["magic-numbers", "dead-code"]);

    let tagged = tag_findings(findings, "code-hygiene", &shown, &roster);

    assert_eq!(tagged[0].validator, "code-hygiene");
    assert_eq!(
        tagged[0].rule, None,
        "an invented rule name must not survive into the report: {tagged:?}"
    );
}
