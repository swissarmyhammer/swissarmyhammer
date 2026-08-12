//! What the shipped complexity tool-rule tests share.
//!
//! One test holds the whole roster to its fixture pair and to the prompt
//! rules each tool decides. The tests of one language stand in the module
//! that language names: `complexity_rust`, `complexity_typescript`,
//! `complexity_swift`, `complexity_python` and `complexity_go`.
//!
//! One module stands for each language of the family, because one file for
//! the whole family runs past the byte cap a review prompt holds.

use super::*;

/// Acceptance: every shipped complexity tool rule passes its fixture pair
/// in doctor, and supersedes exactly the gates its own tool decides.
///
/// The `supersedes` assertion is the load-bearing half. `complexity-rust`
/// must name both prompt rules, because one `cargo clippy` run answers
/// both; naming only one would leave an agent re-reading the probe for the
/// gate the tool already decided. The two Python rules must name one each,
/// because ruff decides one gate per lint; naming both from either rule
/// would silence a gate no tool measures.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_complexity_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(
        SHIPPED_COMPLEXITY_RULES,
        COGNITIVE_COMPLEXITY_PROMPT_RULE,
    );
}

/// A one-validator work-list over `path` for the builtin `code-hygiene`
/// set, naming both complexity prompt rules and the tool rule `rule`.
///
/// `rule` is a parameter because two languages drive this shape end to end:
/// `complexity-rust` for the nesting gate, and `complexity-typescript` for
/// the test carve-out.
pub(super) fn complexity_work(rule: &str, path: &str, content: &str) -> WorkList {
    WorkList::new(
        "a function over a complexity gate",
        vec![ValidatorWork::new(
            CODE_HYGIENE_SET,
            RuleNames::new([
                COGNITIVE_COMPLEXITY_PROMPT_RULE.to_string(),
                FUNCTION_LENGTH_PROMPT_RULE.to_string(),
                rule.to_string(),
            ]),
            ProbeNames::new([]),
            [FileWork::new(path, vec![], vec![], content, vec![])],
        )],
    )
}
