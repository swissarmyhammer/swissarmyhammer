//! The orchestrator, driven against a scripted agent.
//!
//! What one run submits for a change, and how the follow-up sweep continues
//! while findings arrive and stops when it goes dry.

use super::*;

#[tokio::test]
async fn fan_out_two_validators_two_files_submits_one_prime_and_one_fork_per_validator() {
    // Two validators over the same two files. Under the new grain — fork per
    // VALIDATOR, files in the shared prime — the run primes ONCE and forks ONE
    // task per validator: 2 validators = 2 forks, regardless of how many files
    // each validator reviews or how many rules it carries.
    let rs_a = ruleset("val-a", "mandate a", &[("ra", "body a")]);
    let rs_b = ruleset("val-b", "mandate b", &[("rb", "body b")]);
    let loader = loader_with(vec![rs_a, rs_b]);

    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work(
                "val-a",
                vec![
                    file_work("src/a.rs", "alpha", "src/x.rs"),
                    file_work("src/b.rs", "beta", "src/y.rs"),
                ],
            ),
            validator_work(
                "val-b",
                vec![
                    file_work("src/a.rs", "alpha", "src/x.rs"),
                    file_work("src/b.rs", "beta", "src/y.rs"),
                ],
            ),
        ],
    );

    // Script: a finding for each validator. The fork inherits the shared
    // prime (all files) and appends the validator suffix carrying the
    // validator header, so we key on that header.
    let agent = forking_agent(vec![
        // Each validator's first pass is exhaustive, so its completeness
        // re-scan finds nothing more — this test asserts the first-pass
        // fan-out shape (one prime + one fork per validator + one re-scan).
        rescan_finds_nothing(),
        (
            format!("{VALIDATOR_HEADER}val-a\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text(findings_json(
                "src/a.rs",
                TEST_FINDING_LINE,
                "ra",
                "dup in a",
            )),
        ),
        (
            format!("{VALIDATOR_HEADER}val-b\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text(findings_json(
                "src/b.rs",
                TEST_FINDING_LINE,
                "rb",
                "dup in b",
            )),
        ),
    ]);
    let agent_probe = Arc::clone(&agent);

    let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None)
            .await
            .findings
    })
    .await;

    let seen = agent_probe.seen_prompts();
    // Exactly ONE shared prime for the whole run (not one per validator).
    let primes = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).count();
    assert_eq!(
        primes, 1,
        "the run primes the shared prefix exactly once: {seen:#?}"
    );
    // One forked validator task per validator: 2 validators = 2 forks.
    let validator_tasks = seen
        .iter()
        .filter(|p| p.starts_with("# Validator:"))
        .count();
    assert_eq!(
        validator_tasks, 2,
        "one forked task per validator: {seen:#?}"
    );
    // Two validator forks PLUS one completeness re-scan fork each (the
    // re-scan inherits the validator session) = four forks total.
    assert_eq!(
        agent_probe.fork_count(),
        4,
        "one validator fork plus one completeness re-scan fork per validator"
    );

    // Every finding is tagged with its validator (overriding the agent's
    // self-reported `ignored-by-agent`), and the rule tag survives.
    let a = findings
        .iter()
        .find(|f| f.claim == "dup in a")
        .expect("val-a finding");
    assert_eq!(a.validator, "val-a");
    assert_eq!(a.rule.as_deref(), Some("ra"));
    let b = findings
        .iter()
        .find(|f| f.claim == "dup in b")
        .expect("val-b finding");
    assert_eq!(b.validator, "val-b");
    assert_eq!(b.rule.as_deref(), Some("rb"));
    assert!(
        findings.iter().all(|f| f.validator != "ignored-by-agent"),
        "the agent's self-reported validator must be overridden"
    );
}

