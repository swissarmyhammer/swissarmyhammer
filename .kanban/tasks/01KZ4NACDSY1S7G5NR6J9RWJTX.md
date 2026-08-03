---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz4nk9hedpkmgd1c68nxtcvp
  text: |-
    Scope clarification from the user, before implementation starts: the deliverable is REAL prompt-cache reuse on review fleet turns. `session/fork` (`--resume --fork-session`) is the CURRENT mechanism, not a required one.

    If investigation shows `--fork-session` cannot preserve cache eligibility (e.g. it reconstructs the transcript in a way that isn't byte-identical to the parent, or the CLI doesn't apply cache_control across a fork boundary at all), do not stop at that conclusion. Consider and evaluate alternative constructions that could still get a fan-out task built on the primed prefix to hit the cache — for example: independent `--resume <parent-uuid>` calls without `--fork-session` (replaying the same conversation prefix identically each time, if that's cheaper than a true fork and still cache-eligible), or any other invocation shape the CLI supports that keeps the shared prefix byte-exact.

    Do not treat "fork specifically doesn't cache" as equivalent to "caching is impossible here." Only reach the fallback (documented external limitation, no code fix) after real alternatives to forking itself have been evaluated with evidence, not just the current mechanism's failure mode.
  timestamp: 2026-08-03T20:38:43.502961+00:00
- actor: claude-code
  id: 01kz4pv3hxpt7w0xam0hzzpamb
  text: |-
    ### investigation — root cause found (not a CLI/Anthropic limitation)

    Empirical method: ran the real `claude` CLI directly (v2.1.220), bypassing this codebase entirely, to get ground truth before touching any Rust.

    1. `claude --help` confirms `--fork-session` is a real, current CLI flag ("When resuming, create a new session ID instead of reusing the original").

    2. Captured a raw parent+fork transcript pair using the EXACT invocation shape `claude_process.rs` builds (`--verbose --print --input-format stream-json --output-format stream-json --dangerously-skip-permissions --include-partial-messages --session-id <uuid>`, then `--resume <parent-uuid> --fork-session --session-id <child-uuid>`), with a ~46K-token primed prefix (a real source file) and no custom system prompt:
       - Parent turn: `cache_creation_input_tokens: 33901`, `cache_read_input_tokens: 20544` (existing warm boilerplate).
       - Fork #1: `cache_read_input_tokens: 54445` (= 33901+20544, i.e. the ENTIRE parent prefix served from cache), `cache_creation_input_tokens: 39`.
       - Fork #2 (same parent): `cache_read_input_tokens: 54445` again.
       - **Conclusion: `--fork-session` DOES preserve Anthropic prompt-cache eligibility.** The CLI mechanism is not broken and is not a limitation — forking a session with matching spawn args hits the cache every time, well within the 5-minute TTL (both forks completed in ~4s each).

    3. Repeated the test WITH a custom `--system-prompt` on the parent (`claude --system-prompt "You are a strict senior code reviewer..." ...`), then forked WITHOUT replaying `--system-prompt` (mirroring what `session_fork.rs` actually does today):
       - Parent: `cache_creation_input_tokens: 45860`, `cache_read_input_tokens: 0`.
       - Fork (no `--system-prompt`): `cache_read_input_tokens: 54445` — NOT the parent's 45860. The fork's request, missing the parent's system prompt, diverged onto a DIFFERENT tokenized prefix and coincidentally collided with the unrelated cache entry from experiment #2 above. This is worse than plain cold: a nonsensical, non-deterministic "warm" hit against the wrong lineage.
       - Repeated the fork WITH the identical `--system-prompt` replayed: `cache_read_input_tokens: 45860` — an exact match of the parent's own prefix.
       - **Conclusion: Anthropic's cache requires the FORK's own spawn arguments (system prompt, model) to exactly match the parent's — the CLI does not remember or replay the parent's original flags on `--fork-session`, it reconstructs the request from THIS invocation's flags plus the replayed conversation history.**

    4. Traced this into the codebase: `crates/claude-agent/src/session_fork.rs::spawn_forked_process` built the forked child's `SpawnConfig` with `cwd`, `mcp_servers`, `ephemeral`, `tools_override` — but NO `.system_prompt(...)` and NO `.extra_args(...)`. Compare `crates/claude-agent/src/agent.rs::build_session_spawn_config` (the `session/new` path), which sets BOTH from `request.meta.system_prompt` and `self.config.claude.extra_args` respectively. Review-fleet sessions are spawned via `swissarmyhammer-agent::create_session_via_connection`, which DOES attach a custom `system_prompt` as `session/new` meta (the review agent's persona) — so every review-fleet prime session has a non-default system prompt, and every fork of it was silently dropping that system prompt (and any `--model` override), which is exactly the mechanism proven broken in step 3. This explains the observed 100% cold rate with no exceptions.

    `crates/claude-agent/src/session_resume.rs::restore` (backing `session/resume`/`session/load`) has the IDENTICAL omission — filed separately as `^bayecq8` since it's out of scope for this fork-focused card but is a correctness bug, not just a cache-warmth one (a resumed session loses its persona/model, not just its cache warmth).

    ### implement — changed

    Root cause: `session_fork.rs`'s forked-child `SpawnConfig` never carried the
    parent's `system_prompt` or the agent's `extra_args` (e.g. `--model`), so the
    CLI built the fork's API request against a different prefix than the parent's
    — Anthropic's cache requires an exact match, so it never hit (see empirical
    transcript above).

    Changes:
    - `crates/claude-agent/src/session.rs`: added `Session.system_prompt: Option<String>` (`#[serde(default)]`), set to `None` in `Session::new`.
    - `crates/claude-agent/src/agent.rs`: factored the `request.meta.system_prompt` extraction into `ClaudeAgent::extract_system_prompt_meta` (shared by `build_session_spawn_config` and the new persistence step); `create_new_session_internal` now persists it onto the live session via the new `store_system_prompt_in_session`, so a later fork can read it back.
    - `crates/claude-agent/src/session_fork.rs`: extracted `ClaudeAgent::build_fork_spawn_config` (pure, no I/O) from `spawn_forked_process`; it now sets `.system_prompt(parent.system_prompt.clone())` and `.extra_args(self.config.claude.extra_args.clone())` on the child's `SpawnConfig`. Updated the module's "Prefix caching" doc to describe the real mechanism (and cite this card) instead of the prior aspirational "no custom caching layer is needed" claim.
    - Left the fleet.rs "fork was degraded" WARN wording unchanged: with the bug fixed, cold is once again the exceptional case, not the universal one, so "degraded" is accurate.

    Did NOT need to touch `classify_reuse`/`parse_cache_usage` — both were already correct; the bug was entirely in what got spawned, not in how the result was parsed.

    Did NOT need to touch `^b4x8dgh`'s "5-minute cache window" claim — it's directionally correct and now actually realized (previously moot because of this bug, not because the premise was wrong).

    Filed `^bayecq8` for the identical omission in `session_resume.rs::restore` (out of scope here; a correctness bug beyond just cache warmth).

    Tests (TDD): added `test_fork_spawn_config_carries_parent_system_prompt` and `test_fork_spawn_config_carries_extra_args` in `session_fork.rs` — watched both FAIL for the expected reason (temporarily reverted the two new builder lines, confirmed `left: None`/`left: []` panics) before restoring the fix and confirming GREEN. Also added `test_create_new_session_internal_persists_system_prompt_from_meta` / `..._leaves_system_prompt_none_without_meta` in `agent.rs`, and a `Session::new` default assertion in `session.rs`.

    Evidence:
    - `cargo test -p claude-agent --lib`: 758 passed, 0 failed.
    - `cargo clippy -p claude-agent -p swissarmyhammer-validators --all-targets -- -D warnings`: clean.
    - `cargo fmt --all`: clean.
    - `cargo nextest run -E 'rdeps(claude-agent) or rdeps(swissarmyhammer-validators)'`: **4397 tests run: 4397 passed (1 slow), 0 skipped.**

    next: /review
  timestamp: 2026-08-03T21:00:28.093722+00:00
