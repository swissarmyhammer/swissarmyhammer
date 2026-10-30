---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m66kvx8yy1cqmt5jj4w12hxm
  text: 'Picked up. Research: rg confirms the card''s file list is complete — register_questions_tools appears only in the listed sites (tool_registry.rs, tools/mod.rs, mcp/mod.rs, lib.rs, server/mod.rs, health_registry.rs, tool_config.rs, apps/swissarmyhammer-cli/src/mcp_integration.rs). Doc references match the card (CLI README, integrated-sdlc.md, task SKILL.md, general-purpose AGENT.md). ARCHITECTURE.md does not mention the question tool, so it needs no update. ideas/agent/agent-notification-types.md uses "question" as a notification type, unrelated to the tool; left alone.'
  timestamp: 2026-10-30T14:36:50.334205+00:00
- actor: claude-code
  id: 01m66mb97ffe0v7ej1d2bzxk2v
  text: 'Implementation landed (TDD). RED: added `question_tool_is_not_registered` in tool_registry.rs tests; it failed with "fully registered registry must not register a `question` tool". Then I deleted crates/swissarmyhammer-tools/src/mcp/tools/questions/ (with git rm, so the deletion is staged but not committed) and removed every registration, re-export and call site the card lists: tool_registry.rs (macro block, the create_fully_registered_tool_registry call, the cli_category arm, the names_in fixture), tools/mod.rs, mcp/mod.rs, lib.rs, server/mod.rs, health_registry.rs, the CLI mcp_integration.rs, and tool_config.rs (KNOWN_TOOL_NAMES plus test registration). I also updated the docs: CLI README, integrated-sdlc.md, task SKILL.md and general-purpose AGENT.md. GREEN: the new test passes. Both acceptance rg checks return no matches. cargo fmt is done. cargo build --workspace and cargo clippy --workspace --all-targets -D warnings are clean. swissarmyhammer-cli: 702/702 passed. swissarmyhammer-tools: 1541/1543 passed. 2 failures, both caused by the environment rather than this change: integration::diagnostics_request_socket_e2e::follower_routes_diagnose_through_the_workspace_socket_to_the_leader and integration::code_context_ops_e2e::qwen_embedding_lsp_layered_e2e. rust-analyzer is not installed for the active rustup toolchain, so spawning it fails ("`rust-analyzer` is unavailable for the active toolchain ... error: infinite recursion detected", then LSP initialize "unexpected EOF reading headers"). Fix: `rustup component add rust-analyzer`, then re-run those two tests. Neither test touches the question tool.'
  timestamp: 2026-10-30T14:45:14.095191+00:00
- actor: claude-code
  id: 01m66mbdda4pbe40bj67583kt0
  text: |-
    ### implement — changed
    - evidence: 18 files: questions/ dir deleted (6 files: mod.rs, ask/mod.rs, summary/mod.rs, persistence.rs, schema.rs, description.md) + 12 edited (tool_registry.rs, tool_config.rs, tools/mod.rs, mcp/mod.rs, lib.rs, server/mod.rs, health_registry.rs, apps/swissarmyhammer-cli/src/mcp_integration.rs, apps/swissarmyhammer-cli/README.md, doc/src/concepts/integrated-sdlc.md, builtin/skills/task/SKILL.md, builtin/agents/general-purpose/AGENT.md). The question_tool_is_not_registered test went RED then GREEN. clippy -D warnings is clean. nextest -p swissarmyhammer-cli: 702/702. nextest -p swissarmyhammer-tools: 1541/1543. The 2 failures are rust-analyzer e2e tests that fail because rust-analyzer is not installed for the toolchain.
    - next: install rust-analyzer (`rustup component add rust-analyzer`) and re-run the 2 LSP e2e tests, then /review
  timestamp: 2026-10-30T14:45:18.378155+00:00
- actor: claude-code
  id: 01m66mcfbe84dcjmsy5c920c0f
  text: |-
    ### implement — changed
    - evidence: questions/ dir deleted (6 files) + 12 edited; question_tool_is_not_registered RED→GREEN; build + clippy -D warnings clean; nextest -p swissarmyhammer-cli 702/702; nextest -p swissarmyhammer-tools 1541/1543 (diagnostics_request_socket_e2e::follower_routes_diagnose_through_the_workspace_socket_to_the_leader and code_context_ops_e2e::qwen_embedding_lsp_layered_e2e fail: rust-analyzer unavailable for active toolchain)
    - next: /test
  timestamp: 2026-10-30T14:45:53.134380+00:00
- actor: claude-code
  id: 01m66ms681q51enp8a19rs5z4t
  text: |-
    ### finish iteration 1 — red
    - implement: changed — 18 files (questions/ deleted, 12 edited)
    - test: red — nextest rdeps(tools|skills|templating) 8858 run, 8852 passed, 6 failed; clippy + fmt clean. All 6 environmental, none from this change: 2 LSP e2e tests (rust-analyzer unavailable for active toolchain), 4 validators missing_docs typescript tests (eslint-plugin-jsdoc absent from /opt/homebrew/lib/node_modules)
    - commit: not run (blocked on red)
    - review: not run
    - next: needs rust-analyzer + eslint-plugin-jsdoc installed, then re-run /test
  timestamp: 2026-10-30T14:52:49.793941+00:00
