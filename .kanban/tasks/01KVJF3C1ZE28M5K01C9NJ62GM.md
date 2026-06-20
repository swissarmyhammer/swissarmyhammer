---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvjfy5703bttrg5df3ft6tnf
  text: '/finish picked up (single-task mode). Starting implement → test → review loop. Plan per card: (1) reclassify FilesTool category Agent→Replacement so Host::serves advertises `files` to Claude; (2) drop the PreToolUse hook from the edit_redirect fragment in mirdan, keep permissions.deny only. TDD: failing host-surface test + deny-only fragment test first.'
  timestamp: 2026-06-20T12:26:22.816262+00:00
- actor: claude-code
  id: 01kvjgdnqyymyw1w99sfs16ds2
  text: |-
    Implemented via TDD (RED watched fail, then GREEN). Changes (3 files):
    1. files/mod.rs — FilesTool::category() flipped Agent -> Replacement{native:"Edit"}; both in-file category assertions updated to expect Replacement.
    2. tool_registry.rs — new regression test files_replacement_served_to_claude_not_llama: register_file_tools then assert list_tools_for_host(Claude) contains "files", Llama and Other do not.
    3. mirdan/src/install.rs — dropped the PreToolUse redirect hook from desired_edit_redirect_fragment (now deny-only); deleted unused builders EDIT_REDIRECT_MATCHER, edit_redirect_command, edit_redirect_hook_group, HOOKS_PRETOOLUSE_POINTER, POINTER_KEY_HOOKS, POINTER_KEY_PRETOOLUSE; simplified apply_edit_redirect_at; updated module/Profile docs; flipped edit_redirect_tests + the init_profile test to assert deny present, hooks ABSENT.

    Kept permissions.deny [Edit,Write,MultiEdit] (closes the surface). Note: because files is now Replacement{native:"Edit"}, serve-time apply_serve_time_native_deny will also deny "Edit" for Claude — consistent with the deny, no conflict. Minimal 2-part fix, no multi-native extension.

    Verification (all green):
    - cargo test -p mirdan edit_redirect: 5 passed
    - cargo test -p swissarmyhammer-tools --lib -- files host serves replacement: 203 passed
    - cargo test -p mirdan --lib: 403 passed
    - cargo test -p swissarmyhammer-tools --lib: 1108 passed (incl. test_validator_endpoint_lists_only_validator_tools — validator set unchanged)
    - cargo clippy -p mirdan -p swissarmyhammer-tools --lib: clean, no warnings
  timestamp: 2026-06-20T12:34:51.262479+00:00
- actor: claude-code
  id: 01kvjgqs84mtb876rb8fr9ddpx
  text: |-
    double-check returned REVISE: only findings were two stale doc comments on fields this change touched (no correctness bug, no dead code, validator/agent-tools surface confirmed unchanged). Fixed both:
    - apps/swissarmyhammer-cli/src/commands/profile.rs sah_profile() doc: removed the "add a PreToolUse redirect" wording -> now deny-only.
    - crates/swissarmyhammer-tools/src/mcp/tool_registry.rs replacement_natives() doc: dropped the stale 'exactly ["Bash"]' claim; now notes files supersedes "Edit" too and the set grows from registered Replacement tools.
    Re-verified: mirdan edit_redirect 5/5, swissarmyhammer-tools --lib files host serves replacement 203/203, swissarmyhammer-cli builds clean.
  timestamp: 2026-06-20T12:40:22.532320+00:00
- actor: claude-code
  id: 01kvjgs59ggpmtrpysfm70wh5k
  text: 'Implement landed in `review` (4 files: files/mod.rs category Agent→Replacement{native:"Edit"}; tool_registry.rs new host-surface regression test; mirdan install.rs deny-only fragment + removed hook builders + flipped tests; profile.rs doc). Implementer reported green: `swissarmyhammer-tools --lib` 1108/1108, `mirdan --lib` 403/403, clippy clean. Running /test then /review gates next.'
  timestamp: 2026-06-20T12:41:07.632473+00:00