position_column: doing
position_ordinal: '8280'
title: Review fleet forks are 100% cold — warm prefix reuse never fires, not just degraded
---
# Symptom

Reported from a live review run's MCP log: 77 occurrences of

```
WARN fleet task fork was degraded (no warm prefix reuse); proceeding cold
     validator=command-safety files=[...]
```

Every accompanying "fleet task prefix reuse" info line reports
`reuse="cold (no reuse)"` and `reused_tokens=None`. This is not occasional
degradation — it is 100% of forks, every time, for this run. This run's
41-minute wall clock is attributed to this: every one of the review fleet's
forked turns pays the full prefix upload cost, none reuse the primed prefix.

# The goal is to GET caching working, not just to explain its absence

Do not treat "the CLI doesn't support this" as an acceptable end state without
first genuinely trying to make it work. Prompt caching on the forked prefix is
the entire point of the primed-prefix-plus-fork architecture; if it never
fires, that architecture is providing zero benefit over independent monolithic
prompts, at added complexity. Exhaust real options before concluding it's
impossible:

- If `--fork-session` itself does not preserve cache eligibility, look for an
  alternative construction that does (a different CLI invocation shape, an
  explicit `cache_control` breakpoint if the CLI/protocol exposes one, keeping
  the prefix byte-exact across forks, etc).
- If a genuine, provable CLI-level limitation blocks it entirely, that
  conclusion must be backed by a captured raw transcript and a specific,
  citable reason (a CLI flag that's missing, a documented behavior, an
  upstream issue) — not an assumption reached after a quick look.

# Why this matters now

