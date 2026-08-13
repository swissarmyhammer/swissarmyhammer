//! What a run reports while it runs.
//!
//! The progress stream carries one pair event for every (validator, file)
//! pair, including a pair whose task fails. The failure tests read that same
//! stream to show a failed task is never retried, so they stand here.

use super::*;

/// Drain every buffered progress event off the receiver, synchronously.
fn drain_progress(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<ReviewProgressEvent>,
) -> Vec<ReviewProgressEvent> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

/// The fan-out emits one `Planned` carrying the (validator, file) pair total,
/// then `PairStarted`/`PairDone` for EVERY pair — including a validator task
/// that errors — so a consumer counting `PairDone` events always reaches the
/// planned total.
#[tokio::test]
async fn fleet_emits_progress_events_per_validator_file_pair_including_failed_tasks() {
    let rs_ok = ruleset("val-ok", "mandate ok", &[("ok-rule", "OK_BODY")]);
    let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
    let loader = loader_with(vec![rs_ok, rs_bad]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work(
                "val-ok",
                vec![
                    file_work("src/a.rs", "alpha", "src/x.rs"),
                    file_work("src/b.rs", "beta", "src/y.rs"),
                ],
            ),
            validator_work("val-bad", vec![file_work("src/c.rs", "gamma", "src/z.rs")]),
        ],
    );

    // The `val-bad` fork errors; `val-ok` resolves with the default (empty)
    // findings reply. Both must still emit a PairDone for every file.
    let agent = forking_agent(vec![("BAD_BODY".to_string(), ScriptedReply::Error)]);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let tally = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        let mut outcome = run_fleet(
            &work,
            &loader,
            &pool,
            &ToolSuppression::default(),
            Some(&progress_tx),
        )
        .await;
        if let Some(guard) = outcome.prime.take() {
            unpin_prefix_session(guard).await;
        }
        (outcome.attempted(), outcome.failed())
    })
    .await;
    assert_eq!(
        tally,
        (2, 1),
        "two tasks attempted, the erroring one failed"
    );

    let events = drain_progress(&mut progress_rx);
    assert_eq!(
        events.first(),
        Some(&ReviewProgressEvent::Planned { total_pairs: 3 }),
        "the first event is the plan with the pair total: {events:#?}"
    );

    let pairs = [
        ("val-ok", "src/a.rs"),
        ("val-ok", "src/b.rs"),
        ("val-bad", "src/c.rs"),
    ];
    for (validator, file) in pairs {
        assert!(
            events.iter().any(|e| matches!(
                e,
                ReviewProgressEvent::PairStarted { validator: v, file: f }
                    if v == validator && f == file
            )),
            "missing PairStarted for ({validator}, {file}): {events:#?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                ReviewProgressEvent::PairDone { validator: v, file: f }
                    if v == validator && f == file
            )),
            "missing PairDone for ({validator}, {file}) — a failed task must still \
             emit PairDone so progress reaches the total: {events:#?}"
        );
    }

    let done = events
        .iter()
        .filter(|e| matches!(e, ReviewProgressEvent::PairDone { .. }))
        .count();
    assert_eq!(
        done, 3,
        "exactly one PairDone per planned pair, so progress closes at the \
         planned total: {events:#?}"
    );
}

