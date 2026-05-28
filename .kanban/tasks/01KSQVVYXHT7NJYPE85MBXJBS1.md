---
assignees:
- claude-code
position_column: todo
position_ordinal: '9280'
project: llama-coverage
title: Cover agent.rs AgentServer::generate path (tool retry, parallel dispatch, title-via-model, auto-compact)
---
## What

Lift `crates/llama-agent/src/agent.rs` coverage (currently ~50%) by covering the non-ACP `AgentServer::generate` API path, which the ACP-server card (01KSQBGPHT216JC640GNAA5NRA) deliberately left out of scope.

The ACP card covered the `acp/server.rs` prompt loop (rose to 77%) and `session.rs` lifecycle (89%), but the combined >90% acceptance target was missed (measured 76.01%) because the shortfall is concentrated in `agent.rs`'s own generate-path, which is a different API surface.

## Cover (the uncovered blocks identified during review)

- `AgentServer::generate`'s own agentic loop (distinct from the ACP `prompt` loop).
- `execute_tool_with_retry` — retry/backoff on tool failure.
- `execute_tools_parallel` / `process_tool_calls` — parallel tool dispatch.
- `create_summary_generator` / `title_via_model` — the model-success title branch.
- `maybe_auto_compact` — feature-gated compaction.

## Acceptance Criteria

- [ ] `AgentServer::generate` single-turn and tool-loop paths covered end-to-end.
- [ ] Tool retry path exercised (a tool that fails then succeeds).
- [ ] Parallel tool dispatch path exercised (a turn emitting >1 tool call).
- [ ] Title-via-model success branch covered.
- [ ] Combined region coverage of `acp/server.rs` + `agent.rs` + `session.rs` reaches >90%.

## Context

Follow-up filed from the review of card 01KSQBGPHT216JC640GNAA5NRA. See that card's "Coverage justification" section for the exact uncovered blocks and measured percentages. Coordinate with the generation-core / queue-lifecycle sibling cards — these paths overlap their territory.