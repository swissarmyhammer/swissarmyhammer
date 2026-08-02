---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyz187f5e4atkkz1apxxk3r8
  text: |-
    Picked up. Research done.

    Survey of capitalized error Display messages in the four named crates:

    | crate | sites with a leading capital |
    |---|---|
    | swissarmyhammer-tools | 80 `McpError`/`ErrorData` ctors + 5 `anyhow!`/`bail!` + 1 `#[error(` |
    | agent-client-protocol-extras | 4 `#[error(` + 1 `Unknown tool` MCP payload |
    | mirdan | ~70 `RegistryError` ctors + 10 `Display` arms + `InitResult::error` messages |
    | claude-agent | 135 `#[error(` + 224 `AgentError::*` ctors, in a 49k-LOC crate |

    The `Unknown tool` grep also finds three sibling MCP call handlers the card does not name: `apps/code-context-cli/src/commands/serve.rs`, `apps/shelltool-cli/src/commands/serve.rs`, `crates/llama-agent/tests/integration/read_file_mcp_server.rs`. The acceptance grep matches them literally, so they are in scope.

    Scope call: sweep swissarmyhammer-tools, agent-client-protocol-extras and mirdan to completion, plus the three sibling handlers and the one named claude-agent site (`tools.rs` `Unknown tool`). The remaining ~358 claude-agent typed-error messages are a crate-wide error-API normalization with an ACP-conformance test blast radius, not "the remaining MCP error Display messages" the card title names. Split to its own card.

    Lowercase rule applied: lowercase the first character unless the first word is an all-caps acronym (`I/O`, `JSON`, `ZIP`, `API`, `MCP`, `SHA-512`), a CamelCase identifier (`LoadSession`, `HookEvaluator`), or a proper noun (`Git`). Also strip a trailing full stop.
  timestamp: 2026-08-01T16:06:57.253226+00:00
- actor: claude-code
  id: 01kyz3scpgykqyjdvpn5q25pgn
  text: |-
    Implementation landed.

    **Swept to completion** — swissarmyhammer-tools, mirdan, agent-client-protocol-extras. Forms covered: `#[error("...")]`, manual `write!(f, ...)` Display arms, `McpError`/`ErrorData` constructors, `RegistryError` constructors, `SwissArmyHammerError::Other { message }`, `anyhow!`/`bail!`, `.context(...)`/`.with_context(...)`, `InitResult::error` messages, and String-typed errors built by `map_err(|e| format!(...))` / `Err(format!(...))` / `Err("...")`.

    **Named sites, all done**: `server.rs` (both `unknown tool` payloads), `test_mcp_server.rs`, `claude-agent/src/tools.rs`, `mirdan::install::detected_agents_or_error`.

    **Sibling handlers the card did not name, found by the acceptance grep**: `apps/code-context-cli/src/commands/serve.rs`, `apps/shelltool-cli/src/commands/serve.rs`, `crates/llama-agent/tests/integration/read_file_mcp_server.rs`.

    **Also finished** `apps/swissarmyhammer-cli`, because two of its integration tests pinned the old `Unknown tool` casing and had to change: `commands/tools/mod.rs` `unknown tool(s): ...`, and three `tool not found: {}` sites in `main.rs` and `cli_executor.rs`.

    **Deliberately left capitalized**
    - UI titles in `claude-agent/src/tool_classification.rs`, and the two `assert_eq!(title, "Unknown tool")` tests.
    - Acronym starts: `I/O`, `JSON`, `ZIP`, `API`, `MCP`, `DNS`, `FSEvents`, `TOOL.md`.
    - Proper nouns: `Git operations not available`, `Git clone failed for ...`, `Claude Code agent not found ...`.
    - `InitResult::ok` success messages, `tracing::*` log lines, `.expect()`/`panic!` text.
    - `Check { message }` report rows in `mirdan/src/doctor.rs` — human-readable doctor report text, sibling to `Found at ...` and `Token present ...`, not an error type's Display.
    - Case-insensitive-matching test fixtures in `file_watcher.rs` (`File Is Locked By Another Process`, `RESOURCE TEMPORARILY UNAVAILABLE`).

    **Also fixed** `McpValidation::capitalize_first_letter` in `swissarmyhammer-tools/src/mcp/shared_utils.rs`. That helper machine-capitalized every validation error's Display text, so lowercasing the literals alone would not have held. The helper is deleted.

    **Split out** — `^f70h1pr` (claude-agent, 135 `#[error(` + 224 `AgentError::*` sites, ACP-conformance blast radius) and `^p3qx2hc` (the shared library crates below these, which now disagree with mirdan on `Invalid JSON in ...` and `Invalid glob pattern`).

    **Not run**: the workspace-wide clippy the card's acceptance names. Clippy ran per package instead, on every package touched.

    ### implement — changed
    - evidence: 65 files. `cargo nextest run -p swissarmyhammer-tools -p agent-client-protocol-extras -p claude-agent -p mirdan -p code-context-cli -p shelltool-cli -p swissarmyhammer-cli --no-fail-fast` → 4194 passed, 0 failed, 0 skipped. `cargo clippy --all-targets -- -D warnings` clean on all 7 packages plus llama-agent. `cargo fmt` applied. `rg 'format!("Unknown tool'` → no hits.
    - next: ready for /review
  timestamp: 2026-08-01T16:51:16.816932+00:00
