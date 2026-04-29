//! Integration tests for the validator recording / replay loop.
//!
//! These tests verify two related properties of the recording infrastructure
//! introduced for AVP validator agents:
//!
//! 1. Recordings produced by `agent_client_protocol_extras::RecordingAgent`
//!    are exactly the right shape to feed
//!    `agent_client_protocol_extras::PlaybackAgent` back to the validator
//!    pipeline. A test loads a checked-in fixture, drives the production Stop
//!    chain against the playback agent, and asserts on the produced
//!    `ChainOutput`.
//!
//! 2. Recording is unconditional. `AvpContext::agent` always returns a
//!    `RecordingAgent`-wrapped agent that flushes a `RecordedSession` JSON
//!    file at drop time under `<AVP_DIR>/recordings/`.
//!
//! These tests are hermetic: no network, no real model, deterministic. The
//! fixtures under `tests/fixtures/recordings/` are checked in and replay via
//! `PlaybackAgent`, so the suite stays green even on machines without
//! credentials.

mod test_helpers;

use std::sync::Arc;

use agent_client_protocol_extras::PlaybackAgent;
use avp_common::chain::ChainFactory;
use avp_common::context::AvpContext;
use avp_common::validator::{ValidatorLoader, ValidatorSource};
use tempfile::TempDir;
use test_helpers::{
    build_stop_input, create_playback_context, recording_fixture_path,
    setup_turn_state_with_changes, write_stop_error_ruleset, ClaudeAcpGuard,
};

/// Drive a single-rule RuleSet through the production Stop chain using the
/// given playback fixture. Returns the chain's `ChainOutput`.
///
/// We go through the chain (rather than calling `ctx.execute_rulesets`
/// directly) for two reasons:
/// 1. It exactly mirrors how production code reaches the runner — same
///    starter, same link wiring, same diff/turn-state plumbing.
/// 2. It guarantees the notification bridge has actually had time to deliver
///    playback notifications by the time the runner reads them. Chain links
///    yield to the runtime between phases, which is enough to let the bridge
///    forward `replay_notifications` output through `send_update` and into
///    the per-session collector.
async fn replay_ruleset(
    fixture_name: &str,
    ruleset_name: &str,
    rule_name: &str,
    rule_body: &str,
) -> avp_common::chain::ChainOutput {
    let path = recording_fixture_path(fixture_name);
    let (temp, ctx) = create_playback_context(&path);

    // Stop validators read accumulated changed files from turn state — provide
    // at least one so the ruleset has a non-empty `changed_files` to feed into
    // the prompt, matching what a real Stop hook would see.
    let session_id = "replay-session";
    let turn_state =
        setup_turn_state_with_changes(&temp, session_id, &["src/replay-fixture-target.rs"]);

    write_stop_error_ruleset(&temp, ruleset_name, rule_name, rule_body);

    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            ValidatorSource::Project,
        )
        .expect("load rulesets");

    // RAII guard: clears CLAUDE_ACP for the test and restores it on drop,
    // so a panic in `chain.execute` cannot leak the cleared env var into the
    // next serial test.
    let _claude_acp_guard = ClaudeAcpGuard::new();

    let factory = ChainFactory::new(Arc::new(ctx), Arc::new(loader), turn_state);
    let mut chain = factory.stop_chain();

    let input = build_stop_input(&temp, session_id);

    let (chain_output, _exit_code) = chain.execute(&input).await.expect("chain.execute");

    chain_output
}

// ============================================================================
// Replay tests — fixture corpus drives the pipeline deterministically.
// ============================================================================

