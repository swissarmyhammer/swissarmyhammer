//! The primed prefix, and the validator forks that hang off it.
//!
//! The prefix is primed once for a run, and each validator forks the suffix.
//! A fork that fails, or an agent that cannot fork at all, falls back to a
//! monolithic prompt without losing a task. The pin is always released.

use super::*;

#[tokio::test]
#[tracing_test::traced_test]
async fn prefix_is_primed_once_per_run_and_validators_fork_suffix_only() {
    // One validator, two rules, over four files. The new grain: the change +
    // every file diff is primed ONCE for the whole run, and each VALIDATOR
    // forks it sending only its validator suffix (its full ruleset). So: 1
    // prime + 1 validator fork carrying BOTH rules, never one fork per rule
    // and never one fork per file/batch.
    let rs = ruleset(
        "val",
        "MANDATE_MARKER mandate",
        &[("r1", "RULE1_MARKER body"), ("r2", "RULE2_MARKER body")],
    );
    let loader = loader_with(vec![rs]);

    let files: Vec<FileWork> = (0..4)
        .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
        .collect();
    let work = WorkList::new("purpose".to_string(), vec![validator_work("val", files)]);

    // The validator's fork emits a finding. The fork inherits the shared
    // prime (all files) and appends the validator suffix (which carries the
    // mandate marker), so we key on that marker.
    let agent = forking_agent(vec![
        // The first pass is exhaustive; its completeness re-scan finds
        // nothing more, so this test asserts the unchanged one-fork-per-
        // validator prime shape (plus the bounded re-scan fork).
        rescan_finds_nothing(),
        (
            "MANDATE_MARKER".to_string(),
            ScriptedReply::Text(findings_json(
                "src/f0.rs",
                TEST_FINDING_LINE,
                "r1",
                "warm finding",
            )),
        ),
    ]);
    let agent_probe = Arc::clone(&agent);

    // Drive the prime lifecycle the way `run_review` does: run the fleet,
    // then release the returned shared-prime guard once the run drains.
    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        let outcome = run_fleet(&work, &loader, &pool, &ToolSuppression::default(), None).await;
        if let Some(guard) = outcome.prime {
            unpin_prefix_session(guard).await;
        }
        FleetOutcome {
            prime: None,
            ..outcome
        }
    })
    .await;

    let seen = agent_probe.seen_prompts();
    let primes: Vec<&String> = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).collect();
    assert_eq!(
        primes.len(),
        1,
        "the shared prefix is primed exactly once per RUN: {seen:#?}"
    );
    // The prime carries the change + every file diff, and NO validator text.
    assert!(
        primes[0].contains("# Files under review") && primes[0].contains("## File: src/f0.rs"),
        "the prime carries the diffs: {}",
        primes[0]
    );
    assert!(
        !primes[0].contains("MANDATE_MARKER")
            && !primes[0].contains("RULE1_MARKER")
            && !primes[0].contains("RULE2_MARKER"),
        "the prime must NOT carry any validator text: {}",
        primes[0]
    );

    // One forked task per validator, carrying ONLY its validator suffix (the
    // validator/mandate/full-ruleset/contract) and never re-sending file
    // content.
    let validator_tasks: Vec<&String> = seen
        .iter()
        .filter(|p| p.starts_with("# Validator:"))
        .collect();
    assert_eq!(
        validator_tasks.len(),
        1,
        "the validator forks the primed session and sends ONLY its validator suffix: {seen:#?}"
    );
    assert!(
        validator_tasks.iter().all(|p| !p.contains("## File: ")),
        "validator forks must not re-send the file diffs: {validator_tasks:#?}"
    );
    // The single validator fork carries BOTH of the validator's rules.
    assert!(validator_tasks[0].contains("RULE1_MARKER"));
    assert!(validator_tasks[0].contains("RULE2_MARKER"));
    // One validator fork plus its one bounded completeness re-scan fork.
    assert_eq!(
        agent_probe.fork_count(),
        2,
        "one validator fork plus one completeness re-scan fork"
    );

    assert_eq!(outcome.attempted(), 1);
    assert_eq!(outcome.failed(), 0);
    assert_eq!(outcome.findings.len(), 1, "{:#?}", outcome.findings);
    assert_eq!(outcome.findings[0].claim, "warm finding");
    assert_eq!(outcome.findings[0].validator, "val");

    // The shared prime was pinned for the run and unpinned when it drained.
    assert_eq!(
        agent_probe.pin_calls(),
        vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
        "pin the shared prime for the run, unpin when it drains"
    );

    // Observability: each fork task logs the warm reuse and token count,
    // classified as a warm KV fork (the native-KV path).
    assert!(logs_contain("fleet task prefix reuse"));
    assert!(logs_contain("reuse=\"warm KV fork\""));
    assert!(logs_contain(&format!(
        "reused_tokens=Some({MOCK_PREFIX_TOKENS})"
    )));
    assert!(logs_contain("primed shared run prefix session"));
}

