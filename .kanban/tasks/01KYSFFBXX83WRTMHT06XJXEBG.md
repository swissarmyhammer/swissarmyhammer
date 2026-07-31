---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyvy3rmehewfd71xkaptvcnf
  text: |-
    Picked up by /finish ^6xjxebg. No prior attempts.

    Re-verified the defect still reproduces at HEAD: `create_tool_registry` in `apps/swissarmyhammer-cli/src/mcp_integration.rs` has 8 registrations, `register_all_tools` in `crates/swissarmyhammer-tools/src/mcp/server.rs:968-979` has 12. Missing: ralph, agent, diagnostics, skill.

    Sibling card ^t7ebyn8 is now DONE, so the Stop hook survives skill deployment and reaches `~/.skills/finish/SKILL.md`. This card is the other half — it makes the command the hook invokes actually exist. After this lands, the ralph loop should work end to end for the first time.

    Note the argument-shape difference the implementer must handle: `register_agent_tools` and `register_skill_tools` take library handles (`agent_library`, `skill_library`, `prompt_library`), so the CLI registry needs those too. The other two are plain `(&mut registry)`.

    The parity test is the real deliverable. A doc comment on `create_tool_registry` already said it "should mirror the registration in register_all_tools" and it drifted anyway. Prose did not hold; a test will.
  timestamp: 2026-07-31T11:14:21.966481+00:00