/// Loading a "passed" recording fixture drives the validator pipeline
/// end-to-end and produces a structurally-correct `ExecutedRuleSet`.
///
/// This is the proof that the recording → playback round-trip plumbs all the
/// way through to the runner: a real `RecordedSession` JSON file flows through
/// `PlaybackAgent`, [`AvpContext::execute_rulesets`] consumes it, and the
/// runner emits one `ExecutedRuleSet` whose `ruleset_name` and `rule_name`
/// reflect the recorded RuleSet/rule.
///
/// We bypass the chain (which is exercised by the `clean_fail` and
/// `unparseable` tests below) and call `ctx.execute_rulesets` directly so
/// the assertion is hard regardless of the chain's notification-timing
/// properties — the original chain-based version of this test had to accept
/// "block emitted" *and* "no block emitted" because the bridge sometimes
/// hadn't delivered the playback response by the time the chain link checked,
/// which made it possible for the test to pass even when the recording never
/// reached the runner.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn replay_clean_pass_fixture_drives_runner_to_executed_ruleset() {
    use avp_common::types::HookType;
    use avp_common::validator::{ValidatorLoader, ValidatorSource};

    let path = recording_fixture_path("rule_clean_pass.json");
    let (temp, ctx) = create_playback_context(&path);

    // Mirror the on-disk ruleset layout the chain-based tests use, but read it
    // back via `ValidatorLoader` and pass it straight to `execute_rulesets`.
    write_stop_error_ruleset(
        &temp,
        "playback-pass",
        "always-pass",
        "Validate that the change set has no issues.",
    );

    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            ValidatorSource::Project,
        )
        .expect("load rulesets");

    let rulesets = loader.list_rulesets();
    assert_eq!(rulesets.len(), 1, "expected exactly one ruleset on disk");

    let _claude_acp_guard = ClaudeAcpGuard::new();

    let input = serde_json::json!({
        "session_id": "replay-session",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "Stop",
        "stop_hook_active": true,
    });

    let executed = ctx
        .execute_rulesets(
            &rulesets,
            HookType::Stop,
            &input,
            Some(&["src/replay-fixture-target.rs".to_string()]),
            None,
        )
        .await;

    // The runner returned exactly one ExecutedRuleSet, with one RuleResult,
    // and the names reflect the recorded RuleSet/rule. This is the hard
    // signal that the playback recording reached the runner — anything weaker
    // (e.g. "either a block or no block is fine") would also pass if the
    // playback agent never reached the runner at all.
    assert_eq!(
        executed.len(),
        1,
        "expected one ExecutedRuleSet, got {:?}",
        executed
    );
    let ruleset = &executed[0];
    assert_eq!(ruleset.ruleset_name, "playback-pass");
    assert_eq!(
        ruleset.rule_results.len(),
        1,
        "expected one rule result, got {:?}",
        ruleset.rule_results
    );
    assert_eq!(ruleset.rule_results[0].rule_name, "always-pass");
}

/// Loading a "failed" recording fixture produces a validator block on the
/// chain output. The block's `validator_name` matches the recorded RuleSet/
/// rule, proving the recording flowed through the runner all the way to the
/// chain link's blocking decision.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn replay_clean_fail_fixture_produces_validator_block() {
    let output = replay_ruleset(
        "rule_clean_fail.json",
        "playback-fail",
        "no-secrets",
        "Detect hard-coded secrets in the change set.",
    )
    .await;

    let block = output
        .validator_block
        .as_ref()
        .expect("clean-fail fixture should produce a validator block");
    assert!(
        block.validator_name.contains("no-secrets"),
        "block validator_name should reflect the recorded RuleSet/rule, got: {}",
        block.validator_name
    );
    assert_eq!(
        block.hook_type,
        avp_common::types::HookType::Stop,
        "block hook_type should be Stop (the trigger we configured)"
    );
}

/// Loading an unparseable recording — `<think>` block + free-form prose with
/// no JSON — locks in the parser's fail-closed behaviour: the chain produces
/// a validator block rather than silently passing.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn replay_unparseable_response_fixture_fails_closed() {
    let output = replay_ruleset(
        "rule_unparseable_response.json",
        "playback-unparseable",
        "tolerant-parser",
        "Verify the parser handles non-JSON responses gracefully.",
    )
    .await;

    assert!(
        output.validator_block.is_some(),
        "unparseable response should fail closed and produce a validator block, got: {:?}",
        output
    );
}

