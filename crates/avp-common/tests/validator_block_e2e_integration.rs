//! End-to-end integration test for the error-severity validator block path.
//!
//! When a Stop-hook validator with `severity: error` returns
//! `{"status": "failed", ...}`, AVP must:
//!
//! 1. Mark the rule as a blocking failure on the executed RuleSet.
//! 2. Have `ValidatorExecutorLink` produce
//!    `ChainResult::stop(LinkOutput::from_validator_block(...))` so the chain
//!    short-circuits.
//! 3. Have `Chain::execute` promote the chain output's exit code to
//!    [`avp_common::chain::VALIDATOR_BLOCK_EXIT_CODE`] (2) because
//!    `continue_execution` is false.
//! 4. Have `ClaudeCodeHookStrategy` transform the chain output into the
//!    Stop-hook block format that claude-code parses:
//!    `{"continue": true, "decision": "block", "reason": "...", "stopReason": "..."}`
//!    on stdout, with strategy exit code 0 (Stop is a stdout-style block surface).
//!
//! Until kanban task `01KQ7M20F27D0Z67H9XX0XQ4QZ` this whole path was untested
//! end-to-end. Coverage is split across three files:
//!
//! - **Unit half** —
//!   `avp-common/src/chain/links/validator_executor.rs::tests` covers
//!   `handle_ruleset_results` for items 1, 2, and 3 (per-hook exit-code
//!   handling) in isolation against synthetic `ExecutedRuleSet`s.
//!
//! - **Chain integration (this file)** — drives a real `PlaybackAgent` through
//!   the production Stop chain against a checked-in `RecordedSession`
//!   fixture, asserting on the aggregated `ChainOutput` and the exit code
//!   `Chain::execute` reports for a blocked Stop. This is item 3.
//!
//! - **Strategy-render unit test** —
//!   `avp-common/src/strategy/claude/strategy.rs::tests::block_stop_renders_claude_parseable_json`
//!   takes a hand-built `ChainOutput` and runs it through
//!   `transform_to_claude_output` directly, asserting on the JSON shape
//!   claude-code parses. That is item 4 — kept as a unit test because the
//!   transform is a pure function and using `PlaybackAgent` for it would
//!   exhaust the recorded prompts when builtin Stop validators run alongside
//!   the test ruleset.
//!
//! ### Notes on what's covered here vs. elsewhere
//!
//! `recording_replay_integration.rs::replay_clean_fail_fixture_produces_validator_block`
//! already proves a fail recording flows to a `validator_block` on the chain
//! output, but it does not assert on the chain's reported exit code. The
//! exit-code contract is the load-bearing property for the Stop hook to
//! actually block claude-code, so it gets its own focused test here.

mod test_helpers;

use std::sync::Arc;

use avp_common::chain::{ChainFactory, VALIDATOR_BLOCK_EXIT_CODE};
use avp_common::types::HookType;
use avp_common::validator::{ValidatorLoader, ValidatorSource};
use tempfile::TempDir;
use test_helpers::{
    build_stop_input, create_playback_context, recording_fixture_path,
    setup_turn_state_with_changes, write_stop_error_ruleset, ClaudeAcpGuard,
};

/// Drive a single-rule Stop ruleset through the full chain pipeline and
/// return both the chain output and the exit code the chain reported.
async fn execute_stop_chain_with_fixture(
    fixture_name: &str,
    ruleset_name: &str,
    rule_name: &str,
    rule_body: &str,
) -> (avp_common::chain::ChainOutput, i32, TempDir) {
    let path = recording_fixture_path(fixture_name);
    let (temp, ctx) = create_playback_context(&path);

    let session_id = "e2e-block-session";
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

    let _claude_acp_guard = ClaudeAcpGuard::new();

    let factory = ChainFactory::new(Arc::new(ctx), Arc::new(loader), turn_state);
    let mut chain = factory.stop_chain();

    let input = build_stop_input(&temp, session_id);
    let (chain_output, exit_code) = chain.execute(&input).await.expect("chain.execute");

    (chain_output, exit_code, temp)
}

// ============================================================================
// E2E test 1 — chain output for an error-severity Stop block.
// ============================================================================

/// Driving the Stop chain against the magic-number fail fixture must:
/// - produce a `validator_block` whose name is `<ruleset>:<rule>` and a
///   non-empty failure message;
/// - flip the chain's `continue_execution` flag to false;
/// - return [`VALIDATOR_BLOCK_EXIT_CODE`] (2) as the chain's exit code, which
///   is the executor's contract for a blocked output regardless of whether
///   the agent strategy ultimately re-renders that as exit-2-stderr or
///   exit-0-with-decision-block.
///
/// We do NOT assert on the literal recorded text round-tripping through the
/// runner. The `PlaybackAgent` notification bridge is async and can race the
/// runner's response collector, so the runner sometimes sees an empty
/// response and reports "Validator returned empty response - agent stopped
/// with reason: EndTurn". Either outcome is a blocking failure with a
/// non-empty message — that is the contract this test pins down. The
/// PlaybackAgent notification timing is a separate concern.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn stop_hook_error_severity_failure_blocks_chain_with_exit_code_2() {
    // Use a ruleset/rule name that does NOT collide with any builtin
    // (e.g. `code-quality:no-magic-numbers`) so the on-disk project ruleset is
    // unambiguously the one driving the chain.
    let (chain_output, exit_code, _temp) = execute_stop_chain_with_fixture(
        "rule_magic_number_fail.json",
        "e2e-block",
        "magic-number-finder",
        "Detect magic numbers in the change set.",
    )
    .await;

    // Chain output: validator_block carries the qualified rule name and a
    // non-empty failure message.
    let block = chain_output
        .validator_block
        .as_ref()
        .expect("error-severity Stop fixture must produce a validator_block");
    assert_eq!(
        block.validator_name, "e2e-block:magic-number-finder",
        "validator_name should be `<ruleset>:<rule>`"
    );
    assert!(
        !block.message.trim().is_empty(),
        "block.message must be non-empty so claude-code has something to surface"
    );
    assert_eq!(
        block.hook_type,
        HookType::Stop,
        "block.hook_type should reflect the trigger we configured"
    );

    // Chain output: continue_execution flipped to false.
    assert!(
        !chain_output.continue_execution,
        "blocked Stop must clear continue_execution"
    );

    // Chain output's stop_reason mirrors the validator message so agent
    // strategies can format the user-facing "reason" without re-reading
    // validator_block.
    assert_eq!(
        chain_output.stop_reason.as_deref(),
        Some(block.message.as_str()),
        "chain stop_reason should mirror the blocking validator's message"
    );

    // Exit code: the chain executor promotes the exit code to
    // VALIDATOR_BLOCK_EXIT_CODE whenever the aggregated output has
    // continue_execution=false. This is the "exit 2 on block" contract from
    // the chain layer — agent strategies may transform it further.
    assert_eq!(
        exit_code, VALIDATOR_BLOCK_EXIT_CODE,
        "Chain::execute must report VALIDATOR_BLOCK_EXIT_CODE (2) for a blocked Stop"
    );
}

// ============================================================================
// Note on the strategy-level rendering test
//
// A test that the `ClaudeCodeHookStrategy` renders a Stop block as the JSON
// shape claude-code parses lives next to the strategy itself, in
// `avp-common/src/strategy/claude/strategy.rs::tests`
// (`block_stop_renders_claude_parseable_json`). It builds a `ChainOutput`
// in memory and calls `transform_to_claude_output` directly, so the rendering
// is verified without having to round-trip through `PlaybackAgent`'s async
// notification bridge — which the chain-level test above already exercises.
// ============================================================================
