//! What the shipped function-length tool-rule tests share.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rule
//! each tool decides. The tests of one language stand in the module that
//! language names: `function_length_rust`, `function_length_typescript`,
//! `function_length_swift`, `function_length_python`, `function_length_go` and
//! `function_length_dart`.
//!
//! One module stands for each language of the family, because one file for the
//! whole family runs past the byte cap a review prompt holds.

use super::*;

/// Acceptance: every shipped function-length tool rule passes its fixture pair
/// in doctor, and supersedes the one prompt rule its own tool decides.
///
/// The `supersedes` assertion is the load-bearing half. Every row names
/// `function-length` and nothing else, because that is the ONE size gate this
/// set states; a row that named a second rule would silence a gate no tool
/// measures.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_function_length_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(
        SHIPPED_FUNCTION_LENGTH_RULES,
        FUNCTION_LENGTH_PROMPT_RULE,
    );
}

/// A one-validator work-list over `path` for the builtin `code-hygiene` set,
/// naming the `function-length` prompt rule and the tool rule `rule`.
///
/// `rule` is a parameter because two languages drive this shape end to end:
/// `function-length-rust` for the workspace gate, and
/// `function-length-typescript` for the test carve-out.
pub(super) fn function_length_work(rule: &str, path: &str, content: &str) -> WorkList {
    WorkList::new(
        "a function over the length gate",
        vec![ValidatorWork::new(
            CODE_HYGIENE_SET,
            RuleNames::new([FUNCTION_LENGTH_PROMPT_RULE.to_string(), rule.to_string()]),
            ProbeNames::new([]),
            [FileWork::new(path, vec![], vec![], content, vec![])],
        )],
    )
}
