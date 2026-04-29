---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8e80
title: 'Stop-hook: per-rule validator results not logged for hook_type="Stop"'
---
## Symptom

Today's 16:06–16:19 Stop-hook run successfully fired the regression-fix path:
- `ValidatorExecutorLink: Stop hook resolved changed files changed_files_count=1` ✅
- `ValidatorExecutorLink: Executing 2 RuleSets for Stop ruleset_count=2 rulesets=["code-quality", "test-integrity"]` ✅
- 11 per-rule llama-agent sessions spawned, each with its own MCP client to the in-process validator server, each successfully calling `read_file`/`glob_files`/`grep_files` against the test fixture
- 9 of those rules ended cleanly with `[Llama] response: stop_reason=EndTurn`

**But: zero `validator result ... hook_type="Stop"` log lines were emitted for any of those rules.** Compare to PostToolUse, which logs one such line per rule (e.g. `validator result validator="security-rules:no-secrets" passed=true hook_type="PostToolUse" ...`).

## Recording-derived evidence: agent IS producing valid JSON

The 4 existing recordings in `.avp/recordings/` (PostToolUse runs from earlier — Stop-hook recording for the 16:06 run was never persisted, see sibling task) show that qwen's terminal text per rule is clean:

```
<think>
</think>

{
  "status": "passed",
  "message": "No hardcoded secrets, API keys, passwords, ..."
}
```

Every recording, every rule, qwen produces exactly the format `builtin/prompts/.system/rule.md` requires. The agent works. The parser works (`hook_type="PostToolUse"` lines log identical messages, so the runner extracts the JSON and emits the result line on that path).

**Conclusion: the bug is purely in the Stop hook's log-emit step.** The PostToolUse path emits `validator result ... hook_type="PostToolUse"`, the Stop path doesn't emit the equivalent for `hook_type="Stop"`. Most likely option (A) below — the log call is just missing on one branch.

## What this means

- **(A) [LIKELY]** the Stop-hook codepath that aggregates per-rule results isn't calling `tracing::info!` at the same site PostToolUse uses, so verdicts ARE happening but invisibly. Lowest-effort fix: find the PostToolUse log site and add a parallel call from the Stop path.
- **(B)** the runner is not parsing the rule's terminal text into a verdict at all. (Refuted by recordings — terminal text is parseable.)
- **(C)** the runner is parsing the verdict but only stores it in some aggregated structure (for the Stop hook decision/exit code) and never logs the per-rule outcome.

## Where to look

- `avp-common/src/validator/runner.rs` — search for `validator result`. There are PostToolUse-flavored emit sites; see whether Stop has an equivalent. The per-hook-type branching is the suspect.
- `avp-common/src/chain/links/validator_executor.rs::execute_rulesets` — does each rule produce a `RuleVerdict` that's logged before it's folded into the ruleset summary?
- Extend the test in `avp-common/tests/stop_hook_code_quality_regression.rs` to assert that for a fully-mocked-out 1-rule Stop, exactly one `validator result` line is emitted with `hook_type="Stop"`.

## Note on root cause depth

If the missing logs are because the runner timed out / crashed before the aggregation point, that's a different bug (see sibling task `01KQAFFZDX40GSKXQVS0MTNDWV` on Stop-hook total runtime). But missing per-rule logging is independently bad regardless of whether the Stop hook completes:
- **The user can't tell what each rule decided** — even if the overall hook never returns.
- **No debugging signal** — for the rules that DID succeed (9 of 11), we have no visibility into pass/fail without parsing transcripts.

This task is purely "emit the verdict log line for Stop hook rules, exact-mirror of the PostToolUse path". Total runtime / partial completion is filed separately.

## Acceptance

- For a Stop hook that runs N rules, `.avp/log` contains exactly N lines matching `validator result validator="..." passed=(true|false) hook_type="Stop"`.
- Each line is emitted as soon as the rule's verdict is known, not at the end of the entire ruleset (so a hook that times out mid-ruleset still has logs for the rules that completed).
- Format matches the existing PostToolUse format byte-for-byte (same field order, same level=INFO, same `passed` boolean).
- Regression test: simulate a Stop chain over a 2-rule ruleset, capture `tracing` output via `tracing-subscriber` test-writer, assert two `validator result` lines with `hook_type="Stop"`.

## Depends on

- `01KQAFCT6B4EP1ENW5RHFVFZB2` (more MCP server tracing) — not strictly blocking, but the deeper diagnosis of "why didn't rule X log a verdict" is much easier with the per-tool-call tracing in place.

#avp #observability