---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvbrn4rz4k8g97y4c3qrzmhw
  text: Picked up by /finish (card 3/5). Independent precondition — guarantees the rendered prefix is byte-stable so the prime donor's LCP stays full. Starting /implement.
  timestamp: 2026-06-17T21:44:03.615979+00:00
- actor: wballard
  id: 01kvbsm9av1dyj2hmct35strnv
  text: |-
    Research + fix complete.

    BACKING COLLECTION: The intrinsic agent-tools mount's tool/list is built by `ToolRegistry::list_tools_filtered` in crates/swissarmyhammer-tools/src/mcp/tool_registry.rs. The registry is backed by `HashMap<String, Box<dyn McpTool>>` (field `tools`), and the emission point iterated `self.tools.values()` with NO sort -> NON-DETERMINISTIC under hash randomization. This is the production path: the MCP server's `list_tools` handler (server.rs) calls `list_tools()` / `list_tools_for_host()`, both of which route through `list_tools_filtered`.

    FIX NEEDED: Yes. Sorted at the emission boundary only. `list_tools_filtered` now collects kept tools into a Vec and calls `sort_unstable_by_key(|tool| McpTool::name(*tool))` before building the `Tool` objects. Both `list_tools()` and `list_tools_for_host()` inherit the sort. Routing/dispatch is UNCHANGED — `get_tool`/`get_tool_by_cli_name`/`call_tool` still use the HashMap by name; tool identity untouched.

    TEST: Added `test_list_tools_deterministic_sorted_order` (tool_registry.rs). Registers 11 tools in non-sorted insertion order, asserts `list_tools()` and `list_tools_for_host(Host::Claude)` return names in EXACT sorted order (ordered equality, not set), and that two successive calls are identical.

    RED->GREEN:
    - RED (before fix): FAILED — left: ["git","files","question","skill","agent","code_context","kanban","review","web","shell","ralph"] vs sorted right. Proves HashMap order is non-deterministic.
    - GREEN (after fix): test ok.

    VERIFICATION:
    - `cargo fmt -p swissarmyhammer-tools` clean.
    - `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` clean (exit 0).
    - `cargo test -p swissarmyhammer-tools` exit 0 (full run passed). Note: one flaky pre-existing isolation failure (`test_client_call_tool`, a YAML-frontmatter parse race at server startup, unrelated to tool ordering) appeared in an earlier full run but passed both in isolation and on a clean full re-run; confirmed independent of this change by stashing the diff (test passed without the change too).
    - Adversarial double-check: PASS.

    Moving to review.
  timestamp: 2026-06-17T22:01:04.091763+00:00
- actor: claude-code
  id: 01kvbt1yvymtk5kvs35t47j118
  text: 'Review complete (engine: 0 failed, 0 blockers, 1 warning). The lone warning — make ToolContext''s 13 public fields private with getters — is pre-existing API design this card did not touch (diff is only the list_tools_filtered sort + a regression test) and a large API change out of scope here. No in-scope findings → done. Real win: this card fixed an actual non-determinism (HashMap.values() tool ordering) that would have shrunk the prime donor''s LCP below the full prefix. Verified green + double-check PASS.'
  timestamp: 2026-06-17T22:08:32.126763+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbf80
project: kv-prefix-reuse
title: Guarantee deterministic tools/list ordering for the agent-tools MCP mount
---
## What
The shared-prefix donor only stays valid if the rendered prefix (system prompt + tool definitions) is byte-identical across sibling review sessions. The system prompt is already a static string, and llama-agent passes tools as order-preserving `Vec`s — but the intrinsic agent-tools MCP mount in `swissarmyhammer-tools` is the one place a tool list could be built from a `HashMap`, making `tools/list` order vary run-to-run and silently shrinking the LCP below the full prefix (the divergence diagnostic at `crates/llama-agent/src/queue.rs:2582` exists to catch exactly this).

Audit and guarantee deterministic tool ordering:
- Find where the agent-tools MCP server builds its `tools/list` response in `swissarmyhammer-tools` (search the tool router / `list_tools` / tool registry; e.g. `swissarmyhammer-tools/src/mcp/`). Confirm whether iteration order is stable (Vec / BTreeMap / IndexMap) or `HashMap`-based.
- If non-deterministic, sort by a stable key (tool name) or switch the backing collection to a deterministic one at the emission point. Do not change tool identity or routing (the routing `tool_index` HashMap at `agent.rs:119` is fine — only the rendered LIST order matters).

## Acceptance Criteria
- [ ] `tools/list` from the agent-tools mount returns tools in a deterministic, stable order across repeated calls and process restarts.
- [ ] Tool routing/dispatch behavior is unchanged.

## Tests
- [ ] Add a unit/integration test in `swissarmyhammer-tools` asserting two successive `list_tools` calls return identical ordered name sequences, and that the order matches an expected stable ordering (e.g. sorted by name).
- [ ] If a HashMap was the source, add a test that would fail under hash randomization (assert exact order, not set-equality).
- [ ] `cargo test -p swissarmyhammer-tools` green.

## Workflow
- Use `/tdd` — write the ordering assertion first; if it's already deterministic the test is a regression guard (note that in the task) and no code change is needed beyond the test.