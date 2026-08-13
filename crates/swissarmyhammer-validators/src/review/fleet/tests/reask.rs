//! What a forked task does when its first reply cannot be read.
//!
//! A warm fork is re-asked once. A second unreadable reply fails the pair,
//! which is what makes the run report itself incomplete.

use super::*;

/// The needle that matches ONLY a re-ask turn: a distinctive phrase of
/// [`REASK_PROMPT`], which no other prompt in the run carries.
const REASK_NEEDLE: &str = "could not be read as JSON";

/// The one-validator, one-file work the re-ask tests share. The validator's
/// suffix carries `MANDATE_MARKER`, so a script entry keyed on that marker
/// matches the first-pass fork.
fn reask_work_and_loader() -> (WorkList, ValidatorLoader) {
    let loader = loader_with(vec![ruleset(
        "val",
        "MANDATE_MARKER mandate",
        &[("r1", "RULE1_MARKER body")],
    )]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![file_work("src/f0.rs", "sym0", "src/x.rs")],
        )],
    );
    (work, loader)
}

/// A warm forked task whose first reply cannot be read is re-asked once on a
/// fork of the session that answered, and the second reply's findings land —
/// the task is not failed, so the run stays complete.
#[tokio::test]
async fn a_forked_task_recovers_from_an_unreadable_reply_on_one_re_ask() {
    let (work, loader) = reask_work_and_loader();

    // Script order matters: a re-ask fork inherits the first pass's context, so
    // its needle must be tried BEFORE the first-pass marker. Likewise the sweep
    // needle comes first of all.
    let agent = forking_agent(vec![
        rescan_finds_nothing(),
        (
            REASK_NEEDLE.to_string(),
            ScriptedReply::Text(findings_json(
                "src/f0.rs",
                TEST_FINDING_LINE,
                "r1",
                "recovered on the re-ask",
            )),
        ),
        (
            "MANDATE_MARKER".to_string(),
            ScriptedReply::Text(malformed_findings_json()),
        ),
    ]);
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    let reasks = agent_probe
        .seen_prompts()
        .iter()
        .filter(|prompt| prompt.contains(REASK_NEEDLE))
        .count();
    assert_eq!(reasks, 1, "the validator is re-asked exactly once");

    assert_eq!(outcome.attempted(), 1);
    assert_eq!(
        outcome.failed(),
        0,
        "a task that answers on the re-ask is not a failed task"
    );
    assert_eq!(outcome.findings.len(), 1, "{:#?}", outcome.findings);
    assert_eq!(outcome.findings[0].claim, "recovered on the re-ask");
    assert_eq!(outcome.findings[0].validator, "val");
}

/// The re-ask is ONE re-ask on the warm path too: a validator that answers
/// unreadably twice fails its pair, which is what makes the run report itself
/// INCOMPLETE.
#[tokio::test]
async fn a_forked_task_fails_after_a_second_unreadable_reply() {
    let (work, loader) = reask_work_and_loader();

    // The re-ask fork inherits the first pass's context, so this single entry
    // answers both turns with the same unreadable reply.
    let agent = forking_agent(vec![(
        "MANDATE_MARKER".to_string(),
        ScriptedReply::Text(malformed_findings_json()),
    )]);
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    let reasks = agent_probe
        .seen_prompts()
        .iter()
        .filter(|prompt| prompt.contains(REASK_NEEDLE))
        .count();
    assert_eq!(reasks, 1, "the task is re-asked once and only once");

    assert_eq!(outcome.attempted(), 1);
    assert_eq!(
        outcome.failed(),
        1,
        "two unreadable replies fail the pair: {:#?}",
        outcome.findings
    );
    assert!(outcome.findings.is_empty());
}
