//! Regression tests for the Stop-hook code-quality dispatch path.
//!
//! These tests pin down the failure mode captured by kanban task
//! `01KQ8CXYMBGN1VTV4S89FGQYCA`: after the single-path validator pipeline
//! refactor (commit `9ecf3e70e`), the entire Stop-hook validator path went
//! silently inert. PostToolUse worked, but Stop dropped every ruleset on the
//! floor before reaching the runner.
//!
//! The tests here lock the contract that:
//!
//! 1. **Stop chain end-to-end** — a Stop hook with one changed `*.rs` file
//!    drives a `code-quality`-style ruleset (Stop trigger,
//!    `match.files: ["*.rs"]`) all the way through the chain to the runner,
//!    producing a non-empty `ExecutedRuleSet`. This is the regression test
//!    promised by the kanban description.
//!
//! 2. **Log format** — when the runner produces results, the structured log
//!    line `validator result validator="<ruleset>:<rule>" ... hook_type="Stop"`
//!    is emitted at info level. The kanban description grep'd for this exact
//!    shape (`grep -c 'code-quality' .avp/log`) and saw zero hits, so the
//!    log-format test is the canary that fires first if the dispatch path
//!    silently regresses again.
//!
//! 3. **Sidecar-diff fallback** — when `turn_state.changed` is empty but
//!    sidecar `.diff` files exist for the session, Stop still resolves a
//!    non-empty changed-files list and dispatches the matching ruleset.
//!
//! 4. **Real Pre→Post→Stop pipeline** — the previous tests pre-populate
//!    `turn_state.changed` (or sidecar diffs) directly via
//!    `setup_turn_state_with_changes` / `write_diff`. They assert the
//!    **reader** end of the pipeline: given primed state, does Stop resolve a
//!    non-empty changed-files list and dispatch the matching ruleset? They
//!    do not exercise the **writer** end. Test 4 drives the full chain
//!    pipeline through `ChainFactory::pre_tool_use_chain` →
//!    `post_tool_use_chain` → `stop_chain`, the same code path the avp-cli
//!    takes in production. No turn-state is primed manually. This is the
//!    test the kanban description's "writer-end" failure mode (path A) is
//!    locked against.
//!
//! 5. **No-op Write does not accumulate** — the production scenario that
//!    motivated the kanban task was a Write that overwrote a file with the
//!    exact same bytes. The transcript at session 23fb66fc-... shows
//!    `tool_result.content` and `tool_result.originalFile` are byte-identical
//!    for the 22:38:23 Write, so the file did not actually change.
//!    PostToolUseFileTracker correctly detected this and kept
//!    `state.changed` empty. Test 5 locks that contract — a no-op Write
//!    must not be flagged as a change. Without this test, a future
//!    implementer might "fix" the no-Stop-validation symptom by always
//!    flagging Writes as changes, which would thrash the validator pipeline
//!    by running code-quality on every tool call.
//!
//! All tests use a `code-quality`-named on-disk ruleset (rather than the
//! builtin) to avoid collision with the workspace builtins and to keep the
//! tests hermetic. The ruleset's structure mirrors the real
//! `builtin/validators/code-quality/VALIDATOR.md`: Stop trigger,
//! `match.files: ["*.rs"]`, error severity.

mod test_helpers;

use std::path::PathBuf;
use std::sync::Arc;

use avp_common::chain::ChainFactory;
use avp_common::turn::TurnStateManager;
use avp_common::types::{PostToolUseInput, PreToolUseInput};
use avp_common::validator::{ValidatorLoader, ValidatorSource};
use tempfile::TempDir;
use test_helpers::{
    build_stop_input, create_playback_context, recording_fixture_path,
    setup_turn_state_with_changes, ClaudeAcpGuard,
};

/// Lay down a multi-rule ruleset matching the structure of the real
/// `builtin/validators/code-quality/VALIDATOR.md`:
/// - `trigger: Stop`
/// - `match.files: ["*.rs"]`
/// - `severity: error`
/// - one rule per name in `rule_names`
///
/// All rules share the same body text — what matters is that each one drives
/// the runner once so the `validator result` log line fires per-rule.
fn write_code_quality_style_ruleset(temp: &TempDir, ruleset_name: &str, rule_names: &[&str]) {
    let ruleset_dir = temp
        .path()
        .join(".avp")
        .join("validators")
        .join(ruleset_name);
    let rules_dir = ruleset_dir.join("rules");
    std::fs::create_dir_all(&rules_dir).expect("create rules dir");

    // VALIDATOR.md — the manifest. `match.files: ["*.rs"]` is the load-bearing
    // part: it is what the bug regressed. Without changed_files reaching the
    // matcher, this manifest's `match.files` rejected every Stop input and the
    // ruleset never ran.
    std::fs::write(
        ruleset_dir.join("VALIDATOR.md"),
        format!(
            "---\nname: {ruleset_name}\ndescription: Regression-test code-quality ruleset\nversion: 1.0.0\ntrigger: Stop\nmatch:\n  files:\n    - \"*.rs\"\nseverity: error\n---\n\n# {ruleset_name} RuleSet\n\nRegression-test code-quality ruleset.\n",
        ),
    )
    .expect("write VALIDATOR.md");

    for rule_name in rule_names {
        std::fs::write(
            rules_dir.join(format!("{rule_name}.md")),
            format!(
                "---\nname: {rule_name}\ndescription: {rule_name} rule\n---\n\n# {rule_name} Rule\n\nProbe rule body.\n",
            ),
        )
        .expect("write rule");
    }
}