/// A file containing several instances of ONE rule, touched by a single
/// commit, must yield ALL of them on the FIRST review pass — the whole-file
/// sweep, not a dribble of one-instance-per-re-review. Driven end-to-end
/// through `run_fleet` with a scripted agent that reports every instance.
#[tokio::test]
async fn one_rule_with_many_instances_reports_them_all_on_the_first_pass() {
    let rs = ruleset(
        "magic-numbers",
        "no unexplained numeric literals",
        &[("no-magic", "name your constants")],
    );
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "magic-numbers",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    // The agent reports several instances of the one rule across the whole
    // file in a single reply; its completeness re-scan then finds nothing
    // more. Each instance sits on its own line derived from TEST_FINDING_LINE
    // so the findings are distinct file:line instances, not a shared-line
    // collapse — the exact lines are immaterial.
    let instances = [
        ("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7"),
        (
            "src/a.rs",
            TEST_FINDING_LINE + 1,
            "no-magic",
            "magic number 13",
        ),
        (
            "src/a.rs",
            TEST_FINDING_LINE + 2,
            "no-magic",
            "magic number 99",
        ),
        (
            "src/a.rs",
            TEST_FINDING_LINE + 3,
            "no-magic",
            "magic number 256",
        ),
    ];
    let first_pass = findings_array_json(&instances);
    let agent = forking_agent(vec![
        rescan_finds_nothing(),
        (
            format!("{VALIDATOR_HEADER}magic-numbers\n\n{MANDATE_HEADER}"),
            ScriptedReply::Text(first_pass),
        ),
    ]);

    let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None)
            .await
            .findings
    })
    .await;

    let magic: Vec<_> = findings
        .iter()
        .filter(|f| f.rule.as_deref() == Some("no-magic"))
        .collect();
    assert_eq!(
        magic.len(),
        instances.len(),
        "all instances of the one rule must report on the first pass, \
         not dribble one per round: {findings:#?}"
    );
    assert!(
        magic.iter().all(|f| f.validator == "magic-numbers"),
        "every instance is tagged with its validator: {findings:#?}"
    );
}

/// A magic-numbers single-validator `WorkList` over one file — the shared
/// setup for the follow-up-sweep tests, which all drive the loop on one
/// validator and assert on what it surfaced and how many sweeps it took.
fn magic_numbers_work() -> (ValidatorLoader, WorkList) {
    let rs = ruleset(
        "magic-numbers",
        "no unexplained numeric literals",
        &[("no-magic", "name your constants")],
    );
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "magic-numbers",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );
    (loader, work)
}

/// The first-pass script entry: keyed on the validator header so it answers
/// the first review turn (never a follow-up sweep, which carries the sweep
/// header instead) with `findings`.
fn first_pass_entry(findings: String) -> (String, ScriptedReply) {
    (
        "# Validator: magic-numbers".to_string() + "\n\n## Mandate",
        ScriptedReply::Text(findings),
    )
}

/// The sessions each follow-up sweep turn ran on, in order — the prompts
/// carrying the sweep header, mapped to their session. The loop drives the
/// session forward, so these must be a chain of DISTINCT sessions (one fresh
/// fork per sweep), never the same session re-forked.
fn sweep_sessions(probe: &ScriptedAgent) -> Vec<String> {
    probe
        .prompted_sessions()
        .into_iter()
        .zip(probe.seen_prompts())
        .filter(|(_, prompt)| prompt.contains(RESCAN_NEEDLE))
        .map(|(session, _)| session)
        .collect()
}

/// Lever 2 (a) — the follow-up sweep keeps going while turns return findings
/// and STOPS when a turn goes dry (`[]`). The first pass under-reports one
/// instance; sweep 1 recovers one more, sweep 2 one more, sweep 3 is empty
/// and ends the loop. All four findings merge on the first review, distinct.
#[tokio::test]
async fn followup_sweep_continues_while_findings_arrive_and_stops_when_dry() {
    let (loader, work) = magic_numbers_work();

    let first_pass =
        findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
    // ONE script entry keyed on the sweep header answers EVERY sweep, with a
    // different delta each turn — findings, findings, then dry. A constant
    // prompt is re-sent each sweep, so this sequence is the only way to script
    // the model converging across the loop.
    let sweep_deltas = ScriptedReply::sequence([
        findings_array_json(&[(
            "src/a.rs",
            TEST_FINDING_LINE + 1,
            "no-magic",
            "magic number 13",
        )]),
        findings_array_json(&[(
            "src/a.rs",
            TEST_FINDING_LINE + 2,
            "no-magic",
            "magic number 99",
        )]),
        "[]".to_string(),
    ]);
    let agent = forking_agent(vec![
        (RESCAN_NEEDLE.to_string(), sweep_deltas),
        first_pass_entry(first_pass),
    ]);
    let probe = Arc::clone(&agent);

    let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None)
            .await
            .findings
    })
    .await;

    // First pass (1) + sweep 1 (1) + sweep 2 (1) = 3 findings; sweep 3 is dry.
    assert_eq!(
        findings.len(),
        3,
        "every instance recovered across the sweeps must merge: {findings:#?}"
    );
    let lines: std::collections::BTreeSet<u32> = findings.iter().map(|f| f.line).collect();
    assert_eq!(
        lines.len(),
        3,
        "the merged findings are distinct file:line instances, not re-reports: {findings:#?}"
    );
    assert!(
        findings
            .iter()
            .all(|f| f.validator == "magic-numbers" && f.rule.as_deref() == Some("no-magic")),
        "merged findings keep their validator and rule tags: {findings:#?}"
    );

    // Three sweep turns fired: two that returned findings plus the dry one
    // that stopped the loop — well under the runaway cap.
    let sessions = sweep_sessions(&probe);
    assert_eq!(
        sessions.len(),
        3,
        "the loop runs sweeps until one goes dry, then stops: {sessions:#?}"
    );
}