- actor: claude-code
  id: 01m66p283bc4qecx12thqx9ye7
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (fe532a658) — 0 findings (confirmed 0, refuted 0; 7 validator runs attempted, 0 failed). 13 files reviewed; 5 Markdown files had no matching validator; .kanban/ excluded by .reviewignore.
    - next: none — task moved to done.
  timestamp: 2026-10-30T15:15:15.179666+00:00
- actor: claude-code
  id: 01m66p2zf9h619wc60kgw0n7qc
  text: |-
    ### finish iteration 2 — clean
    - environment: installed rust-analyzer (rustup) and eslint-plugin-jsdoc@63.3.3 (global npm), with user approval
    - test: green — the 6 previously failing tests re-run: 65 run, 65 passed, 0 skipped (source unchanged since the full rdeps run of 8858 with 8852 passing; clippy -D warnings and fmt clean)
    - commit: fe532a658 (question tool removal); 7aee9ae76 is the separate skills-docs commit
    - review: clean — review sha HEAD~1..HEAD, 0 findings; task moved to done
  timestamp: 2026-10-30T15:15:39.113113+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffc780
title: Remove the sah `question` MCP tool
---
## What
Delete the `question` MCP tool (ops `ask question` and `summarize questions`) and every reference to it. It is the only sah tool that sends an MCP elicitation (`peer.create_elicitation` in `crates/swissarmyhammer-tools/src/mcp/tools/questions/ask/mod.rs`), and we no longer want it.

This is one concern, but it touches more than 5 files. It cannot be split: once the module is deleted, the build only compiles again after every `register_questions_tools` call site is removed in the same change. Every edit apart from the module deletion is a one-line removal.

- [ ] **Delete the module:** remove the whole `crates/swissarmyhammer-tools/src/mcp/tools/questions/` directory (`mod.rs`, `ask/mod.rs`, `summary/mod.rs`, `persistence.rs`, `schema.rs`, `description.md`), and remove `pub mod questions;` from `crates/swissarmyhammer-tools/src/mcp/tools/mod.rs:45`.
- [ ] **Remove registration in `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs`:**
  - the `register_tool_category!(register_questions_tools, questions, ...)` block (~line 2117)
  - the `register_questions_tools(&mut registry)` call in `create_fully_registered_tool_registry` (~line 2173)
  - the `"question" => Some("question"),` match arm (~line 921)
  - `"question"` in the sorted-names test fixture `names_in` (~line 3083)
- [ ] **Remove the remaining call sites and re-exports:**
  - `crates/swissarmyhammer-tools/src/mcp/mod.rs:95` and `crates/swissarmyhammer-tools/src/lib.rs:76` (the `register_questions_tools` re-exports)
  - `crates/swissarmyhammer-tools/src/mcp/server/mod.rs:49,354`
  - `crates/swissarmyhammer-tools/src/health_registry.rs:15,215`
  - `apps/swissarmyhammer-cli/src/mcp_integration.rs:23,175`
  - `crates/swissarmyhammer-tools/src/mcp/tool_config.rs`: the `"question"` entry in `KNOWN_TOOL_NAMES` (line 37) and the test registration at lines 434/448
- [ ] **Fix docs, skills, and agents that tell agents to use the tool:**
  - `apps/swissarmyhammer-cli/README.md:73-77`: delete the `### Questions` section
  - `doc/src/concepts/integrated-sdlc.md:35`: drop the "`question` to ask clarifying questions" clause
  - `builtin/skills/task/SKILL.md:23`: change to "Vague requests: ask the user a clarifying question before writing the task."
  - `builtin/agents/general-purpose/AGENT.md:20`: drop "(via the `question` tool)"
- [ ] **Keep these as they are:** `ToolContext.peer` (other tools use it for progress and notifications), the doc example at `tool_registry.rs:310`, the `"questions"` entry in the forbidden list in `unified_server.rs:1933` (still a valid negative assertion), and the elicitation bridge in `crates/claude-agent` (it relays CLI elicitations and does not belong to this tool).

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/questions/` no longer exists.
- [ ] `rg -n "register_questions_tools|tools::questions|save_question_answer" --glob '!target' --glob '!.kanban'` returns no matches.
- [ ] `KNOWN_TOOL_NAMES` does not contain `"question"`, and `create_fully_registered_tool_registry()` registers no tool named `question`.
- [ ] `rg -n "\`question\` tool|ask question|summarize questions" builtin doc apps/swissarmyhammer-cli/README.md` returns no matches.
- [ ] `cargo build --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Tests
- [ ] Add `#[tokio::test] async fn question_tool_is_not_registered()` to the `#[cfg(test)]` module of `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs`. It builds `create_fully_registered_tool_registry().await`, asserts that `registry.get_tool("question").is_none()` (`ToolRegistry::get_tool`, ~line 1190), and asserts `!crate::mcp::tool_config::KNOWN_TOOL_NAMES.contains(&"question")`. Write it first: it fails before the removal and passes after.
- [ ] Update the sorted-names fixture test in `tool_registry.rs` (~line 3083) and the `tool_config.rs` registration test (~line 434) so they no longer reference the tool.
- [ ] `cargo nextest run -p swissarmyhammer-tools` passes.
- [ ] `cargo nextest run -p swissarmyhammer-cli` passes.

## Workflow
- Use `/tdd`: write the failing test first, then implement until it passes.