/// The shared run prime is born pinned through the PRODUCTION prime path:
/// `prime_run_prefix` → `submit_primed` → the prompt's `_meta` pin-on-save
/// intent → the agent saving its prefix pinned atomically at turn completion
/// — BEFORE any separate `session/pin` confirm runs. This is the end-to-end
/// (scripted agent, no real model) assertion for the structural close of the
/// prime→pin eviction race: the prefix is never an unpinned eviction
/// candidate, independent of any post-turn pin.
#[tokio::test]
async fn primed_prefix_is_born_pinned_through_the_production_path() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let files: Vec<FileWork> = (0..2)
        .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
        .collect();
    let work = WorkList::new("purpose".to_string(), vec![validator_work("val", files)]);

    let agent = forking_agent(vec![]);
    let agent_probe = Arc::clone(&agent);

    with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    // The shared prime session (`sess-0`) was born pinned by the prime turn's
    // `_meta` intent — recorded at turn completion, before the post-turn
    // `session/pin` confirm. Forked validator sessions are NOT born pinned
    // (they save their own cold state unpinned).
    assert_eq!(
        agent_probe.born_pinned_sessions(),
        vec!["sess-0".to_string()],
        "the run prime must be born pinned through the production prime path, \
         and only the prime (not the forked validator sessions)"
    );
}

#[tokio::test]
#[tracing_test::traced_test]
async fn fork_failure_falls_back_to_monolithic_without_losing_tasks() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![
                file_work("src/a.rs", "alpha", "src/x.rs"),
                file_work("src/b.rs", "beta", "src/y.rs"),
            ],
        )],
    );

    // Every `session/fork` is rejected; the validator task must fall back to
    // a fresh-session monolithic prompt and still deliver its findings.
    let agent = agent_with_fork_mode(
        vec![(
            "## File: src/a.rs".to_string(),
            ScriptedReply::Text(findings_json(
                "src/a.rs",
                TEST_FINDING_LINE,
                "r",
                "found despite fork failure",
            )),
        )],
        ForkMode::RejectFork,
    );
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 1, "one validator task");
    assert_eq!(outcome.failed(), 0, "a failed fork is never a lost task");
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].claim, "found despite fork failure");

    // The fallback prompt is the full monolithic shape (rules + files).
    let seen = agent_probe.seen_prompts();
    let monolithic = seen
        .iter()
        .filter(|p| p.contains(MANDATE_HEADER) && p.contains("# Files under review"))
        .count();
    assert_eq!(
        monolithic, 1,
        "the validator fell back to a monolithic prompt: {seen:#?}"
    );
    assert!(logs_contain("falling back to a monolithic"));

    // The prime succeeded, so it was pinned and is unpinned when the run drains.
    assert_eq!(
        agent_probe.pin_calls(),
        vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
    );
}

#[tokio::test]
#[tracing_test::traced_test]
async fn unsupported_fork_extension_degrades_to_monolithic_prompts() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![
                file_work("src/a.rs", "alpha", "src/x.rs"),
                file_work("src/b.rs", "beta", "src/y.rs"),
            ],
        )],
    );

    // The backend implements NO extension methods: the prime turn runs but
    // its state can never be confirmed, so the whole run degrades to
    // monolithic per-validator prompts — never a lost task.
    let agent = agent_with_fork_mode(
        vec![(
            "## File: src/b.rs".to_string(),
            ScriptedReply::Text(findings_json(
                "src/b.rs",
                TEST_FINDING_LINE,
                "r",
                "found without forks",
            )),
        )],
        ForkMode::Unsupported,
    );
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 1, "one validator task");
    assert_eq!(outcome.failed(), 0);
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].claim, "found without forks");

    let seen = agent_probe.seen_prompts();
    let monolithic = seen
        .iter()
        .filter(|p| p.contains("## Mandate") && p.contains("# Files under review"))
        .count();
    assert_eq!(monolithic, 1, "{seen:#?}");
    assert_eq!(
        agent_probe.fork_count(),
        0,
        "no forks on an unsupported backend"
    );
    assert!(
        agent_probe.pin_calls().is_empty(),
        "nothing is pinned when state confirmation fails"
    );
    assert!(logs_contain("falling back to monolithic prompts"));
}

