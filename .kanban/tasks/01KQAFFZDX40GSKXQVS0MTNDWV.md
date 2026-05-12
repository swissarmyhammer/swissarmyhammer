---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8f80
title: 'Stop-hook total runtime: 11 rules @ ~30–90s each = 10+ minutes; last rule never finishes'
---
## What happened

Today's Stop-hook test (16:06–16:19) successfully matched 2 rulesets with 11 rules total, each spinning up an isolated llama-agent (qwen) session against the in-process validator MCP server. The per-rule sessions completed sequentially (within each ruleset; the two rulesets ran in parallel-ish per the 16:06:20 dual-prompt timestamp):

```
16:06:20  cognitive-complexity         (start, ruleset=code-quality)
16:06:20  no-test-cheating             (start, ruleset=test-integrity)
16:07:49  function-length              (90s after first start)
16:08:52  missing-docs                 (63s)
16:10:01  naming-consistency           (69s)
16:13:25  no-commented-code            (3m 24s — large gap)
16:13:58  no-hard-code                 (33s)
16:14:33  no-log-truncation            (35s)
16:15:35  no-magic-numbers             (62s after start; never completed)
[no further `stop_reason=EndTurn` logged after 16:15:34]
16:17:18  hook event hook_type="Notification" decision=allow
16:19:22  hook event hook_type="SubagentStop" decision=allow
```

The `Stop` hook event itself never fired a `hook event hook_type="Stop"` line. The parent process (claude-code) appears to have moved on to a Notification + SubagentStop instead, suggesting it gave up waiting on the hook.

**Per-rule cost is dominated by qwen overthinking.** Several rule runs produced 7,000–14,800 generated tokens just for one rule (line 8588: `Agentic loop completed: 14879 tokens generated, 3 tool calls executed`). With 11 rules at 30–90s each, total wall time exceeded 10 minutes, easily past any reasonable Stop-hook timeout.

**Recording evidence (refines diagnosis):** The 4 existing recordings in `.avp/recordings/` show qwen-on-PostToolUse producing tight JSON verdicts in `<think></think>` + `{"status": "...", "message": "..."}` form. The `<think>` block in PostToolUse recordings is empty (zero reasoning tokens). Yet the same model on Stop-hook rules generated 7K–14K tokens per rule. The differential isn't model capacity — it's that **Stop-hook rule prompts are getting qwen to ramble where PostToolUse rule prompts don't**. Likely culprits: (a) the Stop-hook prompt is structurally different / longer / more ambiguous, (b) the `# Files Changed This Turn` block invites file enumeration / explanation, (c) per-rule-fresh-session means each rule starts with no shared cache, so each one repeats setup overhead.

## Two distinct problems mixed in here

### Problem 1: Total runtime is unviable

For a Stop hook to be useful, it has to complete in a small fraction of human attention. 10+ minutes is unusable. Options:

- **Run rules in parallel within a ruleset.** The two *rulesets* appear to already run in parallel (cognitive-complexity and no-test-cheating started ~93ms apart). Within a ruleset, rules run sequentially. Convert that to a `tokio::task::JoinSet` over rules-in-ruleset.
- **Per-rule timeout** (e.g. 30s wall, 2048 tokens) so a single runaway rule can't drag the whole hook past the parent's tolerance window.
- **Smaller model / quantization** — qwen-3.6 27B is a lot of model for a per-rule code-quality check. A 0.6B or 1.5B variant per task `01KQ4WEHKG6E3X6ZPPBGJNRA5T` may be enough for the JSON-only output we want.
- **Disable thinking mode for validator runs** — qwen's `<think>...</think>` block is often 1000+ tokens on a 200-token verdict. The chat template can be configured to drop `enable_thinking` for these runs. Recording evidence: PostToolUse verdicts have empty `<think></think>` (model already reasoned silently or thinking is already disabled there). Compare what the runner sets for Stop vs PostToolUse — the Stop-side prompt may be re-enabling thinking unintentionally.

### Problem 2: Last rule (no-magic-numbers) never completed

