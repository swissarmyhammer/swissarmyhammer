---
assignees:
- claude-code
position_column: todo
position_ordinal: f080
project: drop-llama-agent
title: Clear the stale llama-agent cross references from doc comments
---
## What

The `llama-agent` crate is deleted (card ^3y5n9g6), but about 30 doc
comments in other crates still name it as a live sibling — "`llama-agent`
carries an identical...", "matching llama-agent", "both agents". A reader
cannot follow those references any more.

Find them with:

```
grep -rn "llama.agent\|llama_agent" --include=*.rs crates/ apps/ | grep -v '^crates/swissarmyhammer-config'
```

Rewrite each one to state the rule it is really making, without naming a crate
that does not exist. Where the sentence only exists to say "the other agent
does the same", drop the comparison.

Files with hits:

- `crates/claude-agent/` — `acp_error.rs`, `agent.rs`, `agent_prompt_handling.rs`,
  `agent_trait_impl.rs`, `agent_validation.rs`, `content_capability_validator.rs`,
  `lib.rs`, `session_fork.rs`
- `crates/agent-client-protocol-extras/` — `hookable_agent.rs`, `lib.rs`,
  `raw_messages.rs`, `session_fork.rs`, `session_title.rs`, `test_support.rs`,
  `turn_complete.rs`, `tests/e2e_hooks/helpers.rs`
- `crates/swissarmyhammer-common/src/prompt_visibility.rs`
- `crates/swissarmyhammer-diagnostics/src/watcher.rs`
- `crates/swissarmyhammer-skills/src/lib.rs`
- `crates/swissarmyhammer-validators/src/validators/pool.rs`
- `crates/swissarmyhammer-embedding/src/embedder.rs`

DO NOT touch `crates/llama-common/`, `crates/llama-embedding/`, or
`crates/model-loader/`. Those also name llama-agent in prose, but the
embedding stack is out of scope for this project and card ^3y5n9g6 was
required to leave them byte-for-byte unchanged.

Do not touch `crates/swissarmyhammer-config/` — card ^hm82t0z owns the
`ModelConfig::llama_agent` executor type.

### Subtasks

- [ ] Rewrite the claude-agent doc comments.
- [ ] Rewrite the agent-client-protocol-extras doc comments.
- [ ] Rewrite the remaining single-file hits.

## Acceptance Criteria

- [ ] No doc comment outside `crates/llama-common/`,
      `crates/llama-embedding/`, `crates/model-loader/`, and
      `crates/swissarmyhammer-config/` names `llama-agent`.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

## Tests

- [ ] Run `cargo nextest run --workspace` — comment-only change, the suite
      stays green.

## Workflow

- Documentation only, no behavior change. #llama-agent #cleanup #docs