/// Drive a Stop chain against the `rule_clean_pass.json` playback fixture
/// with the given `code-quality`-style ruleset. Returns the `ChainOutput` and
/// the `TempDir` (kept alive so the tempdir doesn't drop while assertions
/// run).
async fn drive_stop_chain_with_ruleset(
    ruleset_name: &str,
    rule_names: &[&str],
    changed_files: &[&str],
) -> (avp_common::chain::ChainOutput, TempDir) {
    drive_stop_chain_with_ruleset_and_fixture(
        ruleset_name,
        rule_names,
        changed_files,
        "rule_clean_pass.json",
    )
    .await
}

/// Like [`drive_stop_chain_with_ruleset`] but the playback fixture is
/// caller-supplied. Multi-rule rulesets need a fixture that contains an
/// `initialize` plus one `new_session` + `prompt` pair per rule, since
/// [`avp_common::validator::ValidatorRunner::execute_ruleset`] runs each
/// rule in its own fresh ACP session.
async fn drive_stop_chain_with_ruleset_and_fixture(
    ruleset_name: &str,
    rule_names: &[&str],
    changed_files: &[&str],
    fixture_name: &str,
) -> (avp_common::chain::ChainOutput, TempDir) {
    let path = recording_fixture_path(fixture_name);
    let (temp, ctx) = create_playback_context(&path);

    let session_id = "code-quality-regression-session";
    let turn_state = setup_turn_state_with_changes(&temp, session_id, changed_files);

    write_code_quality_style_ruleset(&temp, ruleset_name, rule_names);

    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            ValidatorSource::Project,
        )
        .expect("load rulesets");
    assert_eq!(
        loader.list_rulesets().len(),
        1,
        "regression test ruleset should be the only one loaded",
    );

    let _claude_acp_guard = ClaudeAcpGuard::new();

    let factory = ChainFactory::new(Arc::new(ctx), Arc::new(loader), turn_state);
    let mut chain = factory.stop_chain();

    let input = build_stop_input(&temp, session_id);
    let (chain_output, _exit_code) = chain.execute(&input).await.expect("chain.execute");

    (chain_output, temp)
}

// ============================================================================
// Test 1 — Stop chain end-to-end regression test.
// ============================================================================

