# Validator Replay Fixtures

This directory holds `RecordedSession` JSON files captured by
[`agent_client_protocol_extras::RecordingAgent`]. They drive the integration
tests in [`recording_replay_integration.rs`](../../recording_replay_integration.rs)
by feeding [`PlaybackAgent`] the exact `initialize` / `new_session` / `prompt`
exchanges that a real validator run produced.

Each file represents **one rule's session** — the runner spins up a fresh
session per rule, so a fixture maps cleanly onto a `RuleSet` containing one
rule. To exercise a multi-rule `RuleSet`, concatenate the per-rule recordings
into a single file in execution order.

## Current corpus

| Fixture | Failure mode covered |
|---------|----------------------|
| `rule_clean_pass.json` | Agent returns `{"status":"passed"}` — happy path. |
| `rule_clean_fail.json` | Agent returns `{"status":"failed"}` with a finding. |
| `rule_magic_number_fail.json` | Agent returns `{"status":"failed"}` with a magic-number finding — drives the end-to-end Stop-hook block path covered by `01KQ7M20F27D0Z67H9XX0XQ4QZ`. |
| `rule_unparseable_response.json` | Agent returns a `<think>` block + free-form prose, no JSON. Locks in the parser's fail-closed behaviour for unparseable responses (see `parse_validator_response`). |

A future fixture should cover **tool-call exchanges** (rule asks the agent to
read a file via the `files` MCP server, agent returns content, rule decides).
That recording must be captured against a real model run — synthetic
hand-written tool-call traces drift from the live ACP shape and stop matching
[`PlaybackAgent`]'s deserialisation. See *Regenerating fixtures* below.

> **Tracking note.** The original task spec
> (`01KQ369KBDK6Y5DRN53WB7FDXQ`) listed four required failure modes; this
> corpus currently covers three. Capturing the tool-call exchange depends on
> the upstream tool-wiring task `01KQ35MHFJQPMEKQ08PZKBKFY0`. Once that
> task lands, do one real-model run against a rule that asks the agent to
> read a file, copy the resulting recording from `<repo>/.avp/recordings/`
> into this directory as `rule_tool_call_exchange.json`, and add a fourth
> replay test that asserts the tool round-trip drove the rule to a decision.
> Until then this corpus is intentionally incomplete.

## Regenerating fixtures

These fixtures should be **captured from real runs**, not hand-written.
Recording is unconditional — every `avp` run produces transcripts under
`<repo>/.avp/recordings/`. To capture a fresh corpus:

1. Pick a real validation scenario (e.g. a Stop-hook run on a small change set).
2. Trigger the hook normally (e.g. invoke `avp` with the desired hook input).
3. The recorder writes one JSON file per `AvpContext` lifetime under
   `<repo>/.avp/recordings/<session_id>-<unix_micros>.json`. Each file is a
   valid `RecordedSession` for the entire run — one initialize, then one
   new_session+prompt per rule.
4. Pick a representative subset, copy them up to this directory, rename them
   to descriptive names (`rule_<scenario>.json`), and reference them from the
   integration test.
5. Clean stale captures out of `<repo>/.avp/recordings/` afterwards if you
   don't want them — the directory is gitignored, but recordings accumulate
   on every run.