- actor: claude-code
  id: 01kyvzrdfepa8etekjzk3zdwd8
  text: |-
    Implemented. The CLI now registers the same tools as the MCP server, and `sah tool ralph ralph check --` works from the local build.

    ## What changed

    `apps/swissarmyhammer-cli/src/mcp_integration.rs`
    - `create_tool_registry` adds the four missing registrations: `register_ralph_tools`, `register_diagnostics_tools`, `register_agent_tools`, `register_skill_tools`. The last two need library handles, so the function builds a default `SkillLibrary`, `AgentLibrary` and `TemplateLibrary`, the same way `collect_all_health_checks` does in `health_registry.rs`.
    - New parity test `test_cli_tool_registry_matches_server_registry`. It compares the CLI registry against a real `McpServer` registry and fails on any difference in three directions: missing from the CLI, present only in the CLI, and registered but with no `sah tool` command. `CLI_REGISTRY_EXCLUSIONS` is an explicit list of `(tool, reason)` pairs; it is empty on purpose.

    `crates/swissarmyhammer-tools/src/mcp/tools/agent/mod.rs` and `.../skill/mod.rs`
    - Added `cli_category()`. This was necessary and the card did not know it. The default `cli_category()` derives the category from the tool-name prefix and its match arm list has no `agent` or `skill`, so both tools stayed invisible on the CLI even after registration. Registration alone is not enough; the parity test now checks CLI visibility too.

    `crates/swissarmyhammer-tools/src/lib.rs` and `.../mcp/mod.rs`
    - Re-export `register_agent_tools` and `register_skill_tools` at the crate root, beside the other ten `register_*_tools`.

    `apps/swissarmyhammer-cli/src/main.rs`
    - New `relax_required_tool_args`. This is the third blocker, also unknown to the card. Even after registration, `echo '{"session_id":"probe"}' | sah tool ralph ralph check --` failed with `error: the following required arguments were not provided: --session_id`. The schema marks `session_id` required, clap enforces it, and clap cannot see stdin — so `merge_stdin_arguments` never ran. The fix clears `required` recursively, but only inside the `tool` subcommand tree, and only when stdin is piped. Static commands keep their clap checks (`sah model use` with piped stdin still reports the missing `<NAME>`). A field absent from both the flags and stdin is still rejected by the tool's own schema validation.

    ## RED proof

    Round 1, before the registrations:
    ```
    Registered by the server, missing from the CLI: ["agent", "diagnostics", "ralph", "skill"]
    ```
    Round 2, after the registrations, before `cli_category`:
    ```
    Registered by the CLI but with no `sah tool` command (needs `cli_category`): ["agent", "skill"]
    ```
    Both green after the fix. The clap-required blocker was proved RED at the CLI, then by unit test `required_tool_arg_is_enforced_without_relaxing`.

    ## End-to-end proof, local build only

    ```
    $ ./target/debug/sah tool --help
      agent, code_context, diagnostics, files, git, kanban, question, ralph, review, shell, skill, web

    $ echo '{"session_id":"probe"}' | ./target/debug/sah tool ralph ralph check --
    decision: allow

    $ echo '{"session_id":"probe","instruction":"keep going"}' | ./target/debug/sah tool ralph ralph set --
    $ echo '{"session_id":"probe"}' | ./target/debug/sah tool ralph ralph check --
    decision: block
    iteration: 1
    reason: keep going. Iteration 1 of 50.
    ```

    ## Side effects of the four new tool families

    - Startup cost: `create_tool_registry` now loads the builtin skills and agents to build the two libraries. `sah tool --help` still answers in under 0.2 s. The MCP server already did this work, so the CLI now duplicates it once per process.
    - No new required configuration.
    - No tool needs a live server. All four run in process through the same `execute_tool` path as the other eight.
    - The CLI's skill and agent libraries are separate instances from the server's. Both load only the builtins, so they hold the same content. Nothing writes to them at run time.
    - Help text for the two new commands reads `<agents_instructions>` and `<skills_instructions>`, because that is the first line of their `description.md`. Cosmetic only.

    ## Did not work / found on the way

    - The ralph decision prints as YAML, not JSON. A Claude Code Stop hook reads JSON from stdout, so the hook still cannot read the decision. The global `--format json` flag does not reach the tool output path. New card ^634hqth.
    - The registration list now exists in three places: `register_all_tools`, `create_tool_registry` and `collect_all_health_checks`. The parity test guards the second. New card ^pwaxzy2 proposes one shared entry point.

    ## Test status

    - `cargo nextest run -p swissarmyhammer-cli`: 848 passed, 0 failed.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`: 4995 passed, 4 failed. None of the four belongs to this change:
      - `review_working_emits_progress_notifications_per_pair_when_token_supplied` — the known flake (^pxhzk1s, ^t681xdv, ^yh4m6ed).
      - `skill_e2e::test_task_skill_renders_short_id_guidance` — asserts on builtin content. The marker `last 7 characters of the ULID` now lives only in `builtin/skills/kanban/SKILL.md`, and `builtin/_partials/short-ids.md` is deleted in the working tree.
      - `kanban-app::workspace_init opening_a_board_deploys_the_kanban_tool_skills` and `state::tests::test_open_board_deploys_kanban_tool_skills_at_board_folder` — both need `task` in the kanban profile. `builtin/skills/task/SKILL.md` carries no `profiles:` key, at the working tree and at HEAD.
    - `cargo fmt`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.

    Task stays in `doing`. Nothing committed.
  timestamp: 2026-07-31T11:43:07.246257+00:00
- actor: claude-code
  id: 01kyw0v03t59r8mfsnkx53wca4
  text: |-
    Adversarial review round. Verdict was REVISE with six findings. Four are fixed, two are deferred with reasons.

    ## Fixed

    **Missing `tools.yaml` pass (HIGH).** `register_all_tools` ends with `load_merged_tool_config()` + `apply_tool_config()`; `create_tool_registry` stopped at the registrations. Two consequences: the CLI offered a `sah tool <name>` command for a tool the server then refuses to execute, and the new parity test compared the server's config-filtered `list_tool_names()` against the CLI's unfiltered set — so any user with one tool disabled in `tools.yaml` would see a false drift failure. `create_tool_registry` now runs the same pass. The test only stayed green because `IsolatedTestEnvironment` hides the real config; the shipped binary had no such cover.

    **Raw XML tag as the help summary (HIGH).** `sah tool --help` showed `agent  <agents_instructions>` and `skill  <skills_instructions>`, because `McpTool::cli_about` takes the first line that is neither empty nor a markdown header, and those two descriptions open with a wrapper tag. `cli_about` now also skips a lone tag line, so both commands read as prose. New `is_wrapper_tag` helper in `tool_registry.rs` with test `test_cli_about_skips_wrapper_tag_lines`, proved RED first (`left: Some("<skills_instructions>")`).

    **`STDIN_ARGS_SUBCOMMAND` untied from the real tree (MEDIUM).** All three relax tests built a synthetic clap tree, so renaming the dynamic `tool` command would make the relaxation a silent no-op with every test still green — the exact failure mode this card exists to kill. New `stdin_args_subcommand_names_a_real_command` builds the real CLI through `CliBuilder`, asserts the constant names one of its subcommands, then runs the Stop hook's argv against it: `MissingRequiredArgument` before relaxing, success after. Real-path RED and GREEN in one test.

    **Stale comments (LOW).** `sah tool <category> <name>` was wrong — the category is only the iteration bucket, the path is `sah tool <tool_name> <noun> <verb>`. Also dropped the claim that the CLI has "no server-owned libraries", and recorded that `load_defaults()` resolves from the process working directory, not from any `working_dir` argument.

    Also softened the `relax_required_tool_args` doc. It claimed schema validation still rejects a missing field; there is no shared validator on the `execute_tool` path. The doc now says the caller loses clap's named-argument diagnostic for the `tool` tree whenever stdin is not a terminal (including `< /dev/null` and CI), and that the field surfaces as the tool's own `execute` error instead. The reviewer sampled `ralph`, `kanban` and `code_context`: all three reject.

    ## Deferred, with reasons

    **Use the server's registry instead of a second one (MEDIUM).** The reviewer is right that `McpServer::get_tool_registry()` would make drift structurally impossible and delete both `create_tool_registry` and the parity test. That is a different design from the one this card specifies, and `McpServerHandle::server()` returns `Option` — one construction path sets it to `None` — so it needs its own verification. Recorded on ^pwaxzy2, which also still owns the third copy in `health_registry.rs`.

    **`sah tool skill --help` prints the whole skill catalog (LOW).** `long_about` is the full description, which carries the dynamic `<available_skills>` block — 142 lines for `skill`, similar for `agent`. Accepted: at `--help` the catalog is the tool's real documentation and is useful. The one-line summary, which is what `sah tool --help` shows, now reads as prose.

    ## Re-verification

    - `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`: 4998 passed, 4 failed — the same four as before, none in this diff (the known review-progress flake, plus three tests that assert on builtin skill content the user is editing in parallel).
    - `cargo nextest run -p swissarmyhammer-cli`: all green.
    - `cargo fmt` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - `sah tool --help` now reads `agent  You have access to subagents...` and `skill  You have access to skills...`.
    - `echo '{"session_id":"probe"}' | ./target/debug/sah tool ralph ralph check --` still returns allow, and returns block after a `set`.
  timestamp: 2026-07-31T12:02:00.442615+00:00
- actor: claude-code
  id: 01kyw24fc687gjsrhks24x2wcj
  text: |-
    ## Independent test verification

    Commands and results:

    - `cargo nextest run -p swissarmyhammer-cli` — 849 passed, 0 failed, 0 skipped.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'` — 5003 run, 4999 passed, 4 failed, 2 skipped. No failure comes from this card.
    - `cargo fmt --all -- --check` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, no warnings.

    ### Parity test proved to fail on drift

    - Removed `register_ralph_tools` from `create_tool_registry`. Test failed: `missing from the CLI: ["ralph"]`. Restored.
    - Removed `SkillTool::cli_category()`. Test failed: `Registered by the CLI but with no sah tool command (needs cli_category): ["skill"]`. Restored.

    ### `relax_required_tool_args` bounds proved

    - Piped stdin + `sah model use` — still errors on the missing `<NAME>` positional.
    - Terminal stdin (pty) + `sah tool ralph ralph check` — still errors on the missing `--session_id`.
    - Piped stdin + `sah tool ralph ralph check --` — returns `decision: allow`.

    ### Two real defects found and fixed

    1. `is_wrapper_tag` was inserted between the `McpTool` doc comment and the trait. The whole public trait documentation attached to the private helper, and `pub trait McpTool` had no docs. Moved the helper above the doc block.

    2. The added `tools.yaml` disable pass did not do what its comment claims. `get_cli_categories` and `filter_cli_tools` tested only `hidden_from_cli()`, never `disabled_tools`. A disabled tool stayed in `sah tool --help` and failed at execution with `Tool 'ralph' not found ... Available tools: [... ralph ...]`. Added `ToolRegistry::is_cli_visible`, which both accessors now use. Verified RED then GREEN with `test_disabled_tool_excluded_from_cli_command_tree`, and end to end: a disabled tool is now absent from `sah tool --help`.

    ### Failures not caused by this card

    - `workspace_init::opening_a_board_deploys_the_kanban_tool_skills` and `state::tests::test_open_board_deploys_kanban_tool_skills_at_board_folder` — both need the `task` skill in the `kanban` profile. `builtin/skills/task/SKILL.md` has no `profiles:` key, in the working tree and at HEAD.
    - `skill_e2e::test_task_skill_renders_short_id_guidance` — the marker text is only in `builtin/skills/kanban/SKILL.md`. `builtin/_partials/short-ids.md` is deleted in the working tree.
    - `review_working_emits_progress_notifications_per_pair_when_token_supplied` — the known flake (^pxhzk1s, ^t681xdv, ^yh4m6ed). It passed in an earlier full run and passes alone.

    The 2 skipped tests are `#[ignore]`d heavy real-model proofs in `llama-agent` (`kv_prefix_reuse_recurrent`). They predate this card.
  timestamp: 2026-07-31T12:24:39.558110+00:00