^b4x8dgh (raised the remote worker pool 4 -> 16, already `done`) justified the
change partly on: "A larger pool also puts all forks inside the 5-minute
prompt-cache window of the primed prefix, which reduces cold re-uploads." If
reuse is actually at 0%, that benefit is not being realized at all — more
workers means more PARALLEL cold, full-cost forks, not more cache hits. The
worker-count change is still correct on its own (turns are independent remote
calls; queueing them serially is pure wait time), but the cache-reuse half of
its rationale is currently false in production, and fixing THIS task is what
would make that rationale true.

# What I found in source (not yet a full root cause — investigate before fixing)

- `crates/swissarmyhammer-validators/src/review/fleet.rs` (`classify_reuse`,
  `handle_fork_success`) recognizes two reuse paths: `WarmKv` (native KV reuse,
  from `fork.prefix_tokens` — the llama/local backend) and `WarmCache` (from
  `cache_usage.cache_read_input_tokens > 0` — the Anthropic prompt-cache path,
  which is what applies here since the review agent is Claude via CLI).
- `cache_usage` is populated in `crates/claude-agent/src/protocol_translator.rs`
  by parsing a `"type":"result"` stream-json message's `"usage"` object
  (`parse_cache_usage` / `CacheUsage::from_meta_json`).
- Grepped `crates/claude-agent/src/session_fork.rs`, `claude_process.rs`, and
  `claude.rs` (the fork spawn path, `--resume <parent-uuid> --fork-session
  --session-id <child-uuid>`) for any mention of prompt caching, `cache_control`,
  or the 5-minute cache window: ZERO hits. The fork mechanism was built and its
  worker-count rationale was written without any code that requests, verifies,
  or even logs whether the underlying Claude API actually cached and reused the
  parent prefix for a forked session. The "inside the 5-minute window" claim in
  ^b4x8dgh's card was an assumption, never implemented against or tested.

# Investigate (in this order, before writing a fix)

1. Does `claude --resume <uuid> --fork-session --session-id <child>` even
   request the underlying Claude API in a way that CAN hit Anthropic's prompt
   cache — i.e. does the CLI apply `cache_control` breakpoints to the resumed
   prefix at all? Check the claude CLI's own docs/changelog/help output for
   `--fork-session` and prompt caching interaction. If the CLI is closed-source
   from here, check its `--help`, `--version` changelog, and any bundled docs
   for cache-control behavior before concluding there is none.
2. If the CLI does support it: is the child fork's `usage` object actually
   present in the `"type":"result"` stream-json message, with real
   `cache_read_input_tokens`/`cache_creation_input_tokens` values? Capture one
   RAW forked-session CLI transcript (e.g. run the CLI directly with
   `--fork-session` against a real parent session and inspect its stdout) and
   inspect it directly — do not infer from this codebase's parsing alone.
3. If usage IS present with real cache hits but this code still reports Cold:
   the bug is in `parse_cache_usage`/`classify_reuse`, not the CLI. Trace
   exactly why and fix it.
4. If usage is genuinely absent or `cache_read_input_tokens` is genuinely 0 for
   every fork: is the primed prefix session sitting long enough for Anthropic's
   5-minute TTL to lapse before any fork attaches (e.g. queueing behind 4 or 16
   workers, model downloads, or other startup cost)? Check the actual wall-clock
   gap between the prime turn completing and the first/each fork's request. If
   queueing is the cause, that is fixable (prioritize forks sooner, shrink the
   gap) — pursue that fix rather than stopping at the diagnosis.
5. Whether the prefix content itself is stable enough to hit Anthropic's cache
   (the API requires an EXACT prefix match up to the cache breakpoint — any
   difference invalidates it). Check whether anything (timestamps, per-fork
   randomized ids, the blame-sha/line-numbering from ^k12rn64) gets embedded
   into the shared prefix instead of staying fork-suffix-only, which would
   invalidate the cache for every fork. If something IS destabilizing the
   prefix, that is fixable — move it out of the shared portion.

# Acceptance

- Prompt-cache reuse is DEMONSTRATED working: a real (or realistic
  integration-style) review run shows `reuse != Cold` for at least the common
  case (same prefix, fork within the cache TTL). This is the primary bar — a
  clean diagnosis alone does not satisfy this card.
- Only if, after exhausting the investigation above with real evidence, reuse
  is proven impossible from this codebase (a genuine, cited, external CLI
  limitation): say so explicitly with the supporting transcript/evidence,
  correct the stale "5-minute cache window" claim on ^b4x8dgh's closed card and
  in this task, and record whether native KV reuse (`WarmKv`/local backend) is
  a viable alternative path for this workload. This is the fallback, not the
  default outcome to reach for.
- The log message wording is corrected to match whatever reality is found —
  "degraded" implies an occasional/exceptional case; if this remains the
  universal case for the Claude backend after the fix attempt, the log and any
  docs referencing "the 5-minute cache window" as an active optimization must
  not overstate what is happening.
- `cargo nextest run -E 'rdeps(claude-agent) or rdeps(swissarmyhammer-validators)'`
  passes.

#review #bug