- actor: claude-code
  id: 01kvjhevkm653ysx27srzxkc5f
  text: |-
    Review gate ran (review working). Verdict: 1 blocker + 3 warnings + 1 nit — but ALL out-of-scope for this task. The blocker, 2 warnings and nit are in code_context/watcher.rs (a different concurrent task's file, dirty in the shared tree); the 2 files/mod.rs warnings target the pre-existing operation-inference dispatch chains (lines 64/127), NOT the one-line FilesTool::category() flip (line ~139) this task made. The task's own diff drew ZERO findings.

    DISPOSITION: not looping /implement on tangential/pre-existing findings (would force forbidden bonus-refactoring of another task's file + the known review-churn loop). Acceptance criteria are machine-verified green by the test gate: category()==Replacement, list_tools_for_host(Claude) includes "files" & Llama/Other don't, fragment is deny-only with no PreToolUse, validator/agent-tools surface unchanged (test_validator_endpoint passed). Moving to done; committing only this task's 4 files (the 2 watcher.rs files belong to the concurrent diagnostics task and are excluded).
  timestamp: 2026-06-20T12:52:58.612343+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffcd80
project: agent-builtins
title: Serve the `files` write/edit replacement to Claude Code and drop the edit_redirect PreToolUse hook (deny alone closes the surface)
---
## What

`sah init` closes Claude Code's native write surface but never serves a replacement, so the model writes files via `shell` heredocs. It also installs a `PreToolUse` redirect hook that is harmful: it fires even where the `files` replacement isn't mounted (the host's own write attempts, nested contexts), producing a dead-end redirect to a tool that isn't there.

**Root cause (verified in source):**

1. `sah_profile()` sets `edit_redirect: true` (`apps/swissarmyhammer-cli/src/commands/profile.rs:45`). On `sah init`, mirdan writes `permissions.deny: ["Edit","Write","MultiEdit"]` **plus** a `PreToolUse` hook into Claude's `.claude/settings.json` that denies the native mutators and prints "use the `files` MCP tool (op \"edit file\" / \"write file\")" — see `edit_redirect_command` / `edit_redirect_hook_group` / `desired_edit_redirect_fragment` at `crates/mirdan/src/install.rs:1569-1618`.
2. But the unified `files` MCP tool is `ToolCategory::Agent` (`crates/swissarmyhammer-tools/src/mcp/tools/files/mod.rs:139-144`). The primary `sah serve` instance (`compose_per_client = true`) composes its advertised tool list per connecting host via `Host::serves` (`crates/swissarmyhammer-tools/src/mcp/host.rs:80-86`), which returns `false` for `Agent`-category tools for **every** host — including Claude. So `files` is stripped from the list Claude Code sees (`list_tools_for_host`, `crates/swissarmyhammer-tools/src/mcp/server.rs:2180-2197`).
3. Net: Claude's native `Edit`/`Write`/`MultiEdit` are denied AND the `files` replacement it is told to use is never advertised → with no write tool, the model falls back to `shell` heredocs (the `shell` tool is `Replacement{native:\"Bash\"}`, which **is** served to Claude).

**Precedent — the `shell` tool already does this correctly with NO hook:** `shell` is `ToolCategory::Replacement { native: \"Bash\" }` (`crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:536`) → served to Claude (`Host::serves(Replacement) == true` for Claude, `host.rs:84`) AND its native `Bash` is denied at serve time via plain `permissions.deny` (`apply_serve_time_native_deny` → `replacement_natives()` → `mirdan::install::deny_tool`, `server.rs:1061-1107`). No `PreToolUse` hook is involved — the deny alone closes the native surface. The `files` write surface should follow the same pattern.

**Fix (two parts, one concern — make the closed write surface usable via a served replacement, with deny-only blocking):**

1. **Serve the replacement to Claude.** Reclassify the unified `files` tool from `ToolCategory::Agent` to `ToolCategory::Replacement { native: \"Edit\" }` in `files/mod.rs:139-144`, so `Host::serves` advertises it to Claude (mirroring `shell`). Llama/validator paths are unaffected — `create_agent_tools_server` / `create_validator_server` register file tools explicitly and serve verbatim (`compose_per_client = false`), independent of category.
2. **Drop the redirect hook; keep the deny.** Remove the `PreToolUse` hook from the `edit_redirect` fragment (`desired_edit_redirect_fragment`, `install.rs:1609-1618`) and the now-unused hook builders (`edit_redirect_command` 1569, `edit_redirect_hook_group` 1591, `EDIT_REDIRECT_MATCHER` 1560, `HOOKS_PRETOOLUSE_POINTER`). Keep `permissions.deny: [\"Edit\",\"Write\",\"MultiEdit\"]` (the `EDIT_REDIRECT_DENY_TOOLS` constant + the deny apply path). Blocking the native tools via `permissions.deny` is sufficient to close the surface; the hook is redundant and breaks wherever the replacement isn't mounted.

**Optional consolidation (only if it stays ≤5 files; otherwise a follow-up):** aligning with the `agent-builtins` direction "move the deny from `sah init` to serve-time", extend `ToolCategory::Replacement` to carry multiple natives (`native: &'static str` → slice; `tool_registry.rs:1090-1093`, `replacement_natives` `1315-1325`, `shell/mod.rs:536` `\"Bash\"` → `[\"Bash\"]`) so `files` declares `[\"Edit\",\"Write\",\"MultiEdit\"]` and `apply_serve_time_native_deny` issues the full deny, letting `edit_redirect` be retired entirely. Prefer the minimal two-part fix above to honor sizing.

- [ ] Flip `FilesTool::category()` (`files/mod.rs:139-144`) `Agent` → `Replacement`.
- [ ] Update the two in-file assertions that expect `Agent` (`files/mod.rs:576,583`).
- [ ] Remove the `PreToolUse` hook from `desired_edit_redirect_fragment` + delete the unused hook builders; keep `permissions.deny`.
- [ ] Add a host-surface regression test (Claude sees `files`, Llama via primary serve does not).

## Acceptance Criteria
- [ ] `McpTool::category(&FilesTool::new())` returns `ToolCategory::Replacement { .. }` (not `Agent`).
- [ ] Building the full registry via `register_file_tools` and calling `registry.list_tools_for_host(Host::Claude)` yields a tool named `\"files\"`.
- [ ] `registry.list_tools_for_host(Host::Llama)` and `Host::Other` do NOT include `\"files\"` via the primary serve (llama still gets it through its in-process agent-tools mount — unchanged).
- [ ] `desired_edit_redirect_fragment()` contains `permissions.deny` with `Edit`/`Write`/`MultiEdit` and **no** `hooks.PreToolUse` entry; the installed Claude settings have the deny but no edit-redirect hook.
- [ ] Edit/Write/MultiEdit remain denied for Claude (write surface stays closed) with `files` now advertised as the replacement.
- [ ] The validator server (`create_validator_server`) and agent-tools server (`create_agent_tools_server`) advertised tool sets are unchanged.

## Tests
- [ ] Update the category assertions at `crates/swissarmyhammer-tools/src/mcp/tools/files/mod.rs:576,583` to expect `ToolCategory::Replacement { .. }`.
- [ ] Add a test in `crates/swissarmyhammer-tools/src/mcp/host.rs` (next to the `serves` tests at `host.rs:133-151`) or `tool_registry.rs`: build a registry with `register_file_tools`, assert `list_tools_for_host(Host::Claude)` contains `\"files\"` and `list_tools_for_host(Host::Llama)` does not.
- [ ] Update the mirdan `edit_redirect_tests` (`crates/mirdan/src/install.rs:6337+`) so the fragment/idempotency tests assert `permissions.deny` is present and `hooks.PreToolUse` is **absent** (these currently assert the hook is installed — they must flip).
- [ ] Run `cargo test -p swissarmyhammer-tools files host serves replacement` and `cargo test -p mirdan edit_redirect` — all green; existing `Host::serves` tests (`host.rs:133-151`) and shell `Replacement{native:\"Bash\"}` tests (`tool_registry.rs:3424+`) must stay green.

## Workflow
- Use `/tdd` — write the failing host-surface test (`files` visible to `Host::Claude`) and the updated mirdan deny-only fragment test first, watch them fail, then make the category + fragment changes to pass.

## Review Findings (2026-06-20 07:43)

### Blockers
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs:166` — Test helper functions `get_ts_indexed` and `get_lsp_indexed` are near-verbatim duplicates that differ only in the column name queried ('ts_indexed' vs 'lsp_indexed'). Two functions with identical structure and logic are one function with an argument — this duplication inflates surface area for maintenance and invites drift. Extract a single parameterized helper: `fn get_indexed_flag(conn: &Connection, path: &str, column: &str) -> Option<i64>` that accepts the column name as a parameter. Replace both functions with calls to this helper, or inline the helper calls at callsites to eliminate the duplication entirely.

### Warnings
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs:91` — Deep nesting exceeds 4 levels (while → match → Ok → for → for → if), making control flow hard to follow and test. Extract the nested loops and conditionals into a helper function, e.g. `fn process_debounced_events(debounced_events: &[...]) -> Vec<FileEvent>` to reduce nesting to 2–3 levels in the main loop.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/files/mod.rs:64` — Operation inference logic is duplicated in read-only vs. all-operations code paths. Both branches (lines 64–81 and 83–101) contain nearly identical if-else chains that check for argument keys and dispatch to handlers. The chains differ only in which operations are checked, not in the logic structure. This violates the data-driven principle: parallel code paths that differ only in data (which operations are available) should be unified with a single inference function operating over a filtered table of operation metadata. Extract operation signatures as data: define a `struct OperationMeta { op_name: &str, key_predicates: fn(&JsonMap) -> bool, handler: fn(...) -> ... }` slice listing all operations with their key signatures. Create a single `infer_operation(args: &Map, available_ops: &[OperationMeta])` function. Filter the operation table by `FileOperationSubset` before inference. This unifies the logic and ensures consistency between modes.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/files/mod.rs:127` — Long conditional chain with 8+ branches in the operation inference logic (lines 129–154) makes it hard to reason about all execution paths. Separate operation inference from read-only validation: extract a pure function `fn infer_operation(args: &Map) -> Option<&'static str>` that returns the operation name based on present keys, then call validation outside. This reduces the function from one 8-branch conditional to a simpler dispatch.

### Nits
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs:105` — Magic number '1' hardcodes debounce timeout as a configuration value. Debounce intervals (timeouts) should be named constants for clarity, maintainability, and to enable tuning without source changes. Define a named constant like `const WATCHER_DEBOUNCE_SECS: u64 = 1;` at module scope and use `Duration::from_secs(WATCHER_DEBOUNCE_SECS)` instead.

### Reviewer note (scope)
The `review working` engine swept the entire uncommitted working tree (the full `diagnostic` branch vs `main`), not only this task's 4 files. None of the findings above touch the change this task made: the blocker, both `watcher.rs` warnings, and the nit are in `code_context/watcher.rs` (not a task file); the two `files/mod.rs` warnings are about the pre-existing **operation-inference dispatch chains**, not the `FilesTool::category()` flip this task changed. The task-scoped change (category `Agent` → `Replacement { native: "Edit" }`, the `files_replacement_served_to_claude_not_llama` regression test, the deny-only `install.rs` edit_redirect change, and the profile.rs doc update) drew **zero** findings. Triage these as tangential/pre-existing.