- actor: claude-code
  id: 01kyw49nj0w5smbkr3e5v8hmgr
  text: |-
    Closing. The 54-finding section: 1 fixed here, 53 SPLIT OUT, none dropped.

    The reviewer classified each against the commit's real hunks. Exactly one landed on code this commit wrote: `cli_category()` returned the literal `"skill"` / `"agent"`, duplicating what `McpTool::name()` already returns. Both now return `Some(Self::name(self))`.

    The other 53 are pre-existing code the diff displaced:
    - 36 missing `pub mod` docs in `mcp/mod.rs` → ^mz0s6bf (67% of the report, one cause; two validators double-reported the same modules at different offsets)
    - XML injection in `agent`/`skill` `build_description` → ^gt1h2sc (the only security-class item, kept separate so it is not buried in a style sweep)
    - dead `convert_result`, duplicated operation lists, missing Debug, deep nesting → ^6qjyz7r
    - `tool_registry.rs` prefix table, duplicate macros, manual `register_file_tools` → ^fjs8tqv
    - `main.rs` magic numbers and `&Arc` params → ^pxr6rxe

    The clearest evidence of displacement: `main.rs:351` resolves to `report_validation_issues(&cli_builder, false, 5)`, a CONTEXT line inside the hunk range. Inserting `relax_required_tool_args` above it shifted it down, and a validator read the new line number as new work.

    Verified after the fix: 40/40 on the targeted selection, 849/849 on swissarmyhammer-cli, fmt clean, clippy clean, and `sah tool ralph ralph check --` still answers.

    Shipped: the card named one blocker; there were five.
    1. 8 registrations vs 12 — ralph, agent, diagnostics, skill missing.
    2. `agent`/`skill` invisible even once registered — `cli_category()` derives from the name prefix and had no arm for either. Root cause now filed as ^fjs8tqv.
    3. clap rejected the piped call before stdin was read — `session_id` is schema-required and clap cannot see stdin. `relax_required_tool_args` is bounded to the `tool` subtree and to piped stdin only; both bounds proved.
    4. The `tools.yaml` disable pass did not work as first written — a disabled tool still appeared in `--help`, then failed with `Tool 'ralph' not found ... Available tools: [... ralph ...]`. `get_cli_categories`/`filter_cli_tools` consulted only `hidden_from_cli`, unlike `get_tool`/`list_tool_names`. One `is_cli_visible` predicate now backs all of them.
    5. `is_wrapper_tag` had been inserted between the `McpTool` doc comment and the trait, silently reattaching ~60 lines of public trait docs to a private helper.

    The parity test is the real deliverable and is proved to fail in both modes — missing registration, and registered-but-invisible. Prose saying the two registries "should mirror" each other did not hold for either.

    REMAINING GAP — the acceptance criterion said "returns the block/allow JSON". It returns YAML:

    ```
    $ echo '{"session_id":"probe"}' | ./target/debug/sah tool ralph ralph check --
    decision: allow
    ```

    `--format json` does not reach the tool output path. A Stop hook parses JSON from stdout, so the ralph loop still does not work. Filed as ^634hqth and being driven next — both ralph cards are closed but the hook is not restored until that lands.
  timestamp: 2026-07-31T13:02:26.880887+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8180