/// Lever 2 (c) — the loop drives the SAME accumulating session forward, not a
/// re-fork of the first pass. Each sweep forks the session that delivered the
/// PRIOR sweep's answer, so the sweeps run on a chain of distinct sessions and
/// the model's own earlier answers are in context — the structural reason it
/// converges instead of oscillating.
#[tokio::test]
async fn followup_sweep_drives_the_session_forward_not_reforking_the_first_pass() {
    let (loader, work) = magic_numbers_work();

    let first_pass =
        findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
    let sweep_deltas = ScriptedReply::sequence([
        findings_array_json(&[(
            "src/a.rs",
            TEST_FINDING_LINE + 1,
            "no-magic",
            "magic number 13",
        )]),
        "[]".to_string(),
    ]);
    let agent = forking_agent(vec![
        (RESCAN_NEEDLE.to_string(), sweep_deltas),
        first_pass_entry(first_pass),
    ]);
    let probe = Arc::clone(&agent);

    with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None).await;
    })
    .await;

    let sessions = sweep_sessions(&probe);
    assert_eq!(
        sessions.len(),
        2,
        "two sweeps fired (one with findings, one dry): {sessions:#?}"
    );
    let distinct: std::collections::BTreeSet<&String> = sessions.iter().collect();
    assert_eq!(
        distinct.len(),
        sessions.len(),
        "each sweep runs on a fresh fork of the prior sweep's session — a forward chain, \
         never the same first-pass session re-forked: {sessions:#?}"
    );

    // The load-bearing proof: the SECOND sweep ran on a session forked from
    // the FIRST sweep's session, so its accumulated context already carries
    // the first sweep's nudge — the sweep header appears TWICE. Re-forking the
    // first pass each time would leave it appearing only once, the model would
    // never see its own prior answer, and the loop could not converge.
    let last_sweep_history = probe
        .session_history(sessions.last().unwrap())
        .expect("the last sweep's session ran");
    let header_occurrences = last_sweep_history.matches(RESCAN_NEEDLE).count();
    assert_eq!(
        header_occurrences, 2,
        "the second sweep continues the first sweep's session (forward chain), so its context \
         holds the nudge twice — not a re-fork of the first pass: {last_sweep_history}"
    );
}

/// Lever 2 (b) — the runaway cap. A model that never goes dry (every sweep
/// returns the same finding) is bounded: the loop stops after exactly
/// [`MAX_FOLLOWUP_SWEEPS`] sweeps rather than looping forever. The re-reported
/// duplicates are harmless — downstream `dedup_exact` collapses them.
#[tokio::test]
async fn followup_sweep_stops_at_the_cap_when_never_dry() {
    let (loader, work) = magic_numbers_work();

    let first_pass =
        findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
    // Every sweep returns a (non-empty) finding, so the model never says
    // "none left" — only the cap can terminate the loop.
    let never_dry = findings_array_json(&[(
        "src/a.rs",
        TEST_FINDING_LINE + 1,
        "no-magic",
        "magic number 13",
    )]);
    let agent = forking_agent(vec![
        (RESCAN_NEEDLE.to_string(), ScriptedReply::Text(never_dry)),
        first_pass_entry(first_pass),
    ]);
    let probe = Arc::clone(&agent);

    with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None).await;
    })
    .await;

    let sessions = sweep_sessions(&probe);
    assert_eq!(
        sessions.len() as u32,
        MAX_FOLLOWUP_SWEEPS,
        "a never-dry model is bounded at the runaway cap, not looped forever: {sessions:#?}"
    );
}