/// Each COMPLETED validator task emits one `Findings` event carrying that
/// task's parsed, validator-tagged findings — a validator that came back clean
/// emits an EMPTY vec (clean, not silence), while a FAILED task emits no
/// `Findings` event at all (its `PairDone` accounting still fires).
#[tokio::test]
async fn fleet_emits_findings_events_per_completed_validator_task() {
    let rs_hit = ruleset("val-hit", "mandate hit", &[("hit-rule", "HIT_BODY")]);
    let rs_clean = ruleset(
        "val-clean",
        "mandate clean",
        &[("clean-rule", "CLEAN_BODY")],
    );
    let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
    let loader = loader_with(vec![rs_hit, rs_clean, rs_bad]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-hit", vec![file_work("src/a.rs", "alpha", "src/x.rs")]),
            validator_work("val-clean", vec![file_work("src/b.rs", "beta", "src/y.rs")]),
            validator_work("val-bad", vec![file_work("src/c.rs", "gamma", "src/z.rs")]),
        ],
    );

    // val-hit returns one finding; val-clean returns an empty array (clean);
    // val-bad's fork errors — a failed task that must emit NO Findings event.
    let agent = forking_agent(vec![
        rescan_finds_nothing(),
        (
            format!("{VALIDATOR_HEADER}val-hit\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text(findings_json(
                "src/a.rs",
                TEST_FINDING_LINE,
                "hit-rule",
                "real issue in a",
            )),
        ),
        (
            format!("{VALIDATOR_HEADER}val-clean\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text("[]".to_string()),
        ),
        ("BAD_BODY".to_string(), ScriptedReply::Error),
    ]);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    with_pool(agent, PoolConfig::remote(3), move |pool| async move {
        let mut outcome = run_fleet(
            &work,
            &loader,
            &pool,
            &ToolSuppression::default(),
            Some(&progress_tx),
        )
        .await;
        if let Some(guard) = outcome.prime.take() {
            unpin_prefix_session(guard).await;
        }
    })
    .await;

    let events = drain_progress(&mut progress_rx);
    let findings_events: Vec<(&str, &Vec<Finding>)> = events
        .iter()
        .filter_map(|e| match e {
            ReviewProgressEvent::Findings {
                validator,
                findings,
            } => Some((validator.as_str(), findings)),
            _ => None,
        })
        .collect();

    // val-hit emitted its one validator-tagged finding as its task resolved.
    let hit = findings_events
        .iter()
        .find(|(v, _)| *v == "val-hit")
        .expect("a Findings event for val-hit");
    assert_eq!(hit.1.len(), 1, "val-hit streamed its single finding");
    assert_eq!(
        hit.1[0].validator, "val-hit",
        "the streamed finding is validator-tagged, not the agent's self-report"
    );
    assert_eq!(hit.1[0].claim, "real issue in a");

    // val-clean emitted an EMPTY Findings event — clean, not silent.
    let clean = findings_events
        .iter()
        .find(|(v, _)| *v == "val-clean")
        .expect("a Findings event for the clean validator");
    assert!(
        clean.1.is_empty(),
        "a clean validator emits an empty Findings vec: {clean:#?}"
    );

    // val-bad FAILED → no Findings event (PairDone still fires for it).
    assert!(
        !findings_events.iter().any(|(v, _)| *v == "val-bad"),
        "a failed validator task must emit no Findings event: {findings_events:#?}"
    );
}

/// Poll `condition` every [`POLL_INTERVAL`] until it holds, panicking after
/// [`POLL_TIMEOUT`]. The retry count is derived from the two so the wait
/// budget is expressed once, not as a product of two coupled literals.
async fn wait_for(what: &str, condition: impl Fn() -> bool) {
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);
    const POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
    let attempts = POLL_TIMEOUT.as_millis() / POLL_INTERVAL.as_millis();
    for _ in 0..attempts {
        if condition() {
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    panic!("timed out waiting for {what}");
}

/// Cancellation-safety regression: a run future dropped mid-collect
/// (review cancelled, caller timeout) must STILL release the prefix pin —
/// a pinned session is exempt from cache eviction, so a leaked pin
/// outlives the review until process restart.
#[tokio::test]
async fn prefix_pin_is_released_when_the_fanout_future_is_dropped_mid_collect() {
    let rs = ruleset("val", "mandate", &[("r", "WEDGE_BODY")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    // The validator fork turn wedges forever (its suffix carries the rule
    // body), holding the fan-out mid-collect AFTER the prime has been pinned.
    let agent = forking_agent(vec![("WEDGE_BODY".to_string(), ScriptedReply::Stall)]);
    let agent_probe = Arc::clone(&agent);

    with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        let fanout = tokio::spawn(async move {
            run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None).await
        });

        // Wait until the prefix is pinned and the wedged validator fork is in
        // flight — the run is now mid-collect.
        wait_for("the prefix pin and the wedged validator fork", || {
            agent_probe
                .pin_calls()
                .contains(&("sess-0".to_string(), true))
                && agent_probe
                    .seen_prompts()
                    .iter()
                    .any(|p| p.starts_with("# Validator:"))
        })
        .await;

        // Cancel the review: drop the fan-out future mid-collect.
        fanout.abort();
        let _ = fanout.await;

        // The pin must still be released — the cancelled fan-out cannot
        // leak the pinned prefix session.
        wait_for("the cancelled fan-out to release the prefix pin", || {
            agent_probe
                .pin_calls()
                .contains(&("sess-0".to_string(), false))
        })
        .await;
    })
    .await;
}