title: CLI tool registry drifted from register_all_tools (sah tool ralph is missing)
---
The CLI builds its own tool registry and it no longer matches the MCP server registry. Four tool families are absent from the CLI, so their `sah tool <name>` commands do not exist.

## Symptom

```
$ echo '{"session_id":"x"}' | sah tool ralph ralph check --
error: unrecognized subcommand 'ralph'
```

`sah tool --help` lists 8 tools: `code_context` `files` `git` `kanban` `question` `review` `shell` `web`.

This breaks the ralph Stop hook, whose command is `sah tool ralph ralph check --`.

## Cause

`apps/swissarmyhammer-cli/src/mcp_integration.rs:143-152` — `create_tool_registry` makes 8 registrations. Its own doc comment says it "should mirror the registration in `swissarmyhammer_tools::mcp::server::register_all_tools`". It drifted.

`crates/swissarmyhammer-tools/src/mcp/server.rs:968-979` — `register_all_tools` makes 12. The CLI omits:

- `register_ralph_tools`
- `register_agent_tools`
- `register_diagnostics_tools`
- `register_skill_tools`

The `ralph` tool itself is correct. `cli_category()` returns `Some("ralph")` and `cli_name()` returns `"ralph"`, so it becomes a CLI subcommand as soon as it is registered.