/// Stop hook with one changed `*.rs` file → `code-quality`-style ruleset
/// matches and dispatches to the runner.
///
/// The pre-fix behaviour: `ValidatorExecutorLink::process` built a match
/// context with `changed_files = None` (or empty) and `RuleSet::matches` in
/// `validator/types.rs` rejected every ruleset whose `match.files` was
/// non-empty. The runner was never invoked, so the chain returned
/// `ChainResult::Continue(None)` with `validator_block: None` AND no
/// recording was produced (because `agent.prompt()` was never called).
///
/// The post-fix behaviour: `load_changed_files_for_stop` resolves
/// `["src/sample_avp_test.rs"]` from turn state, the ruleset matches, and
/// the runner is invoked.
///
/// We assert "the runner actually ran" by:
///
/// 1. Checking that the chain output carries a `validator_block` whose
///    `validator_name` is `<ruleset>:<rule>` — the runner produced a
///    rule result and the chain link surfaced it. This is the strongest
///    evidence available: the chain output cannot mention a rule name
///    unless the runner was reached for that rule.
///
/// 2. Checking that a recording session file exists with a `prompt`
///    method call. Recording wraps the playback agent in
///    `AvpContext::with_agent`; if the chain rejected the ruleset before
///    reaching the runner, the recording would not contain a `prompt`
///    call.
///
/// We do NOT assert on `chain_output.continue_execution` or pass/fail —
/// the playback bridge can race the runner's notification collector
/// (documented in `validator_block_e2e_integration.rs`), so the rule
/// result is sometimes "passed" and sometimes "empty response, agent
/// stopped with EndTurn". Either outcome proves the runner ran, which
/// is what this regression test pins down. The pass/fail content is
/// covered by other tests.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn stop_hook_with_one_rs_changed_file_dispatches_code_quality_ruleset() {
    // Single rule keeps the playback fixture's "one prompt" in lockstep with
    // the runner's "one rule per ruleset" iteration.
    let (chain_output, temp) =
        drive_stop_chain_with_ruleset("code-quality", &["probe"], &["src/sample_avp_test.rs"])
            .await;

    // 1. Strongest signal: the chain output mentions our ruleset by name.
    //    `validator_block` is set when a rule failed with error severity;
    //    `stop_reason` mirrors the validator message. Either path proves
    //    the runner was invoked for `code-quality:probe`.
    let runner_was_reached = match &chain_output.validator_block {
        Some(block) => block.validator_name == "code-quality:probe",
        None => false,
    };

    // 2. Independent signal: the recording captured the rule's prompt call.
    //    Drop chain_output to release the AvpContext so the RecordingAgent
    //    can flush its session file on Drop, then poll briefly.
    drop(chain_output);
    let recordings_dir = temp.path().join(".avp").join("recordings");
    let mut any_prompt_call = false;
    for _ in 0..20 {
        if let Ok(entries) = std::fs::read_dir(&recordings_dir) {
            let recordings: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
                .collect();
            if recordings.iter().any(|path| {
                std::fs::read_to_string(path)
                    .map(|content| content.contains("\"method\": \"prompt\""))
                    .unwrap_or(false)
            }) {
                any_prompt_call = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert!(
        runner_was_reached || any_prompt_call,
        "Stop chain must reach the runner for the `code-quality:probe` rule, but \
         neither the chain output nor the recordings dir under {} showed any \
         evidence of a runner invocation. Chain output validator_block: missing \
         for `code-quality:probe`; no recording with a `prompt` method call \
         found. This is the regression — the ruleset was rejected before \
         agent.prompt() was invoked.",
        recordings_dir.display()
    );
}

// ============================================================================
// Test 2 — Log-format test.
// ============================================================================

/// When the Stop chain dispatches a `code-quality`-style ruleset, the runner
/// emits a structured `validator result` log line per rule, with
/// `validator="<ruleset>:<rule>"` and `hook_type="Stop"`.
///
/// The kanban description noted that the broken-state symptom was
/// `grep -c 'code-quality' .avp/log` returning 0. This test pins the log
/// shape so a future regression in the dispatch path immediately fails this
/// assertion rather than silently falling back to "no log line at all".
///
/// Implementation note: `tracing-test`'s `#[traced_test]` macro installs a
/// per-test subscriber that records all events in a thread-local buffer.
/// `logs_contain` then scans that buffer for substring matches. We assert
/// on:
///   - the literal log message `validator result`
///   - the structured field `validator="code-quality:probe"`
///   - the structured field `hook_type="Stop"`
///
/// All three are produced by `AvpContext::log_validator` via
/// `tracing::info!(validator=..., passed=..., hook_type=..., message=..., "validator result")`.
/// The `tracing-test` formatter renders structured fields as `key="value"`,
/// matching the on-disk `.avp/log` layout from `avp-cli/src/main.rs`'s
/// `tracing_subscriber::fmt::layer()` setup.
#[tokio::test]
#[serial_test::serial(cwd, env)]
#[tracing_test::traced_test]
async fn stop_hook_emits_validator_result_log_line_with_code_quality_prefix() {
    let (_chain_output, _temp) =
        drive_stop_chain_with_ruleset("code-quality", &["probe"], &["src/sample_avp_test.rs"])
            .await;

    // Core message — same literal that production scrapes: `validator result`.
    assert!(
        logs_contain("validator result"),
        "expected `validator result` log message; tracing buffer was empty for that string. \
         If this fires, the runner never logged its per-rule result — meaning the chain \
         dropped the ruleset before dispatch (the kanban regression)."
    );

    // The fully-qualified validator name `<ruleset>:<rule>`. This is what
    // `grep -c 'code-quality' .avp/log` would match against — the literal
    // shape the kanban description observed as missing.
    assert!(
        logs_contain("validator=\"code-quality:probe\""),
        "expected structured field `validator=\"code-quality:probe\"` in log buffer"
    );

    // `hook_type` field — the dimension that distinguishes Stop runs from
    // PostToolUse runs in the same log file.
    assert!(
        logs_contain("hook_type=\"Stop\""),
        "expected structured field `hook_type=\"Stop\"` in log buffer"
    );
}

/// For an N-rule Stop ruleset, the runner must emit **exactly N**
/// `validator result` log lines with `hook_type="Stop"` — one per rule,
/// fired as soon as that rule's verdict is known.
///
/// This pins down the kanban task `01KQAFE5WGYJK3HZ8WE3B8N86K` regression.
/// Before the fix, the per-rule `validator result` line was emitted in a
/// deferred batch after `execute_rulesets()` returned. That batch never
/// fired in production because the Stop hook ran 11 rules over ~13 minutes
/// and the upstream caller dropped the await before the deferred emit
/// reached the await point. Worse, the only log site for Stop was that
/// deferred call, so production saw zero `validator result … hook_type="Stop"`
/// lines for the rules that DID complete cleanly.
///
/// The fix moves the emit inside `ValidatorRunner::execute_ruleset` so each
/// rule's verdict logs as soon as the rule completes. This test asserts:
///
/// 1. **Exactly N lines** — no duplicates from a residual batch emit, no
///    misses from a runner that only logged on success paths.
/// 2. **`hook_type="Stop"`** on each line — the runner threads the hook
///    type through to the eager emit, byte-for-byte matching PostToolUse.
#[tokio::test]
#[serial_test::serial(cwd, env)]
#[tracing_test::traced_test]
async fn stop_hook_emits_exactly_n_validator_result_lines_for_two_rule_run() {
    const N_RULES: usize = 2;
    // Build N rule names (`probe_0`, `probe_1`) so the qualified names
    // emitted by the runner are distinguishable in the log buffer.
    let rule_names: Vec<String> = (0..N_RULES).map(|i| format!("probe_{i}")).collect();
    let rule_refs: Vec<&str> = rule_names.iter().map(|s| s.as_str()).collect();

    let _ = drive_stop_chain_with_ruleset_and_fixture(
        "code-quality",
        &rule_refs,
        &["src/sample_avp_test.rs"],
        "rule_clean_pass_two_rules.json",
    )
    .await;

    // Count lines that satisfy ALL three criteria:
    //   - the literal log message "validator result"
    //   - structured field `hook_type="Stop"`
    //   - structured field `validator="code-quality:..."` (qualified prefix
    //     proves the runner reached the rule, not just the placeholder path)
    //
    // `tracing_test::traced_test` injects the `logs_assert` helper into this
    // function. It exposes the captured lines as `&[&str]`, one per emitted
    // event. Counting lines that contain all three substrings gives exactly
    // the per-rule emits.
    logs_assert(|lines: &[&str]| {
        let count = lines
            .iter()
            .filter(|line| {
                line.contains("validator result")
                    && line.contains("hook_type=\"Stop\"")
                    && line.contains("validator=\"code-quality:")
            })
            .count();
        if count == N_RULES {
            Ok(())
        } else {
            Err(format!(
                "expected exactly {N_RULES} `validator result … hook_type=\"Stop\"` line(s) for a {N_RULES}-rule Stop run, but found {count}. \
                 Captured lines:\n{}",
                lines.join("\n")
            ))
        }
    });
}

// ============================================================================
// Test 3 — Sidecar-diff fallback when turn_state.changed is empty.
// ============================================================================

/// Sidecar-diff fallback: when `turn_state.changed` is empty but per-file
/// `.diff` sidecars exist under `.avp/turn_diffs/<session_id>/`, the Stop
/// chain still resolves a non-empty changed-files list and dispatches the
/// matching ruleset.
///
/// This is a direct unit-style test of the bug-mode #2 from the kanban
/// description: "Stop-hook changed-files computation produces an empty list."
///
/// Pre-fix: `load_changed_files_for_stop` only consulted `turn_state.changed`.
/// If PostToolUse wrote diff sidecars but did not also append to
/// `turn_state.changed` (e.g. across a process boundary, or when
/// `state.pending` was missing for the tool_use_id), Stop would resolve
/// `None` and reject every ruleset with `match.files`.
///
/// Post-fix: when `turn_state.changed` is empty, `load_changed_files_for_stop`
/// falls back to enumerating sidecar diff filenames. The resulting list is
/// what drives matching — exactly what the broken state needed.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn stop_hook_falls_back_to_sidecar_diffs_when_turn_state_changed_is_empty() {
    let path = recording_fixture_path("rule_clean_pass.json");
    let (temp, ctx) = create_playback_context(&path);

    let session_id = "sidecar-fallback-session";

    // Crucial: do NOT set turn_state.changed. The TurnStateManager exists
    // but its state file either has no `changed` entries or doesn't exist.
    // Then write a sidecar diff directly — mirroring what PostToolUseFileTracker
    // does when it computes a real diff but, hypothetically, the state save
    // dropped the path from `changed`.
    let turn_state = std::sync::Arc::new(TurnStateManager::new(temp.path()));
    turn_state
        .write_diff(
            session_id,
            std::path::Path::new("src/sample_avp_test.rs"),
            "--- a/src/sample_avp_test.rs\n+++ b/src/sample_avp_test.rs\n@@ -1 +1 @@\n-old\n+new\n",
        )
        .expect("write sidecar diff");

    // Confirm the precondition: state.changed really is empty.
    let state_pre = turn_state.load(session_id).expect("load state");
    assert!(
        state_pre.changed.is_empty(),
        "test precondition: turn_state.changed must be empty for the fallback to be exercised, got: {:?}",
        state_pre.changed
    );

    write_code_quality_style_ruleset(&temp, "code-quality", &["probe"]);

    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            ValidatorSource::Project,
        )
        .expect("load rulesets");

    let _claude_acp_guard = ClaudeAcpGuard::new();

    let factory = ChainFactory::new(
        std::sync::Arc::new(ctx),
        std::sync::Arc::new(loader),
        turn_state,
    );
    let mut chain = factory.stop_chain();

    let input = build_stop_input(&temp, session_id);
    let (chain_output, _exit_code) = chain.execute(&input).await.expect("chain.execute");

    // The fallback resolved `["src/sample_avp_test.rs"]` from the sidecar,
    // the ruleset matched, and the runner produced a `code-quality:probe`
    // result that the chain link surfaced.
    //
    // Like Test 1, we don't assert pass/fail of the runner — under ACP 0.11
    // the playback fixture's "passed" verdict is now delivered reliably (no
    // bridge race), so a passed verdict produces NO `validator_block`. The
    // strongest signal that the dispatch path reached the runner is either:
    //   1. the chain output carries a `validator_block` whose name is
    //      `code-quality:probe` (block fired), OR
    //   2. the recording captured a `prompt` method call (the runner ran
    //      and the rule prompt reached the playback agent).
    // Either path proves the sidecar fallback resolved a non-empty
    // changed-files list and the chain dispatched the matching ruleset —
    // the regression this test pins down.
    let runner_was_reached = chain_output
        .validator_block
        .as_ref()
        .is_some_and(|block| block.validator_name == "code-quality:probe");

    drop(chain_output);
    let recordings_dir = temp.path().join(".avp").join("recordings");
    let mut any_prompt_call = false;
    for _ in 0..20 {
        if let Ok(entries) = std::fs::read_dir(&recordings_dir) {
            let recordings: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
                .collect();
            if recordings.iter().any(|path| {
                std::fs::read_to_string(path)
                    .map(|content| content.contains("\"method\": \"prompt\""))
                    .unwrap_or(false)
            }) {
                any_prompt_call = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert!(
        runner_was_reached || any_prompt_call,
        "Sidecar fallback must reach the runner for `code-quality:probe`. \
         Empty validator_block AND no recording with a `prompt` method call \
         under {} means the chain rejected the ruleset before dispatch — the \
         regression. Expected the fallback path in `load_changed_files_for_stop` \
         to derive the changed-files list from sidecar diffs.",
        recordings_dir.display(),
    );
}

/// Sentinel test: when neither `turn_state.changed` nor sidecar diffs exist
/// for a Stop hook, no rulesets with `match.files` patterns match and the
/// chain continues without dispatch. This is the correct quiet-path
/// behaviour and ensures the fallback isn't accidentally treating a
/// genuinely-empty session as having phantom files.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn stop_hook_with_no_changes_does_not_dispatch_code_quality_ruleset() {
    let path = recording_fixture_path("rule_clean_pass.json");
    let (temp, ctx) = create_playback_context(&path);

    let session_id = "no-changes-session";
    // Turn state manager with no changed files and no sidecar diffs.
    let turn_state = std::sync::Arc::new(TurnStateManager::new(temp.path()));

    write_code_quality_style_ruleset(&temp, "code-quality", &["probe"]);

    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            ValidatorSource::Project,
        )
        .expect("load rulesets");

    let _claude_acp_guard = ClaudeAcpGuard::new();

    let factory = ChainFactory::new(
        std::sync::Arc::new(ctx),
        std::sync::Arc::new(loader),
        turn_state,
    );
    let mut chain = factory.stop_chain();

    let input = build_stop_input(&temp, session_id);
    let (chain_output, _exit_code) = chain.execute(&input).await.expect("chain.execute");

    // No changes anywhere ⇒ no ruleset match ⇒ chain continues with no
    // validator_block. This is the correct behaviour: a Stop hook firing
    // with nothing to validate should not run validators at all.
    assert!(
        chain_output.validator_block.is_none(),
        "Stop hook with no changed files anywhere must not dispatch any \
         ruleset, but got validator_block: {:?}. The fallback should only \
         resolve from sidecars when sidecars exist; an empty session must \
         stay quiet.",
        chain_output.validator_block
    );
    assert!(
        chain_output.continue_execution,
        "Stop hook with no changes must let the chain continue"
    );

    // Suppress unused-variable lint on `setup_turn_state_with_changes` —
    // we deliberately do not use it here (this test's precondition is
    // "no changes anywhere"), but importing it keeps the test file's
    // helper surface uniform.
    let _ = setup_turn_state_with_changes;
}

// ============================================================================
// Test 4 — Real PreToolUse(Write) → PostToolUse(Write) → Stop pipeline.
// ============================================================================
// Rationale: see module-level docs (test 4 entry).

/// Build a PreToolUse(Write) input for the given absolute file path and
/// session/tool ids. Mirrors the JSON shape Claude Code sends: top-level
/// `file_path` (absolute) and `content`, with `tool_use_id` populated.
fn build_pre_tool_use_write_input(
    cwd: &std::path::Path,
    session_id: &str,
    tool_use_id: &str,
    file_path: &std::path::Path,
    content: &str,
) -> PreToolUseInput {
    serde_json::from_value(serde_json::json!({
        "session_id": session_id,
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": cwd.to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path.to_string_lossy(),
            "content": content,
        },
        "tool_use_id": tool_use_id,
    }))
    .expect("build PreToolUseInput")
}

