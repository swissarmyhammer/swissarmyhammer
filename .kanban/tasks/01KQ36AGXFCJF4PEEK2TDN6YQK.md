---
assignees:
- claude-code
depends_on:
- 01KQ7KP7HVASAD4V45AJ41P39W
position_column: todo
position_ordinal: fa80
project: acp-upgrade
title: Adapt acp-conformance to ACP 0.11 (incl. mock Agent impls and fixtures)
---
## What

**REWRITE** `acp-conformance/` against the new ACP 0.11.1 SDK design. Each scenario file contains a mock `Agent` written against the old trait surface — those mocks all need to be re-implemented in the new builder/handler pattern (see spike findings on task 01KQ367HE0Z8ZSXY90CTT8QYGG and migration guide https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html).

## Spike-confirmed scope

### Mock Agent rewrites
Every scenario file under `acp-conformance/src/` defines a mock `Agent` to drive the conformance harness. Each one rewrites:
- `acp-conformance/src/initialization.rs`
- `acp-conformance/src/sessions.rs`
- `acp-conformance/src/prompt_turn.rs`
- `acp-conformance/src/content.rs`
- `acp-conformance/src/tool_calls.rs`
- `acp-conformance/src/terminals.rs`
- `acp-conformance/src/slash_commands.rs`
- `acp-conformance/src/agent_plan.rs`
- `acp-conformance/src/file_system.rs`

### Helper module migration
- `acp-conformance/src/responses.rs`, `validation.rs` — schema-type imports move from `agent_client_protocol::X` → `agent_client_protocol::schema::X`.

### Tests
- `acp-conformance/tests/integration/*.rs` — drive `AgentWithFixture` (depends on extras crate's new wrapper API).
- `acp-conformance/tests/integration/serialization.rs` — explicit serde round-trips on `Plan`, `PlanEntry`, etc. The schema crate jumped between minor versions; spot-check that the wire format hasn't shifted (likely fine — `#[non_exhaustive]` enables additive changes without wire breakage).
- `acp-conformance/tests/common/mod.rs` — shared fixture loading.

### Fixture validation
- `.fixtures/llama/*.json` and `.fixtures/claude/*.json` — recorded ACP sessions. Per the spike, the schema-level wire format appears unchanged, but a full deserialize replay is required to confirm. **Treat fixture changes carefully** — they're the canonical wire-format snapshot. If a fixture round-trip fails, investigate whether it's a real protocol change (regenerate by re-running the recording flow) or a regression in the agent.

## Acceptance Criteria
- [ ] `cargo check -p acp-conformance --all-targets` passes.
- [ ] `cargo clippy -p acp-conformance --all-targets -- -D warnings` passes.
- [ ] All conformance tests pass for both `llama` and `claude` fixture sets.
- [ ] Any regenerated fixtures are committed; the diff is reviewed and the new wire format is documented in the task comments.

## Tests
- [ ] `cargo nextest run -p acp-conformance` — green for both fixture sets.

## Workflow
- Adaptation + fixture refresh.
- Treat fixture changes carefully — they're the canonical wire-format snapshot. A binary "regenerate everything" pass is wrong; understand each diff.

## Depends on
- 01KQ7KP7HVASAD4V45AJ41P39W (the atomic SDK-rewrite task — extras + claude-agent + llama-agent must all compile and test green before this task is workable).
- Spike findings: 01KQ367HE0Z8ZSXY90CTT8QYGG.