Even ignoring total runtime, the very last rule started at 16:15:35 and *never* logged `stop_reason=EndTurn`. The activity trail:

```
16:15:35.070  prompt (6980 chars): # Rule: no-magic-numbers
16:15:35.073  Processing prompt for session 01KQADZQZ1PMTHYRJSE4GBDE3M
16:16:07.430  Agent generation turn completed: 1484 tokens in this turn, 1484 total
16:16:07.436  Detected 1 tool calls in generated text
16:16:07.444  AgentMessage: <think>\n\n</think>\n<tool_call>{"name": "read_file", ...}</tool_call>
16:16:07.454  read_file complete duration_ms=9
16:16:07.464  Continuing agentic loop after executing 1 tool calls
[no more entries for this session in .avp/log]
```

The agent's tool call succeeded, then the loop "continued", and then nothing. This could be:
- The model's next generation hit a max-token cap silently.
- The agent loop deadlocked on a channel/mutex.
- The MCP server's session got terminated mid-flight (we see `Session error: Session service terminated` errors at 16:11–16:14, though the last one is before this rule started).
- The parent SubagentStop killed the process before the rule could continue, and no shutdown-trace exists to confirm.

**Recording evidence (refines diagnosis):** the recording for this Stop run was supposed to be at `.avp/recordings/no-session-1777392379807437.json` (per the 16:06:19 log line `Wrapping validator agent with RecordingAgent`) — but **that file does not exist on disk**. The most recent recording is 16:06 from the *PostToolUse* path. So the Stop-hook RecordingAgent was created but its data was never flushed to disk. This is consistent with the hook dying mid-flight before flush. Sibling task `01KQAFT5H1CYQ8YDNAM4J0HD1Q` (filed alongside this) tracks that flush-on-drop bug specifically; it's needed to make the 70+ second silent gap diagnosable for next time.

The MCP server tracing task (`01KQAFCT6B4EP1ENW5RHFVFZB2`) will help here too — once we have session-lifecycle logs, we can disambiguate whether the silence is on the agent side, MCP transport side, or process-death side.

## What this task should produce

A two-pronged fix:

1. **Parallelism within a ruleset** — execute all rules of a ruleset concurrently via a `JoinSet`. The ruleset-level concurrency that today exists between `code-quality` and `test-integrity` should also exist within each. Cap concurrency at `num_cpus / 2` or a config value to avoid memory pressure from many simultaneous llama sessions.

2. **Per-rule wall-clock timeout** — `tokio::time::timeout` per rule. Default 30 seconds. If a rule times out, log it as `validator result validator="X:Y" passed=true hook_type="Stop" reason="timeout"` (passing-with-warning per the existing convention from task `01KQ35V5GTDS4ED3VWG8SAH4DQ`) and move on. The hook must never block more than `timeout × max_in_flight × ceil(N/max_in_flight)` total.

3. (Optional, follow-up) — disable thinking mode on the validator agent. The agent doesn't need to reason aloud to emit a JSON verdict. Recording evidence shows PostToolUse already does this implicitly; replicate for Stop.

## Acceptance

- For the 11-rule fixture used today, a Stop hook completes in under 90 seconds wall time (assuming `max_in_flight=4`).
- Every rule that does NOT timeout logs a `validator result` line.
- Rules that timeout log a `validator result ... reason="timeout"` line and are treated as passes (not blocks).
- Stop hook always emits a final `hook event hook_type="Stop"` line within timeout × ceil(N/max_in_flight) seconds. If the parent process isn't observing that log line today, we have an even bigger lifecycle problem.

## Depends on

- `01KQAFCT6B4EP1ENW5RHFVFZB2` (MCP server tracing) — soft dependency. Helps diagnose problem 2.
- `01KQAFE5WGYJK3HZ8WE3B8N86K` (per-rule verdict log lines) — hard dependency for the timeout case (need to log the timeout outcome).
- `01KQAFT5H1CYQ8YDNAM4J0HD1Q` (RecordingAgent flush-on-drop) — soft dependency. Without it the next Stop-hook deadlock is again undiagnosable.

#avp #performance