// ============================================================================
// Recording-on-disk tests — wrapping is wired up unconditionally.
// ============================================================================

/// Driving the validator chain produces a `RecordedSession` JSON file under
/// `<AVP_DIR>/recordings/`. There is no env-var gate — recording is always on.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn recording_directory_is_populated_unconditionally() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(temp.path().join(".git")).expect("create .git");

    let original_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(temp.path()).expect("chdir");
    std::env::set_var("AVP_SESSION_ID", "rec-session");

    let record_dir = temp.path().join(".avp").join("recordings");

    // Drive a real round-trip so the wrapper has at least one call to record.
    // Going through the chain mirrors how production reaches the runner and
    // gives the notification bridge time to forward playback notifications.
    {
        let agent = PlaybackAgent::new(recording_fixture_path("rule_clean_pass.json"), "claude");
        let notifications = agent.subscribe_notifications();
        let agent_arc: Arc<dyn agent_client_protocol::Agent + Send + Sync> = Arc::new(agent);
        let ctx = AvpContext::with_agent(agent_arc, notifications).expect("with_agent");

        let session_id = "rec-session";
        let turn_state =
            setup_turn_state_with_changes(&temp, session_id, &["src/recording-target.rs"]);

        write_stop_error_ruleset(
            &temp,
            "rec-test",
            "always-pass",
            "Recording-pipeline smoke test.",
        );

        let mut loader = ValidatorLoader::new();
        loader
            .load_rulesets_directory(
                &temp.path().join(".avp").join("validators"),
                ValidatorSource::Project,
            )
            .expect("load rulesets");

        let _claude_acp_guard = ClaudeAcpGuard::new();

        let factory = ChainFactory::new(Arc::new(ctx), Arc::new(loader), turn_state);
        let mut chain = factory.stop_chain();

        let input = build_stop_input(&temp, session_id);
        let _ = chain.execute(&input).await.expect("chain.execute");

        // Chain holds an Arc<AvpContext>; dropping it (via leaving scope)
        // releases the inner context which flushes the RecordingAgent.
    }

    assert!(
        record_dir.exists(),
        "recording directory should exist after a recorded run, looked at {}",
        record_dir.display()
    );
    let entries: Vec<_> = std::fs::read_dir(&record_dir)
        .expect("read recording dir")
        .filter_map(Result::ok)
        .collect();
    assert!(
        !entries.is_empty(),
        "recording dir should contain at least one file, dir={}",
        record_dir.display()
    );

    // Verify each file is a parseable RecordedSession with the session id we set.
    let saw_session_id = entries.iter().any(|e| {
        let name = e.file_name();
        name.to_string_lossy().starts_with("rec-session-")
            && name.to_string_lossy().ends_with(".json")
    });
    assert!(
        saw_session_id,
        "recording filename should include the configured session id, entries: {:?}",
        entries.iter().map(|e| e.file_name()).collect::<Vec<_>>()
    );

    // Verify each recording is a parseable RecordedSession with the expected
    // method calls — proves the recorder captured the agent trait calls we
    // care about (initialize / new_session / prompt) rather than just writing
    // an empty file.
    use agent_client_protocol_extras::recording::RecordedSession;
    let mut total_calls = 0usize;
    for entry in &entries {
        let path = entry.path();
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read recording {}: {}", path.display(), e));
        let session: RecordedSession = serde_json::from_str(&content).unwrap_or_else(|e| {
            panic!(
                "parse recording {} as RecordedSession: {}\ncontent:\n{}",
                path.display(),
                e,
                content
            )
        });
        let methods: Vec<&str> = session.calls.iter().map(|c| c.method.as_str()).collect();
        assert!(
            methods.contains(&"initialize"),
            "recording at {} should include an initialize call, got methods: {:?}",
            path.display(),
            methods
        );
        total_calls += session.calls.len();
    }
    assert!(
        total_calls > 0,
        "across all recordings, at least one call should have been captured"
    );

    std::env::remove_var("AVP_SESSION_ID");
    std::env::set_current_dir(original_cwd).expect("restore cwd");
}