/// Build a PostToolUse(Write) input for the given absolute file path and
/// session/tool ids. Pairs with `build_pre_tool_use_write_input` to drive
/// the full Pre → Post → Stop pipeline through the chain factory.
fn build_post_tool_use_write_input(
    cwd: &std::path::Path,
    session_id: &str,
    tool_use_id: &str,
    file_path: &std::path::Path,
    content: &str,
) -> PostToolUseInput {
    serde_json::from_value(serde_json::json!({
        "session_id": session_id,
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": cwd.to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": {
            "file_path": file_path.to_string_lossy(),
            "content": content,
        },
        "tool_response": {
            "filePath": file_path.to_string_lossy(),
            "success": true,
        },
        "tool_use_id": tool_use_id,
    }))
    .expect("build PostToolUseInput")
}

/// End-to-end pipeline regression: a real PreToolUse(Write) followed by a
/// PostToolUse(Write) populates `turn_state.changed` such that a subsequent
/// Stop hook resolves a non-empty changed-files list and matches the
/// `code-quality`-style ruleset.
///
/// The kanban description's bug-mode A: "PostToolUse(Write) doesn't write to
/// turn-state at all". If `PostToolUseFileTracker` silently skips Write (or
/// fails to look up `state.pending` by `tool_use_id`), this test's
/// `state.changed.is_empty()` assertion fires and the regression is locked
/// in.
///
/// We deliberately use **absolute** paths (the shape Claude Code sends).
/// `extract_tool_paths` requires `is_path_structural` to return true for the
/// candidate — bare relative paths like `"foo.rs"` fail that check, so
/// passing one would silently drop the file from tracking. The fact that
/// production sends absolute paths is what keeps the writer reachable in the
/// real world; this test mirrors that contract.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn full_pipeline_pre_post_write_then_stop_dispatches_code_quality() {
    let path = recording_fixture_path("rule_clean_pass.json");
    let (temp, ctx) = create_playback_context(&path);

    let session_id = "full-pipeline-write-stop-session";
    let tool_use_id = "toolu_full_pipeline_test";

    // The Write target is an absolute path inside the temp workspace —
    // matching the shape Claude Code sends (`file_path: /Users/.../foo.rs`).
    // The parent directory must exist so `validate_path_candidate` returns
    // Some(...) for the new-file case.
    let target_dir = temp.path().join("src");
    std::fs::create_dir_all(&target_dir).expect("create target dir");
    let target_file = target_dir.join("regression_target.rs");
    let content = "// regression target\nfn main() {}\n";

    // Shared turn state — the same on-disk YAML the avp-cli would load
    // across the three separate hook processes.
    let turn_state = std::sync::Arc::new(TurnStateManager::new(temp.path()));

    write_code_quality_style_ruleset(&temp, "code-quality", &["probe"]);

    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            ValidatorSource::Project,
        )
        .expect("load rulesets");

    let _claude_acp_guard = ClaudeAcpGuard::new();

    let factory = ChainFactory::new(
        std::sync::Arc::new(ctx),
        std::sync::Arc::new(loader),
        turn_state.clone(),
    );

    // Step 1: PreToolUse(Write) — file does not yet exist on disk.
    // PreToolUseFileTracker hashes None for it and stashes that under
    // state.pending[tool_use_id].
    let pre_input =
        build_pre_tool_use_write_input(temp.path(), session_id, tool_use_id, &target_file, content);
    let mut pre_chain = factory.pre_tool_use_chain();
    let _ = pre_chain
        .execute(&pre_input)
        .await
        .expect("pre_chain.execute");

    let state_after_pre = turn_state.load(session_id).expect("load state after pre");
    assert!(
        state_after_pre.pending.contains_key(tool_use_id),
        "PreToolUseFileTracker must populate state.pending[{tool_use_id}], got pending keys: {:?}",
        state_after_pre.pending.keys().collect::<Vec<_>>()
    );

    // Step 2: simulate the Write tool's effect (creates the file). In real
    // execution Claude Code performs this between PreToolUse and PostToolUse.
    std::fs::write(&target_file, content).expect("simulate Write tool output");

    // Step 3: PostToolUse(Write) — looks up state.pending[tool_use_id], hashes
    // the now-extant file, sees the hash differ from None, and accumulates
    // the path into state.changed.
    let post_input = build_post_tool_use_write_input(
        temp.path(),
        session_id,
        tool_use_id,
        &target_file,
        content,
    );
    let mut post_chain = factory.post_tool_use_chain();
    let _ = post_chain
        .execute(&post_input)
        .await
        .expect("post_chain.execute");

    // Bug-mode A assertion: PostToolUseFileTracker MUST have appended the
    // Write target to state.changed. If state.changed is empty here, the
    // Stop chain's load_changed_files_for_stop will return None and every
    // ruleset with `match.files` patterns will be silently rejected — the
    // exact regression captured by kanban task 01KQ8CXYMBGN1VTV4S89FGQYCA.
    let state_after_post = turn_state.load(session_id).expect("load state after post");
    assert!(
        state_after_post.changed.contains(&target_file),
        "PostToolUseFileTracker must append the Write target {target_file:?} to state.changed. \
         Got state.changed = {:?}. This is the kanban task 01KQ8CXYMBGN1VTV4S89FGQYCA \
         regression — the writer is dropping the path even though PreToolUseFileTracker \
         populated state.pending and the file's hash changed (None → Some). \
         If this assertion fires, the bug is in `PostToolUseFileTracker::process` in \
         `avp-common/src/chain/links/file_tracker.rs`.",
        state_after_post.changed,
    );
    assert!(
        !state_after_post.pending.contains_key(tool_use_id),
        "PostToolUseFileTracker must consume state.pending[{tool_use_id}], leaving it empty",
    );

    // Step 4: Stop chain — should resolve the Write target as a changed file
    // and dispatch the `code-quality:probe` ruleset.
    let stop_input = build_stop_input(&temp, session_id);
    let mut stop_chain = factory.stop_chain();
    let (chain_output, _exit_code) = stop_chain
        .execute(&stop_input)
        .await
        .expect("stop_chain.execute");

    // The strongest signal that the dispatch path reached the runner:
    // chain output carries a `validator_block` whose `validator_name` is
    // `code-quality:probe`. The runner cannot mention a rule name unless it
    // was invoked for that rule.
    //
    // Like Test 1, we don't assert pass/fail of the runner — the playback
    // bridge can race the notification collector, so the rule result is
    // sometimes "passed" and sometimes "EndTurn before response". Either
    // outcome proves the runner ran, which is what this regression test
    // pins down.
    let runner_was_reached_for_code_quality = chain_output
        .validator_block
        .as_ref()
        .is_some_and(|block| block.validator_name == "code-quality:probe");

    // Independent signal: the recording captured a prompt call. Drop the
    // chain_output so the RecordingAgent flushes its session file on Drop.
    drop(chain_output);
    let recordings_dir = temp.path().join(".avp").join("recordings");
    let mut any_prompt_call = false;
    for _ in 0..20 {
        if let Ok(entries) = std::fs::read_dir(&recordings_dir) {
            let recordings: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
                .collect();
            if recordings.iter().any(|path| {
                std::fs::read_to_string(path)
                    .map(|content| content.contains("\"method\": \"prompt\""))
                    .unwrap_or(false)
            }) {
                any_prompt_call = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert!(
        runner_was_reached_for_code_quality || any_prompt_call,
        "Stop chain must reach the runner for `code-quality:probe` after a Write \
         tool change. Pre/Post wiring populated state.changed = {:?}, but the \
         Stop dispatch dropped the ruleset. Chain output validator_block did not \
         mention `code-quality:probe` and no recording with a `prompt` method \
         call was found under {}. This proves the kanban task \
         01KQ8CXYMBGN1VTV4S89FGQYCA writer-end regression — even though the \
         turn-state is correctly populated, the Stop chain still drops the \
         ruleset.",
        state_after_post.changed,
        recordings_dir.display(),
    );
}

// ============================================================================
// Test 5 — No-op Write does NOT accumulate into state.changed (Path C).
// ============================================================================

/// A Write that overwrites a file with the **same bytes** must not appear in
/// `state.changed`. The hash diff detects no change; the validator pipeline
/// correctly stays silent.
///
/// This is the production scenario that motivated the kanban task. The user's
/// 22:38 qwen test re-wrote `swissarmyhammer-common/src/sample_avp_test.rs`
/// with byte-identical content (the transcript's `tool_result.content` and
/// `tool_result.originalFile` are equal strings). PostToolUseFileTracker
/// correctly detected no change, `state.changed` stayed empty, and the Stop
/// hook's `code-quality` ruleset (which requires at least one `*.rs` in the
/// changed-files list) was correctly rejected.
///
/// The kanban description reported this as a regression, but the writer is
/// behaving correctly: a no-op Write should not trigger code-quality.
/// Without this test, a future implementer might "fix" the symptom by always
/// flagging writes as changes — which would cause code-quality to run on
/// every tool call regardless of whether the file actually changed,
/// thrashing the validator pipeline.
///
/// This test pairs with `full_pipeline_pre_post_write_then_stop_dispatches_code_quality`
/// (which proves: Write that *does* change content → state.changed populated)
/// to lock the contract from both sides.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn noop_write_does_not_accumulate_into_changed() {
    let path = recording_fixture_path("rule_clean_pass.json");
    let (temp, ctx) = create_playback_context(&path);

    let session_id = "noop-write-session";
    let tool_use_id = "toolu_noop_write";

    // Create the file with the content we'll later "Write" — same bytes.
    let target_dir = temp.path().join("src");
    std::fs::create_dir_all(&target_dir).expect("create target dir");
    let target_file = target_dir.join("noop_target.rs");
    let content = "// no-op test\nfn main() {}\n";
    std::fs::write(&target_file, content).expect("seed file with original content");

    let turn_state = std::sync::Arc::new(TurnStateManager::new(temp.path()));

    write_code_quality_style_ruleset(&temp, "code-quality", &["probe"]);

    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            ValidatorSource::Project,
        )
        .expect("load rulesets");

    let _claude_acp_guard = ClaudeAcpGuard::new();

    let factory = ChainFactory::new(
        std::sync::Arc::new(ctx),
        std::sync::Arc::new(loader),
        turn_state.clone(),
    );

    // Step 1: PreToolUse(Write) — file already exists. Pre hashes the
    // existing bytes and stashes that under state.pending[tool_use_id].
    let pre_input =
        build_pre_tool_use_write_input(temp.path(), session_id, tool_use_id, &target_file, content);
    let mut pre_chain = factory.pre_tool_use_chain();
    let _ = pre_chain
        .execute(&pre_input)
        .await
        .expect("pre_chain.execute");

    let state_after_pre = turn_state.load(session_id).expect("load state after pre");
    assert!(
        state_after_pre.pending.contains_key(tool_use_id),
        "Pre must stash a snapshot for the no-op-write tool_use_id",
    );

    // Step 2: simulate the Write tool's effect — overwrite with the SAME
    // bytes. This is what Claude Code's Write does when the user's request
    // matches what's already on disk: it issues a successful Write with no
    // actual byte change. The mtime updates but the content hash does not.
    std::fs::write(&target_file, content).expect("simulate no-op Write");

    // Step 3: PostToolUse(Write) — looks up state.pending[tool_use_id], hashes
    // the file (now byte-identical to before), sees the hashes match, and
    // correctly does NOT append to state.changed.
    let post_input = build_post_tool_use_write_input(
        temp.path(),
        session_id,
        tool_use_id,
        &target_file,
        content,
    );
    let mut post_chain = factory.post_tool_use_chain();
    let _ = post_chain
        .execute(&post_input)
        .await
        .expect("post_chain.execute");

    let state_after_post = turn_state.load(session_id).expect("load state after post");

    // Path C contract: no-op Write produces an empty changed list. If this
    // assertion fires, someone "fixed" the kanban task by always flagging
    // Writes as changed — which would trigger code-quality on every tool
    // call regardless of actual file content change.
    assert!(
        state_after_post.changed.is_empty(),
        "No-op Write must not accumulate into state.changed. Got: {:?}. \
         If this fires, the writer is over-reporting changes — a Write that \
         overwrites a file with identical bytes should be invisible to the \
         Stop validator pipeline (the file did not actually change).",
        state_after_post.changed,
    );

    // Pending should still be consumed (the tool_use_id was processed).
    assert!(
        !state_after_post.pending.contains_key(tool_use_id),
        "Post must consume state.pending[{tool_use_id}] even when no change is detected",
    );
}