- actor: claude-code
  id: 01kyz451yx5s5r760hv61qfkf0
  text: |-
    ### test — green

    **Commands run**
    ```
    cargo nextest run -p swissarmyhammer-tools -p agent-client-protocol-extras -p claude-agent -p mirdan -p code-context-cli -p shelltool-cli -p swissarmyhammer-cli --no-fail-fast
    cargo fmt --check
    cargo clippy -p swissarmyhammer-tools -p mirdan -p agent-client-protocol-extras --all-targets -- -D warnings
    ```

    **Results**
    - nextest: 4194 tests run, 4194 passed, 0 failed, 0 skipped (14 slow, 2 leaky). Matches the implementer-reported total exactly — no vanished tests.
    - fmt --check: clean, exit 0.
    - clippy -D warnings: clean, exit 0.
    - Did not run `-E 'rdeps(...)'` or full workspace, per instructions.
    - Known flake `review_working_emits_progress_notifications_per_pair_when_token_supplied` not hit this run; not chased.

    **Check 1 — assertion classification (test-side diff, crates/ + apps/)**
    Scanned all 65 changed files under `crates/` + `apps/`. Found 59 changed `contains(`/`assert!`/`assert_eq!` assertion lines plus 1 `assert_error_contains_any(&error, &[...])` array-form assertion (comprehensive_cli_mcp_integrations.rs) — all **casing-only**. Cross-checked every multi-branch `||` fallback assertion against current production message text; all branches still match real emitted strings. Zero semantic/weakened assertions found — no case of a `contains("X")` narrowing to drop a word (e.g. no "Invalid config" → "invalid" pattern).

    **Check 2 — tautological predicate collapse / reverts**
    Confirmed present in `crates/swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs`:
    - `contains("Path too long") || contains("path too long")` → `contains("path too long")` (duplicate collapsed)
    - `contains("too long") || contains("Path too long") || contains("4096")` → `contains("too long") || contains("4096")` (redundant branch dropped — "too long" is already a substring of "Path too long", so this is a safe dedup, not a weakening; production message at lines 248/658 is `"path too long (...)"`, still matched).
    No evidence of a half-reverted `||` fallback referencing a since-changed string from another crate — checked all remaining `||` assertion branches against current production text, all consistent.

    **Check 3 — `McpValidation::capitalize_first_letter` deletion**
    Confirmed deleted from `crates/swissarmyhammer-tools/src/mcp/shared_utils.rs`. `rg -rn "capitalize_first_letter"` across the whole repo returns zero hits — no remaining callers anywhere. Its two call sites now interpolate the field name directly (lowercase), consistent with the new lowercase-message convention.

    **Check 4 — `Unknown tool` UI-title sweep guard**
    `rg 'format!\("Unknown tool'` — zero hits, confirmed. `crates/claude-agent/src/tool_classification.rs` has **zero diff** (untouched) — its `"Unknown tool".to_string()` literal (line 196) remains capitalized, and `crates/claude-agent/tests/integration/coverage_tests.rs:2221` (`assert_eq!(title, "Unknown tool")`) is intact and unmodified. Not swept.

    No commit made, no kanban task moved.
  timestamp: 2026-08-01T16:57:39.037447+00:00