#[tokio::test]
async fn one_failing_task_yields_zero_findings_without_aborting_the_rest() {
    // Two validators: the `val-bad` fork errors, the `val-good` fork finds an
    // issue. One bad validator task never aborts the rest.
    let rs_good = ruleset("val-good", "mandate good", &[("good-rule", "GOOD_BODY")]);
    let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
    let loader = loader_with(vec![rs_good, rs_bad]);

    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-good", vec![file_work("src/a.rs", "alpha", "src/x.rs")]),
            validator_work("val-bad", vec![file_work("src/b.rs", "beta", "src/y.rs")]),
        ],
    );

    // The fork carrying `BAD_BODY` errors; the `GOOD_BODY` one returns a
    // finding. Both keys appear only in their own validator's suffix.
    let agent = forking_agent(vec![
        // The good validator's first pass is exhaustive; its completeness
        // re-scan finds nothing more, so the surviving count is unchanged.
        rescan_finds_nothing(),
        ("BAD_BODY".to_string(), ScriptedReply::Error),
        (
            "GOOD_BODY".to_string(),
            ScriptedReply::Text(findings_json(
                "src/a.rs",
                TEST_FINDING_LINE,
                "good-rule",
                "real issue",
            )),
        ),
    ]);
    let agent_probe = Arc::clone(&agent);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        let mut outcome = run_fleet(
            &work,
            &loader,
            &pool,
            &ToolSuppression::default(),
            Some(&progress_tx),
        )
        .await;
        if let Some(guard) = outcome.prime.take() {
            unpin_prefix_session(guard).await;
        }
        outcome
    })
    .await;

    // The erroring task contributed nothing; the good one still returned.
    assert_eq!(
        outcome.findings.len(),
        1,
        "the failing task degrades to zero findings"
    );
    assert_eq!(outcome.findings[0].claim, "real issue");
    assert_eq!(outcome.findings[0].validator, "val-good");
    // The tally records both tasks attempted and exactly the one that failed.
    assert_eq!(outcome.attempted(), 2, "two validator tasks attempted");
    assert_eq!(
        outcome.failed(),
        1,
        "the erroring task is counted as failed"
    );

    // No-retry regression guard: the failing validator's prompt reaches the
    // agent exactly once — no fallback resubmission, no re-queue of the same
    // (validator, file) unit after it errors. This is the guard against ever
    // reintroducing the per-unit retry the fleet decided against.
    let bad_submissions = agent_probe
        .seen_prompts()
        .iter()
        .filter(|p| p.contains("BAD_BODY"))
        .count();
    assert_eq!(
        bad_submissions,
        1,
        "a failing fan-out task must be submitted to the agent exactly once, \
         never retried: {:?}",
        agent_probe.seen_prompts()
    );

    // Single-attempt semantics show up in the progress stream too: exactly one
    // `PairStarted`/`PairDone` per (validator, file) — including the failing
    // one — and the failure never triggers a re-plan (a second `Planned` event
    // growing the total, which would signal a re-queue).
    let events = drain_progress(&mut progress_rx);
    let planned: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ReviewProgressEvent::Planned { total_pairs } => Some(*total_pairs),
            _ => None,
        })
        .collect();
    assert_eq!(
        planned,
        vec![2],
        "the failure must not grow/replan the announced total: {events:#?}"
    );

    for (validator, file) in [("val-good", "src/a.rs"), ("val-bad", "src/b.rs")] {
        let started = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    ReviewProgressEvent::PairStarted { validator: v, file: f }
                        if v == validator && f == file
                )
            })
            .count();
        assert_eq!(
            started, 1,
            "exactly one PairStarted for ({validator}, {file}), no retry: {events:#?}"
        );
        let done = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    ReviewProgressEvent::PairDone { validator: v, file: f }
                        if v == validator && f == file
                )
            })
            .count();
        assert_eq!(
            done, 1,
            "exactly one PairDone for ({validator}, {file}), no retry: {events:#?}"
        );
    }
}

#[tokio::test]
async fn all_tasks_failing_yields_zero_findings_and_a_full_failure_tally() {
    // Three validators; every validator fork errors.
    let loader = loader_with(vec![
        ruleset("val-a", "mandate a", &[("r1", "body 1")]),
        ruleset("val-b", "mandate b", &[("r2", "body 2")]),
        ruleset("val-c", "mandate c", &[("r3", "body 3")]),
    ]);

    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-a", vec![file_work("src/a.rs", "a", "src/x.rs")]),
            validator_work("val-b", vec![file_work("src/b.rs", "b", "src/y.rs")]),
            validator_work("val-c", vec![file_work("src/c.rs", "c", "src/z.rs")]),
        ],
    );

    // Every validator fork errors (every validator suffix carries the
    // validator header).
    let agent = forking_agent(vec![("# Validator:".to_string(), ScriptedReply::Error)]);

    let outcome = with_pool(agent, PoolConfig::remote(3), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert!(
        outcome.findings.is_empty(),
        "every task failed, so there are no findings"
    );
    assert_eq!(outcome.attempted(), 3, "three validator tasks attempted");
    assert_eq!(outcome.failed(), 3, "all three failed");
}

#[tokio::test]
async fn validator_missing_from_loader_is_skipped_not_panicked() {
    // The work-list names a validator the loader does not know.
    let loader = loader_with(vec![ruleset("known", "mandate", &[("r", "body")])]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "unknown",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    let agent = forking_agent(vec![]);
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None).await
    })
    .await;

    assert!(
        outcome.findings.is_empty(),
        "an unknown validator yields no findings"
    );
    assert_eq!(
        outcome.attempted(),
        0,
        "no task is attempted for a validator missing from the loader"
    );
    assert_eq!(outcome.failed(), 0);
    assert!(
        agent_probe.seen_prompts().is_empty(),
        "no task is submitted for a validator missing from the loader"
    );
}