#[tokio::test]
#[tracing_test::traced_test]
async fn degraded_fork_runs_cold_but_still_parses_findings() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    // Forks succeed but attach no parent state — the task proceeds on the
    // forked session (history is intact, just cold) and is logged.
    let agent = agent_with_fork_mode(
        vec![
            rescan_finds_nothing(),
            (
                "## File: src/a.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    TEST_FINDING_LINE,
                    "r",
                    "cold but correct",
                )),
            ),
        ],
        ForkMode::DegradedAttach,
    );

    let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 1);
    assert_eq!(outcome.failed(), 0);
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].claim, "cold but correct");
    assert!(logs_contain("fleet task fork was degraded"));
}

/// The claude backend shape: a fork that attaches no native KV state
/// (`fork.prefix_tokens == None`) but whose turn reports Anthropic
/// prompt-cache reads. The forked task must resolve through the real
/// `collect_forked_task` path without error AND log the warm-cache reuse
/// (`classify_reuse` → `WarmCache`), so warm/cold is observable on claude
/// even though the native KV reuse log is blind.
#[tokio::test]
#[tracing_test::traced_test]
async fn forked_task_with_claude_cache_usage_logs_warm_cache() {
    let rs = ruleset("val", "mandate", &[("r", "body")]);
    let loader = loader_with(vec![rs]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![validator_work(
            "val",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        )],
    );

    // Forks succeed but attach no native parent state (claude shape:
    // `prefix_tokens == None`); the turn's `_meta` reports a warm cache read,
    // which is what makes the reuse observable on claude.
    let agent = ScriptedAgent::with_config(
        vec![
            rescan_finds_nothing(),
            (
                "## File: src/a.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    TEST_FINDING_LINE,
                    "r",
                    "warm on claude",
                )),
            ),
        ],
        ScriptedAgentConfig {
            fork_mode: ForkMode::DegradedAttach,
            cache_usage: Some(CacheUsage {
                cache_read_input_tokens: Some(2048),
                cache_creation_input_tokens: Some(16),
                input_tokens: Some(2064),
                output_tokens: Some(40),
            }),
            ..ScriptedAgentConfig::default()
        },
    );

    let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 1);
    assert_eq!(
        outcome.failed(),
        0,
        "the forked task resolved through collect_forked_task without error"
    );
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.findings[0].claim, "warm on claude");
    assert!(
        logs_contain("warm prompt cache"),
        "the warm-cache reuse must be logged so claude reuse is observable"
    );
}

#[tokio::test]
async fn prefix_session_is_unpinned_even_when_a_validator_task_errors() {
    // Two validators; the second's fork errors. The shared-prime pin must
    // still be released once the run drains, regardless of a failed validator
    // task.
    let rs_ok = ruleset("val-ok", "mandate ok", &[("ok-rule", "OK_BODY")]);
    let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
    let loader = loader_with(vec![rs_ok, rs_bad]);
    let work = WorkList::new(
        "purpose".to_string(),
        vec![
            validator_work("val-ok", vec![file_work("src/a.rs", "alpha", "src/x.rs")]),
            validator_work("val-bad", vec![file_work("src/b.rs", "beta", "src/y.rs")]),
        ],
    );

    // The `val-bad` fork carries the `bad-rule` body and errors; the `val-ok`
    // one is empty. One forked validator task errors → the unpin must still
    // happen.
    let agent = forking_agent(vec![("BAD_BODY".to_string(), ScriptedReply::Error)]);
    let agent_probe = Arc::clone(&agent);

    let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
        run_fleet_and_unpin(&work, &loader, &pool).await
    })
    .await;

    assert_eq!(outcome.attempted(), 2, "two validator tasks");
    assert_eq!(
        outcome.failed(),
        1,
        "the erroring validator task is a failed task"
    );
    assert_eq!(
        agent_probe.pin_calls(),
        vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
        "the prefix pin is released even when a validator task errors"
    );
}