- actor: claude-code
  id: 01kyz4ayg5y3phh5fxg3zrc192
  text: |
    ### commit — changed
    - evidence: 0c8b969b8 fix(error-handling): lowercase error Display messages, no trailing punctuation
    - 71 files changed, 570 insertions(+), 414 deletions(-); staged confined to crates/swissarmyhammer-tools, crates/mirdan, crates/agent-client-protocol-extras, crates/claude-agent (tools.rs), crates/llama-agent (test fixture), apps/code-context-cli, apps/shelltool-cli, apps/swissarmyhammer-cli, plus .kanban/tasks (^p4mp9n6 update + new cards ^f70h1pr, ^p3qx2hc). No builtin/ files, no .trash pairs.
    - next: none — commit is local only, not pushed
  timestamp: 2026-08-01T17:00:52.101173+00:00
- actor: claude-code
  id: 01kz17q19ycyxt21kc15ycv48z
  text: |-
    ### review — stuck
    - evidence: `{"op": "review sha", "sha": "0c8b969b8~1..0c8b969b8"}` returned `128/128 review tasks failed — results are INCOMPLETE` on two separate attempts (06:30 and 06:47), and again at 07:11 when narrowed to `validators: [\"rust\"]` (still 128/128 failed, count unchanged by the narrowing — a sign the failure happens before per-task dispatch, not per validator).
    - diagnosis: `sah` binary was rebuilt today at 06:26 (fresh, not stale) and `check validators` reports all 21 validators OK. The review engine itself is not broken in general — `review sha HEAD~1..HEAD` succeeded cleanly (16 attempted, 0 failed, real findings returned) and `review file crates/swissarmyhammer-tools/src/mcp/server.rs` with `validators: ["rust"]` also succeeded (16 attempted, 0 failed). The failure is specific to scoping `review sha` at the large 71-file range `0c8b969b8~1..0c8b969b8` — the commit resolves fine in git (`git cat-file -t 0c8b969b8` → commit, `git diff --stat` confirms 71 files / 570(+) / 414(-)), so this is not a bad ref. This looks like a scale-triggered systemic infrastructure fault in the review engine's task fan-out, not real findings.
    - next: a human should re-run `{"op": "review sha", "sha": "0c8b969b8~1..0c8b969b8"}` after the engine's large-batch/fan-out path is checked, or split the review into smaller per-crate `review file` globs as a workaround. Task left in `review`, no findings recorded, no column move made.
  timestamp: 2026-08-02T12:38:22.782863+00:00
- actor: claude-code
  id: 01kz182re9w2hk8ztypdc5sv8e
  text: |-
    ### done — manual override by user
    - evidence: tests green (cargo nextest 4194 passed, clippy clean, fmt clean) on checkpoint commit 0c8b969b8. Review could not run — engine fails 128/128 on this commit's large diff, confirmed as a review-engine defect (^cbg6zsw), not a content issue. Not a fixable code problem on this task.
    - next: user directed moving this task to done despite the review gate not completing, since the blocker is an external engine bug already filed separately.
  timestamp: 2026-08-02T12:44:46.921512+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8f80
title: Lowercase the remaining capitalized MCP error Display messages outside the kanban tool
---
`builtin/validators/rust/rules/error-handling.md` states: Display messages on errors are lowercase, with no trailing punctuation.

`^1t92gnj` lowercased every error message in the two files it touched — `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` and `apps/kanban-cli/src/commands/serve.rs`. The same message text stays capitalized in the sibling MCP call handlers, which were out of that card's scope:

- `crates/swissarmyhammer-tools/src/mcp/server.rs` — `format!("Unknown tool: {}", name)` and `format!("Unknown tool: {}", request.name)`. A test in the same file asserts `msg.contains("Unknown tool")`, so it must change with them.
- `crates/agent-client-protocol-extras/src/test_mcp_server.rs` — `format!("Unknown tool: {}", request.name)`.
- `crates/claude-agent/src/tools.rs` — `"Unknown tool: {}"`. Note that `Unknown tool` ALSO appears there as a UI TITLE (`tool_classification.rs`, and the `assert_eq!(title, "Unknown tool")` tests). A title is not an error Display message — leave those capitalized.
- `mirdan::install::detected_agents_or_error` — `"Failed to load agents config: {e}"`, recorded as out of scope on `^1t92gnj` round 7.

## Scope

Sweep each crate for error Display messages that start with a capital. Lowercase only the ERROR messages. Leave capitalized: `InitResult::ok` success messages (already adjudicated on `^1t92gnj`), UI titles, log lines that are not failures, and `.expect()` panic text.

Update every test that pins the old casing in the same change.

## Acceptance

- A grep for `format!("Unknown tool` returns only lowercase forms, except the UI-title sites.
- `cargo nextest run` green, `cargo clippy --workspace --all-targets -- -D warnings` clean. #bug