/// Lever 2 (d) — an empty first pass spends ZERO follow-up turns. A clean
/// validator has nothing to be incomplete about, so the loop is skipped
/// entirely: one validator fork, no sweeps.
#[tokio::test]
async fn empty_first_pass_spends_no_followup_sweeps() {
    let (loader, work) = magic_numbers_work();

    // The first pass finds nothing; the sweep header still has a (would-be)
    // entry so a stray sweep would be observable — it must not fire.
    let agent = forking_agent(vec![
        (
            RESCAN_NEEDLE.to_string(),
            ScriptedReply::Text("[]".to_string()),
        ),
        first_pass_entry("[]".to_string()),
    ]);
    let probe = Arc::clone(&agent);

    let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None)
            .await
            .findings
    })
    .await;

    assert!(
        findings.is_empty(),
        "a clean validator reports nothing: {findings:#?}"
    );
    let sessions = sweep_sessions(&probe);
    assert!(
        sessions.is_empty(),
        "an empty first pass must not spend any follow-up sweep turn: {sessions:#?}"
    );
    assert_eq!(
        probe.fork_count(),
        1,
        "exactly one validator fork and no sweep fork on a clean validator"
    );
}

#[tokio::test]
async fn multi_rule_validator_forks_one_task_carrying_all_rules_against_one_prime() {
    // One validator with three rules over ten files. The files all live in
    // the single shared prime; the fan-out is per VALIDATOR, so this mints
    // exactly one prime + ONE validator fork carrying ALL THREE rules — never
    // per-rule, per-file, or per-batch.
    let rs = ruleset(
        "val",
        "mandate",
        &[
            ("r1", "RULE1_MARKER body 1"),
            ("r2", "RULE2_MARKER body 2"),
            ("r3", "RULE3_MARKER body 3"),
        ],
    );
    let loader = loader_with(vec![rs]);

    let files: Vec<FileWork> = (0..10)
        .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
        .collect();
    let work = WorkList::new("purpose".to_string(), vec![validator_work("val", files)]);

    let agent = forking_agent(vec![]);
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None).await
    })
    .await;

    let seen = agent_probe.seen_prompts();
    let primes = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).count();
    assert_eq!(primes, 1, "one shared prime for the whole run: {seen:#?}");
    let validator_tasks = seen
        .iter()
        .filter(|p| p.starts_with("# Validator:"))
        .count();
    assert_eq!(
        validator_tasks, 1,
        "one validator → one forked validator task (not three rule tasks, not ten file tasks): {seen:#?}"
    );
    assert_eq!(outcome.attempted(), 1, "one validator task attempted");

    // The single prime carries ALL ten files' diffs; the validator fork
    // carries every rule of the validator (no file content re-sent).
    let prime = seen
        .iter()
        .find(|p| p.contains(PRIME_HANDOFF))
        .expect("the run prime");
    assert_eq!(
        prime.matches("## File: ").count(),
        10,
        "the shared prime inlines every file once: {prime}"
    );
    let validator_suffix = seen
        .iter()
        .find(|p| p.starts_with("# Validator:"))
        .expect("a validator fork");
    assert!(
        validator_suffix.contains("RULE1_MARKER")
            && validator_suffix.contains("RULE2_MARKER")
            && validator_suffix.contains("RULE3_MARKER"),
        "the validator fork must carry ALL of its rules: {validator_suffix}"
    );
    assert!(
        !validator_suffix.contains("## File: "),
        "a validator fork must NOT re-send file content (it is in the prime): {validator_suffix}"
    );
}

#[tokio::test]
#[tracing_test::traced_test]
async fn fan_out_logs_the_rule_names_being_applied_per_validator() {
    // A validator with two distinctively-named rules; the fan-out log must
    // name the rules being applied (sourced from the loader's RuleSet) so the
    // logs show exactly which validator×rules ran.
    let rs = ruleset(
        "deduplicate",
        "mandate",
        &[("no-copy-paste", "body a"), ("prefer-reuse", "body b")],
    );
    let loader = loader_with(vec![rs]);

    let files: Vec<FileWork> = vec![file_work("src/a.rs", "alpha", "src/x.rs")];
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work("deduplicate", files)],
    );

    let agent = forking_agent(vec![]);
    let _findings = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
        run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None).await
    })
    .await;

    // The batching log carries the rule names from the loader's RuleSet as a
    // structured field (the exact bracketed list only this log emits — the
    // rendered prompt spells rules as `### Rule: ...` prose, not this shape).
    assert!(logs_contain("rules=[\"no-copy-paste\", \"prefer-reuse\"]"));
}