## Required change

1. Add the 4 missing registrations to `create_tool_registry`. Match the argument shapes that `register_all_tools` uses — `register_agent_tools` and `register_skill_tools` take library handles.
2. Add a parity test that compares the tool-name set from `create_tool_registry` against the set from `register_all_tools` and fails on any difference. A comment that says "should mirror" did not hold. A test will.

If a tool must stay out of the CLI on purpose, the parity test must name it in an explicit exclusion list with a reason. Silence is what caused this.

## Acceptance

- The parity test fails before the change and passes after it.
- `echo '{"session_id":"probe"}' | sah tool ralph ralph check --` returns the block/allow JSON, not a clap error.
- `sah tool --help` lists `ralph`, `agent`, `diagnostics`, and `skill`.

Note: the hook is also stripped at skill-install time, in a separate card. Both cards are needed before the Stop hook works. #bug #cli #ralph

## Review Findings (2026-07-31 07:26)

- [ ] `apps/swissarmyhammer-cli/src/main.rs:200` — Function takes `&Arc<CliToolContext>` but should take `Arc<CliToolContext>` directly. Arc is Copy-cheap to pass by value. Creating a reference to Arc is an unnecessary indirection that forces callers to pass a borrowed Arc when passing by value is more ergonomic. Change parameter to `cli_tool_context: Arc<CliToolContext>` and update the call to `display_verbose_validation_report(cli_tool_context.clone(), ...)` or `display_verbose_validation_report(cli_tool_context, ...)`.
- [ ] `apps/swissarmyhammer-cli/src/main.rs:318` — Hardcoded 10 is a magic number for max errors display threshold — numeric thresholds should be named constants for readability and maintainability. Define `const MAX_CLI_VALIDATION_ERRORS_DISPLAY: usize = 10;` at module level and use it: `display_numbered_items(&warnings, false, MAX_CLI_VALIDATION_ERRORS_DISPLAY, "errors");`.
- [ ] `apps/swissarmyhammer-cli/src/main.rs:351` — Hardcoded 5 is a magic number for max warnings display threshold — numeric thresholds should be named constants for readability and maintainability. Define `const MAX_CLI_VALIDATION_WARNINGS_DISPLAY: usize = 5;` at module level and use it: `report_validation_issues(&cli_builder, false, MAX_CLI_VALIDATION_WARNINGS_DISPLAY);`.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:43` — Public module `diagnostics_resource` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the diagnostics_resource module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:44` — Public module `error_handling` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the error_handling module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:45` — Public module `file_watcher` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the file_watcher module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:46` — Public module `host` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the host module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:47` — Public module `inline_diagnostics` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the inline_diagnostics module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:49` — Public module `notify_types` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the notify_types module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:51` — Public module `plan_notifications` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the plan_notifications module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:52` — Public module `progress` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the progress module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:53` — Public module `responses` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the responses module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:54` — Public module `server` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the server module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:56` — Public module `tool_config` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the tool_config module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:57` — Public module `tool_descriptions` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the tool_descriptions module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:58` — Public module `tool_handlers` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the tool_handlers module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:60` — Public module `tools` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the tools module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:61` — Public module `types` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the types module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:62` — Public module `unified_server` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the unified_server module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:63` — Public module `utils` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the utils module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:65` — Public module `test_utils` lacks documentation — its purpose is undocumented. Add a doc comment explaining what the test_utils module provides.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:68` — Public module `diagnostics_resource` lacks a doc comment. All public items must be documented. Add a doc comment describing the module's purpose and contents.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:69` — Public module `error_handling` lacks a doc comment. Add a doc comment describing error handling utilities.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:70` — Public module `file_watcher` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:71` — Public module `host` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:72` — Public module `inline_diagnostics` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:74` — Public module `notify_types` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:75` — Public module `op_tool_helpers` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:76` — Public module `plan_notifications` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:78` — Public module `responses` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:79` — Public module `server` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:81` — Public module `tool_config` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:82` — Public module `tool_descriptions` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:83` — Public module `tool_handlers` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:84` — Public module `tool_registry` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:85` — Public module `tools` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:86` — Public module `types` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:87` — Public module `unified_server` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/mod.rs:88` — Public module `utils` lacks a doc comment. Add a doc comment.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:161` — Hardcoded match statement maps tool name prefixes to CLI category strings. This is a fixed mapping table (memo→memo, file→file, files→file, web→web, shell→shell, outline→outline, notify→notify, kanban→kanban, git→git, cel→cel, question→question) whose arms differ only in constant string values. Should be expressed as a named constant data table interpreted by one code path, not as a parallel match arms. Define a const mapping table like `const PREFIX_TO_CATEGORY: &[(\"&str\", &str)] = &[(\"memo\", \"memo\"), (\"file\", \"file\"), (\"files\", \"file\"), ...];` and replace the match with a lookup: `PREFIX_TO_CATEGORY.iter().find(|(p, _)| *p == prefix).map(|(_, c)| *c)` to interpret data instead of parallel code paths.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:1134` — Macro `impl_default_doctorable!` is near-verbatim duplicate of `impl_empty_initializable!` at line 1159; both have identical bodies that differ only in the trait name. These should be consolidated into a single parameterized macro that takes the trait as an argument. Extract a shared parameterized macro `impl_trait_with_defaults!($trait_path, $tool_type)` that both doctorable and initializable impls can use, or wrap one as a thin macro over the other.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:1159` — Macro `impl_empty_initializable!` is near-verbatim duplicate of `impl_default_doctorable!` at line 1134; body is identical except for the trait name. These two macros should be consolidated into one. Consolidate into a single parameterized macro or use one as a wrapper over the other to avoid maintenance burden of keeping identical code in sync.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:1604` — Function `register_file_tools` manually implements what the `register_tool_category!` macro already does (line 1609+). The function is a verbatim match for the macro expansion: `use super::tools::files; files::register_file_tools(registry);` follows the exact same pattern as other register functions defined via the macro. Replace the manual `register_file_tools` implementation with `register_tool_category!(register_file_tools, files, "Register all file-related tools with the registry");` to use the existing macro and eliminate duplication.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/agent/mod.rs:26` — `AgentMcpTool` is a public struct that should derive `Clone` and `Debug`. All fields are `Clone`-capable (static reference and Arc), and `Debug` is always applicable to public types. Without these derives, downstream crates cannot implement them due to orphan rules. Add `#[derive(Clone, Debug)]` above the struct definition at line 25.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/agent/mod.rs:49` — Function `build_description` takes `&Arc<RwLock<AgentLibrary>>` but should take `Arc<RwLock<AgentLibrary>>` directly (Arc is Copy-cheap to clone/pass) or `&RwLock<AgentLibrary>` (generic over how the lock is obtained). Requiring a reference to an Arc is unnecessarily specific and forces callers into a particular borrowing pattern. Change to `fn build_description(library: Arc<RwLock<AgentLibrary>>) -> String` and update the call at line 45 to `let description = build_description(library.clone());` or `let description = build_description(library);` if Arc is consumed.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/agent/mod.rs:67` — Agent properties (name, description, source) are inserted into an XML string without escaping special characters. If the description is displayed in an HTML/XML context by a client, unescaped characters like <, >, &, or " could lead to XSS or XML injection. Escape XML special characters before inserting: replace `&` with `&amp;`, `<` with `&lt;`, `>` with `&gt;`, `"` with `&quot;`. Alternatively, use an XML escaping library like `xml-escape` crate.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/agent/mod.rs:152` — Three consecutive match arms (List at line 152–155, Use at line 156–159, Search at line 160–163) have identical bodies: each imports `Execute` trait and calls `op.execute(&ctx).await`. The arms differ only in the pattern being matched, not the logic executed. This is code that could drift and should be consolidated. Refactor to avoid the duplication: if all three operation types implement a common trait with `execute`, destructure and handle outside the match; or use a macro to collapse the three identical arms into one. Alternatively, implement a helper method or trait that centralizes the execution logic.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:44` — Public struct SkillTool has non-empty representation (three fields) but does not implement Debug. The documentation rule requires Debug for all public types with non-empty representation. Add #[derive(Debug)] annotation before the struct: add a line `#[derive(Debug)]` immediately before line 44 `pub struct SkillTool {`.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:60` — Deep nesting (4 levels): match → Ok arm → if/else conditional → for loop. The flow requires tracking multiple control-flow levels to understand what happens inside the loop. Extract the XML-building logic into a separate function to reduce nesting to 2–3 levels. Example: move the loop and its string operations into a helper `fn build_skills_xml(skills: &[SkillInfo]) -> String` and call it from the else branch.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:84` — XML injection: unescaped skill metadata (skill.name, skill.description, skill.source) is inserted directly into XML tags without character escaping. Special characters like <, >, &, or " could malform the XML structure or inject unintended elements. Escape XML special characters before insertion. Create a helper function: fn escape_xml(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") } then apply it to each field before formatting.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:101` — convert_result is a private function with no inbound callers. It is not an entry point, exported API, or test. Dead code adds maintenance burden and confuses readers. Delete the convert_result function and remove the unused ExecutionResult import from the swissarmyhammer_skills use statement.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:160` — Error message lists only 'list skill', 'use skill', 'search skill' as valid operations, but the code accepts additional aliases: 'get skill', 'load skill', 'activate skill', 'invoke skill', 'find skill', 'lookup skill'. Users who encounter an unknown operation error won't see the complete list of valid operations, creating confusion about what they can actually call. Update the error message to list all valid operations including aliases, or acknowledge aliases exist: "Unknown operation '{}'. Valid operations: 'list skill', 'use skill' (aliases: 'get skill', 'load skill', 'activate skill', 'invoke skill'), 'search skill' (aliases: 'find skill', 'lookup skill')".
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:177` — The cli_category returns a hardcoded 'skill' string that duplicates the tool name defined at line 135. If the tool name changes, both places must be updated manually. Define a module-level constant (e.g., `const TOOL_NAME: &str = "skill"`) and use it in both name() and cli_category(), or derive cli_category from self.name().
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:220` — Error message hardcodes the list of valid operations, duplicating the match arms above. This must be manually kept in sync with lines 198-204 — if operations are added or renamed, both places must change, violating the data-driven principle. Define operation names and aliases as constants (e.g., `const VALID_OPS: &[&str] = &[...]`) and use them to generate both the match arms and error message, or derive the error message programmatically from the match statement to ensure consistency.

### Provenance of the above (classified against the commit's real hunks)

The engine's cited line numbers track the PRE-image and are offset — two validators
gave 160 and 220 for the same error message, and 43-65 and 68-88 for the same
`pub mod` list. Classification below uses the subject, not the number.

Commit `18d62792a` touched these hunks only:

- `mcp_integration.rs` — the four registrations + the `tools.yaml` pass. **Zero findings.**
- `main.rs` — post 342-402 (`STDIN_ARGS_SUBCOMMAND`, `clear_required_args`, `relax_required_tool_args`, the `is_terminal` gate) and post 1446-1548 (new tests). **Zero findings.**
- `tool_registry.rs` — post 791-798 (`is_wrapper_tag` moved), 1024-1030, 1461-1474 (`is_cli_visible`), 2669+ (new test). **Zero findings.**
- `mcp/mod.rs` — post 100-104 (two `pub use`). **Zero findings.**
- `agent/mod.rs` — post 166-171 (`cli_category`). **Zero findings.**
- `skill/mod.rs` — post 151-156 (`cli_category`). **One finding: `:177`.**

INTRODUCED by this commit (1 of 54):

- `skill/mod.rs:177` — `cli_category` returns the literal `"skill"`, which `name()`
  already returns. This is the added arm. Note `agent/mod.rs` has the identical
  added arm returning `"agent"`, which the engine did not flag; fix both together.

PRE-EXISTING, merely touched or displaced (53 of 54):

- All 36 `mcp/mod.rs` module-doc items — the `pub mod` block is at post 60-82 and is
  untouched. Double-reported by two validators.
- All 4 `tool_registry.rs` items (`:161` prefix match, `:1134`/`:1159` macros,
  `:1604` `register_file_tools`) — all outside every hunk. `:161` is the very
  prefix match the commit worked around, but the match itself was not edited.
- All 4 `agent/mod.rs` items — the struct, `build_description`, XML escaping and
  the match arms all sit above the hunk at 163.
- 6 of 7 `skill/mod.rs` items (`:44`, `:60`, `:84`, `:101`, `:160`, `:220`) — all
  outside the hunk at 148.
- All 3 `main.rs` items. `:200`/`:318` resolve to post 201 and post 523, outside
  every hunk. `:351` resolves to `report_validation_issues(&cli_builder, false, 5)`
  at post 386 — inside the hunk range but a CONTEXT line, displaced by the
  insertion above it, not written by this commit.
