---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8780
project: local-review
title: 'Root-cause fix: real-claude review HANGS — review Client never answers the agent''s permission/fs requests (ACP client-side deadlock)'
---
## Symptom
A real `sah serve --model claude-code` `review working` across many validators HANGS indefinitely (observed 30+ min idle). A single-validator scoped run COMPLETES fine. The user hit this as a wedged review with an idle `claude` child process.

## Evidence (gathered by hand)
- Scoped to 1 validator → completes in ~54s, returns a confirmed finding.
- Full/multi-validator → freezes. In the frozen run's `.sah/mcp.log`: exactly ONE `new_session`, one prompt reaches `claude_agent::claude: Sending final chunk with stop_reason=end_turn`, then total silence. Earlier runs show `claude_agent::permissions: Tool call 'Read' requires user consent` / `Applying policy '*' to tool 'Read'`.
- `run_prompt` in `crates/swissarmyhammer-validators/src/validators/pool.rs` awaits ACP requests with `.block_task().await` and NO timeout, so any wedge becomes an infinite hang.

## Root cause (high confidence)
The review's ACP **Client** (built in `crates/swissarmyhammer-validators/src/review/drive.rs` via `Client.builder().name("swissarmyhammer-review").connect_with(...)`) wires only notification forwarding — it does **NOT** register an `on_receive_request` handler. But a real claude agent, during a prompt turn, sends requests BACK to the client: `session/request_permission` (tool consent) and `fs/read_text_file`/`fs/write_text_file`. With no client handler, those requests never get a response → claude blocks → the prompt never returns → `run_prompt` hangs → the pool never drains → the whole review hangs. This is the documented agent→client deadlock pattern (a wired client connection is required). The scripted/mock test agents in `pool.rs`/`drive.rs`/`review_fixture.rs` never send these requests, so every test false-passes while production hangs.

Also relevant (already changed, keep): `initialize` was being called per-prompt over the shared connection; it is now done ONCE per connection in `drive.rs::run_pipeline_in_connection` (ACP initialize is a once-per-connection handshake). This is correct but does not by itself fix the hang.

## Fix
1. **Wire `Client.builder().on_receive_request(...)` in `drive.rs`** to answer the agent's requests:
   - `session/request_permission` → auto-approve: respond `RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(PermissionOptionId::new("allow")))`. (Reference: `crates/claude-agent/tests/common/test_client.rs::request_permission`.)
   - `fs/read_text_file` → read the file from disk under `repo_path` (honor optional line/limit) and return `ReadTextFileResponse`. (Reference: `test_client.rs::read_text_file`.)
   - `fs/write_text_file` → a review is read-only; respond success WITHOUT writing (or a clean error) so the agent doesn't hang — do not actually mutate the repo.
   - Any other agent request → method-not-found error (never leave it unanswered).
   - CRITICAL: handle these WITHOUT blocking the dispatch loop (spawn off if needed, per the agent↔client deadlock note — see `wrap_claude_into_handle` / `dispatch_claude_request` "why prompt is spawned off the dispatch loop").
2. **Declare client capabilities** in the `InitializeRequest` (e.g. `ClientCapabilities` with `fs.read_text_file(true)`, write false) so the agent knows the client serves reads. (Reference: `crates/llama-agent/tests/integration/acp_read_file.rs`.)
3. **Defensive timeout** (anti-infinite-hang backstop): wrap each prompt turn in `pool.rs::run_prompt` in a generous `tokio::time::timeout` (e.g. 300s) → on elapse return an `AgentError` so the fleet degrades that task to zero findings and the review COMPLETES instead of hanging forever. Tune generous enough not to false-fire on a legitimately slow turn.

## Tests (must be deterministic — NO real model)
- **Reproduction test (the keystone):** a mock agent (extend the `pool.rs` MockAgent harness) whose `prompt` handler, BEFORE responding, sends a `session/request_permission` (and/or `fs/read_text_file`) request to the client and only returns `end_turn` AFTER receiving the client's response. Drive the real `run_review_over_agent`/pool with this agent under `#[tokio::test]` with a wrapping `tokio::time::timeout` so a HANG FAILS the test. With today's drive.rs (no client handler) this test hangs/fails; after the fix it passes. This is the test that would have caught the bug.
- Keep all existing pool/drive/e2e tests green.
- A multi-task variant (2+ validators × file → 2+ sequential prompts, each requiring a permission round-trip) to prove the pool advances past the first task.

## Verify
- The reproduction test fails before the fix, passes after.
- `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` green; build + clippy clean.
- A real `sah serve --model claude-code` full `review working` over a small planted repo COMPLETES and returns a report (capture evidence; it is slow but must not hang).

## Notes / references
- `crates/claude-agent/tests/common/test_client.rs` — canonical client handlers (request_permission auto-allow, read_text_file, write_text_file).
- `crates/swissarmyhammer-agent/src/lib.rs::wrap_claude_into_handle` — the agent-side builder + the "spawn off the dispatch loop" deadlock note.
- `crates/claude-agent/tests/integration/elicitation_bridge_flow.rs` — `Client.builder().on_receive_request(...)` wiring shape.
- Do NOT trust scripted/mock agents that never send client-bound requests as proof the real path works.