//! Acceptance tests for the shipped dead-code tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rule
//! it supersedes. The test under it drives Python through the real tool.

use super::*;

/// Acceptance: every shipped dead-code tool rule passes its fixture pair in
/// doctor, and supersedes the `dead-code` prompt rule.
///
/// The pass fixture is the load-bearing half. Each pass fixture holds the
/// same dead shapes its fail fixture holds, each behind the language's own
/// suppression marker, so a marker the tool stops reading makes the pair
/// fail. That marker is the whole staging contract: with it the tool
/// replaces the prompt rule's judgment, and without it the tool would
/// report staged work as dead.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_dead_code_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(SHIPPED_DEAD_CODE_RULES, DEAD_CODE_PROMPT_RULE);
}

/// A Python module with one statement stranded behind a `return`.
///
/// `__all__` names the one function, which is how Python declares an
/// exported surface and how vulture is told the function has callers
/// outside the module. Without it the run reports the function as unused
/// too, and the probe would measure two findings rather than the one
/// stranded statement it is built to measure.
const UNREACHABLE_MODULE_PY: &str = concat!(
    "\"\"\"A probe module for the shipped Python dead-code tool rule.\"\"\"\n\n",
    "__all__ = [\"stops_early\"]\n\n\n",
    "def stops_early():\n",
    "    \"\"\"Return a value, then strand the statement below it.\"\"\"\n",
    "    return 1\n",
    "    print(\"stranded\")\n",
);

/// The module path inside the probe repository, as the work-list holds it.
const UNREACHABLE_MODULE_PATH: &str = "src/stops_early.py";

/// A one-validator work-list over `files` for the builtin `code-hygiene`
/// set, naming both the `dead-code` prompt rule and the Python tool rule.
fn dead_code_work(path: &str, content: &str) -> WorkList {
    WorkList::new(
        "a statement behind a return",
        vec![ValidatorWork::new(
            CODE_HYGIENE_SET,
            RuleNames::new([
                DEAD_CODE_PROMPT_RULE.to_string(),
                PYTHON_DEAD_CODE_RULE.to_string(),
            ]),
            ProbeNames::new([]),
            [FileWork::new(path, vec![], vec![], content, vec![])],
        )],
    )
}

/// Acceptance: the shipped Python dead-code tool rule suppresses the
/// `dead-code` prompt rule, and reports the stranded statement through the
/// real vulture pipeline.
///
/// The suppression half is the load-bearing one: a healthy tool rule
/// suppresses whatever its `supersedes` names. Superseding is what makes
/// dead code objective for Python — the prompt rule's carve-outs for the
/// exported surface and for staged work become `__all__` and
/// `# noqa: V1xx`, which the tool reads.
///
/// The test states the tool as a precondition — [`require_tool_installed`]
/// names the missing tool and the command that installs it — and then
/// REQUIRES the run. A rule that planned no run fails the test and names
/// the plan's fallbacks, which carry the doctor's reason. Returning early
/// instead would leave the test asserting nothing, and a test that cannot
/// fail is not a gate.
#[test]
fn the_shipped_python_dead_code_tool_rule_reports_and_suppresses_dead_code() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join(UNREACHABLE_MODULE_PATH),
        UNREACHABLE_MODULE_PY,
    )
    .unwrap();
    let loader = builtin_loader();
    let project_types = ["python"];
    require_tool_installed(&loader, &project_types, PYTHON_DEAD_CODE_RULE);
    let work = dead_code_work(UNREACHABLE_MODULE_PATH, UNREACHABLE_MODULE_PY);

    let plan = plan_tool_rules(&work, &loader, &project_types, None);

    let run = required_run(&plan, PYTHON_DEAD_CODE_RULE);
    assert_eq!(run.files(), [UNREACHABLE_MODULE_PATH.to_string()]);
    assert!(
        plan.suppression()
            .suppressed_rules(CODE_HYGIENE_SET, UNREACHABLE_MODULE_PATH)
            .contains(DEAD_CODE_PROMPT_RULE),
        "a healthy dead-code tool rule must suppress the `dead-code` prompt rule, \
         so no LLM re-reads a question the tool already decided"
    );

    verify_run_reports_one_finding(
        run,
        repo.path(),
        UNREACHABLE_MODULE_PATH,
        CODE_HYGIENE_SET,
        PYTHON_DEAD_CODE_RULE,
        "unreachable code after",
    );
}
